use std::collections::HashMap;
use crate::linear::LinearStmt;
use crate::ir::Expr;

fn stringify_expr(expr: &Expr, symbols: &HashMap<u16, &str>) -> String {
    match expr {
        Expr::Register(r) => format!("var{}", r),
        Expr::Immediate(val) => {
            // Check if this immediate is an address in the symbol table
            let addr_u16 = *val as u16;
            if let Some(name) = symbols.get(&addr_u16) {
                if !(*name).starts_with("LC-3 OBJ FILE") {
                    return name.to_string();
                }
            }
            format!("{}", val)
        },

        Expr::Add(lhs, rhs) => {
            // Check if rhs is a negative immediate - format as subtraction
            if let Expr::Immediate(val) = &**rhs {
                if *val < 0 {
                    return format!("{} - {}", stringify_expr(lhs, symbols), -val);
                }
            }
            format!("{} + {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols))
        },
        Expr::Sub(lhs, rhs) => format!("{} - {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols)),
        Expr::And(lhs, rhs) => format!("{} & {}", stringify_expr(lhs, symbols), stringify_expr(rhs, symbols)),
        Expr::Not(e) => format!("~{}", stringify_expr(e, symbols)),
        Expr::Neg(e) => format!("-{}", stringify_expr(e, symbols)),
        Expr::Load(e) => {
            if let Expr::Immediate(addr) = **e {
                let addr_u16 = addr as u16;
                if let Some(name) = symbols.get(&addr_u16) {
                    if (*name).starts_with("LC-3 OBJ FILE") {
                         format!("*global_{:04X}", addr_u16)
                    } else {
                         format!("*{}", name)
                    }
                } else {
                    format!("*global_{:04X}", addr_u16)
                }
            } else {
                format!("*({})", stringify_expr(e, symbols))
            }
        },
        Expr::Param(idx) => format!("param{}", idx),
        Expr::Call(target, args) => {
             let args_str = args.iter().map(|arg| stringify_expr(arg, symbols)).collect::<Vec<String>>().join(", ");
             if let Expr::Immediate(addr) = **target {
                 let addr_u16 = addr as u16;
                  if let Some(name) = symbols.get(&addr_u16) {
                      format!("{}({})", name, args_str)
                  } else {
                      format!("func_{:04X}({})", addr_u16, args_str)
                  }
             } else {
                 format!("{}({})", stringify_expr(target, symbols), args_str)
             }
        },
        Expr::LogicalAnd(lhs, lhs_cc, rhs, rhs_cc) => {
            format!("{} && {}", 
                    stringify_condition(lhs, *lhs_cc, symbols),
                    stringify_condition(rhs, *rhs_cc, symbols))
        },
        Expr::LogicalOr(lhs, lhs_cc, rhs, rhs_cc) => {
            format!("{} || {}", 
                    stringify_condition(lhs, *lhs_cc, symbols),
                    stringify_condition(rhs, *rhs_cc, symbols))
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
    // Handle LogicalAnd/LogicalOr directly - they carry their own CCs
    if let Expr::LogicalAnd(lhs, lhs_cc, rhs, rhs_cc) = expr {
        return format!("{} && {}", 
                stringify_condition(lhs, *lhs_cc, symbols),
                stringify_condition(rhs, *rhs_cc, symbols));
    }
    if let Expr::LogicalOr(lhs, lhs_cc, rhs, rhs_cc) = expr {
        return format!("{} || {}", 
                stringify_condition(lhs, *lhs_cc, symbols),
                stringify_condition(rhs, *rhs_cc, symbols));
    }

    let (lhs, rhs) = match expr {
        Expr::Sub(l, r) => (Some(l), Some(r)),
        Expr::Add(l, r) => {
            match &**r {
                Expr::Neg(inner) => (Some(l), Some(inner)),
                Expr::Immediate(x) if *x < 0 => {
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
            
            if let Expr::Add(lhs, rhs) = expr {
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
            let addr_str = stringify_expr(addr, symbols);
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
             let target_str = format!("label_{:04X}", target);
             
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
        LinearStmt::Call(expr, args) => {
             let args_str = args.iter().map(|arg| stringify_expr(arg, symbols)).collect::<Vec<String>>().join(", ");
             if let Expr::Immediate(addr) = expr {
                 let addr_u16 = *addr as u16;
                  if let Some(name) = symbols.get(&addr_u16) {
                      format!("{}{}({});", spaces, name, args_str)
                  } else {
                      format!("{}func_{:04X}({});", spaces, addr_u16, args_str)
                  }
             } else {
                 format!("{}{}({});", spaces, stringify_expr(expr, symbols), args_str)
             }
        },
        LinearStmt::Return(val) => {
            if let Some(v) = val {
                format!("{}return {};", spaces, stringify_expr(v, symbols))
            } else {
                format!("{}return;", spaces)
            }
        },
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
                 // Check if else branch is a single If statement - emit as "else if" instead of "else { if }"
                 if false_stmts.len() == 1 {
                     if let LinearStmt::If(_, _, _, _) = &false_stmts[0] {
                         // Emit as "else if" without extra braces
                         let else_if_str = stringify_stmt(&false_stmts[0], symbols, indent);
                         // Remove leading spaces from the else-if since we're joining it inline
                         let trimmed = else_if_str.trim_start();
                         s.push_str(&format!(" else {}", trimmed));
                     } else {
                         s.push_str(" else {\n");
                         for child in false_stmts {
                             s.push_str(&stringify_stmt(child, symbols, indent + 1));
                             s.push('\n');
                         }
                         s.push_str(&format!("{}}}", spaces));
                     }
                 } else {
                     s.push_str(" else {\n");
                     for child in false_stmts {
                         s.push_str(&stringify_stmt(child, symbols, indent + 1));
                         s.push('\n');
                     }
                     s.push_str(&format!("{}}}", spaces));
                 }
             }
             s
        },
        LinearStmt::Break => format!("{}break;", spaces),
        LinearStmt::Continue => format!("{}continue;", spaces),
        LinearStmt::Label(addr) => {
             format!("label_{:04X}:", addr)
        },
    }
}

pub fn generate_code(stmts: &Vec<LinearStmt>, symbols: &HashMap<u16, &str>) -> String {
    let mut output = String::new();
    for stmt in stmts {
        output.push_str(&stringify_stmt(stmt, symbols, 0));
        output.push('\n');
    }
    output
}
