//! Small user-facing commands: diagnostic, debug (SPEC §2.10).
//! OWNER: cli agent.
use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
use crate::shared::infrastructure::output_format::OutputFormat;
use anyhow::Result;
use clap::Args;
use serde_json::{json, Value};

#[derive(Args)]
pub struct DiagnosticArgs {
    #[arg(long, short, value_enum, default_value_t)]
    pub format: OutputFormat,
}

// ---------------------------------------------------------------------------------------------------------------
// diagnostic (SPEC §2.10)

type Section = (&'static str, Vec<(String, Value)>);

pub fn diagnostic(args: DiagnosticArgs) -> Result<i32> {
    let sections = diagnostics();
    println!(
        "{}",
        args.format.render(
            || to_toml(&sections),
            || {
                Value::Object(
                    sections
                        .iter()
                        .map(|(n, rows)| {
                            (n.to_string(), Value::Object(rows.iter().cloned().collect()))
                        })
                        .collect(),
                )
            },
        )
    );
    Ok(0)
}

fn diagnostics() -> Vec<Section> {
    let user = std::env::var(if cfg!(windows) { "USERNAME" } else { "USER" }).unwrap_or_default();
    let clean = |s: String| {
        if user.is_empty() {
            s
        } else {
            s.replace(&format!("/{user}"), "/USER")
                .replace(&format!("\\{user}"), "\\USER")
        }
    };
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let opt = |v: Option<String>| v.map(|s| Value::String(clean(s))).unwrap_or(Value::Null);

    // ponytail: shell = $SHELL (not the parent process); unset on Windows → omitted.
    let shell = env("SHELL");
    let shell_version = shell
        .as_ref()
        .and_then(|sh| {
            std::process::Command::new(sh)
                .arg("--version")
                .output()
                .ok()
        })
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|l| l.trim().to_string())
        });
    let terminal = env("TERM_PROGRAM")
        .or_else(|| env("WT_SESSION").map(|_| "Windows Terminal".into()))
        .or_else(|| env("TERM"));
    let in_ssh = ["SSH_CLIENT", "SSH_CONNECTION", "SSH_TTY"]
        .iter()
        .any(|k| env(k).is_some());
    let in_wsl = cfg!(target_os = "linux")
        && (env("WSL_DISTRO_NAME").is_some()
            || std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|s| s.to_lowercase().contains("microsoft")));

    let mut env_vars: Vec<(String, Value)> = std::env::vars_os()
        .filter_map(|(k, v)| {
            let k = k.into_string().ok()?;
            let keep = k.starts_with("TABSMITH_")
                || ["SHELL", "TERM", "COLORTERM", "ZDOTDIR", "PATH"].contains(&k.as_str());
            keep.then(|| (k, Value::String(clean(v.to_string_lossy().into_owned()))))
        })
        .collect();
    env_vars.sort_by(|a, b| a.0.cmp(&b.0));

    let row = |k: &str, v: Value| (k.to_string(), v);
    vec![
        (
            "tabsmith",
            vec![
                row("version", json!(env!("CARGO_PKG_VERSION"))),
                row(
                    "os",
                    json!(format!(
                        "{} {}",
                        std::env::consts::OS,
                        std::env::consts::ARCH
                    )),
                ),
            ],
        ),
        (
            "environment",
            vec![
                row(
                    "cwd",
                    opt(std::env::current_dir()
                        .ok()
                        .map(|p| p.display().to_string())),
                ),
                row(
                    "cli-path",
                    opt(std::env::current_exe()
                        .ok()
                        .map(|p| p.display().to_string())),
                ),
                row("shell-path", opt(shell)),
                row("shell-version", opt(shell_version)),
                row("terminal", opt(terminal)),
                row("in-ssh", json!(in_ssh)),
                row("in-ci", json!(env("CI").is_some())),
                row("in-wsl", json!(in_wsl)),
            ],
        ),
        ("env-vars", env_vars),
    ]
}

/// Hand-written TOML: JSON scalars are valid TOML values; nulls are omitted.
fn to_toml(sections: &[Section]) -> String {
    let mut out = String::new();
    for (name, rows) in sections {
        out += &format!("[{name}]\n");
        for (k, v) in rows.iter().filter(|(_, v)| !v.is_null()) {
            let bare = k
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            let key = if bare {
                k.clone()
            } else {
                Value::String(k.clone()).to_string()
            };
            out += &format!("{key} = {v}\n");
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------------------------------------------
// debug (SPEC §2.10)

pub fn debug_logs() -> Result<i32> {
    let dir = data_directory_from_env().logs_dir()?;
    let pty = dir.join("pty.log");
    println!("{}\n{}", dir.join("cli.log").display(), pty.display());
    if let Ok(bytes) = std::fs::read(&pty) {
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        println!();
        lines[lines.len().saturating_sub(50)..]
            .iter()
            .for_each(|l| println!("{l}"));
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_toml() {
        let sections: Vec<Section> = vec![
            (
                "tabsmith",
                vec![
                    ("version".into(), json!("0.1.0")),
                    ("missing".into(), Value::Null),
                ],
            ),
            (
                "env-vars",
                vec![
                    ("PATH".into(), json!("/a:\"b\"")),
                    ("in-ci".into(), json!(false)),
                ],
            ),
        ];
        assert_eq!(
            to_toml(&sections),
            "[tabsmith]\nversion = \"0.1.0\"\n\n[env-vars]\nPATH = \"/a:\\\"b\\\"\"\nin-ci = false"
        );
    }
}
