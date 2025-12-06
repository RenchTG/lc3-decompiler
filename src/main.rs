mod disasm;
mod cfg;
mod liveness;
mod ir;
mod control_flow;
mod linear;
mod codegen;
mod stack;

use std::fs;

use disasm::{gen_sections, disassemble, identify_code_data};
use cfg::{create_basic_blocks, compute_dominators, compute_natural_loops};
use control_flow::{identify_loops, identify_conditionals, structure_loops, structure_conditionals, refine_loops, ConditionalType};
use liveness::{compute_local_liveness, propagate_global_liveness, compute_final_liveness};
use ir::{propagate_expressions, collapse_expressions, apply_goto_transformation, propagate_variables}; 
use linear::{linearize};
use codegen::generate_code;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <path to obj file> [-d|--debug] [-e|--entry <addr>] [-c|--convention]", args[0]);
        std::process::exit(1);
    }
    
    let mut debug_mode = false;
    let mut use_stack_convention = false;
    let mut entry_request: Option<String> = None;    
    
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--debug" => {
                debug_mode = true;
                i += 1;
            },
            "-c" | "--convention" => {
                use_stack_convention = true;
                i += 1;
            },
            "-e" | "--entry" => {
                if i + 1 < args.len() {
                    entry_request = Some(args[i+1].clone());
                    i += 2;
                } else {
                    eprintln!("Missing address for --entry");
                    std::process::exit(1);
                }
            },
            _ => {
                i += 1;
            }
        }
    }

    let input_obj = fs::read_to_string(args[1].clone()).expect("Failed to read file");
    assert!(input_obj.starts_with("LC-3 OBJ FILE"));

    let (text, symbols) = gen_sections(&input_obj);
    let (disassembly, origs) = disassemble(text, symbols.clone());

    // Step 1. Disassembly
    if debug_mode {
        println!("Disassembly:");
        let mut sorted_entries: Vec<_> = disassembly.clone().into_iter().collect();
        sorted_entries.sort_by_key(|&(key, _)| key);
        for (addr, stmt) in sorted_entries {
            println!("{:04X}: {}", addr, stmt);
        }
        println!();
    }

    let function_list = identify_code_data(&disassembly, origs);

    if debug_mode {
        println!("Identified functions:");
        for addr in &function_list {
            println!("{:04X}", addr);
        }
        println!();
    }
    
    // Process the requested entry point
    let entry_addr = if let Some(req) = entry_request {
         let clean = req.trim_start_matches("0x").trim_start_matches("x");
         if let Ok(addr) = u16::from_str_radix(clean, 16) {
             addr
         } else {
             let mut found_addr = None;
             for (addr, name) in &symbols {
                 if name == &req {
                     found_addr = Some(*addr);
                     break;
                 }
             }
             
             if let Some(addr) = found_addr {
                 addr
             } else {
                 eprintln!("Error: Could not resolve entry point '{}' as address or label.", req);
                 std::process::exit(1);
             }
         }
    } else {
        0x3000
    };

    {
        println!("-- Decompiling function at {:04X} --", entry_addr);
        println!();
        
        // Step 2. Split to basic blocks
        let mut blocks = create_basic_blocks(entry_addr, &disassembly);

        // Step 3. Build control flow graph
        compute_dominators(&mut blocks, entry_addr);

        if debug_mode {
            println!("Basic blocks:");
            let mut sorted_blocks: Vec<_> = blocks.iter().collect();
            sorted_blocks.sort_by_key(|&(addr, _)| addr);
            for (addr, block) in sorted_blocks {
                let preds: Vec<String> = block.preds.iter().map(|&p| format!("{:04X}", p)).collect();
                let succs: Vec<String> = block.succs.iter().map(|&s| format!("{:04X}", s)).collect();
                let dominators: Vec<String> = block.dominators.iter().map(|&d| format!("{:04X}", d)).collect();
                println!("Block at {:04X}: length = {}, preds = [{}], succs = [{}], dominators = [{}]",
                         addr, block.length, preds.join(", "), succs.join(", "), dominators.join(", "));
            }
            println!();
        }

        // Step 4. Control flow identification
        let mut natural_loops = compute_natural_loops(&blocks, entry_addr);

        if debug_mode {
            println!("Natural loops:");
            for (id, l) in natural_loops.iter().enumerate() {
                let block_strs: Vec<String> = l.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
                println!("Loop {}: header = {:04X}, blocks = [{}]", id + 1, l.header, block_strs.join(", "));
            }
            println!();
        }

        let identified_loops = identify_loops(&mut natural_loops, &blocks);
        let identified_conditionals = identify_conditionals(&blocks);

        if debug_mode {
            println!("Identified Loop Structures:");
            for (i, loop_info) in identified_loops.iter().enumerate() {
                let loop_blocks: Vec<String> = loop_info.blocks.iter().map(|&b| format!("{:04X}", b)).collect();
                println!("Loop {}: header = {:04X}, blocks = [{}], break_block = {:?}, continue_block = {:?}",
                         i + 1, loop_info.header, loop_blocks.join(", "), 
                         loop_info.break_block.map(|b| format!("{:04X}", b)), 
                         loop_info.continue_block.map(|b| format!("{:04X}", b)));
            }
            println!();

            println!("Identified Conditional Structures:");
            for (i, cond) in identified_conditionals.iter().enumerate() {
                match cond.kind {
                    ConditionalType::If => {
                        println!("Conditional {} (If): condition_block = {:04X}, true_branch = {:04X}, join_block = {:04X}",
                                 i + 1, cond.condition_block, cond.true_block, cond.join_block);
                    }
                    ConditionalType::IfElse => {
                        println!("Conditional {} (If-Else): condition_block = {:04X}, true_branch = {:04X}, false_branch = {:04X}, join_block = {:04X}",
                                 i + 1, cond.condition_block, cond.true_block, cond.false_block.unwrap(), cond.join_block);
                    }
                }
            }
            println!();
        }

        // Step 5. Data Flow Analysis
        let (mut instr_liveness, mut block_liveness) = compute_local_liveness(&blocks, &disassembly);
        propagate_global_liveness(&blocks, &mut block_liveness);
        compute_final_liveness(&blocks, &block_liveness, &mut instr_liveness);

        if debug_mode {
            println!("Liveness Analysis:");
            let mut sorted_instrs: Vec<_> = instr_liveness.iter().collect();
            sorted_instrs.sort_by_key(|&(addr, _)| addr);
            for (addr, info) in sorted_instrs {
                println!("{:04X}: defs={:02X}, uses={:02X}, in={:02X}, out={:02X}", 
                         addr, info.defs, info.uses, info.live_in, info.live_out);
            }
            println!();
        }
        
        // Step 6. Expression Propagation
        let lifted_blocks = propagate_expressions(&blocks, &disassembly, &instr_liveness);
        if debug_mode {
            println!("Expressions:");
            let mut sorted_lifted: Vec<_> = lifted_blocks.iter().collect();
            sorted_lifted.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_lifted {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 7. Expression Collapsing
        let collapsed_blocks = collapse_expressions(lifted_blocks);
        let mut goto_blocks = apply_goto_transformation(collapsed_blocks, &instr_liveness);
        
        if debug_mode {
            println!("Collapsed Expressions:");
            let mut sorted_gotos: Vec<_> = goto_blocks.iter().collect();
            sorted_gotos.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_gotos {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 8. Control Flow Structuring
        
        // Merge consecutive conditional gotos to the same target into compound conditions
        // This must run BEFORE structure_loops so blocks are still available at top level
        // We pass natural_loops to skip loop headers (they're handled by While loop merging)
        control_flow::merge_compound_conditions(&mut goto_blocks, &blocks, &natural_loops);
        
        structure_loops(&mut goto_blocks, &identified_loops, &blocks, &identified_conditionals);
        structure_conditionals(&mut goto_blocks, &identified_conditionals);

        if use_stack_convention {
            stack::process_callee(&mut goto_blocks, entry_addr);
            stack::process_caller(&mut goto_blocks);
        }

        // Propagate single-use variables after stack convention processing
        propagate_variables(&mut goto_blocks);

        refine_loops(&mut goto_blocks, &identified_loops, &blocks, &identified_conditionals, &instr_liveness);

        // Clean up redundant control flow (e.g., trailing continues in if branches)
        control_flow::cleanup_control_flow(&mut goto_blocks);

        if debug_mode {
            println!("Structured Control Flow:");
            let mut sorted_structured: Vec<_> = goto_blocks.iter().collect();
            sorted_structured.sort_by_key(|&(addr, _)| addr);
            for (addr, stmts) in sorted_structured {
                println!("Block {:04X}:", addr);
                for stmt in stmts {
                    println!("  {:04X}: {:?}", stmt.addr, stmt.kind);
                }
            }
            println!();
        }

        // Step 9. Linearization
        if debug_mode {
            println!("Linearized Code:");
        }
        let linear_code = linearize(&goto_blocks);
        
        // Step 9b. Structure if-goto patterns into proper If-Else statements
        let structured_code = linear::structure_if_goto_patterns(linear_code.clone());
        
        // Step 9c. Remove orphaned labels (labels with no gotos pointing to them)
        let final_code = linear::remove_orphaned_labels(structured_code);
        
        if debug_mode {
            for stmt in &linear_code {
                println!("{:?}", stmt);
            }
            println!();
        }

        // Step 10. Code Output and Symbol Resolution
        if debug_mode {
            println!("Generated C-Like Code:");
        }
        let code = generate_code(&final_code, &symbols);
        println!("{}", code);
    }
}
