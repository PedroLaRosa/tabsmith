//! `tabsmith inline`: its clap subcommands, the handler and the presenter of the sources (SPEC §2.5).
use crate::modules::ghost_text::domain::value_objects::ghost_text_source::BUILTIN_SOURCES;
use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfile;
use crate::modules::ghost_text::infrastructure::inline_command_module::InlineCommandModule;
use crate::modules::shared::infrastructure::output_format::OutputFormat;
use crate::modules::shared::infrastructure::terminal_style::{bold, sgr, stdout_tty};
use anyhow::Result;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand)]
pub(crate) enum InlineCmd {
    /// Enable inline suggestions
    Enable,
    /// Disable inline suggestions
    Disable,
    /// Show current status
    Status,
    /// Select the inline suggestion source (history, specs, history+specs or an LLM profile)
    #[command(alias = "set-source")]
    SetCustomization { name: Option<String> },
    /// Show the available inline sources
    #[command(alias = "show-sources")]
    ShowCustomizations {
        #[arg(long, short, value_enum, default_value_t)]
        format: OutputFormat,
    },
}

const PROFILE_EXAMPLE: &str = r#"tabsmith settings inline.profiles.claude '{"provider":"anthropic","model":"claude-opus-5-5","api_key_env":"ANTHROPIC_API_KEY","effort":"low"}'"#;

pub(crate) fn run_inline_command(cmd: InlineCmd, module: &InlineCommandModule) -> Result<i32> {
    let color = stdout_tty();
    match cmd {
        InlineCmd::Enable => set_inline_enabled(true, color, module)?,
        InlineCmd::Disable => set_inline_enabled(false, color, module)?,
        InlineCmd::Status => {
            let on = module.get_inline_status.execute()?;
            println!(
                "Inline is {}",
                bold(if on { "enabled" } else { "disabled" }, color)
            );
        }
        InlineCmd::SetCustomization { name } => match module.select_ghost_text_source.execute(name)? {
            Some(name) => println!("Customization {} selected", bold(&name, color)),
            None => println!("Customization unset"),
        },
        InlineCmd::ShowCustomizations { format } => {
            let sources = module.list_ghost_text_sources.execute()?;
            print!(
                "{}",
                render_sources(&sources.selected, &sources.profiles, format, color)
            );
        }
    }
    Ok(0)
}

fn set_inline_enabled(on: bool, color: bool, module: &InlineCommandModule) -> Result<()> {
    module.set_inline_enabled.execute(on)?;
    println!(
        "{}",
        sgr(
            if on {
                "Inline enabled"
            } else {
                "Inline disabled"
            },
            "35",
            color
        )
    );
    Ok(())
}

/// `inline show-customizations` output (with trailing newline).
fn render_sources(
    selected: &str,
    profiles: &[(String, std::result::Result<LlmProfile, String>)],
    format: OutputFormat,
    color: bool,
) -> String {
    let plain = || {
        let block = |n: &str, d: &str| {
            let mark = if n == selected { " (selected)" } else { "" };
            format!("{}{mark}\n{d}\n\n", bold(n, color))
        };
        let mut out: String = BUILTIN_SOURCES.iter().map(|(n, d)| block(n, d)).collect();
        for (n, p) in profiles {
            out += &match p {
                Ok(p) => block(n, &format!("{} {}", p.provider().name(), p.model())),
                Err(reason) => format!("{} (invalid: {reason})\n\n", bold(n, color)),
            };
        }
        if profiles.is_empty() {
            out += &format!("No LLM profiles configured. Add one with:\n  {PROFILE_EXAMPLE}\n");
        }
        out
    };
    let json = || {
        let builtins = BUILTIN_SOURCES
            .iter()
            .map(|(n, d)| json!({"name": n, "kind": "builtin", "description": d, "selected": *n == selected}));
        let profiles = profiles.iter().map(|(n, p)| match p {
            Ok(p) => json!({
                "name": n, "kind": "profile",
                "description": format!("{} {}", p.provider().name(), p.model()),
                "selected": n == selected,
                "provider": p.provider().name(), "model": p.model(),
                "base_url": p.base_url(), "api_key_env": p.api_key_env(),
            }),
            Err(reason) => json!({
                "name": n, "kind": "profile", "description": format!("invalid: {reason}"), "selected": n == selected,
            }),
        });
        builtins.chain(profiles).collect()
    };
    match format {
        OutputFormat::Plain => plain(),
        f => f.render(String::new, json) + "\n",
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::ghost_text::domain::value_objects::llm_profile::LlmProfileParts;
    use crate::modules::ghost_text::domain::value_objects::llm_provider::LlmProvider;
    use serde_json::Value;

    fn profile(name: &str) -> LlmProfile {
        LlmProfile::new(LlmProfileParts {
            name: name.into(),
            provider: LlmProvider::Anthropic,
            model: "claude-opus-5-5".into(),
            base_url: "https://api.anthropic.com".into(),
            api_key_env: "ANTHROPIC_API_KEY".into(),
            max_tokens: 1024,
            effort: None,
        })
    }

    #[test]
    fn inline_show_customizations() {
        let profiles = vec![
            ("claude".to_string(), Ok(profile("claude"))),
            ("bad".to_string(), Err("model missing".into())),
        ];
        let out = render_sources("claude", &profiles, OutputFormat::Plain, false);
        assert!(out.starts_with("history+specs\nLocal history, then completion specs\n\nhistory\n"));
        assert!(out.contains("claude (selected)\nanthropic claude-opus-5-5\n\n"));
        assert!(out.contains("bad (invalid: model missing)\n"));
        assert!(!out.contains(PROFILE_EXAMPLE));

        let out = render_sources("history+specs", &[], OutputFormat::Plain, false);
        assert!(out.starts_with("history+specs (selected)\n"));
        assert!(out.ends_with(&format!("{PROFILE_EXAMPLE}\n")));

        let v: Value = serde_json::from_str(&render_sources(
            "claude",
            &profiles,
            OutputFormat::Json,
            false,
        ))
        .unwrap();
        let rows = v.as_array().unwrap();
        assert_eq!(rows.len(), 5);
        assert_eq!(
            rows[0],
            json!({"name":"history+specs","kind":"builtin","description":"Local history, then completion specs","selected":false})
        );
        assert_eq!(rows[3]["selected"], json!(true));
        assert_eq!(rows[3]["provider"], json!("anthropic"));
        assert_eq!(rows[3]["api_key_env"], json!("ANTHROPIC_API_KEY"));
        assert_eq!(rows[4]["description"], json!("invalid: model missing"));
    }
}
