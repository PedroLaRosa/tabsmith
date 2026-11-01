use crate::shared::domain::value_objects::shell::Shell;
use crate::terminal_session::domain::value_objects::pwsh_prediction_color::PwshPredictionColor;
use crate::terminal_session::domain::value_objects::shell_suggestion_color::ShellSuggestionColor;

/// What the tracker knows that reading the edit buffer off the screen needs (SPEC §5.6): the open cycle whose tag
/// marks the command cells, and the shell's own details.
pub(crate) struct EditBufferContext<'a> {
    parts: EditBufferContextParts<'a>,
}

/// Named fields for `EditBufferContext::new`.
pub(crate) struct EditBufferContextParts<'a> {
    pub(crate) cycle: u64,
    pub(crate) shell: Option<Shell>,
    pub(crate) line_col0: bool,
    pub(crate) pwsh_list_view: bool,
    pub(crate) pwsh_continuation: &'a str,
    pub(crate) pwsh_prompt_text: &'a [String],
    pub(crate) zsh_color: Option<&'a ShellSuggestionColor>,
    pub(crate) fish_color: Option<&'a ShellSuggestionColor>,
    pub(crate) pwsh_color: Option<&'a PwshPredictionColor>,
}

impl<'a> EditBufferContext<'a> {
    pub(crate) fn new(parts: EditBufferContextParts<'a>) -> Self {
        EditBufferContext { parts }
    }

    /// The open command cycle `n`: its cells carry `CellTag::Cycle(n)`.
    pub(crate) fn cycle(&self) -> u64 {
        self.parts.cycle
    }

    pub(crate) fn shell(&self) -> Option<Shell> {
        self.parts.shell
    }

    /// The last NewCmd came at column 0 (a prompt ending in a newline): its line has no prompt cell.
    pub(crate) fn line_starts_at_column_zero(&self) -> bool {
        self.parts.line_col0
    }

    /// PSReadLine ListView predictions own the rows below the prompt.
    pub(crate) fn pwsh_list_view(&self) -> bool {
        self.parts.pwsh_list_view
    }

    pub(crate) fn pwsh_continuation(&self) -> &str {
        self.parts.pwsh_continuation
    }

    pub(crate) fn pwsh_prompt_text(&self) -> &[String] {
        self.parts.pwsh_prompt_text
    }

    pub(crate) fn zsh_color(&self) -> Option<&ShellSuggestionColor> {
        self.parts.zsh_color
    }

    pub(crate) fn fish_color(&self) -> Option<&ShellSuggestionColor> {
        self.parts.fish_color
    }

    pub(crate) fn pwsh_color(&self) -> Option<&PwshPredictionColor> {
        self.parts.pwsh_color
    }
}
