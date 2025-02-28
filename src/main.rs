use std::fs;
use std::collections::HashMap;
use lc3_ensemble::ast::{asm::{disassemble_line, AsmInstr, Stmt, StmtKind}, Label, PCOffset};

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

fn identify_code_data(disassembly: HashMap<u16, Stmt>, origs: Vec<u16>) {
    let mut call_list: Vec<u16> = origs.clone();
    let mut function_list: Vec<u16> = Vec::new();
    let mut work_list: Vec<u16> = Vec::new();

    while !call_list.is_empty() {
        let mut addr = call_list.pop().unwrap();
        function_list.push(addr);

        loop {
            let stmt = &disassembly[&addr];

            match &stmt.nucleus {
                StmtKind::Instr(AsmInstr::JMP(_reg)) => {
                    // no good way to handle this for now
                },
                StmtKind::Instr(AsmInstr::BR(_cc, pcoffset9)) => {
                    let value = match pcoffset9 {
                        PCOffset::Offset(offset) => Some(offset.get()),
                        PCOffset::Label(_) => None,
                    }.unwrap();
                    work_list.push((addr as i16 + value) as u16);
                },
                StmtKind::Instr(AsmInstr::JSRR(_reg)) => {
                    // no good way to handle this for now
                },
                StmtKind::Instr(AsmInstr::JSR(pcoffset11)) => {
                    let value = match pcoffset11 {
                        PCOffset::Offset(offset) => Some(offset.get()),
                        PCOffset::Label(_) => None,
                    }.unwrap();
                    call_list.push((addr as i16 + value) as u16);
                },
                StmtKind::Instr(AsmInstr::RET) => {
                    if work_list.is_empty() {
                        break;
                    }
                },
                StmtKind::Instr(AsmInstr::HALT) => {
                    break;
                },
                StmtKind::Directive(_) => {
                    break;
                },
                _ => {
                    // er
                }
            }

            addr += 1;
        }
    }
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

    let mut sorted_entries: Vec<_> = disassembly.clone().into_iter().collect();
    sorted_entries.sort_by_key(|&(key, _)| key);

    for (addr, stmt) in sorted_entries {
        println!("{:04X}: {}", addr, stmt);
    }

    identify_code_data(disassembly, origs);
}
