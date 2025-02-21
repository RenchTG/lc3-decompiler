use std::fs;
use std::collections::HashMap;

#[derive(Debug)]
enum LineType {
    Instruction(String, Vec<String>),
    Trap(String),
    Directive(String, Vec<String>),
    Label(String)
}

const LC3_INSTRUCTIONS: [&str; 14] = [
    "add", "and", "not", "br", "jmp", "jsr", "jsrr", "ld", "ldi", "ldr", "lea", "st", "sti", "str"
];

const LC3_DIRECTIVES: [&str; 5] = [
    ".orig", ".fill", ".stringz", ".blkw", ".end"
];

const LC3_TRAPS: [&str; 6] = [
    "getc", "out", "puts", "in", "putsp", "halt"
];

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

fn disassemble(text: Vec<&str>, symbols: HashMap<u16, &str>) {
    let mut ops: Vec::<LineType> = vec!();
    let mut index = 0;
    while index < text.len() {
        let start_addr = u16::from_str_radix(text[index], 16).unwrap();
        let length_of_section = text[index+1].parse::<u32>().unwrap();
        index += 2;
        for i in 0..length_of_section {
            if text[index] == "????" {
                index += 1;
                continue;
            }

            let instr = u16::from_str_radix(text[index], 16).unwrap();
            let opcode = (instr >> 12) & 0xF;
            index += 1;

            match opcode {
                0x0 => {
                    ops.push(LineType::Instruction("br".to_string(), vec!()));
                    println!("br");
                }
                0x1 => {
                    ops.push(LineType::Instruction("add".to_string(), vec!()));
                    println!("add");
                }
                0x2 => {
                    ops.push(LineType::Instruction("ld".to_string(), vec!()));
                    println!("ld");
                }
                0x3 => {
                    ops.push(LineType::Instruction("st".to_string(), vec!()));
                    println!("st");
                }
                0x4 => {
                    ops.push(LineType::Instruction("jsr".to_string(), vec!()));
                    println!("jsr");
                }
                0x5 => {
                    ops.push(LineType::Instruction("and".to_string(), vec!()));
                    println!("And!!");
                }
                0x6 => {
                    ops.push(LineType::Instruction("ldr".to_string(), vec!()));
                    println!("ldr");
                }
                0x7 => {
                    ops.push(LineType::Instruction("str".to_string(), vec!()));
                    println!("str");
                }
                0x8 => {
                    ops.push(LineType::Instruction("rti".to_string(), vec!()));
                    println!("rti");
                }
                0x9 => {
                    ops.push(LineType::Instruction("not".to_string(), vec!()));
                    println!("not");
                }
                0xa => {
                    ops.push(LineType::Instruction("ldi".to_string(), vec!()));
                    println!("ldi");
                }
                0xb => {
                    ops.push(LineType::Instruction("sti".to_string(), vec!()));
                    println!("sti");
                }
                0xc => {
                    ops.push(LineType::Instruction("jmp".to_string(), vec!()));
                    println!("jmp");
                }
                0xd => {
                    println!("Reserved opcode");
                }
                0xe => {
                    ops.push(LineType::Instruction("lea".to_string(), vec!()));
                    println!("lea");
                }
                0xf => {
                    ops.push(LineType::Trap("trap".to_string()));
                    println!("trap");
                }
                _ => {
                    println!("Unknown Operation");
                }
            }
        }
    }
}

//fn classify_line(line: &str) -> LineType {
//    let tokens: Vec<&str> = line.split_whitespace().collect();
//
//    if tokens[0].starts_with('.') {
//        return LineType::Directive(tokens[0].to_string(), tokens[1..].iter().map(|s| s.to_string()).collect());
//    } else if tokens[0].starts_with("br") {
//        let mut operands = vec![tokens[0][2..].chars().collect()];
//        operands.extend(tokens[1..].iter().map(|s| s.to_string()));
//        return LineType::Instruction("br".to_string(), operands);
//    } else if LC3_INSTRUCTIONS.contains(&tokens[0]) {
//        return LineType::Instruction(tokens[0].to_string(), tokens[1..].iter().map(|s| s.to_string()).collect());
//    } else if tokens[0] == "trap" {
//        return LineType::Trap(tokens[1].to_string());
//    } else if LC3_TRAPS.contains(&tokens[0]) {
//        return LineType::Trap(tokens[0].to_string());
//    } else if tokens.len() == 1 {
//        return LineType::Label(tokens[0].to_string());
//    } else {
//        println!("{}", tokens[0]);
//        panic!("Unknown line!");
//    }
//}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path to obj file>", args[0]);
        std::process::exit(1);
    }

    let input_obj = fs::read_to_string(args[1].clone()).expect("Failed to read file");
    assert!(input_obj.starts_with("LC-3 OBJ FILE"));

    let (text, symbols) = gen_sections(&input_obj);

    disassemble(text, symbols);
}
