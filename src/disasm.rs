use std::collections::{HashMap, HashSet};
use lc3_ensemble::ast::asm::{disassemble_line, AsmInstr, Stmt, StmtKind};
use lc3_ensemble::ast::{Label, PCOffset};

pub fn gen_sections(content: &str) -> (Vec<&str>, HashMap<u16, &str>) {
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

pub fn disassemble(text: Vec<&str>, symbols: HashMap<u16, &str>) -> (HashMap<u16, Stmt>, Vec<u16>) {
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

pub fn identify_code_data(disassembly: &HashMap<u16, Stmt>, _origs: Vec<u16>) -> Vec<u16> {
    let mut call_list: Vec<u16> = vec![0x3000];
    let mut function_list: Vec<u16> = Vec::new();
    // work_list scans the current function's blocks
    let mut work_list: Vec<u16> = Vec::new();
    let mut visited: HashSet<u16> = HashSet::new();

    while !call_list.is_empty() {
        let mut addr = call_list.pop().unwrap();
        
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

pub fn find_next_label(current_address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<u16> {
    let mut addr = current_address + 1;
    while let Some(stmt) = disassembly.get(&addr) {
        if !stmt.labels.is_empty() {
            return Some(addr);
        }
        addr += 1;
    }
    None
}

pub fn find_next_branch(current_address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<(u16, u16, bool, bool)> {
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

pub fn get_branch_destination(address: u16, disassembly: &HashMap<u16, Stmt>) -> Option<u16> {
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
