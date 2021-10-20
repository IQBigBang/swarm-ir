use std::{collections::HashMap, ops::Range};

use bit_set::BitSet;

use crate::{instr::{BlockId, Instr}, pass::MutableFunctionPass};

/// An instruction rewrite pass.
///
/// As a parameter it takes in data about which instructions to replace
/// and how and does the heavylifting of modyfying instruction offsets etc.
pub struct InstrRewritePass<'ctx> {
    target_function_idx: usize,
    /// The instruction modifications.
    /// For every block, there's a list of what instruction ranges
    /// to replace and with what.
    ///
    /// The instruction ranges may NOT overlap.
    modifications: HashMap<BlockId, Vec<BlobRewriteData<'ctx>>>
}

/// The Range is a range of indexes of instructions which will be replaced
/// by the instructions in the second field
pub type BlobRewriteData<'ctx> = (Range<usize>, Vec<Instr<'ctx>>);

impl<'ctx> InstrRewritePass<'ctx> {
    /// Create a new Instruction Rewrite Pass.
    ///
    /// Returns Err(()) if the requirements are not met, notably
    /// if the instruction ranges overlap.
    pub fn new(target_function_idx: usize, modifications: HashMap<BlockId, Vec<BlobRewriteData<'ctx>>>) -> Result<Self, ()> {
        // Before constructing an instance,
        // we need to verify the modification ranges DON'T overlap
        for changes in modifications.values() {
            // For every block, we create a bitset
            // and add instruction indices for every range the instruction appears in
            let mut bit_set = BitSet::new();
            for change in changes {
                // Add every index in the range to the set
                // btw: we can clone the range, it's a cheap structure (it has two fields)
                for idx in change.0.clone() { 
                    let is_new = bit_set.insert(idx);
                    // If the value already is in the set, then it's invalid
                    if !is_new { return Err(()) }
                }
            }
        }
        // Now that we guaranteed the ranges don't overlap, we can sort them
        // We sort the ranges in opposite order, e.g. 7..10, 3..6, 1..2
        let mut modifications = modifications;
        for changes in modifications.values_mut() {
            changes.sort_by(|(r1, _), (r2, _)| {
                Ord::cmp(&r1.start, &r2.start).reverse()
            })
        }

        Ok(InstrRewritePass { target_function_idx, modifications })
    }
}

impl<'ctx> MutableFunctionPass<'ctx> for InstrRewritePass<'ctx> {
    type Error = ();

    type MutationInfo = ();

    fn visit_function(
        &mut self, 
        _module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::MutationInfo, Self::Error> {
            /* Here, we only validate that block indexes are not out-of-bounds */
            if function.idx == self.target_function_idx {
                for block_id in self.modifications.keys() {
                    if function.get_block(*block_id).is_none() { return Err(()) }
                }
            }
            Ok(())
        }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        _info: Self::MutationInfo) -> Result<(), Self::Error> {
        
        if function.idx != self.target_function_idx { return Ok(()) }
        
        // For every block
        for (block_id, modifications) in &mut self.modifications {
            let block = function.get_block_mut(*block_id).unwrap(); // unwrapping is safe, we verified in `visit_function`
            // The modifications are guaranteed to be sorted
            // in order from the last range to the first one. (we did that in the constructor)
            //
            // Therefore we can directly mutate the vector of instructions
            // and don't have to do any index calculations, because we always modify
            // at the end and don't affect later ranges
            for (range, new_instrs) in modifications {
                if range.end > block.body.len() { panic!() } // TODO

                // The `splice` operator does exactly what we need:
                // remove the range and replace it with new items
                block.body.splice(
                    range.clone(),
                    // To prevent cloning, we drain the modification data, because it's only used once anyway
                    new_instrs.drain(..));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{builder::{FunctionBuilder, InstrBuilder}, instr::{Instr, InstrK}, module::{Functional, Module, WasmModuleConf}};

    use super::InstrRewritePass;

    #[test]
    pub fn instr_rewrite_pass_test() {
        let mut top = Module::new(WasmModuleConf::default());

        let mut builder = FunctionBuilder::new(
            "func".to_string(),
            [top.int32t()],
            [top.int32t()]
        );
        let arg0 = builder.get_arg(0);
        builder.i_ld_local(arg0);
        builder.i_ld_int(1, top.int32t());
        builder.i_iadd();

        builder.finish(&mut top);

        // Now the function is: LdLocal 0, LdInt 1, IAdd

        let mut rewrite_pass = InstrRewritePass::new(
            top.get_function("func").unwrap().idx(),
            {
                let mut m = HashMap::new();
                m.insert(0.into(), vec![
                    // replace the first two instructions with LdInt 3, LdLocal 0
                    (0..2, vec![
                        Instr::new(InstrK::LdInt(3, top.int32t())),
                        Instr::new(InstrK::LdLocal { idx: 0 })
                    ]),
                    // insert LdInt 4, LdSub after the IAdd instruction
                    (3..3, vec![
                        Instr::new(InstrK::LdInt(4, top.int32t())),
                        Instr::new(InstrK::ISub)
                    ])
                ]);
                m
            }
        ).unwrap();
        
        top.do_mut_pass(&mut rewrite_pass).unwrap();

        let instr_kinds: Vec<InstrK<'_>> = top.get_function("func").unwrap().unwrap_local().entry_block().body.iter().map(|i| i.kind.clone()).collect();
        assert_eq!(instr_kinds, vec![
            InstrK::LdInt(3, top.int32t()),
            InstrK::LdLocal { idx: 0 },
            InstrK::IAdd,
            InstrK::LdInt(4, top.int32t()),
            InstrK::ISub,
        ]);
    }
}