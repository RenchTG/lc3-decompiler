use std::{fs, fmt};
use std::collections::{HashMap, HashSet};
use lc3_ensemble::ast::{asm::{disassemble_line, AsmInstr, Stmt, StmtKind}, Label, PCOffset};

// The HighLevelStmt enum has been removed. It will be reintroduced during the
// "Program Lifting" stage (Step 7), which is a better fit for it.

#[derive(Debug, Clone)]
struct BasicBlock {
    address: u16,
    length: u16,
    preds: Vec<u16>,
    succs: Vec<u16>,
    dominators: Vec<u16>,
    visited: bool,
    // The `statements` field has been removed. The results of structuring
    // are now stored in separate collections.
}

impl BasicBlock {
    fn new(address: u16) -> Self {
        BasicBlock {
            address,
            length: 0,
            preds: Vec::new(),
            succs: Vec::new(),
            dominators: Vec::new(),
            visited: false,
        }
    }
}

/// Represents a natural loop found in the CFG.
#[derive(Debug, Clone)]
struct Loop {
    header: u16,
    blocks: HashSet<u16>,
}

impl Loop {
    fn new(header: u16) -> Self {
        let mut blocks = HashSet::new();
        blocks.insert(header);
        Loop {
            header,
            blocks,
        }
    }
}

// --- New Data Structures for Control Flow Structuring (Step 4) ---

/// Stores detailed information about an identified loop structure.
#[derive(Debug, Clone)]
pub struct IdentifiedLoop {
    header: u16,
    blocks: HashSet<u16>,
    exit_nodes: HashSet<u16>,
}

/// The kind of conditional structure identified.
#[derive(Debug, Clone)]
pub enum ConditionalKind {
    If,
    IfElse,
}

/// Stores detailed information about an identified conditional structure.
#[derive(Debug, Clone)]
pub struct IdentifiedConditional {
    kind: ConditionalKind,
    condition_block: u16,
    true_branch_head: u16,
    false_branch_head: Option<u16>,
    join_block: u16,
}


fn gen_sections(content: &str) -> (Vec<&str>, HashMap<u16, &str>) {
    let sections: Vec<&str> = content.split("\n\n").collect();

    let text: Vec<&str> = sections[1].split("\n").skip(1).collect();

    let mut symbols = HashMap::new();

    for line in sections[2].lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let addr = u16::from_str_radix(parts[0], 16).unwrap();
        symbols.insert(addr, parts[4]);
    }

    (text, symbols)
}

fn disassemble(text: Vec<&str>, symbols: HashMap<u16, &str>) -> (HashMap<u16, Stmt>, Vec<u16>) {
    let mut disassembled = HashMap::new();
    let mut origs = Vec::new();
    let mut index = 0;

    while index < text.len() {
        let start_addr = u16::from_str_radix(text[index], 16).unwrap();
        origs.push(start_addr);
        let length_of_section = text[index+1].parse::<u16>().unwrap();
        index += 2;

        for i in 0..length_of_section {
            let instr = u16::from_str_radix(text[index], 16).unwrap_or(0);
            let mut statement = disassemble_line(instr);

            if symbols.contains_key(&(start_addr+i)) {
                let symbol_len = symbols[&(start_addr+i)].len();
                statement.labels.push(Label::new(symbols[&(start_addr+i)].to_string(), 0..symbol_len));
            }

            disassembled.insert(start_addr + i, statement);

            index += 1;
        }
    }
    (disassembled, origs)
}

fn identify_code_data(disassembly: HashMap<u16, Stmt>, _origs: Vec<u16>) -> Vec<u16> {
    let mut call_list: Vec<u16> = vec![0x3000];
    let mut function_list: Vec<u16> = Vec::new();
    let mut work_list: Vec<u16> = Vec::new();

    while !call_list.is_empty() {
        let mut addr = call_list.pop().unwrap();
        function_list.push(addr);

        loop {
            let stmt = if let Some(s) = disassembly.get(&addr) { s } else { break; };

            match &stmt.nucleus {
                // Instruction is jump
                StmtKind::Instr(AsmInstr::BR(_cc, pcoffset9)) => {
                    if let PCOffset::Offset(offset) = pcoffset9 {
                        work_list.push((addr as i16 + 1 + offset.get()) as u16);
                    }
                },
                // Instruction is call
                StmtKind::Instr(AsmInstr::JSR(pcoffset11)) => {
                     if let PCOffset::Offset(offset) = pcoffset11 {
                        let target = (addr as i16 + 1 + offset.get()) as u16;
                        if !function_list.contains(&target) {
                            call_list.push(target);
                        }
                    }
                },
                // Instruction is ret
                StmtKind::Instr(AsmInstr::RET) => {
                    if work_list.is_empty() {
                        break;
                    }
                },
                // Instruction is unconditional jump/call
                StmtKind::Instr(AsmInstr::JMP(_reg)) | StmtKind::Instr(AsmInstr::JSRR(_reg)) => {
                    if let Some(next_addr) = work_list.pop() {
                        addr = next_addr;
                        continue;
                    } else {
                        break;
                    }
                },
                // If halt or directive, guaranteed end of function
                StmtKind::Instr(AsmInstr::HALT) | StmtKind::Directive(_) => {
                    break;
                },
                _ => {
                    // Any other instruction falls through to increment
                }
            }

            addr += 1;
        }
    }

    function_list
}

fn get_block_at(start_address: u16, block_list: &mut HashMap<u16, BasicBlock>) -> &mut BasicBlock {
    block_list.entry(start_address).or_insert_with(|| BasicBlock::new(start_address))
}

fn find_next_label(current_address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<u16> {
    let mut addr = current_address + 1;
    while let Some(stmt) = disassembly.get(&addr) {
        if !stmt.labels.is_empty() {
            return Some(addr);
        }
        addr += 1;
    }
    None
}

fn find_next_branch(current_address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<(u16, u16, bool, bool)> {
    let mut addr = current_address;
    while let Some(stmt) = disassembly.get(&addr) {
        let is_branch = matches!(&stmt.nucleus,
            StmtKind::Instr(AsmInstr::BR(_, _)) |
            StmtKind::Instr(AsmInstr::JMP(_)) |
            StmtKind::Instr(AsmInstr::RET) |
            StmtKind::Instr(AsmInstr::HALT));

        if is_branch {
            let destination_known = matches!(&stmt.nucleus, StmtKind::Instr(AsmInstr::BR(_, _)));
            let is_conditional = match &stmt.nucleus {
                StmtKind::Instr(AsmInstr::BR(cc, _)) => *cc != 7,
                _ => false,
            };
            return Some((addr, addr + 1, destination_known, is_conditional));
        }
        addr += 1;
    }
    None
}

fn get_branch_destination(address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<u16> {
    if let Some(stmt) = disassembly.get(&address) {
        if let StmtKind::Instr(AsmInstr::BR(_, pcoffset9)) = &stmt.nucleus {
            if let PCOffset::Offset(offset) = pcoffset9 {
                return Some((address as i16 + 1 + offset.get()) as u16);
            }
        }
    }
    None
}

fn create_basic_blocks(entry_address: u16, disassembly: &HashMap<u16, Stmt>) -> HashMap<u16, BasicBlock> {
    let mut block_list: HashMap<u16, BasicBlock> = HashMap::new();
    let mut work_list: Vec<u16> = Vec::new();

    get_block_at(entry_address, &mut block_list);
    work_list.push(entry_address);

    while let Some(block_start) = work_list.pop() {
        if block_list.get(&block_start).map_or(false, |b| b.length > 0) {
            continue;
        }

        let next_label = find_next_label(block_start, disassembly);
        let next_branch = find_next_branch(block_start, disassembly);

        let mut successors = Vec::new();
        let block_end_addr;

        match (next_label, next_branch) {
            (Some(label_addr), Some((branch_addr, branch_after, dest_known, is_cond))) => {
                if label_addr < branch_after {
                    block_end_addr = label_addr;
                    successors.push(label_addr);
                } else {
                    block_end_addr = branch_after;
                    if dest_known {
                        if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                            successors.push(dest_addr);
                        }
                    }
                    if is_cond {
                        successors.push(branch_after);
                    }
                }
            },
            (None, Some((branch_addr, branch_after, dest_known, is_cond))) => {
                block_end_addr = branch_after;
                if dest_known {
                    if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                        successors.push(dest_addr);
                    }
                }
                if is_cond {
                    successors.push(branch_after);
                }
            },
            (Some(label_addr), None) => {
                block_end_addr = label_addr;
                successors.push(label_addr);
            },
            (None, None) => {
                let mut last_addr = block_start;
                while disassembly.contains_key(&(last_addr + 1)) {
                    last_addr += 1;
                }
                block_end_addr = last_addr + 1;
            }
        }

        block_list.get_mut(&block_start).unwrap().length = block_end_addr - block_start;

        for succ_addr in successors {
            get_block_at(succ_addr, &mut block_list);
            let block = block_list.get_mut(&block_start).unwrap();
            if !block.succs.contains(&succ_addr) {
                block.succs.push(succ_addr);
            }
            let succ_block = block_list.get_mut(&succ_addr).unwrap();
            if !succ_block.preds.contains(&block_start) {
                succ_block.preds.push(block_start);
            }
            work_list.push(succ_addr);
        }
    }

    block_list
}

fn compute_dominators(blocks: &mut HashMap<u16, BasicBlock>, entry_addr: u16) {
    let all_blocks: Vec<u16> = blocks.keys().cloned().collect();

    for &block_addr in &all_blocks {
        let block = blocks.get_mut(&block_addr).unwrap();
        if block_addr == entry_addr {
            block.dominators = vec![entry_addr];
        } else {
            block.dominators = all_blocks.clone();
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        let block_addrs_clone = all_blocks.clone();
        for &block_addr in &block_addrs_clone {
            if block_addr == entry_addr {
                continue;
            }

            let preds = blocks[&block_addr].preds.clone();
            if preds.is_empty() { continue; }

            let mut new_doms: HashSet<u16> = blocks[&preds[0]].dominators.iter().cloned().collect();
            for &pred_addr in preds.iter().skip(1) {
                let pred_doms: HashSet<u16> = blocks[&pred_addr].dominators.iter().cloned().collect();
                new_doms = new_doms.intersection(&pred_doms).cloned().collect();
            }
            
            new_doms.insert(block_addr);
            let mut new_doms_vec: Vec<u16> = new_doms.into_iter().collect();
            new_doms_vec.sort();

            let block = blocks.get_mut(&block_addr).unwrap();
            if block.dominators != new_doms_vec {
                block.dominators = new_doms_vec;
                changed = true;
            }
        }
    }
}

fn natural_loop_for_edge(header: u16, tail: u16, blocks: &HashMap<u16, BasicBlock>) -> Loop {
    let mut work_list: Vec<u16> = Vec::new();
    let mut loop_obj = Loop::new(header);

    if header != tail {
        loop_obj.blocks.insert(tail);
        work_list.push(tail);
    }

    while let Some(block_addr) = work_list.pop() {
        if let Some(block) = blocks.get(&block_addr) {
            for &pred in &block.preds {
                if !loop_obj.blocks.contains(&pred) {
                    loop_obj.blocks.insert(pred);
                    work_list.push(pred);
                }
            }
        }
    }

    loop_obj
}

fn compute_natural_loops(blocks: &HashMap<u16, BasicBlock>) -> Vec<Loop> {
    let mut loop_set: Vec<Loop> = Vec::new();

    for (&block_addr, block) in blocks {
        for &succ in &block.succs {
            if blocks[&succ].dominators.contains(&block_addr) {
                let natural_loop = natural_loop_for_edge(succ, block_addr, blocks);
                loop_set.push(natural_loop);
            }
        }
    }
    
    loop_set
}

// --- NEW CONTROL FLOW STRUCTURING FUNCTIONS (STEP 4) ---

/// Analyzes natural loops to find their exit nodes.
fn identify_loops(natural_loops: &[Loop], blocks: &HashMap<u16, BasicBlock>) -> Vec<IdentifiedLoop> {
    let mut identified_loops = Vec::new();

    for loop_obj in natural_loops {
        let mut exit_nodes = HashSet::new();
        // An exit node is a successor of a block inside the loop
        // that is itself not inside the loop.
        for &loop_block_addr in &loop_obj.blocks {
            if let Some(block) = blocks.get(&loop_block_addr) {
                for &succ_addr in &block.succs {
                    if !loop_obj.blocks.contains(&succ_addr) {
                        exit_nodes.insert(succ_addr);
                    }
                }
            }
        }

        identified_loops.push(IdentifiedLoop {
            header: loop_obj.header,
            blocks: loop_obj.blocks.clone(),
            exit_nodes,
        });
    }

    identified_loops
}

/// Analyzes the CFG to find `if` and `if-else` structures.
/// This function does not modify the CFG.
fn identify_conditionals(blocks: &HashMap<u16, BasicBlock>) -> Vec<IdentifiedConditional> {
    let mut conditionals = Vec::new();
    let mut handled_blocks = HashSet::new();

    let mut block_addrs: Vec<u16> = blocks.keys().cloned().collect();
    block_addrs.sort();

    for &block_addr in &block_addrs {
        if handled_blocks.contains(&block_addr) {
            continue;
        }

        let head_block = if let Some(b) = blocks.get(&block_addr) { b } else { continue; };
        if head_block.succs.len() != 2 {
            continue;
        }

        let s1_addr = head_block.succs[0];
        let s2_addr = head_block.succs[1];

        if handled_blocks.contains(&s1_addr) || handled_blocks.contains(&s2_addr) {
            continue;
        }

        let s1_block = if let Some(b) = blocks.get(&s1_addr) { b } else { continue; };
        let s2_block = if let Some(b) = blocks.get(&s2_addr) { b } else { continue; };

        // Pattern 1: If-Else diamond
        if s1_block.succs.len() == 1 && s2_block.succs.len() == 1 && s1_block.succs[0] == s2_block.succs[0] {
            let join_addr = s1_block.succs[0];
            conditionals.push(IdentifiedConditional {
                kind: ConditionalKind::IfElse,
                condition_block: block_addr,
                true_branch_head: s1_addr,
                false_branch_head: Some(s2_addr),
                join_block: join_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s1_addr);
            handled_blocks.insert(s2_addr);
            continue;
        }

        // Pattern 2: Simple If (s1 is body, s2 is join)
        if s1_block.succs.len() == 1 && s1_block.succs[0] == s2_addr {
            conditionals.push(IdentifiedConditional {
                kind: ConditionalKind::If,
                condition_block: block_addr,
                true_branch_head: s1_addr,
                false_branch_head: None,
                join_block: s2_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s1_addr);
            continue;
        }
        
        // Pattern 3: Simple If (s2 is body, s1 is join)
        if s2_block.succs.len() == 1 && s2_block.succs[0] == s1_addr {
             conditionals.push(IdentifiedConditional {
                kind: ConditionalKind::If,
                condition_block: block_addr,
                true_branch_head: s2_addr,
                false_branch_head: None,
                join_block: s1_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s2_addr);
        }
    }

    conditionals
}
    
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path to obj file>", args[0]);
        std::process::exit(1);
    }

    let input_obj = fs::read_to_string(args[1].clone()).expect("Failed to read file");
    assert!(input_obj.starts_with("LC-3 OBJ FILE"));

    let (text, symbols) = gen_sections(&input_obj);
    let (disassembly, origs) = disassemble(text, symbols);

    println!("Disassembly:");
    let mut sorted_entries: Vec<_> = disassembly.clone().into_iter().collect();
    sorted_entries.sort_by_key(|&(key, _)| key);
    for (addr, stmt) in sorted_entries {
        println!("{:04X}: {}", addr, stmt);
    }
    println!();

    let function_list = identify_code_data(disassembly.clone(), origs);

    println!("Identified functions:");
    for addr in &function_list {
        println!("{:04X}", addr);
    }
    println!();
    
    for &entry_addr in &function_list {
        println!("--- Analyzing function at {:04X} ---", entry_addr);
        let mut blocks = create_basic_blocks(entry_addr, &disassembly);

        compute_dominators(&mut blocks, entry_addr);

        println!("Basic blocks:");
        let mut sorted_blocks: Vec<_> = blocks.iter().collect();
        sorted_blocks.sort_by_key(|&(addr, _)| addr);
        for (addr, block) in sorted_blocks {
            let preds: Vec<String> = block.preds.iter().map(|&p| format!("{:04X}", p)).collect();
            let succs: Vec<String> = block.succs.iter().map(|&s| format!("{:04X}", s)).collect();
            let dominators: Vec<String> = block.dominators.iter().map(|&d| format!("{:04X}", d)).collect();
            println!("Block at {:04X}: length = {}, preds = [{}], succs = [{}], dominators = [{}]",
                     addr, block.length, preds.join(", "), succs.join(", "), dominators.join(", "));
        }
        println!();

        let natural_loops = compute_natural_loops(&blocks);

        println!("Natural loops (from back-edges):");
        for (i, loop_obj) in natural_loops.iter().enumerate() {
            let loop_blocks: Vec<String> = loop_obj.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
            println!("Loop {}: header = {:04X}, blocks = [{}]", 
                     i + 1, loop_obj.header, loop_blocks.join(", "));
        }
        println!();

        // --- STEP 4: CONTROL FLOW STRUCTURING ---
        let identified_loops = identify_loops(&natural_loops, &blocks);
        let identified_conditionals = identify_conditionals(&blocks);

        println!("Identified Loop Structures:");
        for (i, loop_info) in identified_loops.iter().enumerate() {
            let loop_blocks: Vec<String> = loop_info.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
            let exit_nodes: Vec<String> = loop_info.exit_nodes.iter().map(|&b| format!("{:04X}", b)).collect();
            println!("Loop {}: header = {:04X}, blocks = [{}], exits = [{}]",
                     i + 1, loop_info.header, loop_blocks.join(", "), exit_nodes.join(", "));
        }
        println!();

        println!("Identified Conditional Structures:");
        for (i, cond) in identified_conditionals.iter().enumerate() {
            match cond.kind {
                ConditionalKind::If => {
                    println!("Conditional {} (If): condition_block = {:04X}, true_branch = {:04X}, join_block = {:04X}",
                             i + 1, cond.condition_block, cond.true_branch_head, cond.join_block);
                }
                ConditionalKind::IfElse => {
                    println!("Conditional {} (If-Else): condition_block = {:04X}, true_branch = {:04X}, false_branch = {:04X}, join_block = {:04X}",
                             i + 1, cond.condition_block, cond.true_branch_head, cond.false_branch_head.unwrap(), cond.join_block);
                }
            }
        }
        println!();
    }
}

