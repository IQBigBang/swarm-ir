//! Control flow verification
//!
//! Ensures that the control flow is regular
//! and obeys some basic principles for easy analysis
//! and compilation (to wasm): namely that every block
//! is used in exactly one place and has a single parent,
//! i.e. there's no goto-ish jumps
//!
//! It also checks that the [`BlockTag`]s are correct
//! 
//! Calculates the innermost-loop-distances as described
//! in the Control Flow part 2 proposal

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
    type MutationInfo = ControlFlowVerifierData;


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
                   InstrK::Loop(child) => {
                        self.assert_parent(&mut block_parents, child, this_block)?;
                        self.assert_tag(BlockTag::Loop, child, function)?;
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

        // Now that we know the parents, calculate the innermost loop distances
        let mut innermost_loop_distances: HashMap<BlockId, usize> = HashMap::new();
        for block in function.blocks_iter() {
            match block.tag() {
                BlockTag::Undefined | BlockTag::Main => continue,
                BlockTag::Loop => {
                    // For `Loop`, the innermost_loop_distance is zero
                    innermost_loop_distances.insert(block.idx, 0usize);
                },
                BlockTag::IfElse => {
                    // For `IfElse`, search though the parents until we find a `Loop` block
                    let mut innermost_loop_distance: isize = 1;
                    let mut current_block = block.idx;
                    loop {
                        let parent = block_parents[&current_block];
                        match function.get_block(parent).unwrap().tag {
                            BlockTag::Undefined | BlockTag::Main => {
                                // The IfElse block is not a part of any kind of loop
                                // because none of its parents is a loop
                                innermost_loop_distance = -1;
                                break
                            },
                            BlockTag::IfElse => {
                                innermost_loop_distance += 1;
                            },
                            // We found the nearest loop
                            BlockTag::Loop => break,
                        }
                        current_block = parent;
                    }
                    if innermost_loop_distance > 0 {
                        innermost_loop_distances.insert(block.idx, innermost_loop_distance as usize);
                    }
                }
            }
        }

        Ok(ControlFlowVerifierData { block_parents, innermost_loop_distances } )
    }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        info: Self::MutationInfo) -> Result<(), Self::Error> {
                
        for block in function.blocks_iter_mut() {
            if info.block_parents.contains_key(&block.idx) {
                block.meta.insert(key!("parent"), info.block_parents[&block.idx]);
            }

            if info.innermost_loop_distances.contains_key(&block.idx) {
                block.meta.insert(
                    key!("innermost_loop_distance"), 
                    info.innermost_loop_distances[&block.idx]);
            } 
        }

        Ok(())
    }
}

pub struct ControlFlowVerifierData {
    block_parents: HashMap<BlockId, BlockId>,
    innermost_loop_distances: HashMap<BlockId, usize>,
}

#[derive(Debug)]
pub enum ControlFlowVerifierError {
    MultipleParents { block: BlockId, parent: BlockId, other_parent: BlockId },
    InvalidBlockTag { block: BlockId, expected: BlockTag, actual: BlockTag }
}