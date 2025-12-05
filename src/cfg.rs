use std::collections::{HashMap, HashSet};
use lc3_ensemble::ast::asm::Stmt;

use crate::disasm::{find_next_label, find_next_branch, get_branch_destination};

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub length: u16,
    pub preds: Vec<u16>,
    pub succs: Vec<u16>,
    pub dominators: Vec<u16>,
}

impl BasicBlock {
    pub fn new() -> Self {
        BasicBlock {
            length: 0,
            preds: Vec::new(),
            succs: Vec::new(),
            dominators: Vec::new(),
        }
    }
}

pub struct NaturalLoop {
    pub header: u16,
    pub blocks: HashSet<u16>,
}

impl NaturalLoop {
    pub fn new(header: u16) -> Self {
        let mut blocks = HashSet::new();
        blocks.insert(header);
        NaturalLoop {
            header,
            blocks,
        }
    }
}

fn get_block_at(start_address: u16, block_list: &mut HashMap<u16, BasicBlock>) -> &mut BasicBlock {
    if !block_list.contains_key(&start_address) {
        block_list.insert(start_address, BasicBlock::new());
    }
    block_list.get_mut(&start_address).unwrap()
}

pub fn create_basic_blocks(entry_address: u16, disassembly: &HashMap<u16, Stmt>) -> HashMap<u16, BasicBlock> {
    let mut block_list: HashMap<u16, BasicBlock> = HashMap::new();
    let mut work_list: Vec<u16> = Vec::new();

    get_block_at(entry_address, &mut block_list);
    work_list.push(entry_address);

    while let Some(block_start) = work_list.pop() {
        if block_list.get(&block_start).map_or(false, |b| b.length > 0) {
            continue;
        }

        let next_label = find_next_label(block_start, disassembly);
        let next_branch = find_next_branch(block_start, disassembly);

        let mut successors = Vec::new();
        let block_end_addr;

        match (next_label, next_branch) {
            // Branch and label found
            (Some(label_addr), Some((branch_addr, branch_after, dest_known, is_cond))) => {
                if label_addr < branch_after {
                    block_end_addr = label_addr;
                    successors.push(label_addr);
                } else {
                    block_end_addr = branch_after;
                    if dest_known {
                        if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                            successors.push(dest_addr);
                        }
                    }
                    if is_cond {
                        successors.push(branch_after);
                    }
                }
            },
            // Only branch is found
            (None, Some((branch_addr, branch_after, dest_known, is_cond))) => {
                block_end_addr = branch_after;
                if dest_known {
                    if let Some(dest_addr) = get_branch_destination(branch_addr, disassembly) {
                        successors.push(dest_addr);
                    }
                }
                if is_cond {
                    successors.push(branch_after);
                }
            },
            // Only label found
            (Some(label_addr), None) => {
                block_end_addr = label_addr;
                successors.push(label_addr);
            },
            // Neither found
            (None, None) => {
                let mut last_addr = block_start;
                while disassembly.contains_key(&(last_addr + 1)) {
                    last_addr += 1;
                }
                block_end_addr = last_addr + 1;
            }
        }

        if let Some(block) = block_list.get_mut(&block_start) {
            block.length = block_end_addr - block_start;
        }

        for succ_addr in successors {
            get_block_at(succ_addr, &mut block_list);

            if let Some(block) = block_list.get_mut(&block_start) {
                if !block.succs.contains(&succ_addr) {
                    block.succs.push(succ_addr);
                }
            }

            if let Some(succ_block) = block_list.get_mut(&succ_addr) {
                if !succ_block.preds.contains(&block_start) {
                    succ_block.preds.push(block_start);
                }
            }

            work_list.push(succ_addr);
        }
    }

    block_list
}

pub fn compute_dominators(blocks: &mut HashMap<u16, BasicBlock>, entry_addr: u16) {
    let all_blocks: Vec<u16> = blocks.keys().cloned().collect();

    for &block_addr in &all_blocks {
        let block = blocks.get_mut(&block_addr).unwrap();
        if block_addr == entry_addr {
            block.dominators = vec![entry_addr];
        } else {
            block.dominators = all_blocks.clone();
        }
    }

    let mut changed = true;
    while changed {
        changed = false;

        for &block_addr in &all_blocks {
            if block_addr == entry_addr {
                continue;
            }

            let preds = blocks[&block_addr].preds.clone();
            let mut new_dominators: Option<HashSet<u16>> = None;

            for &pred in &preds {
                let pred_dominators: HashSet<u16> = blocks[&pred].dominators.iter().cloned().collect();

                match new_dominators {
                    None => new_dominators = Some(pred_dominators),
                    Some(ref mut current) => {
                        *current = current.intersection(&pred_dominators).cloned().collect();
                    }
                }
            }

            let mut final_dominators = new_dominators.unwrap_or_else(HashSet::new);
            final_dominators.insert(block_addr);

            let mut final_dominators_vec: Vec<u16> = final_dominators.into_iter().collect();
            final_dominators_vec.sort();

            let current_dominators = &blocks[&block_addr].dominators;
            if current_dominators != &final_dominators_vec {
                changed = true;
                blocks.get_mut(&block_addr).unwrap().dominators = final_dominators_vec;
            }
        }
    }
}

pub fn natural_loop_for_edge(header: u16, tail: u16, blocks: &HashMap<u16, BasicBlock>) -> NaturalLoop {
    let mut work_list: Vec<u16> = Vec::new();
    let mut loop_obj = NaturalLoop::new(header);

    if header != tail {
        loop_obj.blocks.insert(tail);
        work_list.push(tail);
    }

    while let Some(block_addr) = work_list.pop() {
        if let Some(block) = blocks.get(&block_addr) {
            for &pred in &block.preds {
                if !loop_obj.blocks.contains(&pred) {
                    loop_obj.blocks.insert(pred);
                    work_list.push(pred);
                }
            }
        }
    }

    loop_obj
}

pub fn compute_natural_loops(blocks: &HashMap<u16, BasicBlock>, entry_addr: u16) -> Vec<NaturalLoop> {
    let mut loop_set: Vec<NaturalLoop> = Vec::new();

    for (&block_addr, block) in blocks {
        if block_addr == entry_addr {
            continue;
        }

        for &succ in &block.succs {
            // Every successor that dominates its predecessor
            // must be the header of a loop.
            // That is, block -> succ is a back edge.
            if block.dominators.contains(&succ) {
                let natural_loop = natural_loop_for_edge(succ, block_addr, blocks);
                loop_set.push(natural_loop);
            }
        }
    }
    
    loop_set
}
