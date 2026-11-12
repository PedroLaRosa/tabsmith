use crate::modules::specs::domain::value_objects::process_output::ProcessOutput;
use crate::modules::specs::domain::value_objects::task_id::TaskId;

/// What the spec runtime receives from outside its thread: the output of a process it started, or why the process
/// could not run.
#[derive(Debug)]
pub(crate) struct RuntimeEvent {
    pub(crate) id: TaskId,
    pub(crate) output: Result<ProcessOutput, String>,
}
