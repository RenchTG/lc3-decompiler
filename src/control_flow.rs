use std::collections::{HashMap, HashSet};
use crate::cfg::{BasicBlock, NaturalLoop};
use crate::ir::{IRStmt, IRStmtKind, Expr};
use crate::liveness::LivenessInfo;

#[derive(Debug, Clone)]
pub struct Loop {
    pub header: u16,
    pub blocks: HashSet<u16>,
    pub break_block: Option<u16>,
    pub continue_block: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum ConditionalType {
    If,
    IfElse,
}

#[derive(Debug, Clone)]
pub struct Conditional {
    pub kind: ConditionalType,
    pub condition_block: u16,
    pub true_block: u16,
    pub false_block: Option<u16>,
    pub join_block: u16,
}

pub fn identify_loops(natural_loops: &mut Vec<NaturalLoop>, blocks: &HashMap<u16, BasicBlock>) -> Vec<Loop> {
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

pub fn identify_conditionals(blocks: &HashMap<u16, BasicBlock>) -> Vec<Conditional> {
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

pub fn structure_loops(
    goto_blocks: &mut HashMap<u16, Vec<IRStmt>>, 
    loops: &Vec<Loop>, 
    _blocks: &HashMap<u16, BasicBlock>,
    conditionals: &Vec<Conditional>
) {
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
        if let Some(cont_addr) = continue_block {
            if let Some(stmts) = loop_body.get_mut(&cont_addr) {
                stmts.retain(|stmt| {
                    if let IRStmtKind::Goto(expr, cc, target) = &stmt.kind {
                        if *target == header {
                            let same_cc = *cc == condition_cc;
                            let same_expr = match (expr, &condition) {
                                (Some(e1), Some(e2)) => format!("{:?}", e1) == format!("{:?}", e2), 
                                (None, None) => true,
                                _ => false,
                            };
                            if same_cc && same_expr { return false; }
                        }
                    }
                    true
                });
            }
        }
 
        // Apply Break/Continue
        for (block_addr, stmts) in loop_body.iter_mut() {
            let is_condition_block = conditionals.iter().any(|c| c.condition_block == *block_addr);

            for stmt in stmts.iter_mut() {
                if let IRStmtKind::Goto(expr, cc, target) = &stmt.kind {
                    let target_is_break = Some(*target) == break_block;
                    let target_is_continue = *target == header;
                    
                    if (target_is_break || target_is_continue) && !is_condition_block {
                        let new_kind = if target_is_break { IRStmtKind::Break } else { IRStmtKind::Continue };
                        
                        if *cc == 7 {
                            stmt.kind = new_kind;
                        } else {
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
    for (reg, val) in assignments.iter().rev() {
        expr = substitute_register(&expr, *reg, val);
    }
    expr
}

pub fn refine_loops(
    goto_blocks: &mut HashMap<u16, Vec<IRStmt>>,
    identified_loops: &Vec<Loop>,
    blocks: &HashMap<u16, BasicBlock>,
    _conditionals: &Vec<Conditional>,
    liveness: &HashMap<u16, LivenessInfo>
) {
    let keys: Vec<u16> = goto_blocks.keys().cloned().collect();
    let mut init_removals: Vec<(u16, u16)> = Vec::new();

    for key in keys.clone() {
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
                                    if true_branch.len() == 1 && matches!(true_branch[0].kind, IRStmtKind::Break) && false_branch.is_none() {
                                        if_stmt_idx = Some(i);
                                        
                                        if !pending_assigns.is_empty() {
                                            if let Some(loop_info) = identified_loops.iter().find(|l| l.header == header_addr) {
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
                                                    dead_on_exit = false;
                                                }
                                                
                                                if dead_on_exit {
                                                    if let Some(cond_expr) = if_cond {
                                                        let folded = fold_assignments(cond_expr.clone(), &pending_assigns);
                                                        new_while_cond = Some(folded);
                                                    }
                                                    new_while_cc = (!if_cc) & 7;
                                                    converted_to_while = true;
                                                }
                                            }
                                        } else {
                                            new_while_cond = if_cond.clone();
                                            new_while_cc = (!if_cc) & 7;
                                            converted_to_while = true;
                                        }
                                    }
                                    break;
                                },
                                _ => {
                                    break;
                                }
                            }
                        }
                     }
                     
                     if converted_to_while {
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
    
    for key in keys {
        if let Some(stmts) = goto_blocks.get_mut(&key) {
            if stmts.len() == 1 {
                let header_addr = stmts[0].addr;
                let stmt = &mut stmts[0];
                if let IRStmtKind::While(ref cond, ref cc, ref mut body) = stmt.kind {
                    let mut loop_var = None;
                    
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
                        
                        if loop_var.is_none() {
                             if let Expr::Sub(_, rhs) = expr {
                                 if let Expr::Register(r) = **rhs {
                                     loop_var = Some(r);
                                 }
                             }
                        }
                    }
                    
                    if let Some(var) = loop_var {
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
                        
                        let mut incr_stmt: Option<IRStmt> = None;
                        let mut continue_block_addr = None;
                        
                        if let Some(loop_info) = identified_loops.iter().find(|l| l.header == header_addr) {
                            if let Some(cont_addr) = loop_info.continue_block {
                                if let Some(stmts) = body.get(&cont_addr) {
                                    continue_block_addr = Some(cont_addr);
                                    
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
                            if let Some(removal) = potential_init_removal {
                                init_removals.push(removal);
                            }
                            
                             if let Some(stmts) = body.get_mut(&cont_addr) {
                                 let is_terminator = if let Some(last) = stmts.last() {
                                     matches!(last.kind, IRStmtKind::Continue | IRStmtKind::Goto(_, _, _) | IRStmtKind::Break | IRStmtKind::Return | IRStmtKind::Trap(_))
                                 } else { false };
                                 
                                 if is_terminator {
                                     if stmts.len() >= 2 { stmts.remove(stmts.len() - 2); }
                                 } else {
                                     stmts.pop();
                                 }
                                 
                                 if let Some(last) = stmts.last_mut() {
                                      if let IRStmtKind::Goto(_, _, target) = last.kind {
                                         if target == header_addr {
                                             last.kind = IRStmtKind::Continue;
                                         }
                                     }
                                 }
                             }
                             
                             stmt.kind = IRStmtKind::For(Box::new(init), cond.clone(), *cc, Box::new(incr), body.clone());
                        }
                    }
                }
            }
        }
    }
    
    for (block_addr, stmt_addr) in init_removals {
        if let Some(stmts) = goto_blocks.get_mut(&block_addr) {
            if let Some(pos) = stmts.iter().position(|s| s.addr == stmt_addr) {
                stmts.remove(pos);
            }
        }
    }
}

pub fn structure_conditionals(
    blocks: &mut HashMap<u16, Vec<IRStmt>>,
    conditionals: &Vec<Conditional>
) {
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

    let mut applicable_indices: Vec<usize> = Vec::new();
    for (i, cond) in conditionals.iter().enumerate() {
        if blocks.contains_key(&cond.condition_block) {
            applicable_indices.push(i);
        }
    }

    applicable_indices.sort_by(|&i_a, &i_b| {
        let a = &conditionals[i_a];
        let b = &conditionals[i_b];
        
        let a_points_to_b = a.true_block == b.condition_block || 
                            a.false_block.map_or(false, |fb| fb == b.condition_block);
        
        let b_points_to_a = b.true_block == a.condition_block || 
                            b.false_block.map_or(false, |fb| fb == a.condition_block);
                            
        if a_points_to_b {
            std::cmp::Ordering::Greater 
        } else if b_points_to_a {
            std::cmp::Ordering::Less 
        } else {
            std::cmp::Ordering::Equal
        }
    });

    for idx in applicable_indices {
        let cond = &conditionals[idx];
        
        if !blocks.contains_key(&cond.condition_block) {
            continue;
        }

        let (expr_opt, cc, target_addr, stmt_addr) = {
             let stmts = &blocks[&cond.condition_block];
             if let Some(last) = stmts.last() {
                 if let IRStmtKind::Goto(expr, cc, target) = &last.kind {
                     (expr.clone(), *cc, *target, last.addr)
                 } else {
                     continue; 
                 }
             } else {
                 continue;
             }
        };
        
        if let Some(expr) = expr_opt {
            let final_cond = expr;
            let mut final_cc = cc;
            
            let mut true_stmts = Vec::new();
            let mut false_stmts = None;
            
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
            
            if let Some(stmts) = blocks.get_mut(&cond.condition_block) {
                 let if_stmt = IRStmt {
                    addr: stmt_addr, 
                    kind: IRStmtKind::If(Some(final_cond), final_cc, true_stmts, false_stmts),
                };
                
                let last_idx = stmts.len() - 1;
                stmts[last_idx] = if_stmt;
            }
        }
    }
}
