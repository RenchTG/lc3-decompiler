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
pub enum IRStmtKind {
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
                                 ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return });
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
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Immediate(target as i16)) });
                        },
                        AsmInstr::JSRR(base) => {
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                             
                             let src_reg = base.reg_no();
                             ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Call(Expr::Register(src_reg)) });
                        },
                        AsmInstr::RET => {
                             for reg in pending_order.drain(..) {
                                 if let Some((expr, stmt_addr)) = pending_assignments.remove(&reg) {
                                     ir_stmts.push(IRStmt { addr: stmt_addr, kind: IRStmtKind::Assign(reg, expr) });
                                 }
                             }
                            ir_stmts.push(IRStmt { addr, kind: IRStmtKind::Return });
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
        }
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
