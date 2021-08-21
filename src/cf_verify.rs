//! Control flow verification
//!
//! Ensures that the control flow is regular
//! and obeys some basic principles for easy analysis
//! and compilation (to wasm).

use std::collections::HashMap;

use crate::{instr::{BlockId, Instr, InstrK}, pass::{MutableFunctionPass}};

pub struct ControlFlowVerifier {}

impl<'ctx> MutableFunctionPass<'ctx> for ControlFlowVerifier {
    type Error = ControlFlowVerifierError;
    // the parent blocks for every block
    type MutationInfo = HashMap<BlockId, BlockId>;


    fn visit_function(
        &mut self, 
        _module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::MutationInfo, Self::Error> {
        
        // For every block, save its parent (where it appears)
        let mut block_parents: HashMap<BlockId, BlockId> = HashMap::new();
        for block in function.blocks_iter() {
            let this_block = block.idx;

            for instr in &block.body {
                #[allow(clippy::single_match)]
                match instr.kind {
                   InstrK::IfElse { then, r#else } => {
                        // If the block is in `block_parents`
                        // that means it was already referenced from another block (it already has a parent block)
                       if block_parents.contains_key(&then) {
                           return Err(ControlFlowVerifierError::MultipleParents {
                               block: then,
                               parent: block_parents[&then], // the original parent
                               other_parent: this_block // the current block
                           })
                       }
                       block_parents.insert(then, this_block);

                       if let Some(else_block) = r#else {
                            if block_parents.contains_key(&else_block) {
                                return Err(ControlFlowVerifierError::MultipleParents {
                                    block: else_block,
                                    parent: block_parents[&else_block], // the original parent
                                    other_parent: this_block // the current block
                                })
                            }
                            block_parents.insert(else_block, this_block);
                        }
                   }
                   _ => {} // ignore other instructions
                }
            }
        }

        // The main block can't have any parent
        if block_parents.contains_key(&0.into()) {
            return Err(ControlFlowVerifierError::MultipleParents {
                block: 0.into(),
                parent: 0.into(), // for the purposes of error reporting, we can pretend the main block is its own parent
                other_parent: block_parents[&0.into()]
            })
        }

        Ok(block_parents)
    }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        info: Self::MutationInfo) -> Result<(), Self::Error> {
        
        let block_parents = info;
        
        for block in function.blocks_iter_mut() {
            if block_parents.contains_key(&block.idx) {
                block.meta.insert("parent", block_parents[&block.idx]);
            
                // also add the "parent" information to the `end` instruction
                assert!(matches!(block.body.last(), Some(Instr { kind: InstrK::End, meta: _ })));
                block.body.last_mut().unwrap().meta.insert("parent", block_parents[&block.idx]);
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ControlFlowVerifierError {
    MultipleParents { block: BlockId, parent: BlockId, other_parent: BlockId }
}