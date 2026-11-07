use anyhow::{bail, Result};
use std::io::{IsTerminal, Write};

/// `[Y/n]` prompt on a tty.
pub(crate) fn confirm(prompt: &str) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        bail!("You must run with --no-confirm if unattended");
    }
    print!("{prompt}");
    std::io::stdout().flush()?;
    answer(std::io::stdin().lock())
}

/// The `[Y/n]` answer on `input`: empty, `y` or `yes` = yes; end of input (Ctrl-D) = no.
fn answer(mut input: impl std::io::BufRead) -> Result<bool> {
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        println!();
        return Ok(false);
    }
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "" | "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ctrl-D (end of input) at a [Y/n] prompt is no, never yes.
    #[test]
    fn integrations_confirm_eof_is_no() {
        assert!(!answer(&b""[..]).unwrap());
        assert!(answer(&b"\n"[..]).unwrap() && answer(&b"Yes\n"[..]).unwrap());
        assert!(!answer(&b"n\n"[..]).unwrap());
    }
}
