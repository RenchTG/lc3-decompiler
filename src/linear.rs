use std::collections::{HashMap, HashSet};
use std::fmt;
use crate::ir::{Expr, IRStmt, IRStmtKind};

#[derive(Clone)]
pub enum LinearStmt {
    Assign(u8, Expr),
    Store(Expr, Expr),
    Goto(Option<Expr>, u8, u16),
    Call(Expr, Vec<Expr>),
    Return(Option<Expr>),
    Trap(u8),
    IndirectGoto(Expr),
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
            LinearStmt::Call(target, args) => {
                write!(f, "Call({:?}", target)?;
                for arg in args {
                    write!(f, ", {:?}", arg)?;
                }
                write!(f, ")")
            },
            LinearStmt::Return(val) => {
                match val {
                    Some(v) => write!(f, "Return({:?})", v),
                    None => write!(f, "Return"),
                }
            },
            LinearStmt::Trap(vect) => write!(f, "Trap(0x{:X})", vect),
            LinearStmt::IndirectGoto(target) => write!(f, "IndirectGoto({:?})", target),
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
            IRStmtKind::While(_, _, body) => {
                for inner_stmts in body.values() {
                    collect_goto_targets(inner_stmts, targets);
                }
            },
             IRStmtKind::For(_init, _, _, _incr, body) => {
                // Init and Incr are single statements but might have Goto (unlikely for Assign/Store but possible in IRStmt general case)
                // Actually For init/incr are IRStmt, which *could* be Goto, but semantically usually Assign.
                // Recurs into body (HashMap)
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
        IRStmtKind::Call(target, args) => LinearStmt::Call(target.clone(), args.clone()),
        IRStmtKind::Return(val) => LinearStmt::Return(val.clone()),
        IRStmtKind::Trap(v) => LinearStmt::Trap(*v),
        IRStmtKind::IndirectGoto(target) => LinearStmt::IndirectGoto(target.clone()),
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

pub fn linearize_blocks(blocks: &HashMap<u16, Vec<IRStmt>>, targets: &HashSet<u16>) -> Vec<LinearStmt> {
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

pub fn linearize(blocks: &HashMap<u16, Vec<IRStmt>>) -> Vec<LinearStmt> {
    let mut targets = HashSet::new();
    // Scan all top-level blocks for gotos
    for stmts in blocks.values() {
        collect_goto_targets(stmts, &mut targets);
    }
    
    linearize_blocks(blocks, &targets)
}

/// Negate a condition code (flip the comparison)
fn negate_cc(cc: u8) -> u8 {
    (!cc) & 7
}

/// Convert a LogicalOr condition to LogicalAnd with negated sub-conditions
/// LogicalOr(a, cc1, b, cc2) with negated CCs becomes LogicalAnd(a, !cc1, b, !cc2)
fn negate_logical_expr(expr: &Expr) -> Expr {
    match expr {
        Expr::LogicalOr(lhs, lhs_cc, rhs, rhs_cc) => {
            // NOT(a OR b) = NOT(a) AND NOT(b)
            Expr::LogicalAnd(
                Box::new((**lhs).clone()),
                negate_cc(*lhs_cc),
                Box::new((**rhs).clone()),
                negate_cc(*rhs_cc)
            )
        },
        Expr::LogicalAnd(lhs, lhs_cc, rhs, rhs_cc) => {
            // NOT(a AND b) = NOT(a) OR NOT(b)
            Expr::LogicalOr(
                Box::new((**lhs).clone()),
                negate_cc(*lhs_cc),
                Box::new((**rhs).clone()),
                negate_cc(*rhs_cc)
            )
        },
        _ => expr.clone()
    }
}

/// Structure if-goto patterns into proper If-Else statements
/// Detects: Goto(cond, cc, Lfalse); [body]; Goto(_, 7, Ljoin); Label(Lfalse); ...
/// Converts to: If(negated_cond, negated_cc, [body], [else_branch or None])
pub fn structure_if_goto_patterns(stmts: Vec<LinearStmt>) -> Vec<LinearStmt> {
    let mut result = Vec::new();
    let mut i = 0;
    
    while i < stmts.len() {
        // Look for pattern: Goto(Some(cond), cc, Lfalse) where cc != 7
        if let LinearStmt::Goto(Some(cond), cc, false_target) = &stmts[i] {
            if *cc != 7 {
                // Find the unconditional goto to join and the label for false_target
                let mut body_end = None;
                let mut join_target = None;
                let mut label_pos = None;
                
                for j in (i + 1)..stmts.len() {
                    // Look for unconditional goto (the end of true body)
                    if let LinearStmt::Goto(None, 7, join) = &stmts[j] {
                        body_end = Some(j);
                        join_target = Some(*join);
                        
                        // Check if next is the false_target label
                        if j + 1 < stmts.len() {
                            if let LinearStmt::Label(lbl) = &stmts[j + 1] {
                                if lbl == false_target {
                                    label_pos = Some(j + 1);
                                }
                            }
                        }
                        break;
                    }
                    // If we hit the label before finding the goto, stop
                    if let LinearStmt::Label(lbl) = &stmts[j] {
                        if lbl == false_target {
                            break;
                        }
                    }
                }
                
                // If we found the pattern
                if let (Some(body_end_idx), Some(join), Some(label_idx)) = (body_end, join_target, label_pos) {
                    // Extract the true body (between conditional goto and unconditional goto)
                    let true_body: Vec<LinearStmt> = stmts[(i + 1)..body_end_idx].to_vec();
                    
                    // Negate the condition
                    let negated_cond = negate_logical_expr(cond);
                    let negated_cc = negate_cc(*cc);
                    
                    // Now check if there's an else branch
                    // Look for statements after the label until we hit the join point or another pattern
                    let mut else_end = None;
                    for j in (label_idx + 1)..stmts.len() {
                        // If we hit the join label, that's where else ends
                        if let LinearStmt::Label(lbl) = &stmts[j] {
                            if *lbl == join {
                                else_end = Some(j);
                                break;
                            }
                        }
                        // If we hit an unconditional goto to join, else ends before it
                        if let LinearStmt::Goto(None, 7, target) = &stmts[j] {
                            if *target == join {
                                // Include up to but not including this goto
                                else_end = Some(j + 1); // Include the goto, it's the else terminator
                                break;
                            }
                        }
                    }
                    
                    // Build the else branch if there is one
                    let else_branch = if let Some(else_end_idx) = else_end {
                        let else_stmts: Vec<LinearStmt> = stmts[(label_idx + 1)..else_end_idx].iter()
                            .filter(|s| !matches!(s, LinearStmt::Goto(None, 7, _)))
                            .cloned()
                            .collect();
                        if else_stmts.is_empty() {
                            None
                        } else {
                            // Recursively structure the else branch
                            Some(structure_if_goto_patterns(else_stmts))
                        }
                    } else {
                        None
                    };
                    
                    // Recursively structure the true body
                    let structured_true = structure_if_goto_patterns(true_body);
                    
                    // Create the If statement
                    result.push(LinearStmt::If(Some(negated_cond), negated_cc, structured_true, else_branch));
                    
                    // Skip past the processed statements
                    if let Some(else_end_idx) = else_end {
                        i = else_end_idx;
                    } else {
                        i = label_idx + 1;
                    }
                    continue;
                }
                
                // Fallback Pattern: Goto(cond); [body]; Label(target)
                // This handles simple if without else, where body falls through to label
                // Pattern: if(cond_fail) goto L; body; L:
                // Becomes: if(!cond_fail) { body }
                let mut simple_label_pos = None;
                for j in (i + 1)..stmts.len() {
                    if let LinearStmt::Label(lbl) = &stmts[j] {
                        if *lbl == *false_target {
                            simple_label_pos = Some(j);
                            break;
                        }
                    }
                }
                
                if let Some(label_idx) = simple_label_pos {
                    // Extract body between conditional goto and label
                    let body: Vec<LinearStmt> = stmts[(i + 1)..label_idx].to_vec();
                    
                    if !body.is_empty() {
                        // Negate the condition
                        let negated_cond = negate_logical_expr(cond);
                        let negated_cc = negate_cc(*cc);
                        
                        // Recursively structure the body
                        let structured_body = structure_if_goto_patterns(body);
                        
                        // Create the If statement (no else branch)
                        result.push(LinearStmt::If(Some(negated_cond), negated_cc, structured_body, None));
                        
                        // Keep the label in result so other gotos can reach it
                        // But skip past the processed body
                        i = label_idx;
                        continue;
                    }
                }
                
                // Third pattern: Goto to external label (not in current stmts)
                // Pattern: if(cond) goto external; body;
                // The body is everything after the goto until end of current slice
                // This becomes: if(!cond) { body }
                // This handles else-if chains where the goto target is outside the else branch
                let remaining: Vec<LinearStmt> = stmts[(i + 1)..].to_vec();
                if !remaining.is_empty() && !remaining.iter().any(|s| matches!(s, LinearStmt::Label(l) if *l == *false_target)) {
                    // The target label is not in this slice - it's an external jump
                    // Treat remaining statements as the "if NOT cond" body
                    let negated_cond = negate_logical_expr(cond);
                    let negated_cc = negate_cc(*cc);
                    
                    let structured_remaining = structure_if_goto_patterns(remaining);
                    
                    result.push(LinearStmt::If(Some(negated_cond), negated_cc, structured_remaining, None));
                    
                    // We've consumed all remaining statements
                    break;
                }
            }
        }
        
        // Handle nested structures recursively
        match &stmts[i] {
            LinearStmt::While(cond, cc, body) => {
                result.push(LinearStmt::While(cond.clone(), *cc, structure_if_goto_patterns(body.clone())));
            },
            LinearStmt::For(init, cond, cc, incr, body) => {
                result.push(LinearStmt::For(init.clone(), cond.clone(), *cc, incr.clone(), structure_if_goto_patterns(body.clone())));
            },
            LinearStmt::DoWhile(cond, cc, body) => {
                result.push(LinearStmt::DoWhile(cond.clone(), *cc, structure_if_goto_patterns(body.clone())));
            },
            LinearStmt::If(cond, cc, true_branch, false_branch) => {
                result.push(LinearStmt::If(
                    cond.clone(),
                    *cc,
                    structure_if_goto_patterns(true_branch.clone()),
                    false_branch.as_ref().map(|fb| structure_if_goto_patterns(fb.clone()))
                ));
            },
            _ => {
                result.push(stmts[i].clone());
            }
        }
        
        i += 1;
    }
    
    result
}

/// Collect all goto targets from a list of statements
fn collect_goto_targets_linear(stmts: &[LinearStmt], targets: &mut HashSet<u16>) {
    for stmt in stmts {
        match stmt {
            LinearStmt::Goto(_, _, target) => {
                targets.insert(*target);
            },
            LinearStmt::While(_, _, body) |
            LinearStmt::DoWhile(_, _, body) => {
                collect_goto_targets_linear(body, targets);
            },
            LinearStmt::For(_, _, _, _, body) => {
                collect_goto_targets_linear(body, targets);
            },
            LinearStmt::If(_, _, true_branch, false_branch) => {
                collect_goto_targets_linear(true_branch, targets);
                if let Some(fb) = false_branch {
                    collect_goto_targets_linear(fb, targets);
                }
            },
            _ => {}
        }
    }
}

/// Remove labels that have no gotos pointing to them
fn remove_orphaned_labels_inner(stmts: Vec<LinearStmt>, targets: &HashSet<u16>) -> Vec<LinearStmt> {
    stmts.into_iter().filter_map(|stmt| {
        match stmt {
            LinearStmt::Label(addr) => {
                if targets.contains(&addr) {
                    Some(LinearStmt::Label(addr))
                } else {
                    None // Remove orphaned label
                }
            },
            LinearStmt::While(cond, cc, body) => {
                Some(LinearStmt::While(cond, cc, remove_orphaned_labels_inner(body, targets)))
            },
            LinearStmt::DoWhile(cond, cc, body) => {
                Some(LinearStmt::DoWhile(cond, cc, remove_orphaned_labels_inner(body, targets)))
            },
            LinearStmt::For(init, cond, cc, incr, body) => {
                Some(LinearStmt::For(init, cond, cc, incr, remove_orphaned_labels_inner(body, targets)))
            },
            LinearStmt::If(cond, cc, true_branch, false_branch) => {
                Some(LinearStmt::If(
                    cond,
                    cc,
                    remove_orphaned_labels_inner(true_branch, targets),
                    false_branch.map(|fb| remove_orphaned_labels_inner(fb, targets))
                ))
            },
            _ => Some(stmt)
        }
    }).collect()
}

/// Remove orphaned labels from structured code
pub fn remove_orphaned_labels(stmts: Vec<LinearStmt>) -> Vec<LinearStmt> {
    let mut targets = HashSet::new();
    collect_goto_targets_linear(&stmts, &mut targets);
    remove_orphaned_labels_inner(stmts, &targets)
}
