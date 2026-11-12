use std::collections::BTreeMap;

/// A JSON value written by spec code, held exactly as read until an adapter interprets it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecValue {
    Null,
    Bool(bool),
    Unsigned(u64),
    Negative(i64),
    Float(f64),
    Text(String),
    List(Vec<SpecValue>),
    Object(BTreeMap<String, SpecValue>),
}
