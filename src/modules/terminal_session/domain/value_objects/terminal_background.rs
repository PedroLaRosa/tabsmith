use crate::modules::dropdown::domain::value_objects::rgb::Rgb;

/// The background colour the terminal reported for OSC 11 (the `system` theme picks light or dark from it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TerminalBackground(Rgb);

impl TerminalBackground {
    pub(crate) fn new(rgb: Rgb) -> Self {
        TerminalBackground(rgb)
    }

    pub(crate) fn rgb(self) -> Rgb {
        self.0
    }
}
