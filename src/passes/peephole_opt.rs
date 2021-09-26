use std::collections::HashMap;

use crate::{abi::{Abi, Wasm32Abi}, instr::{BlockId, Instr, InstrK}, intrinsic::Intrinsics, pass::{FunctionPass}, ty::{Ty, Type}};

use super::BlobRewriteData;


/// Replace two consecutive instructions with something new
fn replace_2<'ctx>(i1: &Instr<'ctx>, i2: &Instr<'ctx>) -> Option<Vec<Instr<'ctx>>> {
    match (&i1.kind, &i2.kind) {
        _ => None
    }
}

pub struct PeepholeOpt {}

impl<'ctx> FunctionPass<'ctx> for PeepholeOpt {
    type Error = ();
    // Returns a type suitable for InstrRewritePass
    type Output = HashMap<BlockId, Vec<BlobRewriteData<'ctx>>>;

    fn visit_function(
        &mut self, 
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::Output, Self::Error> {
        
        let mut rewrite_data = HashMap::new();

        for block in function.blocks_iter() {
            let mut this_block_replacements: Vec<BlobRewriteData<'ctx>> = Vec::new();

            for i in 0..block.body.len() {
                // Check if there's 2 consecutive instructions left
                if (i + 1) < block.body.len() {
                    if let Some(new_instrs) = replace_2(&block.body[i], &block.body[i+1]) {
                        let range = i..(i + 2);
                        this_block_replacements.push((range, new_instrs));
                    }
                }
                // TODO: replace 3 instructions, 4 instructions etc.
            }

            if !this_block_replacements.is_empty() {
                rewrite_data.insert(block.idx, this_block_replacements);
            }
        }

        Ok(rewrite_data)
    }
}