use std::fs;
use std::collections::{HashMap, HashSet};
use lc3_ensemble::ast::{asm::{disassemble_line, AsmInstr, Stmt, StmtKind}, Label, PCOffset};

#[derive(Debug, Clone)]
struct BasicBlock {
    address: u16,
    length: u16,
    preds: Vec<u16>,
    succs: Vec<u16>,
    dominators: Vec<u16>,
    visited: bool,
    statements: Vec<HighLevelStmt>,
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
            statements: Vec::new(),
        }
    }
}

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

#[derive(Debug, Clone)]
enum HighLevelStmt {
    DoWhile {
        expr: Option<String>,
        blocks: Vec<u16>,
        break_block: Option<u16>,
        continue_block: Option<u16>,
    },
    If {
        condition: String,
        true_blocks: Vec<HighLevelStmt>,
        else_blocks: Vec<HighLevelStmt>,
        destination: Option<u16>,
    },
    Goto {
        destination: u16,
    },
    Break,
    Continue,
    Assignment {
        content: String,
    },
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
            let stmt = &disassembly[&addr];

            match &stmt.nucleus {
                // Instruction is jump
                StmtKind::Instr(AsmInstr::BR(_cc, pcoffset9)) => {
                    let value = match pcoffset9 {
                        PCOffset::Offset(offset) => Some(offset.get()),
                        PCOffset::Label(_) => None,
                    }.unwrap();
                    work_list.push((addr as i16 + 1 + value) as u16);
                },
                // Instruction is call
                StmtKind::Instr(AsmInstr::JSR(pcoffset11)) => {
                    let value = match pcoffset11 {
                        PCOffset::Offset(offset) => Some(offset.get()),
                        PCOffset::Label(_) => None,
                    }.unwrap();
                    call_list.push((addr as i16 + 1 + value) as u16);
                },
                // Instruction is ret
                StmtKind::Instr(AsmInstr::RET) => {
                    if work_list.is_empty() {
                        break;
                    }
                },
                // Instruction is unconditional jump/call
                StmtKind::Instr(AsmInstr::JMP(_reg)) => {
                    addr = work_list.pop().unwrap();
                    continue;
                },
                StmtKind::Instr(AsmInstr::JSRR(_reg)) => {
                    addr = work_list.pop().unwrap();
                    continue;
                },
                // If halt or directive, guaranteed end of function
                StmtKind::Instr(AsmInstr::HALT) => {
                    break;
                },
                StmtKind::Directive(_) => {
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
    if !block_list.contains_key(&start_address) {
        block_list.insert(start_address, BasicBlock::new(start_address));
    }
    block_list.get_mut(&start_address).unwrap()
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
        let is_branch = match &stmt.nucleus {
            // Jump, branch, return, and halt instructions all end basic blocks
            // Calls do not end basic blocks
            StmtKind::Instr(AsmInstr::BR(_, _)) |
            StmtKind::Instr(AsmInstr::JMP(_)) |
            StmtKind::Instr(AsmInstr::RET) |
            StmtKind::Instr(AsmInstr::HALT) => true,
            _ => false,
        };

        if is_branch {
            let destination_known = match &stmt.nucleus {
                StmtKind::Instr(AsmInstr::BR(_, _)) => true,
                StmtKind::Instr(AsmInstr::JMP(_)) => false,
                StmtKind::Instr(AsmInstr::RET) => false,
                StmtKind::Instr(AsmInstr::HALT) => false,
                _ => false,
            };
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
        match &stmt.nucleus {
            StmtKind::Instr(AsmInstr::BR(_, pcoffset9)) => {
                if let PCOffset::Offset(offset) = pcoffset9 {
                    return Some((address as i16 + 1 + offset.get()) as u16);
                }
            },
            _ => {}
        }
    }
    None
}

fn create_basic_blocks(entry_address: u16, disassembly: &HashMap<u16, Stmt>) -> HashMap<u16, BasicBlock> {
    let mut block_list: HashMap<u16, BasicBlock> = HashMap::new();
    let mut work_list: Vec<u16> = Vec::new();

    // Initialize with entry block
    {
        let entry_block = get_block_at(entry_address, &mut block_list);
        entry_block.address = entry_address;
    }
    work_list.push(entry_address);

    while let Some(current_address) = work_list.pop() {
        if block_list.contains_key(&current_address) && block_list[&current_address].length > 0 {
            continue;
        }

        let block_start = current_address;

        loop {
            let next_label = find_next_label(current_address, disassembly);
            let next_branch = find_next_branch(current_address, disassembly);

            match (next_label, next_branch) {
                // Both label and branch found
                (Some(label_addr), Some((branch_addr, branch_after, destination_known, is_conditional))) => {
                    // Label is first, block ends at label
                    if label_addr < branch_after {
                        {
                            let block = get_block_at(block_start, &mut block_list);
                            block.length = label_addr - block_start;
                        }

                        // Successor relationship
                        let next_block_addr = label_addr;
                        get_block_at(next_block_addr, &mut block_list);

                        {
                            let block = get_block_at(block_start, &mut block_list);
                            block.succs.push(next_block_addr);
                        }
                        {
                            let next_block = get_block_at(next_block_addr, &mut block_list);
                            next_block.preds.push(block_start);
                        }

                        work_list.push(label_addr);
                        break;
                    }
                    // Branch comes first, block ends at branch
                    else {
                        {
                            let block = get_block_at(block_start, &mut block_list);
                            block.length = branch_after - block_start;
                        }

                        if !destination_known {
                            break;
                        }

                        // Branch destination address
                        if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                            get_block_at(dest_addr, &mut block_list);

                            {
                                let block = get_block_at(block_start, &mut block_list);
                                block.succs.push(dest_addr);
                            }
                            {
                                let dest_block = get_block_at(dest_addr, &mut block_list);
                                dest_block.preds.push(block_start);
                            }

                            work_list.push(dest_addr);
                        }

                        if !is_conditional {
                            break;
                        }

                        // Fall through for conditional branches
                        let fall_through_addr = branch_after;
                        get_block_at(fall_through_addr, &mut block_list);

                        {
                            let block = get_block_at(block_start, &mut block_list);
                            block.succs.push(fall_through_addr);
                        }
                        {
                            let fall_through_block = get_block_at(fall_through_addr, &mut block_list);
                            fall_through_block.preds.push(block_start);
                        }

                        work_list.push(fall_through_addr);
                        break;
                    }
                },
                // Only branch found
                (None, Some((branch_addr, branch_after, destination_known, is_conditional))) => {
                    {
                        let block = get_block_at(block_start, &mut block_list);
                        block.length = branch_after - block_start;
                    }

                    if !destination_known {
                        break;
                    }

                    if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                        get_block_at(dest_addr, &mut block_list);

                        {
                            let block = get_block_at(block_start, &mut block_list);
                            block.succs.push(dest_addr);
                        }
                        {
                            let dest_block = get_block_at(dest_addr, &mut block_list);
                            dest_block.preds.push(block_start);
                        }

                        work_list.push(dest_addr);
                    }

                    if !is_conditional {
                        break;
                    }

                    let fall_through_addr = branch_after;
                    get_block_at(fall_through_addr, &mut block_list);

                    {
                        let block = get_block_at(block_start, &mut block_list);
                        block.succs.push(fall_through_addr);
                    }
                    {
                        let fall_through_block = get_block_at(fall_through_addr, &mut block_list);
                        fall_through_block.preds.push(block_start);
                    }

                    work_list.push(fall_through_addr);
                    break;
                },
                // Only label found
                (Some(label_addr), None) => {
                    {
                        let block = get_block_at(block_start, &mut block_list);
                        block.length = label_addr - block_start;
                    }

                    let next_block_addr = label_addr;
                    get_block_at(next_block_addr, &mut block_list);

                    {
                        let block = get_block_at(block_start, &mut block_list);
                        block.succs.push(next_block_addr);
                    }
                    {
                        let next_block = get_block_at(next_block_addr, &mut block_list);
                        next_block.preds.push(block_start);
                    }

                    work_list.push(label_addr);
                    break;
                },
                // No label or branch found
                (None, None) => {
                    break;
                }
            }
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

        for &block_addr in &all_blocks {
            if block_addr == entry_addr {
                continue;
            }

            let preds = blocks[&block_addr].preds.clone();
            let mut new_dominators: Option<HashSet<u16>> = None;

            for &pred in &preds {
                let pred_dominators: HashSet<u16> = blocks[&pred].dominators.iter().cloned().collect();

                match new_dominators {
                    None => new_dominators = Some(pred_dominators),
                    Some(ref mut current) => {
                        *current = current.intersection(&pred_dominators).cloned().collect();
                    }
                }
            }

            let mut final_dominators = new_dominators.unwrap_or_else(HashSet::new);
            final_dominators.insert(block_addr);

            let mut final_dominators_vec: Vec<u16> = final_dominators.into_iter().collect();
            final_dominators_vec.sort();

            let current_dominators = &blocks[&block_addr].dominators;
            if current_dominators != &final_dominators_vec {
                changed = true;
                blocks.get_mut(&block_addr).unwrap().dominators = final_dominators_vec;
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

fn compute_natural_loops(blocks: &HashMap<u16, BasicBlock>, entry_addr: u16) -> Vec<Loop> {
    let mut loop_set: Vec<Loop> = Vec::new();

    for (&block_addr, block) in blocks {
        if block_addr == entry_addr {
            continue;
        }

        for &succ in &block.succs {
            // Every successor that dominates its predecessor
            // must be the header of a loop.
            // That is, block -> succ is a back edge.

            if block.dominators.contains(&succ) {
                let natural_loop = natural_loop_for_edge(succ, block_addr, blocks);
                loop_set.push(natural_loop);
            }
        }
    }
    
    loop_set
}

fn get_last_compare_instruction(block_addr: u16, disassembly: &HashMap<u16, Stmt>, blocks: &HashMap<u16, BasicBlock>) -> Option<String> {
    // Find last compare instruction in block. For LC-3 this is the instruction before the last
    // branch. Typically an ADD that sets condition codes. Combine this with cc for expr.

    if let Some(block) = blocks.get(&block_addr) {
        let block_start = block_addr;
        let block_end = block_addr + block.length - 1;

        // Start with branch instruction at the end of the block
        let mut branch_cc = 7;
        if let Some(stmt) = disassembly.get(&block_end) {
            if let StmtKind::Instr(AsmInstr::BR(cc, _)) = &stmt.nucleus {
                branch_cc = *cc;
            }
        }

        // Search backwards for last condition code setting instruction
        // ADD, AND, NOT, LD, LDI, LDR set condition codes
        for addr in (block_start..block_end).rev() {
            if let Some(stmt) = disassembly.get(&addr) {
                let sets_condition_code = match &stmt.nucleus {
                    StmtKind::Instr(AsmInstr::ADD(_, _, _)) => true,
                    StmtKind::Instr(AsmInstr::AND(_, _, _)) => true,
                    StmtKind::Instr(AsmInstr::NOT(_, _)) => true,
                    StmtKind::Instr(AsmInstr::LD(_, _)) => true,
                    StmtKind::Instr(AsmInstr::LDI(_, _)) => true,
                    StmtKind::Instr(AsmInstr::LDR(_, _, _)) => true,
                    _ => false,
                };

                if sets_condition_code {
                    let instr_str = format!("{}", stmt);
                    let condition = match branch_cc {
                        0 => "true".to_string(), // never branch = always continue
                        1 => format!("({}) <= 0", instr_str), // branch if positive = continue if not positive
                        2 => format!("({}) != 0", instr_str), // branch if zero = continue if not zero
                        3 => format!("({}) < 0", instr_str), // branch if >= 0 = continue if < 0
                        4 => format!("({}) >= 0", instr_str), // branch if negative = continue if not negative
                        5 => format!("({}) == 0", instr_str), // branch if not zero = continue if zero
                        6 => format!("({}) > 0", instr_str), // branch if <= 0 = continue if > 0
                        7 => "false".to_string(), // always branch = never continue (shouldn't happen in loops)
                        _ => format!("unknown_condition({})", instr_str),
                    };
                    return Some(condition);
                }
            }
        }
    }
    
    Some("true".to_string())
}

fn structure_break_continue(stmt: &mut HighLevelStmt, cont_block: Option<u16>, break_block: Option<u16>) {
    match stmt {
        HighLevelStmt::Goto { destination } => {
            if let Some(cont) = cont_block {
                if *destination == cont {
                    *stmt = HighLevelStmt::Continue;
                    return;
                }
            }
            if let Some(brk) = break_block {
                if *destination == brk {
                    *stmt = HighLevelStmt::Break;
                    return;
                }
            }
        },
        HighLevelStmt::If { true_blocks, else_blocks, .. } => {
            for true_stmt in true_blocks.iter_mut() {
                structure_break_continue(true_stmt, cont_block, break_block);
            }
            for else_stmt in else_blocks.iter_mut() {
                structure_break_continue(else_stmt, cont_block, break_block);
            }
        },
        _ => {}
    }
}

fn structure_loops(loops: &mut Vec<Loop>, blocks: &mut HashMap<u16, BasicBlock>, disassembly: &HashMap<u16, Stmt>) {
    // Sort loops from innermost loop to outermost loop
    loops.sort_by(|a, b| a.blocks.len().cmp(&b.blocks.len()));

    // for each loop in loopSet
    for loop_obj in loops.iter() {
        // doWhile->expr = new Expr(loop->blocks.last->lastCompareInstr)
        let expr = get_last_compare_instruction(loop_obj.header, disassembly, blocks);

        // doWhile->blocks = loop->blocks
        let do_while_blocks: Vec<u16> = loop_obj.blocks.iter().cloned().collect();

        // doWhile->breakBlock = loop->blocks->postDominator
        let post_dominator = {
            let mut result = None;
            for &loop_block in &loop_obj.blocks {
                if let Some(block) = blocks.get(&loop_block) {
                    for &succ in &block.succs {
                        if !loop_obj.blocks.contains(&succ) {
                            result = Some(succ);
                            break;
                        }
                    }
                }
                if result.is_some() {
                    break;
                }
            }
            result
        };
        let break_block = post_dominator;

        // doWhile->continueBlock = loop->blocks.last
        let last_block = loop_obj.blocks.iter()
            .filter(|&&addr| {
                if let Some(block) = blocks.get(&addr) {
                    block.succs.contains(&loop_obj.header)
                } else {
                    false
                }
            })
            .next()
            .copied();
        let continue_block = last_block;

        // doWhile = new Statement(DoWhile)
        let mut do_while = HighLevelStmt::DoWhile {
            expr,
            blocks: do_while_blocks,
            break_block,
            continue_block,
        };
        
        // Check if continue block should be nullified
        // if doWhile->continueBlock->onlyStatement != If or
        //    doWhile->continueBlock->onlyStatement.destination != header
        //     doWhile->continueBlock = NULL
        if let Some(cont_addr) = continue_block {
            if let Some(cont_block) = blocks.get(&cont_addr) {
                let should_nullify = cont_block.statements.len() > 0; // has structured statements

                if should_nullify {
                    if let HighLevelStmt::DoWhile { ref mut continue_block, .. } = do_while {
                        *continue_block = None;
                    }
                }
            }
        }

        // StructureBreakContinue(doWhile, doWhile->continueBlock, doWhile->breakBlock)
        structure_break_continue(&mut do_while, continue_block, break_block);

        // loop->header->lastStatement = doWhile
        if let Some(header_block) = blocks.get_mut(&loop_obj.header) {
            header_block.statements.push(do_while);
        }
    }
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

        let mut loops = compute_natural_loops(&blocks, entry_addr);

        println!("Natural loops:");
        for (i, loop_obj) in loops.iter().enumerate() {
            let loop_blocks: Vec<String> = loop_obj.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
            println!("Loop {}: header = {:04X}, blocks = [{}]", 
                     i + 1, loop_obj.header, loop_blocks.join(", "));
        }
        println!();

        structure_loops(&mut loops, &mut blocks, &disassembly);

        for (addr, block) in blocks.iter() {
            if !block.statements.is_empty() {
                println!("Block {:04X} structured statements:", addr);
                for (i, stmt) in block.statements.iter().enumerate() {
                    match stmt {
                        HighLevelStmt::DoWhile { expr, blocks, break_block, continue_block } => {
                            let true_str = &"true".to_string();
                            let expr_str = expr.as_ref().unwrap_or(true_str);
                            let blocks_str: Vec<String> = blocks.iter().map(|&b| format!("{:04X}", b)).collect();
                            let break_str = break_block.map(|b| format!("{:04X}", b)).unwrap_or("None".to_string());
                            let continue_str = continue_block.map(|b| format!("{:04X}", b)).unwrap_or("None".to_string());
                            println!("  {}: do-while (condition: {}, blocks: [{}], break: {}, continue: {})", 
                                   i, expr_str, blocks_str.join(", "), break_str, continue_str);
                        },
                        _ => {
                            println!("  {}: {:?}", i, stmt);
                        }
                    }
                }
            }
        }
        println!();
    }
}

