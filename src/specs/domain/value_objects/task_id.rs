/// Numbers the runtime's tasks, JS exec ops and sleep ops from ONE counter: a finished op is routed by whether its
/// id is a live task, so the three kinds must never share a number.
pub(crate) type TaskId = u64;
