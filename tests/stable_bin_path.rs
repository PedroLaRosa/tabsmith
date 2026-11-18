//! tabsmith run by its resolved path, as Linux reports a Homebrew install, names itself in the shell integration by
//! the PATH link that `brew upgrade` keeps, not by the versioned Cellar path it deletes (SPEC §5.5).
#![cfg(unix)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output};

/// A fresh home holding `bin/tabsmith`, a symlink to the tabsmith under test; removed on drop.
struct HomebrewLikeHome(PathBuf);

impl HomebrewLikeHome {
    fn new(tag: &str) -> Self {
        let home = std::env::temp_dir().join(format!("tabsmith-it-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_tabsmith"), home.join("bin").join("tabsmith")).unwrap();
        HomebrewLikeHome(home)
    }

    fn link(&self) -> PathBuf {
        self.0.join("bin").join("tabsmith")
    }

    fn data_dir(&self) -> PathBuf {
        self.0.join(".tabsmith")
    }

    /// Runs the tabsmith under test by its own path, with this home's `bin` first on PATH.
    fn run(&self, args: &[&str]) -> Output {
        let system_path = std::env::var_os("PATH").unwrap_or_default();
        let path: OsString = std::env::join_paths(
            std::iter::once(self.0.join("bin")).chain(std::env::split_paths(&system_path)),
        )
        .unwrap();
        Command::new(env!("CARGO_BIN_EXE_tabsmith"))
            .args(args)
            .env("PATH", path)
            .env("HOME", &self.0)
            .env("TABSMITH_HOME", self.data_dir())
            .env_remove("ZDOTDIR")
            .env_remove("XDG_CONFIG_HOME")
            .output()
            .unwrap()
    }
}

impl Drop for HomebrewLikeHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn should_run_the_path_link_from_the_init_script() {
    let home = HomebrewLikeHome::new("init");

    let output = home.run(&["init", "zsh", "post"]);

    let script = String::from_utf8(output.stdout).unwrap();
    assert!(
        script.contains(&format!("'{}' _ pre-cmd", home.link().display())),
        "{script}"
    );
}

/// `doctor` locates the paths the same way, so it finds these scripts current.
#[test]
fn should_guard_the_dotfile_scripts_with_the_path_link() {
    let home = HomebrewLikeHome::new("dotfiles");

    let output = home.run(&["integrations", "install", "dotfiles", "zsh"]);

    let script = std::fs::read_to_string(home.data_dir().join("shell").join("zshrc.pre.zsh"))
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&output.stderr)));
    assert!(
        script.starts_with(&format!("[ -x '{}' ]", home.link().display())),
        "{script}"
    );
}
