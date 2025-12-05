use std::collections::HashMap;
use lc3_ensemble::ast::asm::{Stmt, StmtKind, AsmInstr};
use lc3_ensemble::ast::ImmOrReg;
use crate::cfg::BasicBlock;

#[derive(Debug, Clone, Default)]
pub struct LivenessInfo {
    pub defs: u8,
    pub uses: u8,
    pub live_in: u8,
    pub live_out: u8,
}

pub fn get_use_def(instr: &AsmInstr) -> (u8, u8) {
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
pub fn compute_local_liveness(
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
pub fn propagate_global_liveness(
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
pub fn compute_final_liveness(
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
