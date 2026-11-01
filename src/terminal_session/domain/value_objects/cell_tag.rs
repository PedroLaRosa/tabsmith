/// The tag the wrapper gives the model's cells (an OSC 8 hyperlink on the cursor template, SPEC D3): which cells are
/// prompt and which are the command line of cycle `n`. Never reaches the real terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellTag {
    Prompt,
    Cycle(u64),
}

impl CellTag {
    pub(crate) const PROMPT_URI: &'static str = "tabsmith:p";

    pub(crate) fn uri(self) -> String {
        match self {
            CellTag::Prompt => CellTag::PROMPT_URI.to_string(),
            CellTag::Cycle(n) => format!("tabsmith:c{n}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_encode_the_prompt_tag_and_a_cycle_tag_as_the_uris_the_cells_carry() {
        let uris = (CellTag::Prompt.uri(), CellTag::Cycle(7).uri());

        assert_eq!(
            (uris.0.as_str(), uris.1.as_str(), CellTag::PROMPT_URI),
            ("tabsmith:p", "tabsmith:c7", "tabsmith:p")
        );
    }
}
