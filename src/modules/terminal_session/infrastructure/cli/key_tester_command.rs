use crate::modules::shared::infrastructure::terminal_style::{bold, sgr, stdout_tty};
use anyhow::Result;
use std::io::{Read, Write};

fn key_line(input: &[u8]) -> String {
    let hex: Vec<String> = input.iter().map(|b| format!("{b:#04x}")).collect();
    format!(
        "{} bytes : \"{}\" : [{}]",
        input.len(),
        input.escape_ascii(),
        hex.join(", ")
    )
}

/// ponytail: Windows reads through std's console stdin, so keys without VT input mode show as text only.
pub(crate) fn run_key_tester_command() -> Result<i32> {
    let color = stdout_tty();
    println!(
        "{} (use {} to quit)",
        bold("Testing Key Input", color),
        sgr("ctrl-d", "35", color)
    );
    crossterm::terminal::enable_raw_mode()?;
    let res = (|| -> Result<()> {
        let (mut stdin, mut stdout) = (std::io::stdin().lock(), std::io::stdout().lock());
        let mut buf = [0u8; 1024];
        loop {
            let n = stdin.read(&mut buf)?;
            if n == 0 {
                return Ok(());
            }
            write!(stdout, "{}\r\n", key_line(&buf[..n]))?;
            stdout.flush()?;
            if buf[..n] == [4] {
                return Ok(());
            }
        }
    })();
    crossterm::terminal::disable_raw_mode()?;
    res?;
    println!("ctrl-d");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_tester_line() {
        assert_eq!(
            key_line(b"\x1b[A"),
            "3 bytes : \"\\x1b[A\" : [0x1b, 0x5b, 0x41]"
        );
        assert_eq!(key_line(&[4]), "1 bytes : \"\\x04\" : [0x04]");
    }
}
