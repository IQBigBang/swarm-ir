use crate::{instr::{InstrK}, pass::MutableFunctionPass};

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

        Ok(())
    }
}

pub struct CorrectionPassMutationInfo {}