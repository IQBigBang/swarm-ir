use crate::{instr::{Instr, InstrK}, pass::MutableFunctionPass};

/// A simple correction pass.
///
/// Can correct validity mistakes which don't affect code semantics
/// or which don't have any other alternative.
///
/// Specifically, this means:
/// * adding the `end` instruction at the end of a block if there isn't one
pub struct CorrectionPass {}

impl<'ctx> MutableFunctionPass<'ctx> for CorrectionPass {
    type Error = ();

    type MutationInfo = CorrectionPassMutationInfo;

    fn visit_function(
        &mut self, 
        module: &crate::module::Module<'ctx>,
        function: &crate::instr::Function<'ctx>) -> Result<Self::MutationInfo, Self::Error> {

        /* No analysis of the whole module is required yet */
        Ok(CorrectionPassMutationInfo {})
    }

    fn mutate_function(
        &mut self,
        function: &mut crate::instr::Function<'ctx>,
        info: Self::MutationInfo) -> Result<(), Self::Error> {
        
        for block in function.blocks_iter_mut() {
            if let Some(Instr {
                kind: InstrK::End,
                meta: _
            }) = block.body.last() {/* Everything's OK */}
            else {
                block.add(InstrK::End)
            } 
        }

        Ok(())
    }
}

pub struct CorrectionPassMutationInfo {}