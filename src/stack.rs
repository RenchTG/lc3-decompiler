use std::collections::HashMap;
use crate::ir::{IRStmt, IRStmtKind, Expr};

// Helper to check if an expression is R6
fn is_reg(expr: &Expr, reg: u8) -> bool {
    matches!(expr, Expr::Register(r) if *r == reg)
}

// Helper to check if an expression is an immediate value
fn is_imm(expr: &Expr, val: i16) -> bool {
    matches!(expr, Expr::Immediate(v) if *v == val)
}

pub fn process_callee(blocks: &mut HashMap<u16, Vec<IRStmt>>, entry_addr: u16) {
    // 1. Prologue Detection & Removal (only in entry block)
    if let Some(stmts) = blocks.get_mut(&entry_addr) {
        // Standard Prologue pattern (may be reordered after expression propagation):
        // - Stack adjustments: ADD R6, R6, -N
        // - Saves of R7, R5 to stack: STR R7/R5, R6, 0
        // - FP setup: R5 = R6 + offset
        // - Saves of callee-saved registers R0-R4: STR Rx, R6, 0
        
        // Strategy: Find FP setup, then remove all prologue-related statements
        // regardless of their order in the IR (since expression propagation reorders them).
        
        let mut remove_indices = Vec::new();
        
        // Find where FP is set: Assign(5, R6) or Assign(5, Add(R6, Imm))
        let mut fp_set_idx = None;
        for (i, stmt) in stmts.iter().enumerate() {
            if let IRStmtKind::Assign(5, Expr::Register(6)) = &stmt.kind {
                fp_set_idx = Some(i);
                break;
            }
            // Also support Assign(5, Add(R6, Imm)) - common FP setup
            if let IRStmtKind::Assign(5, Expr::Add(lhs, rhs)) = &stmt.kind {
                 if is_reg(lhs, 6) {
                     fp_set_idx = Some(i);
                     break;
                 }
                 if is_reg(rhs, 6) {
                     fp_set_idx = Some(i);
                     break;
                 }
            }
        }
        
        if let Some(fp_idx) = fp_set_idx {
            remove_indices.push(fp_idx); // Remove FP setup
            
            // Look for `AND R5, R5, 0` just before (clearing R5)
            if fp_idx > 0 {
                if let IRStmtKind::Assign(5, _) = &stmts[fp_idx-1].kind {
                     remove_indices.push(fp_idx-1);
                }
            }
            
            // Find where actual code starts: first instruction that loads from FP-relative address
            // (parameter access like LDR R0, R5, 4) or other computation that's not prologue setup
            let mut first_code_idx = stmts.len();
            for (i, stmt) in stmts.iter().enumerate() {
                if remove_indices.contains(&i) {
                    continue;
                }
                
                match &stmt.kind {
                    // Parameter loads are the start of actual code
                    IRStmtKind::Assign(dst, Expr::Load(addr)) if *dst != 5 && *dst != 6 && *dst != 7 => {
                        // Check if loading from FP-relative address (R5 + offset >= 4)
                        if let Expr::Add(lhs, rhs) = &**addr {
                            let is_param_load = (is_reg(lhs, 5) && matches!(**rhs, Expr::Immediate(off) if off >= 4))
                                || (is_reg(rhs, 5) && matches!(**lhs, Expr::Immediate(off) if off >= 4));
                            if is_param_load {
                                first_code_idx = i;
                                break;
                            }
                        }
                    },
                    // Goto is part of control flow, not prologue
                    IRStmtKind::Goto(_, _, _) => {
                        first_code_idx = i;
                        break;
                    },
                    _ => {}
                }
            }
            
            // Remove all prologue-related statements before first_code_idx
            for i in 0..first_code_idx {
                if remove_indices.contains(&i) {
                    continue;
                }
                
                let stmt = &stmts[i];
                match &stmt.kind {
                    // Stack adjustments (ADD R6, R6, -N)
                    IRStmtKind::Assign(6, _) => {
                        remove_indices.push(i);
                    },
                    // Stack stores (STR Rx, R6, 0) - saving registers
                    IRStmtKind::Store(addr, _val) => {
                        let is_stack_store = match addr {
                            Expr::Register(6) => true,
                            Expr::Add(lhs, _) if is_reg(lhs, 6) => true,
                            _ => false,
                        };
                        if is_stack_store {
                            remove_indices.push(i);
                        }
                    },
                    _ => {}
                }
            }
        }
        
        remove_indices.sort_by(|a, b| b.cmp(a)); // Descending
        remove_indices.dedup();
        for idx in remove_indices {
            if idx < stmts.len() {
                stmts.remove(idx);
            }
        }
    }
    
    // 2. Epilogue Detection & Removal + Return Value
    // Scan all blocks for Return
    let block_addrs: Vec<u16> = blocks.keys().cloned().collect();
    for block_addr in block_addrs {
        if let Some(stmts) = blocks.get_mut(&block_addr) {
            if let Some(last) = stmts.last() {
                if let IRStmtKind::Return(_) = last.kind {
                    // Found a return block. Backtrack.
                    let mut remove_indices = Vec::new();
                    let mut return_val = None;
                    
                    // Walk backwards from end-1
                    let mut i = stmts.len() - 2; 
                    // loop manually
                    let mut done = false;
                    while !done && i < stmts.len() { // i could wrap if usize 0-1
                        let stmt = &stmts[i];
                        
                        match &stmt.kind {
                            IRStmtKind::Assign(6, _) => { // ADD R6, R6, 1
                                remove_indices.push(i);
                            },
                            IRStmtKind::Assign(_, expr) => { // LDR R.., R6, 0
                                // Check if loading from R6 (restore)
                                let is_stack_load = match expr {
                                    Expr::Load(addr) => {
                                         match &**addr {
                                             Expr::Register(6) => true,
                                             Expr::Add(lhs, _) if is_reg(lhs, 6) => true,
                                             _ => false
                                         }
                                    },
                                    _ => false
                                };
                                
                                if is_stack_load {
                                    remove_indices.push(i);
                                } else {
                                    // Maybe setting FP? LDR R5, R6, 0 is covered above.
                                    done = true; 
                                }
                            },
                             IRStmtKind::Store(addr, val) => {
                                 // Check for return value store: STR R0, R5, 3
                                 // Addr = R5 + 3
                                 let is_retval_store = match addr {
                                     Expr::Add(lhs, rhs) => {
                                         if is_reg(lhs, 5) && is_imm(rhs, 3) { true }
                                         else if is_imm(lhs, 3) && is_reg(rhs, 5) { true }
                                         else { false }
                                     },
                                     _ => false
                                 };
                                 
                                 if is_retval_store {
                                     return_val = Some(val.clone());
                                     remove_indices.push(i);
                                     // This is usually the start of teardown sort of?
                                     // Or just before.
                                 } else {
                                     done = true;
                                 }
                             },
                             _ => {
                                 done = true;
                             }
                        }
                        
                        if i == 0 { break; }
                        i -= 1;
                    }
                    
                    // Update Return stmt
                    if let Some(val) = return_val {
                        if let Some(last_stmt) = stmts.last_mut() {
                            last_stmt.kind = IRStmtKind::Return(Some(val));
                        }
                    }
                    
                    remove_indices.sort_by(|a, b| b.cmp(a));
                    for idx in remove_indices {
                        stmts.remove(idx);
                    }
                }
            }
        }
    }
    
    // 3. Parameter Replacement
    for stmts in blocks.values_mut() {
        for stmt in stmts {
            match &mut stmt.kind {
                IRStmtKind::Assign(_, e) |
                IRStmtKind::Store(e, _) | // Store addr
                IRStmtKind::Goto(Some(e), _, _) |
                IRStmtKind::Call(e, _) |
                IRStmtKind::Return(Some(e)) |
                IRStmtKind::If(Some(e), _, _, _) |
                IRStmtKind::DoWhile(Some(e), _, _) |
                IRStmtKind::While(Some(e), _, _) => {
                    replace_params_in_expr(e);
                    if let IRStmtKind::Store(_, v) = &mut stmt.kind { replace_params_in_expr(v); }
                },
                IRStmtKind::For(_init, Some(cond), _, _incr, _) => {
                     replace_params_in_expr(cond);
                }
                _ => {}
            }
            
            // Recurse for nested bodies
            match &mut stmt.kind {
                 IRStmtKind::DoWhile(_, _, body) |
                 IRStmtKind::While(_, _, body) |
                 IRStmtKind::For(_, _, _, _, body) => {
                     process_callee(body, 0xFFFF); // Entry addr irrelevant for inner bodies for prologue
                 },
                 IRStmtKind::If(_, _, true_branch, false_branch) => {
                     process_callee_vec(true_branch);
                     if let Some(fb) = false_branch {
                         process_callee_vec(fb);
                     }
                 },
                 _ => {}
            }
        }
    }
}

fn process_callee_vec(stmts: &mut Vec<IRStmt>) {
     // Helper for If bodies which are Vec<IRStmt> not HashMap
     for stmt in stmts {
          match &mut stmt.kind {
                IRStmtKind::Assign(_, e) => replace_params_in_expr(e),
                IRStmtKind::Store(a, v) => { replace_params_in_expr(a); replace_params_in_expr(v); },
                 // ... handled generically?
                 _ => {} 
          }
          // Recurse...
          // This is getting duplicated.
     }
}

fn replace_params_in_expr(expr: &mut Expr) {
    match expr {
        Expr::Load(inner) => {
             // Check for R5 + Offset
             if let Expr::Add(lhs, rhs) = &**inner {
                 let offset = if is_reg(lhs, 5) {
                     if let Expr::Immediate(off) = **rhs { Some(off) } else { None }
                 } else if is_reg(rhs, 5) {
                     if let Expr::Immediate(off) = **lhs { Some(off) } else { None }
                 } else {
                     None
                 };
                 
                 if let Some(off) = offset {
                     if off >= 4 {
                         *expr = Expr::Param((off - 4) as u8);
                         return;
                     }
                 }
             }
             replace_params_in_expr(inner);
        },
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::And(l, r) => {
            replace_params_in_expr(l);
            replace_params_in_expr(r);
        },
        Expr::Not(e) | Expr::Neg(e) => replace_params_in_expr(e),
        Expr::Call(t, args) => {
            replace_params_in_expr(t);
            for arg in args { replace_params_in_expr(arg); }
        },
        _ => {}
    }
}

pub fn process_caller(blocks: &mut HashMap<u16, Vec<IRStmt>>) {
    // We need to visit all statements to find Call.
    let keys: Vec<u16> = blocks.keys().cloned().collect();
    for key in keys {
        if let Some(stmts) = blocks.get_mut(&key) {
             process_caller_stmts(stmts);
        }
    }
}

fn process_caller_stmts(stmts: &mut Vec<IRStmt>) {
    let mut i = 0;
    while i < stmts.len() {
        // Recurse first
        match &mut stmts[i].kind {
             IRStmtKind::DoWhile(_, _, body) |
             IRStmtKind::While(_, _, body) |
             IRStmtKind::For(_, _, _, _, body) => {
                 process_caller(body);
             },
             IRStmtKind::If(_, _, true_branch, false_branch) => {
                 process_caller_stmts(true_branch);
                 if let Some(fb) = false_branch {
                     process_caller_stmts(fb);
                 }
             },
             _ => {}
        }
        
        let is_call = matches!(stmts[i].kind, IRStmtKind::Call(_, _));
        if is_call {
            // Found a call.
            let mut args = Vec::new();
            let mut remove_indices = Vec::new();
            
            // Backtrack for arguments (pushes)
            // Look for ADD R6, R6, -1 then STR val, R6, 0.
            let mut curr = i;
            let mut arg_detect_state = 0; // 0: expect store, 1: expect add
            let mut arg_val = None;
            let mut store_idx = 0;
            
            while curr > 0 {
                curr -= 1;
                let stmt = &stmts[curr];
                
                if arg_detect_state == 0 {
                    // Expect STR val, R6, 0
                     if let IRStmtKind::Store(addr, val) = &stmt.kind {
                          // Check for Combined Push: Store(Add(R6, -1), val) or Store(Sub(R6, 1), val)
                          let is_combined_push = match addr {
                              Expr::Add(lhs, rhs) => {
                                  if is_reg(lhs, 6) && is_imm(rhs, -1) { true }
                                  else if is_imm(lhs, -1) && is_reg(rhs, 6) { true }
                                  else { false }
                              },
                              Expr::Sub(lhs, rhs) => {
                                  if is_reg(lhs, 6) && is_imm(rhs, 1) { true }
                                  else { false }
                              },
                              _ => false
                          };

                          if is_combined_push {
                              args.push(val.clone());
                              remove_indices.push(curr);
                              arg_detect_state = 0; // Stay in state 0 for next arg
                              continue;
                          }

                          let is_stack_top = match addr {
                              Expr::Register(6) => true,
                              Expr::Add(lhs, _) if is_reg(lhs, 6) => true,
                              _ => false
                          };
                          
                          if is_stack_top {
                              arg_val = Some(val.clone());
                              store_idx = curr;
                              arg_detect_state = 1;
                          } else {
                              break; // Interrupted flow
                          }
                    } else if matches!(stmt.kind, IRStmtKind::Assign(_, _)) {
                         // Ignore assignments to other registers (preparing args)
                    } else {
                        break;
                    }
                } else if arg_detect_state == 1 {
                    // Expect ADD R6, R6, -1
                    if let IRStmtKind::Assign(dst, _) = &stmt.kind {
                         if *dst == 6 {
                             // Found a push
                             if let Some(val) = arg_val.take() {
                                 args.push(val);
                                 remove_indices.push(store_idx);
                                 remove_indices.push(curr);
                                 arg_detect_state = 0;
                             }
                        } else {
                             // Assignment to other register.
                             // Safe to skip.
                        }
                    } else {
                        break;
                    }
                }
            }

            // Check for return handling
            let mut ret_handling_indices = Vec::new();
            let mut ret_val_used = false;
            let mut call_expr = Expr::Call(Box::new(Expr::Immediate(0)), args.clone()); // Placeholder
            
             // Construct the Call Expr
             if let IRStmtKind::Call(target, _) = &stmts[i].kind {
                 call_expr = Expr::Call(Box::new(target.clone()), args.clone());
             }
            
            // First, search for any statement that uses Load(R6) - that's the return value
            let mut ret_val_idx = None;
            for j in (i+1)..stmts.len() {
                let has_r6_load = match &stmts[j].kind {
                    IRStmtKind::Assign(_, expr) => contains_r6_load(expr),
                    IRStmtKind::Store(addr, val) => contains_r6_load(addr) || contains_r6_load(val),
                    _ => false
                };
                if has_r6_load {
                    ret_val_idx = Some(j);
                    break;
                }
            }
            
            if let Some(ret_idx) = ret_val_idx {
                // Replace Load(R6) with the call expression
                let stmt = &mut stmts[ret_idx];
                let replaced = match &mut stmt.kind {
                    IRStmtKind::Assign(_, expr) => replace_r6_load(expr, &call_expr),
                    IRStmtKind::Store(addr, val) => {
                        let u1 = replace_r6_load(addr, &call_expr);
                        let u2 = replace_r6_load(val, &call_expr);
                        u1 || u2
                    },
                    _ => false
                };
                
                if replaced {
                    ret_val_used = true;
                    // The Call is now embedded in the statement at ret_idx.
                    // We should remove the original Call statement at i.
                    remove_indices.push(i);
                    
                    // Look for stack cleanup ADD R6, R6, +N between call and return value usage
                    for j in (i+1)..ret_idx {
                        if let IRStmtKind::Assign(6, expr) = &mut stmts[j].kind {
                            let mut cleanup_val = 0;
                            if let Expr::Add(lhs, rhs) = expr {
                                if is_reg(lhs, 6) { if let Expr::Immediate(v) = **rhs { cleanup_val = v; } }
                                else if is_reg(rhs, 6) { if let Expr::Immediate(v) = **lhs { cleanup_val = v; } }
                            }
                            
                            let expected_cleanup = (args.len() as i16) + 1; // Args + RetVal slot
                            
                            if cleanup_val == expected_cleanup {
                                ret_handling_indices.push(j);
                            } else if cleanup_val < expected_cleanup {
                                let diff = cleanup_val - expected_cleanup;
                                *expr = Expr::Add(Box::new(Expr::Register(6)), Box::new(Expr::Immediate(diff)));
                            } else {
                                ret_handling_indices.push(j);
                            }
                        }
                    }
                    
                    // Also look for cleanup AFTER the return value usage (common pattern)
                    if ret_idx + 1 < stmts.len() {
                        if let IRStmtKind::Assign(6, expr) = &mut stmts[ret_idx + 1].kind {
                            let mut cleanup_val = 0;
                            if let Expr::Add(lhs, rhs) = expr {
                                if is_reg(lhs, 6) { if let Expr::Immediate(v) = **rhs { cleanup_val = v; } }
                                else if is_reg(rhs, 6) { if let Expr::Immediate(v) = **lhs { cleanup_val = v; } }
                            }
                            
                            let expected_cleanup = (args.len() as i16) + 1;
                            
                            if cleanup_val == expected_cleanup {
                                ret_handling_indices.push(ret_idx + 1);
                            } else if cleanup_val < expected_cleanup {
                                let diff = cleanup_val - expected_cleanup;
                                *expr = Expr::Add(Box::new(Expr::Register(6)), Box::new(Expr::Immediate(diff)));
                            } else {
                                ret_handling_indices.push(ret_idx + 1);
                            }
                        }
                    }
                }
            } else {
                // No return value usage found - void call or unused return value
                // Look for immediate stack cleanup at i+1
                if i + 1 < stmts.len() {
                    if let IRStmtKind::Assign(6, expr) = &mut stmts[i+1].kind {
                        let mut cleanup_val = 0;
                        if let Expr::Add(lhs, rhs) = expr {
                            if is_reg(lhs, 6) { if let Expr::Immediate(v) = **rhs { cleanup_val = v; } }
                            else if is_reg(rhs, 6) { if let Expr::Immediate(v) = **lhs { cleanup_val = v; } }
                        }
                        let expected_cleanup = (args.len() as i16) + 1;
                        
                        if cleanup_val == expected_cleanup {
                            ret_handling_indices.push(i+1);
                        } else if cleanup_val < expected_cleanup {
                            let diff = cleanup_val - expected_cleanup;
                            *expr = Expr::Add(Box::new(Expr::Register(6)), Box::new(Expr::Immediate(diff)));
                        } else {
                            ret_handling_indices.push(i+1);
                        }
                    }
                }
            }
            
            // If return value is NOT used, update args in Call stmt at i
            if !ret_val_used {
                 if let IRStmtKind::Call(_, ref mut call_args) = stmts[i].kind {
                    *call_args = args;
                }
            }
            
             remove_indices.extend(ret_handling_indices);
            remove_indices.sort_by(|a, b| b.cmp(a));
             for idx in remove_indices {
                 if idx > i {
                     stmts.remove(idx);
                 } else if idx == i {
                     stmts.remove(idx);
                     i = i.saturating_sub(1);
                 } else if idx < i {
                     stmts.remove(idx);
                     i = i.saturating_sub(1);
                 }
             }
        }
        
        i += 1;
    }
}

fn contains_r6_load(expr: &Expr) -> bool {
    // Checks if expression contains Load(R6) or Load(R6+0)
    match expr {
        Expr::Load(inner) => {
            match &**inner {
                Expr::Register(6) => true,
                Expr::Add(lhs, rhs) => {
                    (is_reg(lhs, 6) && is_imm(rhs, 0)) || (is_imm(lhs, 0) && is_reg(rhs, 6))
                },
                _ => contains_r6_load(inner)
            }
        },
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::And(l, r) => {
            contains_r6_load(l) || contains_r6_load(r)
        },
        Expr::Not(e) | Expr::Neg(e) => contains_r6_load(e),
        Expr::Call(t, args) => {
            contains_r6_load(t) || args.iter().any(|a| contains_r6_load(a))
        },
        _ => false
    }
}

fn replace_r6_load(expr: &mut Expr, replacement: &Expr) -> bool {
    // Replaces Load(R6) or Load(R6+0) with replacement.
    // Returns true if replacement occurred.
    match expr {
        Expr::Load(inner) => {
             let is_stack_top = match &**inner {
                 Expr::Register(6) => true,
                 Expr::Add(lhs, rhs) => {
                     if is_reg(lhs, 6) && is_imm(rhs, 0) { true }
                     else if is_imm(lhs, 0) && is_reg(rhs, 6) { true }
                     else { false }
                 },
                 _ => false
             };
             
             if is_stack_top {
                 *expr = replacement.clone();
                 return true;
             }
             replace_r6_load(inner, replacement)
        },
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::And(l, r) => {
            let c1 = replace_r6_load(l, replacement);
            let c2 = replace_r6_load(r, replacement);
            c1 || c2
        },
        Expr::Not(e) | Expr::Neg(e) => replace_r6_load(e, replacement),
        Expr::Call(t, args) => {
             let mut c = replace_r6_load(t, replacement);
             for arg in args {
                 if replace_r6_load(arg, replacement) { c = true; }
             }
             c
        },
        _ => false
    }
}
