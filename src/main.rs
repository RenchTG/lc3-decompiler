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
    Label(String),
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
            Expr::Label(l) => write!(f, "Label({})", l),
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
    Branch(Expr, u16),      // Condition (CC), Target
    Call(u16),              // JSR/JSRR
    Return,                 // RET
    Trap(u8),               // TRAP vector
}

impl fmt::Debug for IRStmtKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IRStmtKind::Assign(reg, expr) => write!(f, "Assign({}, {:?})", reg, expr),
            IRStmtKind::Store(addr, val) => write!(f, "Store({:?}, {:?})", addr, val),
            IRStmtKind::Branch(cond, target) => write!(f, "Branch({:?}, 0x{:X})", cond, target),
            IRStmtKind::Call(target) => write!(f, "Call(0x{:X})", target),
            IRStmtKind::Return => write!(f, "Return"),
            IRStmtKind::Trap(vect) => write!(f, "Trap(0x{:X})", vect),
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
        let mut pending_assignments: HashMap<u8, Expr> = HashMap::new();
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
                                if let Some(expr) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some(expr) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    // Propagate if NOT live out OR defined by this instr
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr
                                    } else {
                                        ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    // If multi_use, we already flushed src1 (which is r_reg).
                                    // So pending is empty.
                                    if let Some(expr) = pending_assignments.remove(&r_reg) {
                                        pending_order.retain(|&r| r != r_reg);
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr
                                        } else {
                                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(r_reg, expr) });
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
                            pending_assignments.insert(dst_reg, expr);
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
                                if let Some(expr) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some(expr) = pending_assignments.remove(&src1_reg) {
                                    pending_order.retain(|&r| r != src1_reg);
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr
                                    } else {
                                        ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src1_reg, expr) });
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    if let Some(expr) = pending_assignments.remove(&r_reg) {
                                        pending_order.retain(|&r| r != r_reg);
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr
                                        } else {
                                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(r_reg, expr) });
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
                            pending_assignments.insert(dst_reg, expr);
                        },
                        AsmInstr::NOT(dst, src) => {
                            let dst_reg = dst.reg_no();
                            
                            let src_reg = src.reg_no();
                            let op1 = if let Some(expr) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let expr = Expr::Not(Box::new(op1));
                            if pending_assignments.contains_key(&dst_reg) {
                                pending_order.retain(|&r| r != dst_reg);
                            }
                            pending_order.push(dst_reg);
                            pending_assignments.insert(dst_reg, expr);
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
                             pending_assignments.insert(dst_reg, expr);
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
                            pending_assignments.insert(dst_reg, expr);
                        },
                        AsmInstr::LDR(dst, base, offset6) => {
                            let dst_reg = dst.reg_no();
                            
                            let base_reg = base.reg_no();
                            let base_op = if let Some(expr) = pending_assignments.remove(&base_reg) {
                                pending_order.retain(|&r| r != base_reg);
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(base_reg, expr) });
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
                            pending_assignments.insert(dst_reg, expr);
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
                            pending_assignments.insert(dst_reg, Expr::Immediate(target_addr as i16));
                        },
                        AsmInstr::ST(src, pcoffset9) => {
                            let src_reg = src.reg_no();
                            let src_op = if let Some(expr) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src_reg, expr) });
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
                            let src_op = if let Some(expr) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src_reg, expr) });
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
                            let src_op = if let Some(expr) = pending_assignments.remove(&src_reg) {
                                pending_order.retain(|&r| r != src_reg);
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(src_reg, expr) });
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let base_reg = base.reg_no();
                            let base_op = if let Some(expr) = pending_assignments.remove(&base_reg) {
                                pending_order.retain(|&r| r != base_reg);
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr
                                } else {
                                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(base_reg, expr) });
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
                                if let Some(expr) = pending_assignments.remove(&reg) {
                                    ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
                                }
                            }

                            let offset = match pcoffset9 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0,
                            };
                            let target = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt {
                                addr,
                                kind: IRStmtKind::Branch(Expr::Immediate(*cc as i16), target),
                            });
                        },
                        AsmInstr::JMP(base) => {
                             let _base_reg = base.reg_no();
                             // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some(expr) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
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
                                 if let Some(expr) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }

                             let offset = match pcoffset11 {
                                PCOffset::Offset(o) => o.get(),
                                PCOffset::Label(_) => 0,
                            };
                            let target = (addr as i16 + 1 + offset) as u16;
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(target) });
                        },
                        AsmInstr::JSRR(_base) => {
                             // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some(expr) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                             ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(0) });
                        },
                        AsmInstr::RET => {
                            // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some(expr) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return });
                        },
                        AsmInstr::TRAP(vect8) => {
                            // Flush all pending assignments
                             for reg in pending_order.drain(..) {
                                 if let Some(expr) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: 0, kind: IRStmtKind::Assign(reg, expr) });
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
             if let Some(expr) = pending_assignments.remove(&reg) {
                 ir_stmts.push(IRStmt {
                     addr: 0,
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
        Expr::Register(_) | Expr::Immediate(_) | Expr::Label(_) => (expr, false),
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
        IRStmtKind::Branch(cond, _) => {
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

    // Step 1. Disassembly
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
        // Step 2. Split to basic blocks
        let mut blocks = create_basic_blocks(entry_addr, &disassembly);

        // Step 3. Build control flow graph
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

        // Step 4. Control flow identification
        let mut natural_loops = compute_natural_loops(&blocks, entry_addr);

        println!("Natural loops:");
        for (i, loop_obj) in natural_loops.iter().enumerate() {
            let loop_blocks: Vec<String> = loop_obj.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
            println!("Loop {}: header = {:04X}, blocks = [{}]", 
                     i + 1, loop_obj.header, loop_blocks.join(", "));
        }
        println!();

        let identified_loops = identify_loops(&mut natural_loops, &blocks);
        let identified_conditionals = identify_conditionals(&blocks);

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

        // Step 5. Data Flow Analysis
        let (mut instr_liveness, mut block_liveness) = compute_local_liveness(&blocks, &disassembly);
        propagate_global_liveness(&blocks, &mut block_liveness);
        compute_final_liveness(&blocks, &block_liveness, &mut instr_liveness);

        println!("Liveness Analysis:");
        let mut sorted_instrs: Vec<_> = instr_liveness.iter().collect();
        sorted_instrs.sort_by_key(|&(addr, _)| addr);
        for (addr, info) in sorted_instrs {
            println!("{:04X}: defs={:02X}, uses={:02X}, in={:02X}, out={:02X}", 
                     addr, info.defs, info.uses, info.live_in, info.live_out);
        }
        println!();
        
        // Step 6. Expression Propagation
        let lifted_blocks = propagate_expressions(&blocks, &disassembly, &instr_liveness);
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

        // Step 7. Expression Collapsing
        let collapsed_blocks = collapse_expressions(lifted_blocks);
        println!("Collapsed Expressions:");
        let mut sorted_collapsed: Vec<_> = collapsed_blocks.iter().collect();
        sorted_collapsed.sort_by_key(|&(addr, _)| addr);
        for (addr, stmts) in sorted_collapsed {
            println!("Block {:04X}:", addr);
            for stmt in stmts {
                println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
            }
        }
        println!();

        // TODO Step 8. Control Flow Structuring
        // See Structuring.md for details

        // TODO Step 9. Code Output
        // See CodeOutput.md for details
    }
}
