use std::collections::{HashMap, HashSet};
use std::fmt;
use crate::ir::{Expr, IRStmt, IRStmtKind};

#[derive(Clone)]
pub enum LinearStmt {
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
