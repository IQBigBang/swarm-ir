use crate::{pass::MutableFunctionPass};

/// A simple correction pass.
///
/// Can correct validity mistakes which don't affect code semantics
/// or which don't have any other alternative.
///
/// Specifically, this means:
// TODO: unreachability (?), maybe some other things
pub struct CorrectionPass {}

impl<'ctx> MutableFunctionPass<'ctx> for CorrectionPass {
    type Error = ();

    type MutationInfo = CorrectionPassMutationInfo;

    fn visit_function(
        &mut self, 
        _module: &crate::module::Module<'ctx>,
        _function: &crate::instr::Function<'ctx>) -> Result<Self::MutationInfo, Self::Error> {

        /* No analysis of the whole module is required yet */
        Ok(CorrectionPassMutationInfo {})
    }

    fn mutate_function(
        &mut self,
        _function: &mut crate::instr::Function<'ctx>,
        _info: Self::MutationInfo) -> Result<(), Self::Error> {

        Ok(())
    }
}

pub struct CorrectionPassMutationInfo {}