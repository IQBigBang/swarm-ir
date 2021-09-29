//! Control flow verification
//!
//! Ensures that the control flow is regular
//! and obeys some basic principles for easy analysis
//! and compilation (to wasm): namely that every block
//! is used in exactly one place and has a single parent,
//! i.e. there's no goto-ish jumps
//!
//! It also checks that the [`BlockTag`]s are correct

use std::collections::HashMap;

use crate::{instr::{BlockId, BlockTag, Function, InstrK}, pass::{MutableFunctionPass}};

pub struct ControlFlowVerifier {}

impl ControlFlowVerifier {
    /// A helper function.
    /// Check if the block already has a parent and fail if it does,
    /// otherwise add it to the `block_parents` map.
    fn assert_parent(&self, block_parents: &mut HashMap<BlockId, BlockId>, this: BlockId, parent: BlockId) -> Result<(), ControlFlowVerifierError> {
        // If the block is in `block_parents`
        // that means it was already referenced from another block (it already has a parent block)
        // which means the IR is ill-formed
        #[allow(clippy::map_entry)]
        if block_parents.contains_key(&this) {
            Err(ControlFlowVerifierError::MultipleParents {
                block: this,
                parent: block_parents[&this], // the original parent
                other_parent: parent // the current block
            })
        } else {
            block_parents.insert(this, parent);
            Ok(())
        }
    }

    /// A helper function.
    /// Ensure that the block has the correct tag
    fn assert_tag(&self, expected_tag: BlockTag, id: BlockId, function: &Function) -> Result<(), ControlFlowVerifierError> {
        if function.get_block(id).unwrap().tag() != expected_tag {
            Err(ControlFlowVerifierError::InvalidBlockTag {
                block: id,
                expected: expected_tag,
                actual: function.get_block(id).unwrap().tag()
            })
        } else { Ok(()) }
    }
}

impl<'ctx> MutableFunctionPass<'ctx> for ControlFlowVerifier {
    type Error = ControlFlowVerifierError;
    // the parent blocks for every block
    type MutationInfo = HashMap<BlockId, BlockId>;


    fn visit_function(
        &mut self, 
        _module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::MutationInfo, Self::Error> {
        
        // Make sure the main block has the Main tag
        if function.entry_block().tag() != BlockTag::Main {
            return Err(ControlFlowVerifierError::InvalidBlockTag {
                block: function.entry_block().idx,
                expected: BlockTag::Main,
                actual: function.entry_block().tag()
            })
        }
        
        // For every block, save its parent (where it appears)
        let mut block_parents: HashMap<BlockId, BlockId> = HashMap::new();
        for block in function.blocks_iter() {
            let this_block = block.idx;

            for instr in &block.body {
                #[allow(clippy::single_match)]
                match instr.kind {
                   InstrK::IfElse { then, r#else } => {
                        self.assert_parent(&mut block_parents, then, this_block)?;
                        self.assert_tag(BlockTag::IfElse, then, function)?;

                       if let Some(else_block) = r#else {
                           self.assert_parent(&mut block_parents, else_block, this_block)?;
                           self.assert_tag(BlockTag::IfElse, else_block, function)?;
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
                block.meta.insert(key!("parent"), block_parents[&block.idx]);
            
                // also add the "parent" information to the `end` instruction
                match block.body.last().unwrap().kind {
                    InstrK::End => {
                        block.body.last_mut().unwrap().meta.insert(key!("parent"), block_parents[&block.idx]);
                    }
                    // don't add the parent information to 'return'
                    InstrK::Return => {}
                    _ => panic!()
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub enum ControlFlowVerifierError {
    MultipleParents { block: BlockId, parent: BlockId, other_parent: BlockId },
    InvalidBlockTag { block: BlockId, expected: BlockTag, actual: BlockTag }
}