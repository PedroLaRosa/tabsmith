use crate::ghost_text::application::ports::source_chooser::SourceChooser;
use crate::ghost_text::domain::ghost_text_error::GhostTextError;
use crate::ghost_text::domain::value_objects::ghost_text_source::BUILTIN_SOURCES;
use crate::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use std::io::{IsTerminal, Write};

/// Asks on the terminal: the numbered rows of the sources, then the answer read from stdin.
pub(crate) struct StdinSourceChooser;

impl SourceChooser for StdinSourceChooser {
    /// Numbered prompt; `Ok(None)` = "None" picked. Non-tty or bad input → error.
    fn choose(&self, profiles: &[(String, LlmProfile)]) -> Result<Option<String>, GhostTextError> {
        if !std::io::stdin().is_terminal() {
            return Err(GhostTextError::NoCustomizationSelected);
        }
        let mut rows: Vec<(String, String)> = BUILTIN_SOURCES
            .iter()
            .map(|(n, d)| (n.to_string(), d.to_string()))
            .collect();
        rows.extend(profiles.iter().map(|(n, p)| {
            (
                n.clone(),
                format!("{} {}", p.provider().name(), p.model()),
            )
        }));
        for (i, (n, d)) in rows.iter().enumerate() {
            println!("{}) {n} - {d}", i + 1);
        }
        println!("{}) None", rows.len() + 1);
        print!("Select a customization: ");
        std::io::stdout()
            .flush()
            .map_err(GhostTextError::SourceChooser)?;
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(GhostTextError::SourceChooser)?;
        match input.trim().parse::<usize>() {
            Ok(i) if (1..=rows.len()).contains(&i) => Ok(Some(rows[i - 1].0.clone())),
            Ok(i) if i == rows.len() + 1 => Ok(None),
            _ => Err(GhostTextError::NoCustomizationSelected),
        }
    }
}
