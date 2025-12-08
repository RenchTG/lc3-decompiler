use std::collections::HashMap;
use std::fmt;
use lc3_ensemble::ast::asm::{Stmt, StmtKind, AsmInstr};
use lc3_ensemble::ast::{PCOffset, ImmOrReg};
use crate::cfg::BasicBlock;
use crate::liveness::LivenessInfo;

#[derive(Clone)]
pub enum Expr {
    Register(u8),
    Immediate(i16),

    Add(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Load(Box<Expr>), // Represents memory load from address
    Param(u8),       // Function parameter (0-indexed)
    Call(Box<Expr>, Vec<Expr>), // Function call as expression
    LogicalAnd(Box<Expr>, u8, Box<Expr>, u8),  // (lhs_expr, lhs_cc, rhs_expr, rhs_cc)
    LogicalOr(Box<Expr>, u8, Box<Expr>, u8),
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
            Expr::Param(idx) => write!(f, "Param({})", idx),
            Expr::Call(target, args) => {
                write!(f, "CallExpr({:?}", target)?;
                for arg in args {
                    write!(f, ", {:?}", arg)?;
                }
                write!(f, ")")
            },
            Expr::LogicalAnd(lhs, lhs_cc, rhs, rhs_cc) => {
                write!(f, "LogicalAnd({:?}, 0x{:X}, {:?}, 0x{:X})", lhs, lhs_cc, rhs, rhs_cc)
            },
            Expr::LogicalOr(lhs, lhs_cc, rhs, rhs_cc) => {
                write!(f, "LogicalOr({:?}, 0x{:X}, {:?}, 0x{:X})", lhs, lhs_cc, rhs, rhs_cc)
            },
        }
    }
}

#[derive(Clone)]
pub enum IRStmtKind {
    Assign(u8, Expr),       // Reg = Expr
    Store(Expr, Expr),      // Mem[Addr] = Value
    Goto(Option<Expr>, u8, u16), // Expression, Condition (CC), Target
    Call(Expr, Vec<Expr>),  // Target, Arguments
    Return(Option<Expr>),   // RET, Optional Return Value
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
            IRStmtKind::Call(target, args) => {
                write!(f, "Call({:?}", target)?;
                for arg in args {
                    write!(f, ", {:?}", arg)?;
                }
                write!(f, ")")
            },
            IRStmtKind::Return(val) => {
                match val {
                    Some(v) => write!(f, "Return({:?})", v),
                    None => write!(f, "Return"),
                }
            },
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
pub struct IRStmt {
    pub addr: u16,
    pub kind: IRStmtKind,
}

pub fn propagate_expressions(
    blocks: &HashMap<u16, BasicBlock>,
    disassembly: &HashMap<u16, Stmt>,
    instr_liveness: &HashMap<u16, LivenessInfo>
) -> HashMap<u16, Vec<IRStmt>> {
    let mut lifted_blocks = HashMap::new();

    for (&block_addr, block) in blocks {
        let mut ir_stmts = Vec::new();
        let mut pending_assignments: HashMap<u8, (Expr, u16)> = HashMap::new();
        let mut pending_order: Vec<u8> = Vec::new();

        // Helper: Get a pending assignment without flushing (for potential inlining)
        // If the expr will be inlined, we don't need to flush predecessors
        let get_pending = |target_reg: u8,
                          pending_assignments: &mut HashMap<u8, (Expr, u16)>,
                          pending_order: &mut Vec<u8>| -> Option<(Expr, u16)> {
            if let Some((expr, addr)) = pending_assignments.remove(&target_reg) {
                pending_order.retain(|&r| r != target_reg);
                Some((expr, addr))
            } else {
                None
            }
        };
        
        // Helper: flush all pending assignments up to and including target_reg to maintain order
        // Use this when we know we need to EMIT target_reg's assignment (not inline it)
        let flush_and_emit = |target_reg: u8,
                              expr: Expr,
                              stmt_addr: u16,
                              pending_assignments: &mut HashMap<u8, (Expr, u16)>,
                              pending_order: &mut Vec<u8>,
                              ir_stmts: &mut Vec<IRStmt>| {
            // Flush all registers that come before target_reg in the original order
            // Since target_reg is already removed from pending_order (by get_pending),
            // we need to check by address: flush any pending with addr < stmt_addr
            let mut to_flush = Vec::new();
            for &reg in pending_order.iter() {
                if let Some((_, pending_addr)) = pending_assignments.get(&reg) {
                    if *pending_addr < stmt_addr {
                        to_flush.push(reg);
                    }
                }
            }
            for reg in to_flush {
                pending_order.retain(|&r| r != reg);
                if let Some((expr, addr)) = pending_assignments.remove(&reg) {
                    ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Assign(reg, expr) });
                }
            }
            // Now emit the target assignment
            ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(target_reg, expr) });
        };

        for i in 0..block.length {
            let addr = block_addr + i;
            if let Some(stmt) = disassembly.get(&addr) {
                if let StmtKind::Instr(ref instr) = stmt.nucleus {
                    let liveness = &instr_liveness[&addr];
                    
                    match instr {
                        AsmInstr::ADD(dst, src1, src2) => {
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
                                // Multi-use: must emit and use register reference
                                if let Some((expr, stmt_addr)) = get_pending(src1_reg, &mut pending_assignments, &mut pending_order) {
                                    flush_and_emit(src1_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some((expr, stmt_addr)) = get_pending(src1_reg, &mut pending_assignments, &mut pending_order) {
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr // Inline - no flush needed
                                    } else {
                                        flush_and_emit(src1_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    if let Some((expr, stmt_addr)) = get_pending(r_reg, &mut pending_assignments, &mut pending_order) {
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr // Inline - no flush needed
                                        } else {
                                            flush_and_emit(r_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                                // Multi-use: must emit and use register reference
                                if let Some((expr, stmt_addr)) = get_pending(src1_reg, &mut pending_assignments, &mut pending_order) {
                                    flush_and_emit(src1_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
                                }
                                Expr::Register(src1_reg)
                            } else {
                                if let Some((expr, stmt_addr)) = get_pending(src1_reg, &mut pending_assignments, &mut pending_order) {
                                    let killed = (liveness.defs & (1 << src1_reg)) != 0;
                                    if killed || (liveness.live_out & (1 << src1_reg)) == 0 {
                                        expr // Inline - no flush needed
                                    } else {
                                        flush_and_emit(src1_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
                                        Expr::Register(src1_reg)
                                    }
                                } else { Expr::Register(src1_reg) }
                            };

                            let op2 = match src2 {
                                ImmOrReg::Reg(r) => {
                                    let r_reg = r.reg_no();
                                    if let Some((expr, stmt_addr)) = get_pending(r_reg, &mut pending_assignments, &mut pending_order) {
                                        let killed = (liveness.defs & (1 << r_reg)) != 0;
                                        if killed || (liveness.live_out & (1 << r_reg)) == 0 {
                                            expr // Inline - no flush needed
                                        } else {
                                            flush_and_emit(r_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            let op1 = if let Some((expr, stmt_addr)) = get_pending(src_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(src_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            let base_op = if let Some((expr, stmt_addr)) = get_pending(base_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(base_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            let src_op = if let Some((expr, stmt_addr)) = get_pending(src_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(src_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            let src_op = if let Some((expr, stmt_addr)) = get_pending(src_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(src_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            let src_op = if let Some((expr, stmt_addr)) = get_pending(src_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << src_reg)) != 0;
                                if killed || (liveness.live_out & (1 << src_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(src_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
                                    Expr::Register(src_reg)
                                }
                            } else { Expr::Register(src_reg) };

                            let base_reg = base.reg_no();
                            let base_op = if let Some((expr, stmt_addr)) = get_pending(base_reg, &mut pending_assignments, &mut pending_order) {
                                let killed = (liveness.defs & (1 << base_reg)) != 0;
                                if killed || (liveness.live_out & (1 << base_reg)) == 0 {
                                    expr // Inline - no flush needed
                                } else {
                                    flush_and_emit(base_reg, expr, stmt_addr, &mut pending_assignments, &mut pending_order, &mut ir_stmts);
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
                            // Flush all pending assignments before branching
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
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }

                             if base.reg_no() == 7 {
                                 ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return(None) });
                             } else {
                                 ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Trap(0xFF) });
                             }
                        },
                        AsmInstr::JSR(pcoffset11) => {
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
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Immediate(target as i16), Vec::new()) });
                        },
                        AsmInstr::JSRR(base) => {
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                             
                             let src_reg = base.reg_no();
                             ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Register(src_reg), Vec::new()) });
                        },
                        AsmInstr::RET => {
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return(None) });
                        },
                        AsmInstr::TRAP(vect8) => {
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

pub fn collapse_expr(expr: Expr) -> (Expr, bool) {
    match expr {
        Expr::Register(_) | Expr::Immediate(_) | Expr::Param(_) => (expr, false),
        Expr::Call(target, args) => {
             let (new_target, changed_target) = collapse_expr(*target);
             let mut changed_args = false;
             let new_args = args.into_iter().map(|arg| {
                 let (new_arg, changed) = collapse_expr(arg);
                 if changed { changed_args = true; }
                 new_arg
             }).collect();
             
             (Expr::Call(Box::new(new_target), new_args), changed_target || changed_args)
        },
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
                     let equal = match (&lhs, &rhs) {
                         (Expr::Register(r1), Expr::Register(r2)) => r1 == r2,
                         (Expr::Immediate(i1), Expr::Immediate(i2)) => i1 == i2,
                         _ => false, 
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
                (Expr::Not(inner), Expr::Immediate(1)) | (Expr::Immediate(1), Expr::Not(inner)) => {
                    (Expr::Neg(inner), true)
                },
                (a, Expr::Neg(b)) => (Expr::Sub(Box::new(a), b), true),
                (Expr::Neg(b), a) => (Expr::Sub(Box::new(a), b), true),
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
        },
        Expr::LogicalAnd(lhs, lhs_cc, rhs, rhs_cc) => {
            let (new_lhs, changed_lhs) = collapse_expr(*lhs);
            let (new_rhs, changed_rhs) = collapse_expr(*rhs);
            (Expr::LogicalAnd(Box::new(new_lhs), lhs_cc, Box::new(new_rhs), rhs_cc), changed_lhs || changed_rhs)
        },
        Expr::LogicalOr(lhs, lhs_cc, rhs, rhs_cc) => {
            let (new_lhs, changed_lhs) = collapse_expr(*lhs);
            let (new_rhs, changed_rhs) = collapse_expr(*rhs);
            (Expr::LogicalOr(Box::new(new_lhs), lhs_cc, Box::new(new_rhs), rhs_cc), changed_lhs || changed_rhs)
        },
    }
}

pub fn collapse_stmt(stmt: &mut IRStmt) -> bool {
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

pub fn collapse_expressions(mut blocks: HashMap<u16, Vec<IRStmt>>) -> HashMap<u16, Vec<IRStmt>> {
    // Expression simplification only - variable propagation runs separately after stack processing
    let mut changed = true;
    while changed {
        changed = false;
        for stmts in blocks.values_mut() {
            for stmt in stmts.iter_mut() {
                if collapse_stmt(stmt) {
                    changed = true;
                }
            }
        }
    }
    
    blocks
}

// Public function to propagate single-use variables - call after stack convention processing
pub fn propagate_variables(blocks: &mut HashMap<u16, Vec<IRStmt>>) {
    let mut changed = true;
    while changed {
        changed = false;
        for stmts in blocks.values_mut() {
            changed = propagate_single_use_vars(stmts) || changed;
        }
    }
}

// Count how many times a register is used in an expression
fn count_reg_uses_in_expr(expr: &Expr, reg: u8) -> usize {
    match expr {
        Expr::Register(r) => if *r == reg { 1 } else { 0 },
        Expr::Immediate(_) | Expr::Param(_) => 0,
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::And(l, r) => {
            count_reg_uses_in_expr(l, reg) + count_reg_uses_in_expr(r, reg)
        },
        Expr::Not(e) | Expr::Neg(e) | Expr::Load(e) => count_reg_uses_in_expr(e, reg),
        Expr::Call(target, args) => {
            count_reg_uses_in_expr(target, reg) + 
            args.iter().map(|a| count_reg_uses_in_expr(a, reg)).sum::<usize>()
        },
        Expr::LogicalAnd(l, _, r, _) | Expr::LogicalOr(l, _, r, _) => {
            count_reg_uses_in_expr(l, reg) + count_reg_uses_in_expr(r, reg)
        },
    }
}

// Count how many times a register is used in a statement
fn count_reg_uses_in_stmt(stmt: &IRStmt, reg: u8) -> usize {
    match &stmt.kind {
        IRStmtKind::Assign(_, expr) => count_reg_uses_in_expr(expr, reg),
        IRStmtKind::Store(addr, val) => {
            count_reg_uses_in_expr(addr, reg) + count_reg_uses_in_expr(val, reg)
        },
        IRStmtKind::Goto(Some(cond), _, _) => count_reg_uses_in_expr(cond, reg),
        IRStmtKind::Goto(None, _, _) => 0,
        IRStmtKind::Call(target, args) => {
            count_reg_uses_in_expr(target, reg) + 
            args.iter().map(|a| count_reg_uses_in_expr(a, reg)).sum::<usize>()
        },
        IRStmtKind::Return(Some(e)) => count_reg_uses_in_expr(e, reg),
        IRStmtKind::Return(None) => 0,
        IRStmtKind::Trap(_) | IRStmtKind::Break | IRStmtKind::Continue => 0,
        IRStmtKind::DoWhile(cond, _, body) | IRStmtKind::While(cond, _, body) => {
            let cond_uses = cond.as_ref().map_or(0, |c| count_reg_uses_in_expr(c, reg));
            cond_uses + body.values().flat_map(|s| s.iter()).map(|s| count_reg_uses_in_stmt(s, reg)).sum::<usize>()
        },
        IRStmtKind::For(init, cond, _, incr, body) => {
            count_reg_uses_in_stmt(init, reg) +
            cond.as_ref().map_or(0, |c| count_reg_uses_in_expr(c, reg)) +
            count_reg_uses_in_stmt(incr, reg) +
            body.values().flat_map(|s| s.iter()).map(|s| count_reg_uses_in_stmt(s, reg)).sum::<usize>()
        },
        IRStmtKind::If(cond, _, true_branch, false_branch) => {
            let cond_uses = cond.as_ref().map_or(0, |c| count_reg_uses_in_expr(c, reg));
            cond_uses + 
            true_branch.iter().map(|s| count_reg_uses_in_stmt(s, reg)).sum::<usize>() +
            false_branch.as_ref().map_or(0, |fb| fb.iter().map(|s| count_reg_uses_in_stmt(s, reg)).sum::<usize>())
        },
    }
}

// Check if a register is defined in a statement (excluding the assignment itself)
fn defines_reg(stmt: &IRStmt, reg: u8) -> bool {
    match &stmt.kind {
        IRStmtKind::Assign(dst, _) => *dst == reg,
        _ => false,
    }
}

// Replace all uses of a register with an expression
fn replace_reg_in_expr(expr: &mut Expr, reg: u8, replacement: &Expr) {
    match expr {
        Expr::Register(r) => {
            if *r == reg {
                *expr = replacement.clone();
            }
        },
        Expr::Immediate(_) | Expr::Param(_) => {},
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::And(l, r) => {
            replace_reg_in_expr(l, reg, replacement);
            replace_reg_in_expr(r, reg, replacement);
        },
        Expr::Not(e) | Expr::Neg(e) | Expr::Load(e) => {
            replace_reg_in_expr(e, reg, replacement);
        },
        Expr::Call(target, args) => {
            replace_reg_in_expr(target, reg, replacement);
            for arg in args {
                replace_reg_in_expr(arg, reg, replacement);
            }
        },
        Expr::LogicalAnd(l, _, r, _) | Expr::LogicalOr(l, _, r, _) => {
            replace_reg_in_expr(l, reg, replacement);
            replace_reg_in_expr(r, reg, replacement);
        },
    }
}

fn replace_reg_in_stmt(stmt: &mut IRStmt, reg: u8, replacement: &Expr) {
    match &mut stmt.kind {
        IRStmtKind::Assign(_, expr) => replace_reg_in_expr(expr, reg, replacement),
        IRStmtKind::Store(addr, val) => {
            replace_reg_in_expr(addr, reg, replacement);
            replace_reg_in_expr(val, reg, replacement);
        },
        IRStmtKind::Goto(Some(cond), _, _) => replace_reg_in_expr(cond, reg, replacement),
        IRStmtKind::Call(target, args) => {
            replace_reg_in_expr(target, reg, replacement);
            for arg in args {
                replace_reg_in_expr(arg, reg, replacement);
            }
        },
        IRStmtKind::Return(Some(e)) => replace_reg_in_expr(e, reg, replacement),
        _ => {},
    }
}

// Propagate single-use variables in a list of statements
fn propagate_single_use_vars(stmts: &mut Vec<IRStmt>) -> bool {
    let mut changed = false;
    let mut i = 0;
    
    while i < stmts.len() {
        // Check if this is an Assign statement
        if let IRStmtKind::Assign(reg, _) = &stmts[i].kind {
            let reg = *reg;
            
            // Skip R6 (stack pointer) and R7 (return address) - these are special
            if reg == 6 || reg == 7 {
                i += 1;
                continue;
            }
            
            // Count uses in subsequent statements (before next definition of this reg)
            let mut use_count = 0;
            let mut use_stmt_idx = None;
            let mut use_in_loop = false;
            
            for j in (i + 1)..stmts.len() {
                let stmt = &stmts[j];
                
                // Check if this statement redefines the register
                if defines_reg(stmt, reg) {
                    break;
                }
                
                // Check if the use is inside a loop (While, DoWhile, For)
                // Uses inside loops count as "multiple" since the loop can iterate
                let is_loop = matches!(&stmt.kind, 
                    IRStmtKind::While(_, _, _) | 
                    IRStmtKind::DoWhile(_, _, _) | 
                    IRStmtKind::For(_, _, _, _, _)
                );
                
                let uses = count_reg_uses_in_stmt(stmt, reg);
                if uses > 0 {
                    use_count += uses;
                    if use_stmt_idx.is_none() {
                        use_stmt_idx = Some(j);
                    }
                    if is_loop {
                        use_in_loop = true;
                    }
                }
            }
            
            // If used exactly once AND not inside a loop
            if use_count == 1 && !use_in_loop {
                if let Some(use_idx) = use_stmt_idx {
                    // Get the expression from the assignment
                    let expr = if let IRStmtKind::Assign(_, e) = &stmts[i].kind {
                        e.clone()
                    } else {
                        i += 1;
                        continue;
                    };
                    
                    // Replace the register usage with the expression
                    replace_reg_in_stmt(&mut stmts[use_idx], reg, &expr);
                    
                    // Remove the assignment statement
                    stmts.remove(i);
                    changed = true;
                    // Don't increment i since we removed the current element
                    continue;
                }
            }
        }
        i += 1;
    }
    
    changed
}

pub fn apply_goto_transformation(
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
