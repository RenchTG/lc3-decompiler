use std::fs;

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

fn preprocess(content: &str) -> String {
    content
        .lines()
        .filter_map(|line| {
            let line = match line.find(';') {
                Some(pos) => &line[0..pos],
                None => line,
            };

            let line = line.trim().to_lowercase();

            if line.is_empty() {
                None
            } else {
                Some(line.to_string())
            }
        })
    .collect::<Vec<String>>()
    .join("\n")
}

fn classify_line(line: &str) -> LineType {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    if tokens[0].starts_with('.') {
        return LineType::Directive(tokens[0].to_string(), tokens[1..].iter().map(|s| s.to_string()).collect());
    } else if tokens[0].starts_with("br") {
        let mut operands = vec![tokens[0][2..].chars().collect()];
        operands.extend(tokens[1..].iter().map(|s| s.to_string()));
        return LineType::Instruction("br".to_string(), operands);
    } else if LC3_INSTRUCTIONS.contains(&tokens[0]) {
        return LineType::Instruction(tokens[0].to_string(), tokens[1..].iter().map(|s| s.to_string()).collect());
    } else if tokens[0] == "trap" {
        return LineType::Trap(tokens[1].to_string());
    } else if LC3_TRAPS.contains(&tokens[0]) {
        return LineType::Trap(tokens[0].to_string());
    } else if tokens.len() == 1 {
        return LineType::Label(tokens[0].to_string());
    } else {
        println!("{}", tokens[0]);
        panic!("Unknown line!");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path to asm file>", args[0]);
        std::process::exit(1);
    }

    let input_asm = fs::read_to_string(args[1].clone()).expect("Failed to read file");
    let clean_asm = preprocess(&input_asm);
    let mut lines: Vec<LineType> = vec![];
    for line in clean_asm.lines() {
        lines.push(classify_line(line));
    }

    for line in lines.iter() {
        println!("{:?}", line);
    }
}
