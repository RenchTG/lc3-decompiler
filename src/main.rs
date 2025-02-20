use std::fs;

fn preprocess(content: &str) -> String {
    content
        .lines()
        .filter_map(|line| {
            let line = match line.find(';') {
                Some(pos) => &line[0..pos],
                None => line,
            };

            let line = line.trim();

            if line.is_empty() {
                None
            } else {
                Some(line.to_string())
            }
        })
    .collect::<Vec<String>>()
    .join("\n")
}

fn parse_line(line: &str) -> String {
    if line.starts_with('.') {
        return format!("Assembler directive: {line}");
    }
    
    match line {
        "add" => format!("{} = {} + {};", )
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
    for line in clean_asm.lines() {
        let parsed = parse_line(line);
        println!("{}", parsed);
    }
}
