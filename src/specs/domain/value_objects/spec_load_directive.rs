use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::value_objects::spec_function_ref::SpecFunctionRef;
use crate::specs::domain::value_objects::spec_location::SpecLocation;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) enum SpecLoadDirective {
    Locations(Vec<SpecLocation>),
    Inline(Rc<Subcommand>),
    /// `loadSpec(token, executeShellCommand)`.
    Fn(SpecFunctionRef),
}
