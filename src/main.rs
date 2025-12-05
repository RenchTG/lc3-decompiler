use std::fs;
use std::fmt;
use std::collections::{HashMap, HashSet};
use lc3_ensemble::ast::{asm::{disassemble_line, AsmInstr, Stmt, StmtKind}, Label, PCOffset, ImmOrReg};

#[derive(Debug, Clone)]
struct BasicBlock {
    length: u16,
    preds: Vec<u16>,
    succs: Vec<u16>,
    dominators: Vec<u16>,
}

impl BasicBlock {
    fn new() -> Self {
        BasicBlock {
            length: 0,
            preds: Vec::new(),
            succs: Vec::new(),
            dominators: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct NaturalLoop {
    header: u16,
    blocks: HashSet<u16>,
}

impl NaturalLoop {
    fn new(header: u16) -> Self {
        let mut blocks = HashSet::new();
        blocks.insert(header);
        NaturalLoop {
            header,
            blocks,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Loop {
    header: u16,
    blocks: HashSet<u16>,
    break_block: Option<u16>,
    continue_block: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum ConditionalType {
    If,
    IfElse,
}

#[derive(Debug, Clone)]
pub struct Conditional {
    kind: ConditionalType,
    condition_block: u16,
    true_block: u16,
    false_block: Option<u16>,
    join_block: u16,
}

#[derive(Debug, Clone, Default)]
struct LivenessInfo {
    defs: u8,
    uses: u8,
    live_in: u8,
    live_out: u8,
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
    // work_list scans the current function's blocks
    let mut work_list: Vec<u16> = Vec::new();
    let mut visited: HashSet<u16> = HashSet::new();

    while !call_list.is_empty() {
        let mut addr = call_list.pop().unwrap();
        
        // If we've already visited this address (as an entry point or code), we might skip pushing to function list?
        // But function_list tracks *entry points*.
        // Only push if it's a new function entry.
        // But for now, let's just avoid re-scanning code.
        if visited.contains(&addr) {
            continue;
        }
        function_list.push(addr);

        loop {
            // Check visited
            if visited.contains(&addr) {
                 if let Some(next) = work_list.pop() {
                     addr = next;
                     continue;
                 } else {
                     break;
                 }
            }
            visited.insert(addr);

            // Get statement
            let stmt = match disassembly.get(&addr) {
                Some(s) => s,
                None => {
                     // Hit non-code.
                     if let Some(next) = work_list.pop() {
                         addr = next;
                         continue;
                     } else {
                         break;
                     }
                }
            };

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
                    if let Some(next_addr) = work_list.pop() {
                        addr = next_addr;
                        continue;
                    } else {
                        break;
                    }
                },
                // Instruction is unconditional jump
                StmtKind::Instr(AsmInstr::JMP(_reg)) => {
                    // Treat as terminator
                    if let Some(next_addr) = work_list.pop() {
                        addr = next_addr;
                        continue;
                    } else {
                        break;
                    }
                },
                // Instruction is call to register
                StmtKind::Instr(AsmInstr::JSRR(_reg)) => {
                    // Assume returns, fallthrough
                },
                // If halt or directive, guaranteed end of path
                StmtKind::Instr(AsmInstr::HALT) => {
                    if let Some(next_addr) = work_list.pop() {
                        addr = next_addr;
                        continue;
                    } else {
                        break;
                    }
                },
                StmtKind::Directive(_) => {
                    if let Some(next_addr) = work_list.pop() {
                        addr = next_addr;
                        continue;
                    } else {
                        break;
                    }
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
        block_list.insert(start_address, BasicBlock::new());
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
            // Branch and label found
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
            // Only branch is found
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
            // Only label found
            (Some(label_addr), None) => {
                block_end_addr = label_addr;
                successors.push(label_addr);
            },
            // Neither found
            (None, None) => {
                let mut last_addr = block_start;
                while disassembly.contains_key(&(last_addr + 1)) {
                    last_addr += 1;
                }
                block_end_addr = last_addr + 1;
            }
        }

        if let Some(block) = block_list.get_mut(&block_start) {
            block.length = block_end_addr - block_start;
        }

        for succ_addr in successors {
            get_block_at(succ_addr, &mut block_list);

            if let Some(block) = block_list.get_mut(&block_start) {
                if !block.succs.contains(&succ_addr) {
                    block.succs.push(succ_addr);
                }
            }

            if let Some(succ_block) = block_list.get_mut(&succ_addr) {
                if !succ_block.preds.contains(&block_start) {
                    succ_block.preds.push(block_start);
                }
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

fn natural_loop_for_edge(header: u16, tail: u16, blocks: &HashMap<u16, BasicBlock>) -> NaturalLoop {
    let mut work_list: Vec<u16> = Vec::new();
    let mut loop_obj = NaturalLoop::new(header);

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

fn compute_natural_loops(blocks: &HashMap<u16, BasicBlock>, entry_addr: u16) -> Vec<NaturalLoop> {
    let mut loop_set: Vec<NaturalLoop> = Vec::new();

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

fn identify_loops(natural_loops: &mut Vec<NaturalLoop>, blocks: &HashMap<u16, BasicBlock>) -> Vec<Loop> {
    // Sort loops from innermost loop to outermost loop
    natural_loops.sort_by(|a, b| a.blocks.len().cmp(&b.blocks.len()));

    let mut loops = Vec::new();

    // for each loop in loopSet
    for natural_loop in natural_loops.iter() {
        // blocks = loop->blocks
        let loop_blocks: HashSet<u16> = natural_loop.blocks.clone();

        // breakBlock = loop->blocks->postDominator
        let post_dominator = {
            let mut result = None;
            for &loop_block in &natural_loop.blocks {
                if let Some(block) = blocks.get(&loop_block) {
                    for &succ in &block.succs {
                        if !natural_loop.blocks.contains(&succ) {
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

        // continueBlock = loop->blocks.last
        let last_block = natural_loop.blocks.iter()
            .filter(|&&addr| {
                if let Some(block) = blocks.get(&addr) {
                    block.succs.contains(&natural_loop.header)
                } else {
                    false
                }
            })
            .next()
            .copied();
        let continue_block = last_block;

        loops.push(Loop {
            header: natural_loop.header,
            blocks: loop_blocks,
            break_block,
            continue_block,
        })
    }

    loops
}

fn identify_conditionals(blocks: &HashMap<u16, BasicBlock>) -> Vec<Conditional> {
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
            conditionals.push(Conditional {
                kind: ConditionalType::IfElse,
                condition_block: block_addr,
                true_block: s1_addr,
                false_block: Some(s2_addr),
                join_block: join_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s1_addr);
            handled_blocks.insert(s2_addr);
            continue;
        }

        // Pattern 2: Simple If (s1 is body, s2 is join)
        if s1_block.succs.len() == 1 && s1_block.succs[0] == s2_addr {
            conditionals.push(Conditional {
                kind: ConditionalType::If,
                condition_block: block_addr,
                true_block: s1_addr,
                false_block: None,
                join_block: s2_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s1_addr);
            continue;
        }
        
        // Pattern 3: Simple If (s2 is body, s1 is join)
        if s2_block.succs.len() == 1 && s2_block.succs[0] == s1_addr {
             conditionals.push(Conditional {
                kind: ConditionalType::If,
                condition_block: block_addr,
                true_block: s2_addr,
                false_block: None,
                join_block: s1_addr,
            });
            handled_blocks.insert(block_addr);
            handled_blocks.insert(s2_addr);
        }
    }

    conditionals
}

fn get_use_def(instr: &AsmInstr) -> (u8, u8) {
    let mut uses = 0u8;
    let mut defs = 0u8;

    // For each instruction return set of registers "defined" (written) and "used" (read)
    match instr {
        AsmInstr::ADD(dst, src1, src2) => {
            defs |= 1 << dst.reg_no();
            uses |= 1 << src1.reg_no();
            match src2 {
                ImmOrReg::Reg(r) => uses |= 1 << r.reg_no(),
                _ => {}
            }
        },
        AsmInstr::AND(dst, src1, src2) => {
            defs |= 1 << dst.reg_no();
            
            // We check for the edge case where AND has an immediate value of 0.
            // In this case, the first register operand's value is completely destroyed, so this should not count as a use.
            let mut src1_used = true;
            if let ImmOrReg::Imm(imm) = src2 {
                if imm.get() == 0 {
                    src1_used = false;
                }
            }

            if src1_used {
                uses |= 1 << src1.reg_no();
            }

            match src2 {
                ImmOrReg::Reg(r) => uses |= 1 << r.reg_no(),
                _ => {}
            }
        },
        AsmInstr::NOT(dst, src) => {
            defs |= 1 << dst.reg_no();
            uses |= 1 << src.reg_no();
        },
        AsmInstr::LD(dst, _) => {
            defs |= 1 << dst.reg_no();
        },
        AsmInstr::LDI(dst, _) => {
            defs |= 1 << dst.reg_no();
        },
        AsmInstr::LDR(dst, base, _) => {
            defs |= 1 << dst.reg_no();
            uses |= 1 << base.reg_no();
        },
        AsmInstr::LEA(dst, _) => {
            defs |= 1 << dst.reg_no();
        },
        AsmInstr::ST(src, _) => {
            uses |= 1 << src.reg_no();
        },
        AsmInstr::STI(src, _) => {
            uses |= 1 << src.reg_no();
        },
        AsmInstr::STR(src, base, _) => {
            uses |= 1 << src.reg_no();
            uses |= 1 << base.reg_no();
        },
        AsmInstr::JMP(base) => {
            uses |= 1 << base.reg_no();
        },
        AsmInstr::JSR(_) => {
            defs |= 1 << 7; // R7 is link register
        },
        AsmInstr::JSRR(base) => {
            uses |= 1 << base.reg_no();
            defs |= 1 << 7;
        },
        AsmInstr::RET => {
            uses |= 1 << 7;
        },
        AsmInstr::TRAP(_) => {
            defs |= 1 << 7;
            uses |= 1 << 0;
            defs |= 1 << 0;
        },
        _ => {}
    }

    (uses, defs)
}

// Phase 1: Compute the liveness information for each basic block without considering how that basic block is related to the other basic blocks
fn compute_local_liveness(
    blocks: &HashMap<u16, BasicBlock>, 
    disassembly: &HashMap<u16, Stmt>
) -> (HashMap<u16, LivenessInfo>, HashMap<u16, (u8, u8, u8, u8)>) {
    let mut instr_liveness = HashMap::new();
    let mut block_liveness = HashMap::new();

    for (&block_addr, block) in blocks {
        let mut block_use = 0u8;
        let mut block_def = 0u8;
        
        for i in 0..block.length {
            let addr = block_addr + i;
            if let Some(stmt) = disassembly.get(&addr) {
                if let StmtKind::Instr(ref instr) = stmt.nucleus {
                    let (uses, defs) = get_use_def(instr);
                    
                    // Update block use/def
                    // Use[B] |= (uses & !block_def)
                    block_use |= uses & !block_def;
                    // Def[B] |= defs
                    block_def |= defs;
                    
                    // Store initial info for instr
                    instr_liveness.insert(addr, LivenessInfo {
                        defs,
                        uses,
                        live_in: 0,
                        live_out: 0,
                    });
                }
            }
        }
        
        block_liveness.insert(block_addr, (block_use, block_def, 0, 0));
    }
    
    (instr_liveness, block_liveness)
}

// Phase 2: Work on the information in each block by propagating the liveness information from each block to all the other blocks in the CFG
fn propagate_global_liveness(
    blocks: &HashMap<u16, BasicBlock>,
    block_liveness: &mut HashMap<u16, (u8, u8, u8, u8)>
) {
    let mut changed = true;
    while changed {
        changed = false;
        let keys: Vec<u16> = blocks.keys().cloned().collect();
        
        for block_addr in keys {
            let (use_b, def_b, _, _) = block_liveness[&block_addr];
            let block = &blocks[&block_addr];
            
            let mut out_b = 0u8;
            for &succ in &block.succs {
                if let Some((_, _, in_s, _)) = block_liveness.get(&succ) {
                    out_b |= in_s;
                }
            }
            
            let in_b = use_b | (out_b & !def_b);
            
            let entry = block_liveness.get_mut(&block_addr).unwrap();
            if entry.2 != in_b || entry.3 != out_b {
                entry.2 = in_b;
                entry.3 = out_b;
                changed = true;
            }
        }
    }
}

// Phase 3: Add the information collected from the other blocks to each instruction present in each block
fn compute_final_liveness(
    blocks: &HashMap<u16, BasicBlock>,
    block_liveness: &HashMap<u16, (u8, u8, u8, u8)>,
    instr_liveness: &mut HashMap<u16, LivenessInfo>,
) {
    for (&block_addr, block) in blocks {
        let (_, _, _, out_b) = block_liveness[&block_addr];
        let mut current_live = out_b;
        
        for i in (0..block.length).rev() {
            let addr = block_addr + i;
            if let Some(info) = instr_liveness.get_mut(&addr) {
                info.live_out = current_live;
                info.live_in = info.uses | (info.live_out & !info.defs);
                current_live = info.live_in;
            }
        }
    }
}

#[derive(Clone)]
enum Expr {
    Register(u8),
    Immediate(i16),

    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Load(Box<Expr>), // Represents memory load from address
}

impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Register(r) => write!(f, "Register({})", r),
            Expr::Immediate(val) => write!(f, "Immediate(0x{:X})", val),

            Expr::Add(lhs, rhs) => write!(f, "Add({:?}, {:?})", lhs, rhs),
            Expr::And(lhs, rhs) => write!(f, "And({:?}, {:?})", lhs, rhs),
            Expr::Not(e) => write!(f, "Not({:?})", e),
            Expr::Neg(e) => write!(f, "Neg({:?})", e),
            Expr::Sub(lhs, rhs) => write!(f, "Sub({:?}, {:?})", lhs, rhs),
            Expr::Load(e) => write!(f, "Load({:?})", e),
        }
    }
}

#[derive(Clone)]
enum IRStmtKind {
    Assign(u8, Expr),       // Reg = Expr
    Store(Expr, Expr),      // Mem[Addr] = Value
    Goto(Option<Expr>, u8, u16), // Expression, Condition (CC), Target
    Call(Expr),             // JSR/JSRR
    Return,                 // RET
    Trap(u8),               // TRAP vector
    DoWhile(Option<Expr>, u8, HashMap<u16, Vec<IRStmt>>),
    While(Option<Expr>, u8, HashMap<u16, Vec<IRStmt>>),
    For(Box<IRStmt>, Option<Expr>, u8, Box<IRStmt>, HashMap<u16, Vec<IRStmt>>),
    If(Option<Expr>, u8, Vec<IRStmt>, Option<Vec<IRStmt>>),
    Break,
    Continue,
}

impl fmt::Debug for IRStmtKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRStmtKind::Assign(reg, expr) => write!(f, "Assign({}, {:?})", reg, expr),
            IRStmtKind::Store(addr, val) => write!(f, "Store({:?}, {:?})", addr, val),
            IRStmtKind::Goto(expr, cc, target) => write!(f, "Goto({:?}, 0x{:X}, 0x{:X})", expr, cc, target),
            IRStmtKind::Call(target) => write!(f, "Call({:?})", target),
            IRStmtKind::Return => write!(f, "Return"),
            IRStmtKind::Trap(vect) => write!(f, "Trap(0x{:X})", vect),
            IRStmtKind::DoWhile(cond, cc, body) => {
                write!(f, "DoWhile({:?}, 0x{:X}, {{", cond, cc)?;
                let mut sorted_body: Vec<_> = body.iter().collect();
                sorted_body.sort_by_key(|&(addr, _)| addr);
                for (addr, stmts) in sorted_body {
                    write!(f, "\n  Block {:04X}:", addr)?;
                    for stmt in stmts {
                        write!(f, "\n    {:?}", stmt.kind)?;
                    }
                }
                write!(f, "\n}})")
            },
            IRStmtKind::While(cond, cc, body) => {
                write!(f, "While({:?}, 0x{:X}, {{", cond, cc)?;
                let mut sorted_body: Vec<_> = body.iter().collect();
                sorted_body.sort_by_key(|&(addr, _)| addr);
                for (addr, stmts) in sorted_body {
                    write!(f, "\n  Block {:04X}:", addr)?;
                    for stmt in stmts {
                        write!(f, "\n    {:?}", stmt.kind)?;
                    }
                }
                write!(f, "\n}})")
            },
            IRStmtKind::For(init, cond, cc, incr, body) => {
                write!(f, "For({:?}, {:?}, 0x{:X}, {:?}, {{", init.kind, cond, cc, incr.kind)?;
                let mut sorted_body: Vec<_> = body.iter().collect();
                sorted_body.sort_by_key(|&(addr, _)| addr);
                for (addr, stmts) in sorted_body {
                    write!(f, "\n  Block {:04X}:", addr)?;
                    for stmt in stmts {
                        write!(f, "\n    {:?}", stmt.kind)?;
                    }
                }
                write!(f, "\n}})")
            },
            IRStmtKind::If(cond, cc, true_branch, false_branch) => {
                write!(f, "If({:?}, 0x{:X}, {{", cond, cc)?;
                for stmt in true_branch {
                    write!(f, "\n    {:?}", stmt.kind)?;
                }
                write!(f, "\n  }}")?;
                if let Some(false_branch) = false_branch {
                    write!(f, " else {{")?;
                    for stmt in false_branch {
                        write!(f, "\n    {:?}", stmt.kind)?;
                    }
                    write!(f, "\n  }}")?;
                }
                write!(f, ")")
            },
            IRStmtKind::Break => write!(f, "Break"),
            IRStmtKind::Continue => write!(f, "Continue"),
        }
    }
}

#[derive(Debug, Clone)]
struct IRStmt {
    addr: u16,
    kind: IRStmtKind,
}

fn propagate_expressions(
    blocks: &HashMap<u16, BasicBlock>,
    disassembly: &HashMap<u16, Stmt>,
    instr_liveness: &HashMap<u16, LivenessInfo>
) -> HashMap<u16, Vec<IRStmt>> {
    let mut lifted_blocks = HashMap::new();

    for (&block_addr, block) in blocks {
        let mut ir_stmts = Vec::new();
        let mut pending_assignments: HashMap<u8, (Expr, u16)> = HashMap::new();
        let mut pending_order: Vec<u8> = Vec::new();

        for i in 0..block.length {
            let addr = block_addr + i;
            if let Some(stmt) = disassembly.get(&addr) {
                if let StmtKind::Instr(ref instr) = stmt.nucleus {
                    let liveness = &instr_liveness[&addr];
                    
                    match instr {
                        AsmInstr::ADD(dst, src1, src2) => {
                            let dst_reg = dst.reg_no();
                            // Don't remove dst_reg yet! It might be src1 or src2.
                            
                            let src1_reg = src1.reg_no();
                            let src2_reg = match src2 {
                                ImmOrReg::Reg(r) => Some(r.reg_no()),
                                ImmOrReg::Imm(_) => None,
                            };
                            
                            // Check for multiple uses of same register in this instruction
                            let mut multi_use = false;
                            if let Some(r2) = src2_reg {
                                if r2 == src1_reg {
                                    multi_use = true;
                                }
                            }
                            
                            let op1 = if multi_use {
                                // Flush if pending
                                if let Some((expr, stmt_addr)) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some((expr, stmt_addr)) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    // Propagate if NOT live out OR defined by this instr
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr
                                    } else {
                                        ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    // If multi_use, we already flushed src1 (which is r_reg).
                                    // So pending is empty.
                                    if let Some((expr, stmt_addr)) = pending_assignments.remove(&r_reg) {
                                        pending_order.retain(|&r| r != r_reg);
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr
                                        } else {
                                            ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(r_reg, expr) });
                                            Expr::Register(r_reg)
                                        }
                                    } else { Expr::Register(r_reg) }
                                },
                                ImmOrReg::Imm(imm) => Expr::Immediate(imm.get()),
                            };
                            
                            let expr = Expr::Add(Box::new(op1), Box::new(op2));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::AND(dst, src1, src2) => {
                            let dst_reg = dst.reg_no();
                            
                            let src1_reg = src1.reg_no();
                            let src2_reg = match src2 {
                                ImmOrReg::Reg(r) => Some(r.reg_no()),
                                ImmOrReg::Imm(_) => None,
                            };
                            
                            let mut multi_use = false;
                            if let Some(r2) = src2_reg {
                                if r2 == src1_reg {
                                    multi_use = true;
                                }
                            }
                            
                            let op1 = if multi_use {
                                if let Some((expr, stmt_addr)) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some((expr, stmt_addr)) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr
                                    } else {
                                        ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    if let Some((expr, stmt_addr)) = pending_assignments.remove(&r_reg) {
                                        pending_order.retain(|&r| r != r_reg);
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr
                                        } else {
                                            ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(r_reg, expr) });
                                            Expr::Register(r_reg)
                                        }
                                    } else { Expr::Register(r_reg) }
                                },
                                ImmOrReg::Imm(imm) => Expr::Immediate(imm.get()),
                            };
                            
                            let expr = Expr::And(Box::new(op1), Box::new(op2));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::NOT(dst, src) => {
                            let dst_reg = dst.reg_no();
                            
                            let src_reg = src.reg_no();
                            let op1 = if let Some((expr, stmt_addr)) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let expr = Expr::Not(Box::new(op1));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::LD(dst, pcoffset9) => {
                            let dst_reg = dst.reg_no();
                            
                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0, 
                            };
                            let target_addr = (addr as i16 + 1 + offset) as u16;
                            let expr = Expr::Load(Box::new(Expr::Immediate(target_addr as i16)));
                             if pending_assignments.contains_key(&dst_reg) {
                                 pending_order.retain(|&r| r != dst_reg);
                             }
                             pending_order.push(dst_reg);
                             pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::LDI(dst, pcoffset9) => {
                             let dst_reg = dst.reg_no();
                             let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0, 
                            };
                            let target_addr = (addr as i16 + 1 + offset) as u16;
                            let expr = Expr::Load(Box::new(Expr::Load(Box::new(Expr::Immediate(target_addr as i16)))));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::LDR(dst, base, offset6) => {
                            let dst_reg = dst.reg_no();
                            
                            let base_reg = base.reg_no();
                            let base_op = if let Some((expr, stmt_addr)) = pending_assignments.remove(&base_reg) {
                                pending_order.retain(|&r| r != base_reg);
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(base_reg, expr) });
                                    Expr::Register(base_reg)
                                }
                            } else { Expr::Register(base_reg) };

                            let offset = offset6.get();
                            
                            let addr_expr = Expr::Add(Box::new(base_op), Box::new(Expr::Immediate(offset)));
                            let expr = Expr::Load(Box::new(addr_expr));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (expr, addr));
                        },
                        AsmInstr::LEA(dst, pcoffset9) => {
                            let dst_reg = dst.reg_no();
                            
                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0, 
                            };
                            let target_addr = (addr as i16 + 1 + offset) as u16;
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, (Expr::Immediate(target_addr as i16), addr));
                        },
                        AsmInstr::ST(src, pcoffset9) => {
                            let src_reg = src.reg_no();
                            let src_op = if let Some((expr, stmt_addr)) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0, 
                            };
                            let target_addr = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt {
                                addr,
                                kind: IRStmtKind::Store(Expr::Immediate(target_addr as i16), src_op),
                            });
                        },
                        AsmInstr::STI(src, pcoffset9) => {
                            let src_reg = src.reg_no();
                            let src_op = if let Some((expr, stmt_addr)) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0, 
                            };
                            let target_addr = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt {
                                addr,
                                kind: IRStmtKind::Store(
                                    Expr::Load(Box::new(Expr::Immediate(target_addr as i16))),
                                    src_op
                                ),
                            });
                        },
                        AsmInstr::STR(src, base, offset6) => {
                            let src_reg = src.reg_no();
                            let src_op = if let Some((expr, stmt_addr)) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let base_reg = base.reg_no();
                            let base_op = if let Some((expr, stmt_addr)) = pending_assignments.remove(&base_reg) {
                                pending_order.retain(|&r| r != base_reg);
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(base_reg, expr) });
                                    Expr::Register(base_reg)
                                }
                            } else { Expr::Register(base_reg) };

                            let offset = offset6.get();
                            
                            let addr_expr = Expr::Add(Box::new(base_op), Box::new(Expr::Immediate(offset)));
                            ir_stmts.push(IRStmt {
                                addr,
                                kind: IRStmtKind::Store(addr_expr, src_op),
                            });
                        },
                        AsmInstr::BR(cc, pcoffset9) => {
                            // Flush all pending assignments before branching, as they might affect CC
                            for reg in pending_order.drain(..) {
                                if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                    ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                }
                            }

                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0,
                            };
                            let target = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt {
                                addr,
                                kind: IRStmtKind::Goto(None, *cc, target),
                            });
                        },
                        AsmInstr::JMP(base) => {
                             let _base_reg = base.reg_no();
                             // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }

                             if base.reg_no() == 7 {
                                 ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return });
                             } else {
                                 ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Trap(0xFF) });
                             }
                        },
                        AsmInstr::JSR(pcoffset11) => {
                             // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }

                             let offset = match pcoffset11 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0,
                            };
                             let target = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Immediate(target as i16)) });
                        },
                        AsmInstr::JSRR(base) => {
                             // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                             
                             let src_reg = base.reg_no();
                             // We currently don't fold pending assignments into JSRR base to be safe.
                             ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Register(src_reg)) });
                        },
                        AsmInstr::RET => {
                            // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return });
                        },
                        AsmInstr::TRAP(vect8) => {
                            // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Trap(vect8.get() as u8) });
                        },
                        _ => {}
                    }
                }
            }
        }
        
        for reg in pending_order {
             if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                 ir_stmts.push(IRStmt {
                     addr: stmt_addr,
                     kind: IRStmtKind::Assign(reg, expr),
                 });
             }
        }
        
        lifted_blocks.insert(block_addr, ir_stmts);
    }

    lifted_blocks
}

fn collapse_expr(expr: Expr) -> (Expr, bool) {
    match expr {
        Expr::Register(_) | Expr::Immediate(_) => (expr, false),
        Expr::Load(inner) => {
            let (new_inner, changed) = collapse_expr(*inner);
            (Expr::Load(Box::new(new_inner)), changed)
        },
        Expr::Not(inner) => {
            let (new_inner, changed) = collapse_expr(*inner);
            match new_inner {
                Expr::Not(val) => (*val, true), // Not(Not(x)) -> x
                Expr::Immediate(val) => (Expr::Immediate(!val), true), // Not(Imm) -> Imm
                _ => (Expr::Not(Box::new(new_inner)), changed),
            }
        },
        Expr::Neg(inner) => {
            let (new_inner, changed) = collapse_expr(*inner);
            match new_inner {
                Expr::Neg(val) => (*val, true), // Neg(Neg(x)) -> x
                Expr::Immediate(val) => (Expr::Immediate(-val), true), // Neg(Imm) -> Imm
                _ => (Expr::Neg(Box::new(new_inner)), changed),
            }
        },
        Expr::And(lhs, rhs) => {
            let (new_lhs, changed_lhs) = collapse_expr(*lhs);
            let (new_rhs, changed_rhs) = collapse_expr(*rhs);
            let changed = changed_lhs || changed_rhs;
            
            match (new_lhs, new_rhs) {
                (Expr::Immediate(0), _) | (_, Expr::Immediate(0)) => (Expr::Immediate(0), true),
                (Expr::Immediate(-1), other) | (other, Expr::Immediate(-1)) => (other, true),
                (Expr::Immediate(a), Expr::Immediate(b)) => (Expr::Immediate(a & b), true),
                (lhs, rhs) => {
                     // Check for equality (simple cases)
                     let equal = match (&lhs, &rhs) {
                         (Expr::Register(r1), Expr::Register(r2)) => r1 == r2,
                         (Expr::Immediate(i1), Expr::Immediate(i2)) => i1 == i2,
                         _ => false, // Deep equality is hard without PartialEq, assuming simple for now
                     };
                     if equal {
                         (lhs, true)
                     } else {
                         (Expr::And(Box::new(lhs), Box::new(rhs)), changed)
                     }
                }
            }
        },
        Expr::Add(lhs, rhs) => {
            let (new_lhs, changed_lhs) = collapse_expr(*lhs);
            let (new_rhs, changed_rhs) = collapse_expr(*rhs);
            let changed = changed_lhs || changed_rhs;

            match (new_lhs, new_rhs) {
                (Expr::Immediate(0), other) | (other, Expr::Immediate(0)) => (other, true),
                (Expr::Immediate(a), Expr::Immediate(b)) => (Expr::Immediate(a.wrapping_add(b)), true),
                // Add(Not(x), 1) -> Neg(x)
                (Expr::Not(inner), Expr::Immediate(1)) | (Expr::Immediate(1), Expr::Not(inner)) => {
                    (Expr::Neg(inner), true)
                },
                // Add(a, Neg(b)) -> Sub(a, b)
                (a, Expr::Neg(b)) => (Expr::Sub(Box::new(a), b), true),
                // Add(Neg(b), a) -> Sub(a, b)
                (Expr::Neg(b), a) => (Expr::Sub(Box::new(a), b), true),
                // Associativity: Add(Add(a, Imm(x)), Imm(y)) -> Add(a, Imm(x+y))
                (Expr::Add(inner_lhs, inner_rhs), Expr::Immediate(y)) => {
                    if let Expr::Immediate(x) = *inner_rhs {
                        (Expr::Add(inner_lhs, Box::new(Expr::Immediate(x.wrapping_add(y)))), true)
                    } else {
                        (Expr::Add(Box::new(Expr::Add(inner_lhs, inner_rhs)), Box::new(Expr::Immediate(y))), changed)
                    }
                },
                 (Expr::Immediate(y), Expr::Add(inner_lhs, inner_rhs)) => {
                    if let Expr::Immediate(x) = *inner_rhs {
                         (Expr::Add(inner_lhs, Box::new(Expr::Immediate(x.wrapping_add(y)))), true)
                    } else {
                         (Expr::Add(Box::new(Expr::Immediate(y)), Box::new(Expr::Add(inner_lhs, inner_rhs))), changed)
                    }
                },
                (lhs, rhs) => (Expr::Add(Box::new(lhs), Box::new(rhs)), changed),
            }
        },
        Expr::Sub(lhs, rhs) => {
            let (new_lhs, changed_lhs) = collapse_expr(*lhs);
            let (new_rhs, changed_rhs) = collapse_expr(*rhs);
            let changed = changed_lhs || changed_rhs;
            
            match (new_lhs, new_rhs) {
                (a, Expr::Immediate(0)) => (a, true),
                (Expr::Immediate(a), Expr::Immediate(b)) => (Expr::Immediate(a.wrapping_sub(b)), true),
                 (lhs, rhs) => {
                     // Check for equality (simple cases)
                     let equal = match (&lhs, &rhs) {
                         (Expr::Register(r1), Expr::Register(r2)) => r1 == r2,
                         (Expr::Immediate(i1), Expr::Immediate(i2)) => i1 == i2,
                         _ => false, 
                     };
                     if equal {
                         (Expr::Immediate(0), true)
                     } else {
                         (Expr::Sub(Box::new(lhs), Box::new(rhs)), changed)
                     }
                }
            }
        }
    }
}

fn collapse_stmt(stmt: &mut IRStmt) -> bool {
    match &mut stmt.kind {
        IRStmtKind::Assign(_, expr) => {
            let (new_expr, changed) = collapse_expr(expr.clone());
            if changed {
                *expr = new_expr;
                true
            } else {
                false
            }
        },
        IRStmtKind::Store(addr, val) => {
            let (new_addr, changed_addr) = collapse_expr(addr.clone());
            let (new_val, changed_val) = collapse_expr(val.clone());
            if changed_addr { *addr = new_addr; }
            if changed_val { *val = new_val; }
            changed_addr || changed_val
        },
        IRStmtKind::Goto(Some(cond), _, _) => {
            let (new_cond, changed) = collapse_expr(cond.clone());
            if changed {
                *cond = new_cond;
                true
            } else {
                false
            }
        },
        _ => false,
    }
}

fn collapse_expressions(mut blocks: HashMap<u16, Vec<IRStmt>>) -> HashMap<u16, Vec<IRStmt>> {
    let mut changed = true;
    while changed {
        changed = false;
        for stmts in blocks.values_mut() {
            for stmt in stmts {
                if collapse_stmt(stmt) {
                    changed = true;
                }
            }
        }
    }
    blocks
}

fn apply_goto_transformation(
    blocks: HashMap<u16, Vec<IRStmt>>,
    instr_liveness: &HashMap<u16, LivenessInfo>
) -> HashMap<u16, Vec<IRStmt>> {
    let mut new_blocks = HashMap::new();

    for (addr, stmts) in blocks {
        let mut new_stmts: Vec<IRStmt> = Vec::new();
        let mut i = 0;
        while i < stmts.len() {
            let stmt = &stmts[i];
            match &stmt.kind {
                IRStmtKind::Goto(None, cc, target) if *cc != 0 && *cc != 7 => {
                    // Found a conditional branch that needs an expression
                    // Look at the previous statement
                    if let Some(last_stmt) = new_stmts.last() {
                        if let IRStmtKind::Assign(reg, expr) = &last_stmt.kind {
                            let reg = reg;
                            let expr = expr.clone();
                            
                            // Check liveness of reg at the branch instruction
                            // The branch instruction is 'stmt', with address 'stmt.addr'
                            let is_live = if let Some(info) = instr_liveness.get(&stmt.addr) {
                                (info.live_in & (1 << reg)) != 0
                            } else {
                                true // Assume live if unknown
                            };

                            if !is_live {
                                // Register is dead, consume the assignment
                                new_stmts.pop(); // Remove the Assign
                                new_stmts.push(IRStmt {
                                    addr: stmt.addr,
                                    kind: IRStmtKind::Goto(Some(expr), *cc, *target),
                                });
                            } else {
                                // Register is live, keep assignment, use Register in Goto
                                new_stmts.push(IRStmt {
                                    addr: stmt.addr,
                                    kind: IRStmtKind::Goto(Some(Expr::Register(*reg)), *cc, *target),
                                });
                            }
                        } else {
                            // Previous statement is not an Assign (e.g. Store)
                            // Search backwards for the last Assign
                            let mut assign_idx = None;
                            for (idx, s) in new_stmts.iter().enumerate().rev() {
                                if let IRStmtKind::Assign(_, _) = s.kind {
                                    assign_idx = Some(idx);
                                    break;
                                }
                            }

                            if let Some(idx) = assign_idx {
                                if let IRStmtKind::Assign(_, _) = &new_stmts[idx].kind {
                                     // Only merge if Assign is the *immediate* predecessor.
                                     // If there are intervening statements, we assume we can't easily merge and just use Register(reg).
                                     // But we still need to find which register set the CC.
                                     // Since we can't easily know which register set CC if it's not immediate, 
                                     // we might just leave it as None or use a heuristic.
                                     // However, the prompt implies the Assign is "immediately before".
                                     
                                     new_stmts.push(stmt.clone());
                                } else {
                                     new_stmts.push(stmt.clone());
                                }
                            } else {
                                new_stmts.push(stmt.clone());
                            }
                        }
                    } else {
                        new_stmts.push(stmt.clone());
                    }
                },
                _ => {
                    new_stmts.push(stmt.clone());
                }
            }
            i += 1;
        }
        new_blocks.insert(addr, new_stmts);
    }
    new_blocks
}

fn structure_loops(
    goto_blocks: &mut HashMap<u16, Vec<IRStmt>>, 
    loops: &Vec<Loop>, 
    _blocks: &HashMap<u16, BasicBlock>,
    conditionals: &Vec<Conditional>
) {
    // Sort loops from innermost loop to outermost loop
    // We already did this in identify_loops, but let's be safe or rely on the order passed in.
    // The `loops` vector passed here comes from `identify_loops` which sorts by size.
    
    for loop_info in loops {
        let header = loop_info.header;
        let break_block = loop_info.break_block;
        let continue_block = loop_info.continue_block;
        
        // Extract Loop Body
        let mut loop_body: HashMap<u16, Vec<IRStmt>> = HashMap::new();
        for &block_addr in &loop_info.blocks {
            if let Some(stmts) = goto_blocks.remove(&block_addr) {
                loop_body.insert(block_addr, stmts);
            }
        }
        
        // Identify Loop Condition
        let mut condition: Option<Expr> = None;
        let mut condition_cc: u8 = 0;
        
        if let Some(break_addr) = break_block {
             // Find the jump to break_block
             for stmts in loop_body.values() {
                 for stmt in stmts {
                     if let IRStmtKind::Goto(expr, cc, target) = &stmt.kind {
                         if *target == break_addr {
                             // This is an exit edge.
                             // The condition to CONTINUE is the negation.
                             if let Some(e) = expr {
                                 condition = Some(e.clone());
                                 condition_cc = (!cc) & 7;
                             }
                         } else if *target == header {
                             // This is a back edge (do-while style).
                             // The condition to CONTINUE is exactly this condition.
                             if let Some(e) = expr {
                                 condition = Some(e.clone());
                                 condition_cc = *cc;
                             }
                         }
                     }
                 }
             }
        }
        
        // Remove the back-edge statement if it matches the loop condition
        // because it is now subsumed by the DoWhile structure.
        if let Some(cont_addr) = continue_block {
            if let Some(stmts) = loop_body.get_mut(&cont_addr) {
                stmts.retain(|stmt| {
                    if let IRStmtKind::Goto(expr, cc, target) = &stmt.kind {
                        if *target == header {
                            // Check if matches extracted loop condition
                            let same_cc = *cc == condition_cc;
                            let same_expr = match (expr, &condition) {
                                (Some(e1), Some(e2)) => format!("{:?}", e1) == format!("{:?}", e2), 
                                (None, None) => true,
                                _ => false,
                            };
                            // If it matches, verify if extracting it was correct (i.e. it IS the back edge)
                            // Yes, target == header.
                            if same_cc && same_expr { return false; }
                        }
                    }
                    true
                });
            }
        }
 
        // Apply Break/Continue
        for (block_addr, stmts) in loop_body.iter_mut() {
            // Check if this block is a condition block for any conditional
            let is_condition_block = conditionals.iter().any(|c| c.condition_block == *block_addr);

            for stmt in stmts.iter_mut() {
                if let IRStmtKind::Goto(expr, cc, target) = &stmt.kind {
                    let target_is_break = Some(*target) == break_block;
                    // Only jump to header is a Continue (back-edge)
                    // Jumps to continue_block should remain as Goto to ensure latch code is executed
                    let target_is_continue = *target == header;
                    
                    if (target_is_break || target_is_continue) && !is_condition_block {
                        let new_kind = if target_is_break { IRStmtKind::Break } else { IRStmtKind::Continue };
                        
                        if *cc == 7 {
                            // Unconditional
                            stmt.kind = new_kind;
                        } else {
                            // Conditional - wrap in If
                            // If(expr, cc, vec![Break/Continue], None)
                            stmt.kind = IRStmtKind::If(
                                expr.clone(),
                                *cc,
                                vec![IRStmt { addr: stmt.addr, kind: new_kind }],
                                None
                            );
                        }
                    }
                }
            }
        }
        
        // Remove redundant Continue at the end of continue_block
        if let Some(cont_addr) = continue_block {
            if let Some(stmts) = loop_body.get_mut(&cont_addr) {
                if let Some(last_stmt) = stmts.last() {
                    if let IRStmtKind::Continue = last_stmt.kind {
                        stmts.pop();
                    }
                }
            }
        }
        
        // Create DoWhile Statement
        let do_while_stmt = IRStmt {
            addr: header,
            kind: IRStmtKind::DoWhile(condition, condition_cc, loop_body),
        };
        
        goto_blocks.insert(header, vec![do_while_stmt]);
    }
}

fn substitute_register(expr: &Expr, target_reg: u8, replacement: &Expr) -> Expr {
    match expr {
        Expr::Register(r) => if *r == target_reg { replacement.clone() } else { Expr::Register(*r) },
        Expr::Immediate(v) => Expr::Immediate(*v),
        Expr::Add(l, r) => Expr::Add(Box::new(substitute_register(l, target_reg, replacement)), Box::new(substitute_register(r, target_reg, replacement))),
        Expr::Sub(l, r) => Expr::Sub(Box::new(substitute_register(l, target_reg, replacement)), Box::new(substitute_register(r, target_reg, replacement))),
        Expr::And(l, r) => Expr::And(Box::new(substitute_register(l, target_reg, replacement)), Box::new(substitute_register(r, target_reg, replacement))),
        Expr::Not(e) => Expr::Not(Box::new(substitute_register(e, target_reg, replacement))),
        Expr::Neg(e) => Expr::Neg(Box::new(substitute_register(e, target_reg, replacement))),
        Expr::Load(e) => Expr::Load(Box::new(substitute_register(e, target_reg, replacement))),
    }
}

fn fold_assignments(mut expr: Expr, assignments: &[(u8, Expr)]) -> Expr {
    // Apply assignments in reverse order (latest first)
    for (reg, val) in assignments.iter().rev() {
        expr = substitute_register(&expr, *reg, val);
    }
    expr
}

fn refine_loops(
    goto_blocks: &mut HashMap<u16, Vec<IRStmt>>,
    identified_loops: &Vec<Loop>,
    blocks: &HashMap<u16, BasicBlock>,
    conditionals: &Vec<Conditional>,
    liveness: &HashMap<u16, LivenessInfo>
) {
    // 1. DoWhile -> While
    let keys: Vec<u16> = goto_blocks.keys().cloned().collect();
    let mut init_removals: Vec<(u16, u16)> = Vec::new(); // (block_addr, stmt_addr)

    for key in keys.clone() { // Clone keys for the second loop as well
        if let Some(stmts) = goto_blocks.get_mut(&key) {
            if stmts.len() == 1 {
                let header_addr = stmts[0].addr;
                let stmt = &mut stmts[0];
                if let IRStmtKind::DoWhile(_, _, ref mut body) = stmt.kind {
                     let mut converted_to_while = false;
                     let mut new_while_cond = None;
                     let mut new_while_cc = 0;
                     let mut if_stmt_idx = None;
                     
                     if let Some(entry_stmts) = body.get(&header_addr) {
                        let mut pending_assigns = Vec::new();
                        
                        for (i, s) in entry_stmts.iter().enumerate() {
                            match &s.kind {
                                IRStmtKind::Assign(r, e) => {
                                    pending_assigns.push((*r, e.clone()));
                                },
                                IRStmtKind::If(ref if_cond, ref if_cc, ref true_branch, ref false_branch) => {
                                    // Check if this is a Break
                                    if true_branch.len() == 1 && matches!(true_branch[0].kind, IRStmtKind::Break) && false_branch.is_none() {
                                        // Found candidate
                                        if_stmt_idx = Some(i);
                                        
                                        // Verify liveness on exit
                                        if !pending_assigns.is_empty() {
                                            // Find loop info
                                            if let Some(loop_info) = identified_loops.iter().find(|l| l.header == header_addr) {
                                                // Check if any defined reg is live in the break block
                                                let mut dead_on_exit = true;
                                                if let Some(exit) = loop_info.break_block {
                                                     if let Some(live_info) = liveness.get(&exit) {
                                                         for (r, _) in &pending_assigns {
                                                             if (live_info.live_in & (1 << r)) != 0 {
                                                                 dead_on_exit = false;
                                                                 break;
                                                             }
                                                         }
                                                     }
                                                } else {
                                                    // If we don't know where it breaks to, assume unsafe
                                                    dead_on_exit = false;
                                                }
                                                
                                                if dead_on_exit {
                                                    // Fold!
                                                    if let Some(cond_expr) = if_cond {
                                                        let folded = fold_assignments(cond_expr.clone(), &pending_assigns);
                                                        new_while_cond = Some(folded);
                                                    }
                                                    new_while_cc = (!if_cc) & 7;
                                                    converted_to_while = true;
                                                }
                                            } else {
                                                // Should not happen if data consistent
                                            };
                                        } else {
                                            // No pending assignments, simple move
                                            new_while_cond = if_cond.clone();
                                            new_while_cc = (!if_cc) & 7;
                                            converted_to_while = true;
                                        }
                                    }
                                    // Stop scanning after If (whether we converted or not)
                                    break;
                                },
                                _ => {
                                    // Side effect or flow control
                                    break;
                                }
                            }
                        }
                     }
                     
                     if converted_to_while {
                         // Remove the If statement
                         if let Some(entry_stmts) = body.get_mut(&header_addr) {
                             if let Some(idx) = if_stmt_idx {
                                 entry_stmts.remove(idx);
                             }
                         }
                         stmt.kind = IRStmtKind::While(new_while_cond, new_while_cc, body.clone());
                     }
                }
            }
        }
    }
    
    // Pre-calculate last assignments for While -> For conversion
    let mut last_assignments: HashMap<u16, HashMap<u8, IRStmt>> = HashMap::new();
    for (addr, stmts) in goto_blocks.iter() {
        let mut block_assigns = HashMap::new();
        for stmt in stmts {
            if let IRStmtKind::Assign(r, _) = stmt.kind {
                block_assigns.insert(r, stmt.clone());
            }
        }
        last_assignments.insert(*addr, block_assigns);
    }
    
    // 2. While -> For
    // The `keys` variable is already defined and cloned from the previous section.
    for key in keys {
        if let Some(stmts) = goto_blocks.get_mut(&key) {
            if stmts.len() == 1 {
                let header_addr = stmts[0].addr;
                let stmt = &mut stmts[0];
                // We need to match by reference to avoid moving fields out of the struct
                // except for 'cc' which is Copy, but we can't move 'cond' and 'body' partially.
                // So we match everything by reference.
                if let IRStmtKind::While(ref cond, ref cc, ref mut body) = stmt.kind {
                    
                    // Identify init_stmt in predecessor
                    let mut loop_var = None;
                    
                    // Remove box syntax
                    if let Some(expr) = cond {
                        match expr {
                            Expr::Sub(lhs, _) => {
                                if let Expr::Register(r) = **lhs {
                                    loop_var = Some(r);
                                }
                            },
                            Expr::Register(r) => {
                                loop_var = Some(*r);
                            },
                            _ => {}
                        }
                        
                        // Check for Sub(_, box Expr::Register(r)) case if needed
                        if loop_var.is_none() {
                             if let Expr::Sub(_, rhs) = expr {
                                 if let Expr::Register(r) = **rhs {
                                     loop_var = Some(r);
                                 }
                             }
                        }
                    }
                    
                    if let Some(var) = loop_var {
                        // Check predecessors for assignment to 'var'
                        let mut init_stmt: Option<IRStmt> = None;
                        let mut potential_init_removal = None;

                        if let Some(block) = blocks.get(&header_addr) {
                            for pred in &block.preds {
                                if !body.contains_key(pred) {
                                    if let Some(assigns) = last_assignments.get(pred) {
                                        if let Some(s) = assigns.get(&var) {
                                            init_stmt = Some(s.clone());
                                            potential_init_removal = Some((*pred, s.addr));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Identify incr_stmt in loop body
                        let mut incr_stmt: Option<IRStmt> = None;
                        let mut continue_block_addr = None;
                        
                        // Find the loop info
                        if let Some(loop_info) = identified_loops.iter().find(|l| l.header == header_addr) {
                            if let Some(cont_addr) = loop_info.continue_block {
                                if let Some(stmts) = body.get(&cont_addr) {
                                    continue_block_addr = Some(cont_addr);
                                    
                                    // Check last statement or second to last
                                    if let Some(last) = stmts.last() {
                                        let is_terminator = matches!(last.kind, IRStmtKind::Continue | IRStmtKind::Goto(_, _, _) | IRStmtKind::Break | IRStmtKind::Return | IRStmtKind::Trap(_));
                                        
                                        let potential_incr = if is_terminator {
                                            if stmts.len() >= 2 { Some(&stmts[stmts.len() - 2]) } else { None }
                                        } else {
                                            Some(last)
                                        };
                                        
                                        if let Some(stmt) = potential_incr {
                                            if let IRStmtKind::Assign(r, _) = stmt.kind {
                                                if r == var {
                                                    incr_stmt = Some(stmt.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        
                        if let (Some(init), Some(incr), Some(cont_addr)) = (init_stmt, incr_stmt, continue_block_addr) {
                            // Found both! Convert to For loop.
                            if let Some(removal) = potential_init_removal {
                                init_removals.push(removal);
                            }
                            
                             if let Some(stmts) = body.get_mut(&cont_addr) {
                                 // Remove the increment
                                 // We need to know if it was last or second to last.
                                 // Re-check to be safe or store index.
                                 let is_terminator = if let Some(last) = stmts.last() {
                                     matches!(last.kind, IRStmtKind::Continue | IRStmtKind::Goto(_, _, _) | IRStmtKind::Break | IRStmtKind::Return | IRStmtKind::Trap(_))
                                 } else { false };
                                 
                                 if is_terminator {
                                     if stmts.len() >= 2 { stmts.remove(stmts.len() - 2); }
                                 } else {
                                     stmts.pop();
                                 }
                                 
                                 // If we have a terminator that jumps to header, convert to Continue
                                 // If we have no terminator, we don't need to add Continue because For loop implies it?
                                 // Wait, For loop body executes, then Incr, then Check.
                                 // If we fall through body, we go to Incr.
                                 // So we don't need explicit Continue if we fell through.
                                 
                                 if let Some(last) = stmts.last_mut() {
                                      if let IRStmtKind::Goto(_, _, target) = last.kind {
                                         if target == header_addr {
                                             last.kind = IRStmtKind::Continue;
                                         }
                                     }
                                 }
                             }
                             
                             for (block_addr, stmts) in body.iter_mut() {
                                 let is_condition_block = conditionals.iter().any(|c| c.condition_block == *block_addr);
                                 for s in stmts.iter_mut() {
                                     if let IRStmtKind::Goto(_, _, target) = s.kind {
                                         if target == cont_addr && !is_condition_block {
                                             s.kind = IRStmtKind::Continue;
                                         }
                                     }
                                 }
                             }
                             
                             // cc is &u8 here because of ref cc match
                             stmt.kind = IRStmtKind::For(Box::new(init), cond.clone(), *cc, Box::new(incr), body.clone());
                        }
                    }
                }
            }
        }
    }
    
    // Remove redundant initialization statements
    for (block_addr, stmt_addr) in init_removals {
        if let Some(stmts) = goto_blocks.get_mut(&block_addr) {
            if let Some(pos) = stmts.iter().position(|s| s.addr == stmt_addr) {
                stmts.remove(pos);
            }
        }
    }
}



fn structure_conditionals(
    blocks: &mut HashMap<u16, Vec<IRStmt>>,
    conditionals: &Vec<Conditional>
) {
    // 1. Recurse into Loop bodies
    for stmts in blocks.values_mut() {
        for stmt in stmts.iter_mut() {
            match &mut stmt.kind {
                IRStmtKind::DoWhile(_, _, body) |
                IRStmtKind::While(_, _, body) |
                IRStmtKind::For(_, _, _, _, body) => {
                    structure_conditionals(body, conditionals);
                },
                _ => {}
            }
        }
    }

    // 2. Identify Applicable Conditionals
    // A conditional is applicable if its condition_block is in our `blocks` map.
    let mut applicable_indices: Vec<usize> = Vec::new();
    for (i, cond) in conditionals.iter().enumerate() {
        if blocks.contains_key(&cond.condition_block) {
            applicable_indices.push(i);
        }
    }

    // 3. Sort Conditionals by Dependency
    applicable_indices.sort_by(|&i_a, &i_b| {
        let a = &conditionals[i_a];
        let b = &conditionals[i_b];
        
        // A depends on B if A "contains" B in its branches.
        // Practically, if A branches to B, B is inner (or following).
        // If B is inside A, B must be structured first so A can absorb it.
        // A point to B if A.true == B.cond or A.false == B.cond
        
        let a_points_to_b = a.true_block == b.condition_block || 
                            a.false_block.map_or(false, |fb| fb == b.condition_block);
        
        let b_points_to_a = b.true_block == a.condition_block || 
                            b.false_block.map_or(false, |fb| fb == a.condition_block);
                            
        if a_points_to_b {
            std::cmp::Ordering::Greater // B comes before A
        } else if b_points_to_a {
            std::cmp::Ordering::Less // A comes before B
        } else {
            std::cmp::Ordering::Equal
        }
    });

    // 4. Apply Structuring
    for idx in applicable_indices {
        let cond = &conditionals[idx];
        
        // Check existence without borrowing mutably yet
        if !blocks.contains_key(&cond.condition_block) {
            continue;
        }

        // 1. Snapshot the required info from the condition block
        let (expr_opt, cc, target_addr, stmt_addr) = {
             let stmts = &blocks[&cond.condition_block];
             if let Some(last) = stmts.last() {
                 if let IRStmtKind::Goto(expr, cc, target) = &last.kind {
                     (expr.clone(), *cc, *target, last.addr)
                 } else {
                     continue; // Should not happen if well-formed, or already structured?
                 }
             } else {
                 continue;
             }
        };
        
        // 2. We now need `expr` from `expr_opt`. If it's None, we can't really structure it as If?
        // But Goto(None, ...) is unconditional. Conditionals usually have Some(expr).
        // If it is None, it's not a conditional? AsmInstr::BR with cc != 7 checks CC.
        // But `propagate_expressions` puts None if it couldn't find an expression.
        // We probably should handle None by just using CC check on registers?
        // But the previous code assumed Some(expr).
        
        if let Some(expr) = expr_opt {
            let final_cond = expr;
            let mut final_cc = cc;
            
            let mut true_stmts = Vec::new();
            let mut false_stmts = None;
            
            // 3. Mutate blocks (Remove branches)
            match cond.kind {
                ConditionalType::IfElse => {
                    if let Some(mut stmts) = blocks.remove(&cond.true_block) {
                         if let Some(last) = stmts.last() {
                            if let IRStmtKind::Goto(_, _, t) = last.kind {
                                if t == cond.join_block {
                                    stmts.pop();
                                }
                            }
                        }
                        true_stmts = stmts;
                    }
                    
                    if let Some(fb) = cond.false_block {
                        if let Some(mut stmts) = blocks.remove(&fb) {
                             if let Some(last) = stmts.last() {
                                if let IRStmtKind::Goto(_, _, t) = last.kind {
                                    if t == cond.join_block {
                                        stmts.pop();
                                    }
                                }
                            }
                            false_stmts = Some(stmts);
                        }
                    }
                },
                ConditionalType::If => {
                    if let Some(mut stmts) = blocks.remove(&cond.true_block) {
                         if let Some(last) = stmts.last() {
                            if let IRStmtKind::Goto(_, _, t) = last.kind {
                                if t == cond.join_block {
                                    stmts.pop();
                                }
                            }
                        }
                        true_stmts = stmts;
                    }
                    
                    if target_addr == cond.join_block {
                         final_cc = (!final_cc) & 7;
                    }
                }
            }
            
            // 4. Update the condition block
            if let Some(stmts) = blocks.get_mut(&cond.condition_block) {
                 let if_stmt = IRStmt {
                    addr: stmt_addr, 
                    kind: IRStmtKind::If(Some(final_cond), final_cc, true_stmts, false_stmts),
                };
                
                // We assume it's still the last statement
                let last_idx = stmts.len() - 1;
                stmts[last_idx] = if_stmt;
            }
        }
    }
}

#[derive(Clone)]
enum LinearStmt {
    Assign(u8, Expr),
    Store(Expr, Expr),
    Goto(Option<Expr>, u8, u16),
    Call(Expr),
    Return,
    Trap(u8),
    DoWhile(Option<Expr>, u8, Vec<LinearStmt>),
    While(Option<Expr>, u8, Vec<LinearStmt>),
    For(Box<LinearStmt>, Option<Expr>, u8, Box<LinearStmt>, Vec<LinearStmt>),
    If(Option<Expr>, u8, Vec<LinearStmt>, Option<Vec<LinearStmt>>),
    Break,
    Continue,
    Label(u16),
}

impl fmt::Debug for LinearStmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinearStmt::Assign(reg, expr) => write!(f, "Assign({}, {:?})", reg, expr),
            LinearStmt::Store(addr, val) => write!(f, "Store({:?}, {:?})", addr, val),
            LinearStmt::Goto(expr, cc, target) => write!(f, "Goto({:?}, 0x{:X}, 0x{:X})", expr, cc, target),
            LinearStmt::Call(target) => write!(f, "Call({:?})", target),
            LinearStmt::Return => write!(f, "Return"),
            LinearStmt::Trap(vect) => write!(f, "Trap(0x{:X})", vect),
            LinearStmt::DoWhile(cond, cc, body) => {
                write!(f, "DoWhile({:?}, 0x{:X}, {{", cond, cc)?;
                for stmt in body {
                    write!(f, "\n    {:?}", stmt)?;
                }
                write!(f, "\n}})")
            },
            LinearStmt::While(cond, cc, body) => {
                write!(f, "While({:?}, 0x{:X}, {{", cond, cc)?;
                for stmt in body {
                    write!(f, "\n    {:?}", stmt)?;
                }
                write!(f, "\n}})")
            },
            LinearStmt::For(init, cond, cc, incr, body) => {
                write!(f, "For({:?}, {:?}, 0x{:X}, {:?}, {{", init, cond, cc, incr)?;
                for stmt in body {
                    write!(f, "\n    {:?}", stmt)?;
                }
                write!(f, "\n}})")
            },
            LinearStmt::If(cond, cc, true_branch, false_branch) => {
                write!(f, "If({:?}, 0x{:X}, {{", cond, cc)?;
                for stmt in true_branch {
                    write!(f, "\n    {:?}", stmt)?;
                }
                write!(f, "\n  }}")?;
                if let Some(false_branch) = false_branch {
                    write!(f, " else {{")?;
                    for stmt in false_branch {
                        write!(f, "\n    {:?}", stmt)?;
                    }
                    write!(f, "\n  }}")?;
                }
                write!(f, ")")
            },
            LinearStmt::Break => write!(f, "Break"),
            LinearStmt::Continue => write!(f, "Continue"),
            LinearStmt::Label(addr) => write!(f, "Label(0x{:X})", addr),
        }
    }
}

fn collect_goto_targets(stmts: &Vec<IRStmt>, targets: &mut HashSet<u16>) {
    for stmt in stmts {
        match &stmt.kind {
            IRStmtKind::Goto(_, _, target) => {
                targets.insert(*target);
            },
            IRStmtKind::DoWhile(_, _, body) |
            IRStmtKind::While(_, _, body) |
            IRStmtKind::For(_, _, _, _, body) => {
                for inner_stmts in body.values() {
                    collect_goto_targets(inner_stmts, targets);
                }
            },
            IRStmtKind::If(_, _, true_branch, false_branch) => {
                collect_goto_targets(true_branch, targets);
                if let Some(false_branch) = false_branch {
                    collect_goto_targets(false_branch, targets);
                }
            },
            _ => {}
        }
    }
}

fn to_linear_stmt(stmt: &IRStmt, targets: &HashSet<u16>) -> LinearStmt {
    match &stmt.kind {
        IRStmtKind::Assign(r, e) => LinearStmt::Assign(*r, e.clone()),
        IRStmtKind::Store(a, v) => LinearStmt::Store(a.clone(), v.clone()),
        IRStmtKind::Goto(e, cc, t) => LinearStmt::Goto(e.clone(), *cc, *t),
        IRStmtKind::Call(target) => LinearStmt::Call(target.clone()),
        IRStmtKind::Return => LinearStmt::Return,
        IRStmtKind::Trap(v) => LinearStmt::Trap(*v),
        IRStmtKind::DoWhile(cond, cc, body) => {
            LinearStmt::DoWhile(cond.clone(), *cc, linearize_blocks(body, targets))
        },
        IRStmtKind::While(cond, cc, body) => {
            LinearStmt::While(cond.clone(), *cc, linearize_blocks(body, targets))
        },
        IRStmtKind::For(init, cond, cc, incr, body) => {
            LinearStmt::For(
                Box::new(to_linear_stmt(init, targets)),
                cond.clone(),
                *cc,
                Box::new(to_linear_stmt(incr, targets)),
                linearize_blocks(body, targets)
            )
        },
        IRStmtKind::If(cond, cc, true_branch, false_branch) => {
            let tb = true_branch.iter().map(|s| to_linear_stmt(s, targets)).collect();
            let fb = false_branch.as_ref().map(|fb| fb.iter().map(|s| to_linear_stmt(s, targets)).collect());
            LinearStmt::If(cond.clone(), *cc, tb, fb)
        },
        IRStmtKind::Break => LinearStmt::Break,
        IRStmtKind::Continue => LinearStmt::Continue,
    }
}

fn linearize_blocks(blocks: &HashMap<u16, Vec<IRStmt>>, targets: &HashSet<u16>) -> Vec<LinearStmt> {
    let mut result = Vec::new();
    let mut sorted_keys: Vec<u16> = blocks.keys().cloned().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        if targets.contains(&key) {
             result.push(LinearStmt::Label(key));
        }
        
        if let Some(stmts) = blocks.get(&key) {
            for stmt in stmts {
                result.push(to_linear_stmt(stmt, targets));
            }
        }
    }
    
    result
}

fn linearize(blocks: &HashMap<u16, Vec<IRStmt>>) -> Vec<LinearStmt> {
    let mut targets = HashSet::new();
    // Scan all top-level blocks for gotos
    for stmts in blocks.values() {
        collect_goto_targets(stmts, &mut targets);
    }
    
    linearize_blocks(blocks, &targets)
}

fn stringify_expr(expr: &Expr, symbols: &HashMap<u16, &str>) -> String {
    match expr {
        Expr::Register(r) => format!("var{}", r),
        Expr::Immediate(val) => format!("{}", val),

        Expr::Add(lhs, rhs) => format!("{} + {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols)),
        Expr::Sub(lhs, rhs) => format!("{} - {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols)),
        Expr::And(lhs, rhs) => format!("{} & {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols)),
        Expr::Not(e) => format!("~{}", stringify_expr(e, symbols)),
        Expr::Neg(e) => format!("-{}", stringify_expr(e, symbols)),
        Expr::Load(e) => {
            if let Expr::Immediate(addr) = **e {
                let addr_u16 = addr as u16;
                if let Some(name) = symbols.get(&addr_u16) {
                    // Try to resolve global var
                    // If label name is same as address (default) or special?
                    // Gen sections creates "L{addr}" or meaningful names?
                    // Let's assume meaningful.
                    if (*name).starts_with("LC-3 OBJ FILE") {
                         format!("*global_{:04X}", addr_u16)
                    } else {
                         // Dereference valid pointer unless it's just a label?
                         // "Load" means memory access.
                         // But if immediate is an address of a variable, Load(Imm) is reading that variable.
                         // So `*LABEL` is correct if LABEL is a pointer.
                         // But `LD R0, LABEL` loads value at LABEL.
                         // `LDI R0, LABEL` loads value at address stored at LABEL.
                         // My IR:
                         // LD R0, LABEL -> Assign(0, Load(Immediate(Addr)))
                         // So Load(Imm) -> *Addr.
                         // If Addr has name 'FOO', then *FOO.
                         format!("*{}", name)
                    }
                } else {
                    format!("*global_{:04X}", addr_u16)
                }
            } else {
                // If expression, like base + offset
                // LDR R0, R1, #1 -> Assign(0, Load(Add(Reg(1), Imm(1))))
                // -> *(var1 + 1)
                format!("*({})", stringify_expr(e, symbols))
            }
        },
    }
}

// Convert CC bitmask to logical operation condition string
fn stringify_cc(cc: u8) -> String {
    match cc {
        0 => "false".to_string(),
        1 => "> 0".to_string(),
        2 => "== 0".to_string(), // Zero
        3 => ">= 0".to_string(),
        4 => "< 0".to_string(),
        5 => "!= 0".to_string(),
        6 => "<= 0".to_string(),
        7 => "true".to_string(),
        _ => "true".to_string(),
    }
}

fn stringify_condition(expr: &Expr, cc: u8, symbols: &HashMap<u16, &str>) -> String {
    // Check for A - B pattern
    // This could be Add(A, Neg(B)) or Add(A, Imm(-X)) or Sub(A, B) if we had Sub (we do now)
    
    let (lhs, rhs) = match expr {
        Expr::Sub(l, r) => (Some(l), Some(r)),
        Expr::Add(l, r) => {
            match &**r {
                Expr::Neg(inner) => (Some(l), Some(inner)),
                Expr::Immediate(x) if *x < 0 => {
                    // Add(A, -X) => A - X. We want condition A - X < 0 => A < X
                    // So lhs=A, rhs=X
                    // Wait, if expression is A + (-X) < 0 -> A < X.
                    // So we treat it as comparison against X.
                    // We need to construct Expr::Immediate(-x) as rhs.
                    // But we can't easily construct expressions here without ownership or boxes.
                    // Let's handle it differently.
                    (None, None)
                },
                _ => (None, None),
            }
        },
        _ => (None, None),
    };

    if let (Some(l), Some(r)) = (lhs, rhs) {
        let op = match cc {
            1 => ">",
            2 => "==",
            3 => ">=",
            4 => "<",
            5 => "!=",
            6 => "<=",
            _ => "",
        };
        
        if !op.is_empty() {
            return format!("{} {} {}", stringify_expr(l, symbols), op, stringify_expr(r, symbols));
        }
    }
    
    // Handle Imm(-X) case specifically
    if let Expr::Add(l, r) = expr {
        if let Expr::Immediate(x) = &**r {
            if *x < 0 {
                // A - X < 0 => A < X.
                // Comparison is against *positive* X.
                let target_val = -*x;
                let op = match cc {
                    1 => ">",
                    2 => "==",
                    3 => ">=",
                    4 => "<",
                    5 => "!=",
                    6 => "<=",
                    _ => "",
                };
                if !op.is_empty() {
                    return format!("{} {} {}", stringify_expr(l, symbols), op, target_val);
                }
            }
        }
    }

    format!("{} {}", stringify_expr(expr, symbols), stringify_cc(cc))
}

fn stringify_stmt(stmt: &LinearStmt, symbols: &HashMap<u16, &str>, indent: usize) -> String {
    let spaces = " ".repeat(indent * 4);
    match stmt {
        LinearStmt::Assign(reg, expr) => {
            let mut handled = false;
            let mut result = String::new();
            
            // Check for var++ (var = var + 1) or var-- (var = var - 1)
            if let Expr::Add(lhs, rhs) = expr {
                 // Check var = var + 1
                 if let (Expr::Register(r), Expr::Immediate(1)) = (&**lhs, &**rhs) {
                     if *r == *reg {
                         result = format!("{}var{}++;", spaces, reg);
                         handled = true;
                     }
                 } else if let (Expr::Immediate(1), Expr::Register(r)) = (&**lhs, &**rhs) {
                     if *r == *reg {
                         result = format!("{}var{}++;", spaces, reg);
                         handled = true;
                     }
                 } 
                 // Check var = var + -1 (common in LC3 ADD R0, R0, #-1)
                 else if let (Expr::Register(r), Expr::Immediate(-1)) = (&**lhs, &**rhs) {
                     if *r == *reg {
                         result = format!("{}var{}--;", spaces, reg);
                         handled = true;
                     }
                 } else if let (Expr::Immediate(-1), Expr::Register(r)) = (&**lhs, &**rhs) {
                     if *r == *reg {
                         result = format!("{}var{}--;", spaces, reg);
                         handled = true;
                     }
                 }
            } else if let Expr::Sub(lhs, rhs) = expr {
                // Check var = var - 1
                if let (Expr::Register(r), Expr::Immediate(1)) = (&**lhs, &**rhs) {
                    if *r == *reg {
                        result = format!("{}var{}--;", spaces, reg);
                        handled = true;
                    }
                }
            }

            if handled {
                result
            } else {
                format!("{}var{} = {};", spaces, reg, stringify_expr(expr, symbols))
            }
        },
        LinearStmt::Store(addr, val) => {
            // Store(Addr, Val) -> *Addr = Val
            // Check if addr is Load of something? No, Store takes address directly.
            // STR R0, R1, #0 -> Store(Add(Reg(1), Imm(0)), Reg(0)) -> *(var1 + 0) = var0
            // STI R0, LABEL -> Store(Load(Imm(Addr)), Reg(0)) 
            // Wait, STI uses indirect addressing. STI R0, LABEL -> mem[mem[PC + offset]] = R0.
            // My IR for STI: Store(Load(Immediate(Target)), Register(0))?
            // Yes, because STI writes TO the address stored at Target.
            // If addr is just Immediate(X), then it's directly writing to X? (ST instruction)
            
            // `stringify_expr(addr)` will handle the `*` if it's a Load.
            // If `addr` is Immediate, `stringify` gives `X`. `Store` implies dereference of LHS if it's an address.
            // But C semantics: `*ptr = val`. 
            // `stringify_expr` of `Add` is `lhs + rhs`.
            // So `*(lhs + rhs) = val`.
            
            // If `addr` expr is Load(Imm), `stringify` returns `*Imm`.
            // So `*(*Imm) = val`. This matches STI (double indirection).
            
            // If `addr` expr is Imm, `stringify` returns `Imm`.
            // So `*(Imm) = val`. This matches ST implementation?
            // Actually, `Store(A, B)` means `Memory[A] = B`.
            // So we always wrap `A` in `*()`?
            
            // My `stringify_expr` handles `Load` by adding `*`.
            // So `Load` already adds one star.
            // `Store` target is an address. We need to write to that address.
            // So `*Target = Value`.
            
            // Let's refine `addr_str`.
            let addr_str = stringify_expr(addr, symbols);
            // If addr_str already starts with `*`, it's like `**X`.
            // But verify: Store(Load(Imm)) -> *(*Imm) = ...
            // If Store(Imm) -> *Imm = ...
            // Yes, we always dereference the address we are storing to.
            
            // Verify if `addr` needs wrapping in parens.
            if matches!(addr, Expr::Add(_, _) | Expr::Sub(_, _)) {
                 format!("{}*({}) = {};", spaces, addr_str, stringify_expr(val, symbols))
            } else {
                 if let Expr::Immediate(imm) = addr {
                     let addr_u16 = *imm as u16;
                     if let Some(name) = symbols.get(&addr_u16) {
                         format!("{}*{} = {};", spaces, name, stringify_expr(val, symbols))
                     } else {
                         format!("{}*{} = {};", spaces, addr_str, stringify_expr(val, symbols))
                     }
                 } else {
                     format!("{}*{} = {};", spaces, addr_str, stringify_expr(val, symbols))
                 }
            }
        },
        LinearStmt::Goto(cond, cc, target) => {
             let target_str = if let Some(name) = symbols.get(target) {
                 name.to_string()
             } else {
                 format!("global_{:04X}", target)
             };
             
             if *cc == 7 {
                 format!("{}goto {};", spaces, target_str)
             } else {
                 let cond_str = if let Some(expr) = cond {
                     stringify_condition(expr, *cc, symbols)
                 } else {
                     "true".to_string()
                 };
                 format!("{}if ({}) goto {};", spaces, cond_str, target_str)
             }
        },
        LinearStmt::Call(expr) => {
             // Check if it's an immediate (direct call) or expression (indirect call)
             if let Expr::Immediate(addr) = expr {
                 let addr_u16 = *addr as u16;
                  if let Some(name) = symbols.get(&addr_u16) {
                      format!("{}call {}();", spaces, name)
                  } else {
                      format!("{}call func_{:04X}();", spaces, addr_u16)
                  }
             } else {
                 // Indirect call
                 format!("{}call {}();", spaces, stringify_expr(expr, symbols))
             }
        },
        LinearStmt::Return => format!("{}return;", spaces),
        LinearStmt::Trap(v) => format!("{}trap(0x{:X});", spaces, v),
        LinearStmt::DoWhile(cond, cc, body) => {
             let mut s = format!("{}do {{\n", spaces);
             for child in body {
                 s.push_str(&stringify_stmt(child, symbols, indent + 1));
                 s.push('\n');
             }
             let cond_str = if let Some(expr) = cond {
                 stringify_condition(expr, *cc, symbols)
             } else {
                 "true".to_string()
             };
             s.push_str(&format!("{}}} while ({});", spaces, cond_str));
             s
        },
        LinearStmt::While(cond, cc, body) => {
             let cond_str = if let Some(expr) = cond {
                 stringify_condition(expr, *cc, symbols)
             } else {
                 "true".to_string()
             };
             let mut s = format!("{}while ({}) {{\n", spaces, cond_str);
             for child in body {
                 s.push_str(&stringify_stmt(child, symbols, indent + 1));
                 s.push('\n');
             }
             s.push_str(&format!("{}}}", spaces));
             s
        },
        LinearStmt::For(init, cond, cc, incr, body) => {
             // Extract init str (remove trailing semicolon)
             // Since init is Box<LinearStmt>, we can just call stringify_stmt and trim.
             let init_str = stringify_stmt(init, symbols, 0).trim().trim_end_matches(';').to_string();
             
             let incr_str = stringify_stmt(incr, symbols, 0).trim().trim_end_matches(';').to_string();
             
             let cond_str = if let Some(expr) = cond {
                 stringify_condition(expr, *cc, symbols)
             } else {
                 "true".to_string()
             };
             
             let mut s = format!("{}for ({}; {}; {}) {{\n", spaces, init_str, cond_str, incr_str);
             

             for child in body {
                 s.push_str(&stringify_stmt(child, symbols, indent + 1));
                 s.push('\n');
             }
             s.push_str(&format!("{}}}", spaces));
             s
        },
        LinearStmt::If(cond, cc, true_branch, false_branch) => {
             let cond_str = if let Some(expr) = cond {
                 stringify_condition(expr, *cc, symbols)
             } else {
                 "true".to_string()
             };
             
             let mut s = format!("{}if ({}) {{\n", spaces, cond_str);
             for child in true_branch {
                 s.push_str(&stringify_stmt(child, symbols, indent + 1));
                 s.push('\n');
             }
             s.push_str(&format!("{}}}", spaces));
             
             if let Some(false_stmts) = false_branch {
                 s.push_str(" else {\n");
                 for child in false_stmts {
                     s.push_str(&stringify_stmt(child, symbols, indent + 1));
                     s.push('\n');
                 }
                 s.push_str(&format!("{}}}", spaces));
             }
             s
        },
        LinearStmt::Break => format!("{}break;", spaces),
        LinearStmt::Continue => format!("{}continue;", spaces),
        LinearStmt::Label(addr) => {
             if let Some(name) = symbols.get(addr) {
                 format!("{}:", name)
             } else {
                 format!("global_{:04X}:", addr)
             }
        },
    }
}

fn generate_code(stmts: &Vec<LinearStmt>, symbols: &HashMap<u16, &str>) -> String {
    let mut output = String::new();
    for stmt in stmts {
        output.push_str(&stringify_stmt(stmt, symbols, 0));
        output.push('\n');
    }
    output
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path to obj file> [-d|--debug] [-e|--entry <addr>]", args[0]);
        std::process::exit(1);
    }
    
    let mut debug_mode = false;
    let mut entry_request: Option<String> = None;
    
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--debug" => {
                debug_mode = true;
                i += 1;
            },
            "-e" | "--entry" => {
                if i + 1 < args.len() {
                    entry_request = Some(args[i+1].clone());
                    i += 2;
                } else {
                    eprintln!("Missing address for --entry");
                    std::process::exit(1);
                }
            },
            _ => {
                // Ignore unknown args or warn?
                i += 1;
            }
        }
    }

    let input_obj = fs::read_to_string(args[1].clone()).expect("Failed to read file");
    assert!(input_obj.starts_with("LC-3 OBJ FILE"));

    let (text, symbols) = gen_sections(&input_obj);
    let (disassembly, origs) = disassemble(text, symbols.clone());

    // Step 1. Disassembly
    if debug_mode {
        println!("Disassembly:");
        let mut sorted_entries: Vec<_> = disassembly.clone().into_iter().collect();
        sorted_entries.sort_by_key(|&(key, _)| key);
        for (addr, stmt) in sorted_entries {
            println!("{:04X}: {}", addr, stmt);
        }
        println!();
    }

    let function_list = identify_code_data(disassembly.clone(), origs);

    if debug_mode {
        println!("Identified functions:");
        for addr in &function_list {
            println!("{:04X}", addr);
        }
        println!();
    }
    
    // Process the requested entry point
    let entry_addr = if let Some(req) = entry_request {
         // Try parsing as hex first
         let clean = req.trim_start_matches("0x").trim_start_matches("x");
         if let Ok(addr) = u16::from_str_radix(clean, 16) {
             addr
         } else {
             // Try symbol lookup
             let mut found_addr = None;
             for (addr, name) in &symbols {
                 if name == &req {
                     found_addr = Some(*addr);
                     break;
                 }
             }
             
             if let Some(addr) = found_addr {
                 addr
             } else {
                 eprintln!("Error: Could not resolve entry point '{}' as address or label.", req);
                 std::process::exit(1);
             }
         }
    } else {
        0x3000
    };

    {
        println!("Decompiling function at {:04X}", entry_addr);
        
        // Step 2. Split to basic blocks
        let mut blocks = create_basic_blocks(entry_addr, &disassembly);

        // Step 3. Build control flow graph
        compute_dominators(&mut blocks, entry_addr);

        if debug_mode {
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
        }

        // Step 4. Control flow identification
        let mut natural_loops = compute_natural_loops(&blocks, entry_addr);

        if debug_mode {
            println!("Natural loops:");
            for (id, l) in natural_loops.iter().enumerate() {
                let block_strs: Vec<String> = l.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
                println!("Loop {}: header = {:04X}, blocks = [{}]", id + 1, l.header, block_strs.join(", "));
            }
            println!();
        }

        let identified_loops = identify_loops(&mut natural_loops, &blocks);
        let identified_conditionals = identify_conditionals(&blocks);

        if debug_mode {
            println!("Identified Loop Structures:");
            for (i, loop_info) in identified_loops.iter().enumerate() {
                let loop_blocks: Vec<String> = loop_info.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
                println!("Loop {}: header = {:04X}, blocks = [{}], break_block = {:?}, continue_block = {:?}",
                         i + 1, loop_info.header, loop_blocks.join(", "), 
                         loop_info.break_block.map(|b| format!("{:04X}", b)), 
                         loop_info.continue_block.map(|b| format!("{:04X}", b)));
            }
            println!();

            println!("Identified Conditional Structures:");
            for (i, cond) in identified_conditionals.iter().enumerate() {
                match cond.kind {
                    ConditionalType::If => {
                        println!("Conditional {} (If): condition_block = {:04X}, true_branch = {:04X}, join_block = {:04X}",
                                 i + 1, cond.condition_block, cond.true_block, cond.join_block);
                    }
                    ConditionalType::IfElse => {
                        println!("Conditional {} (If-Else): condition_block = {:04X}, true_branch = {:04X}, false_branch = {:04X}, join_block = {:04X}",
                                 i + 1, cond.condition_block, cond.true_block, cond.false_block.unwrap(), cond.join_block);
                    }
                }
            }
            println!();
        }

        // Step 5. Data Flow Analysis
        let (mut instr_liveness, mut block_liveness) = compute_local_liveness(&blocks, &disassembly);
        propagate_global_liveness(&blocks, &mut block_liveness);
        compute_final_liveness(&blocks, &block_liveness, &mut instr_liveness);

        if debug_mode {
            println!("Liveness Analysis:");
            let mut sorted_instrs: Vec<_> = instr_liveness.iter().collect();
            sorted_instrs.sort_by_key(|&(addr, _)| addr);
            for (addr, info) in sorted_instrs {
                println!("{:04X}: defs={:02X}, uses={:02X}, in={:02X}, out={:02X}", 
                         addr, info.defs, info.uses, info.live_in, info.live_out);
            }
            println!();
        }
        
        // Step 6. Expression Propagation
        let lifted_blocks = propagate_expressions(&blocks, &disassembly, &instr_liveness);
        if debug_mode {
            println!("Expressions:");
            let mut sorted_lifted: Vec<_> = lifted_blocks.iter().collect();
            sorted_lifted.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_lifted {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 7. Expression Collapsing
        let collapsed_blocks = collapse_expressions(lifted_blocks);
        let mut goto_blocks = apply_goto_transformation(collapsed_blocks, &instr_liveness);
        
        if debug_mode {
            println!("Collapsed Expressions:");
            let mut sorted_gotos: Vec<_> = goto_blocks.iter().collect();
            sorted_gotos.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_gotos {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 8. Control Flow Structuring
        structure_loops(&mut goto_blocks, &identified_loops, &blocks, &identified_conditionals);
        structure_conditionals(&mut goto_blocks, &identified_conditionals);
        refine_loops(&mut goto_blocks, &identified_loops, &blocks, &identified_conditionals, &instr_liveness);

        if debug_mode {
            println!("Structured Control Flow:");
            let mut sorted_structured: Vec<_> = goto_blocks.iter().collect();
            sorted_structured.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_structured {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 9. Linearization
        if debug_mode {
            println!("Linearized Code:");
        }
        let linear_code = linearize(&goto_blocks);
        if debug_mode {
            for stmt in &linear_code {
                println!("{:?}", stmt);
            }
            println!();
        }

        // Step 10. Code Output and Symbol Resolution
        if debug_mode {
            println!("Generated C-Like Code:");
        }
        let code = generate_code(&linear_code, &symbols);
        println!("{}", code);
    }
}
