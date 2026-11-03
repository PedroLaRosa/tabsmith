use crate::completion::domain::value_objects::generator_outcome::GeneratorOutcome;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::spec_function_call::SpecFunctionCall;

/// What a generator run does next, as one of its step transitions decides.
#[derive(Debug)]
pub(crate) enum GeneratorCommand {
    /// Start this spec function call in the run's generator context.
    StartCall(SpecFunctionCall, GeneratorContext),
    /// Start this process in the run's generator context.
    StartExec(ExecRequest, GeneratorContext),
    /// Start a sleep of this many milliseconds (the debounce).
    StartSleep(u64),
    /// Settle the run with this outcome, starting nothing.
    Settle(GeneratorOutcome),
}
