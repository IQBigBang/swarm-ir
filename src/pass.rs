use crate::{instr::Function, module::Module};

pub trait FunctionPass<'ctx> {
    type Error;

    /// Start visiting the module. Called before any [`visit_function`].
    fn visit_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> { Ok(()) }

    /// Visit a function in a module.
    fn visit_function(
        &mut self, 
        module: &Module<'ctx>,
        function: &Function<'ctx>) -> Result<(), Self::Error>;
    
    /// Invoked at the end of the module after all functions.
    fn end_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> { Ok(()) }
}

/// A pass which is meant to mutate (i.e. change) the function
pub trait MutableFunctionPass<'ctx> {
    /// The error type
    type Error;
    /// The internal information used by the Pass
    /// to mutate the function. It is produced by the [`visit_function`] function which has immutable access
    /// to both the function AND the module and consumed by the [`mutate_function`] function, which only
    /// has mutable access to the function.
    type MutationInfo;

    /// Start visiting the module. Called before any [`visit_function`].
    fn visit_module(&mut self, module: &Module<'ctx>) -> Result<(), Self::Error> { Ok(()) }

    /// Visit a function in a module.
    fn visit_function(
        &mut self, 
        module: &Module<'ctx>,
        function: &Function<'ctx>) -> Result<Self::MutationInfo, Self::Error>;
    
    /// Mutate the function. Invoked after every [`visit_function`] call with the information.
    fn mutate_function(
        &mut self,
        function: &mut Function<'ctx>,
        info: Self::MutationInfo) -> Result<(), Self::Error>;
}