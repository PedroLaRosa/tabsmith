use crate::modules::dropdown::domain::value_objects::theme_choice::ThemeChoice;
use crate::modules::dropdown::domain::value_objects::theme_definition::ThemeDefinition;

/// What `theme NAME` switches to: a custom theme file, whose author the command credits, or a built-in theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ThemeSelection {
    Custom(ThemeDefinition),
    BuiltIn(ThemeChoice),
}
