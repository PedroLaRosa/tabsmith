#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Shell {
    Bash,
    Zsh,
    Fish,
    Pwsh,
}

impl Shell {
    pub(crate) const ALL: [Shell; 4] = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::Pwsh];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::Pwsh => "pwsh",
        }
    }

    /// Accepts a name or path: `zsh`, `/bin/zsh`, `-zsh` (login), `pwsh.exe`, `powershell`.
    pub(crate) fn from_name(s: &str) -> Option<Shell> {
        let base = s.rsplit(['/', '\\']).next().unwrap_or(s);
        let base = base.trim_start_matches('-');
        let base = base.strip_suffix(".exe").unwrap_or(base);
        let base = base.split_whitespace().next().unwrap_or(base); // "zsh (tabsmith)"
        Some(match base.to_ascii_lowercase().as_str() {
            "bash" => Shell::Bash,
            "zsh" => Shell::Zsh,
            "fish" => Shell::Fish,
            "pwsh" | "powershell" => Shell::Pwsh,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_list_every_shell_in_a_fixed_order_by_its_lowercase_name() {
        let shells = Shell::ALL;

        let names: Vec<&str> = shells.iter().map(|s| s.name()).collect();

        assert_eq!(names, ["bash", "zsh", "fish", "pwsh"]);
    }

    #[test]
    fn should_recognise_a_shell_from_its_name_or_path() {
        let names = [
            "bash",
            "/bin/zsh",
            "-zsh",
            "--fish",
            "BASH",
            "zsh (tabsmith)",
            "pwsh.exe",
            "powershell",
            r"C:\Program Files\PowerShell\7\pwsh.exe",
        ];

        let shells: Vec<Option<Shell>> = names.iter().map(|n| Shell::from_name(n)).collect();

        assert_eq!(
            shells,
            [
                Some(Shell::Bash),
                Some(Shell::Zsh),
                Some(Shell::Zsh),
                Some(Shell::Fish),
                Some(Shell::Bash),
                Some(Shell::Zsh),
                Some(Shell::Pwsh),
                Some(Shell::Pwsh),
                Some(Shell::Pwsh),
            ]
        );
    }

    #[test]
    fn should_not_recognise_a_name_the_normalisation_does_not_reduce_to_a_shell() {
        let names = ["", "sh", "PWSH.EXE", "pwsh.exe -NoLogo", "/usr/bin/env bash"];

        let shells: Vec<Option<Shell>> = names.iter().map(|n| Shell::from_name(n)).collect();

        assert_eq!(shells, [None; 5]);
    }
}
