use crate::dropdown::domain::value_objects::key::Key;
use std::ops::Range;

/// A decoded key and the byte range of the chunk it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    pub(crate) key: Key,
    pub(crate) bytes: Range<usize>,
}
