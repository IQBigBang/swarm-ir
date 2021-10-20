use crate::{instr::InstrK, pass::MutableFunctionPass};

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
        function: &mut crate::instr::Function<'ctx>,
        _info: Self::MutationInfo) -> Result<(), Self::Error> {

        // Remove instructions after `Fail`
        for block in function.blocks_iter_mut() {
            let mut fail_instr_pos = None;
            for (n, i) in block.body.iter().enumerate() {
                if let InstrK::Fail = i.kind {
                    fail_instr_pos = Some(n);
                    break;
                }
            }

            if let Some(x) = fail_instr_pos {
                // Remove all instructions after the Fail
                std::mem::drop(block.body.drain((x + 1)..));
            }
        }
        Ok(())
    }
}

pub struct CorrectionPassMutationInfo {}