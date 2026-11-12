use crate::modules::specs::domain::entities::subcommand::Subcommand;
use crate::modules::specs::domain::value_objects::arg::Arg;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Identity of a generator (hash of arg identity + generator index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratorKey(u64);

impl GeneratorKey {
    /// Generator identity: arg identity + generator index.
    pub(crate) fn for_arg(arg: &Rc<Arg>, index: usize) -> GeneratorKey {
        GeneratorKey(identity_hash(Rc::as_ptr(arg) as usize, index))
    }

    #[cfg(test)]
    pub(crate) fn from_raw(raw: u64) -> GeneratorKey {
        GeneratorKey(raw)
    }
}

/// Identity of (current arg, current subcommand): changes when a new argument starts. Same hasher as
/// [`GeneratorKey`].
pub(crate) fn argument_key(arg: Option<&Rc<Arg>>, subcommand: &Rc<Subcommand>) -> u64 {
    identity_hash(
        arg.map_or(0, |a| Rc::as_ptr(a) as usize),
        Rc::as_ptr(subcommand) as usize,
    )
}

fn identity_hash(a: usize, b: usize) -> u64 {
    let mut h = DefaultHasher::new();
    (a, b).hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_keys_differ_per_index() {
        let a = Rc::new(Arg::default());
        assert_ne!(GeneratorKey::for_arg(&a, 0), GeneratorKey::for_arg(&a, 1));
        assert_eq!(GeneratorKey::for_arg(&a, 0), GeneratorKey::for_arg(&a, 0));
    }
}
