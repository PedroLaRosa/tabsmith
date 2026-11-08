//! FileDotfileRepository tests: the io failures it reports and the files it takes for existing, then the dotfile
//! scenarios on disk (install and uninstall, detection, enumeration, the BOM, atomic writes, a moved binary, the
//! manual-install lines) through the test driver over the production installer.
use super::FileDotfileRepository;
use crate::modules::shared::domain::value_objects::shell::Shell;
#[cfg(unix)]
use crate::modules::shared::infrastructure::os::process_info::is_root;
use crate::modules::shared::infrastructure::testing::temp_dir::TempDir;
use crate::modules::shell_integration::domain::entities::integration::Integration;
use crate::modules::shell_integration::domain::repositories::dotfile_repository::DotfileRepository;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use crate::modules::shell_integration::domain::value_objects::dotfile::{POST_MARKER, PRE_MARKER};
use crate::modules::shell_integration::infrastructure::testing::integration_fixtures::{
    ctx, install_dotfiles, integrations, manual_text, syntax_ok, uninstall_dotfiles, IntegrationOnDisk,
};

#[test]
fn should_report_a_missing_dotfile_as_file_does_not_exist() {
    let temp_dir = TempDir::new("dotfile-missing");
    let path = temp_dir.path().join(".bashrc");

    let error = FileDotfileRepository.read(&path).unwrap_err();

    assert!(matches!(error, ShellIntegrationError::FileDoesNotExist(p) if p == path));
}

/// Windows refuses to open a directory (access denied), so this holds on Unix only.
#[cfg(unix)]
#[test]
fn should_report_reading_a_directory_as_an_io_failure_on_its_path() {
    let temp_dir = TempDir::new("dotfile-directory");
    let path = temp_dir.path().join(".bashrc");
    std::fs::create_dir(&path).unwrap();

    let error = FileDotfileRepository.read(&path).unwrap_err();

    assert_eq!(
        (matches!(error, ShellIntegrationError::Io(..)), error.to_string()),
        (true, format!("{}: Is a directory (os error 21)", path.display()))
    );
}

#[cfg(unix)]
#[test]
fn should_report_an_unreadable_dotfile_as_permission_denied() {
    use crate::modules::shared::infrastructure::os::process_info::is_root;
    use std::os::unix::fs::PermissionsExt;
    if is_root() {
        return; // root reads it anyway
    }
    let temp_dir = TempDir::new("dotfile-unreadable");
    let path = temp_dir.path().join(".zshrc");
    std::fs::write(&path, "export A=1\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();

    let error = FileDotfileRepository.read(&path).unwrap_err();

    assert!(matches!(error, ShellIntegrationError::PermissionDenied(p) if p == path));
}

#[test]
fn should_accept_removing_a_file_that_is_not_there() {
    let temp_dir = TempDir::new("dotfile-remove-missing");

    let removed = FileDotfileRepository.remove(&temp_dir.path().join("bashrc.pre.bash"));

    assert!(removed.is_ok());
}

#[cfg(unix)]
#[test]
fn should_not_take_a_dangling_symlink_for_an_existing_dotfile() {
    let temp_dir = TempDir::new("dotfile-dangling");
    let path = temp_dir.path().join(".bashrc");
    std::os::unix::fs::symlink("../dotfiles/bashrc", &path).unwrap();

    let exists = FileDotfileRepository.exists(&path);

    assert!(!exists);
}

#[test]
fn integrations_dotfile_install_uninstall() {
    let temp_dir = TempDir::new("dotfile");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    std::fs::create_dir_all(c.home()).unwrap();
    let zshrc = c.home().join(".zshrc");
    let original = "#!/bin/zsh\nexport A=1\nalias ll='ls -l'\n";
    std::fs::write(&zshrc, original).unwrap();

    let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
    assert!(matches!(
        rc.is_installed(&c),
        Err(ShellIntegrationError::NotInstalled(_))
    ));
    rc.install(&c).unwrap();
    rc.is_installed(&c).unwrap();
    let once = std::fs::read_to_string(&zshrc).unwrap();
    let lines: Vec<&str> = once.lines().collect();
    assert_eq!(lines[0], "#!/bin/zsh");
    assert_eq!(lines[1], PRE_MARKER);
    assert_eq!(
        lines[2],
        r#"[ -f "${HOME}/.tabsmith/shell/zshrc.pre.zsh" ] && . "${HOME}/.tabsmith/shell/zshrc.pre.zsh""#
    );
    assert_eq!(lines[lines.len() - 2], POST_MARKER);
    assert!(lines[lines.len() - 1].ends_with(r#". "${HOME}/.tabsmith/shell/zshrc.post.zsh""#));
    assert_eq!(
        std::fs::read_to_string(c.data_root().join("shell/zshrc.pre.zsh")).unwrap(),
        "[ -x '/opt/tab smith/it'\\''s/tabsmith' ] && eval \"$('/opt/tab smith/it'\\''s/tabsmith' init zsh pre --rcfile zshrc)\"\n"
    );

    // Idempotent: a second install changes nothing.
    rc.install(&c).unwrap();
    assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), once);

    // Backup of the original.
    let backups: Vec<_> = std::fs::read_dir(c.data_root().join("backups"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(
        std::fs::read_to_string(backups[0].path().join(".zshrc")).unwrap(),
        original
    );

    // The installed dotfile and script files still parse.
    syntax_ok(Shell::Zsh, &zshrc);

    // Uninstall restores the original and deletes the script files.
    rc.uninstall(&c).unwrap();
    assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), original);
    assert!(!c.data_root().join("shell/zshrc.pre.zsh").exists());
    assert!(!c.data_root().join("shell/zshrc.post.zsh").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn integrations_dotfile_detection() {
    let temp_dir = TempDir::new("detect");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    let bashrc = c.home().join(".bashrc");
    let rc = Integration::dotfile(Shell::Bash, bashrc.clone());
    assert!(matches!(
        rc.is_installed(&c),
        Err(ShellIntegrationError::FileDoesNotExist(_))
    ));

    rc.install(&c).unwrap(); // creates the file
    let installed = std::fs::read_to_string(&bashrc).unwrap();
    syntax_ok(Shell::Bash, &bashrc);

    std::fs::write(&bashrc, format!("echo first\n{installed}")).unwrap();
    let e = rc.is_installed(&c).unwrap_err().to_string();
    assert!(e.ends_with("does not source pre integration first"), "{e}");

    std::fs::write(
        &bashrc,
        format!("{installed}echo last\n# trailing comment\n\n"),
    )
    .unwrap();
    let e = rc.is_installed(&c).unwrap_err().to_string();
    assert!(
        e.starts_with("Improper integration installation: ")
            && e.ends_with("post integration last"),
        "{e}"
    );

    // A moved binary: script bytes differ.
    std::fs::write(&bashrc, &installed).unwrap();
    let moved = c.clone().with_binary("/elsewhere/tabsmith".into());
    let e = rc.is_installed(&moved).unwrap_err().to_string();
    assert!(e.contains("bashrc.pre.bash should contain:\n[ -n \"$BASH_VERSION\" ] && [ -x '/elsewhere/tabsmith' ]"), "{e}");
    rc.install(&moved).unwrap();
    rc.is_installed(&moved).unwrap();

    // Opt-out marker wins.
    std::fs::write(&bashrc, "  #  TABSMITH ignore\necho hi\n").unwrap();
    rc.is_installed(&c).unwrap();
    rc.install(&c).unwrap();
    assert_eq!(
        std::fs::read_to_string(&bashrc).unwrap(),
        "  #  TABSMITH ignore\necho hi\n"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn integrations_enumeration_and_whole_files() {
    let temp_dir = TempDir::new("enum");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    std::fs::create_dir_all(c.home()).unwrap();
    let names = |s| {
        integrations(&c, s)
            .unwrap()
            .iter()
            .map(Integration::file_name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(Shell::Bash), [".bashrc", ".profile"]);
    std::fs::write(c.home().join(".bash_profile"), "").unwrap();
    assert_eq!(names(Shell::Bash), [".bashrc", ".bash_profile"]);
    assert_eq!(names(Shell::Zsh), [".zshrc", ".zprofile"]);

    let fish = integrations(&c, Shell::Fish).unwrap();
    assert_eq!(
        fish.iter().map(Integration::describe).collect::<Vec<_>>(),
        ["fish pre", "fish post"]
    );
    for i in &fish {
        i.install(&c).unwrap();
        i.is_installed(&c).unwrap();
        syntax_ok(Shell::Fish, i.path());
    }
    assert_eq!(
        std::fs::read_to_string(c.fish_config_dir().join("conf.d/00_tabsmith_pre.fish")).unwrap(),
        "test -x '/opt/tab smith/it\\'s/tabsmith'; and eval ('/opt/tab smith/it\\'s/tabsmith' init fish pre --rcfile 00_tabsmith_pre | string split0)\n"
    );
    for i in &fish {
        i.uninstall(&c).unwrap();
        assert!(!i.path().exists());
    }

    // pwsh: the profile directory is created; script files are `profile.<when>.ps1`.
    let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
    assert_eq!(
        pwsh.describe(),
        "pwsh pre post into Microsoft.PowerShell_profile.ps1"
    );
    pwsh.install(&c).unwrap();
    pwsh.is_installed(&c).unwrap();
    let text = std::fs::read_to_string(pwsh.path()).unwrap();
    assert!(text.contains(r#"if (Test-Path "${HOME}/.tabsmith/shell/profile.pre.ps1") { . "${HOME}/.tabsmith/shell/profile.pre.ps1" }"#));
    syntax_ok(Shell::Pwsh, pwsh.path());
    let _ = std::fs::remove_dir_all(dir);
}

/// A UTF-8 BOM (VS Code, ISE and Windows PowerShell save .ps1 files with one) stays the first bytes: install
/// puts the pre block after it, is_installed sees through it, a reinstall is a no-op, uninstall keeps it.
#[test]
fn integrations_dotfile_bom_stays_first() {
    let temp_dir = TempDir::new("bom");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    let pwsh = &integrations(&c, Shell::Pwsh).unwrap()[0];
    std::fs::create_dir_all(pwsh.path().parent().unwrap()).unwrap();
    let user = "Write-Host USERLINE\r\nSet-PSReadLineOption -PredictionSource History\r\n";
    std::fs::write(pwsh.path(), format!("\u{feff}{user}")).unwrap();
    pwsh.install(&c).unwrap();
    let once = std::fs::read_to_string(pwsh.path()).unwrap();
    assert!(
        once.starts_with(&format!("\u{feff}{PRE_MARKER}\n"))
            && once.matches('\u{feff}').count() == 1,
        "{once:?}"
    );
    assert!(
        once.contains(&format!("\nWrite-Host USERLINE\r\n")),
        "{once:?}"
    );
    pwsh.is_installed(&c).unwrap();
    pwsh.install(&c).unwrap();
    assert_eq!(std::fs::read_to_string(pwsh.path()).unwrap(), once);
    syntax_ok(Shell::Pwsh, pwsh.path());

    // An editor re-saves a plain install with a BOM: still installed; a forced reinstall keeps one block.
    pwsh.uninstall(&c).unwrap();
    let back = std::fs::read_to_string(pwsh.path()).unwrap();
    assert!(
        back.starts_with("\u{feff}Write-Host USERLINE\r\n") && !back.contains("tabsmith"),
        "{back:?}"
    );
    std::fs::write(pwsh.path(), user).unwrap();
    pwsh.install(&c).unwrap();
    let plain = std::fs::read_to_string(pwsh.path()).unwrap();
    std::fs::write(pwsh.path(), format!("\u{feff}{plain}")).unwrap();
    pwsh.is_installed(&c).unwrap();
    pwsh.uninstall(&c).unwrap();
    pwsh.install(&c).unwrap();
    let again = std::fs::read_to_string(pwsh.path()).unwrap();
    assert_eq!(again, format!("\u{feff}{plain}"));
    let _ = std::fs::remove_dir_all(dir);
}

/// Dotfiles are replaced atomically: whoever holds the old file sees it whole, a symlinked dotfile stays a
/// symlink, permissions stay, no temp file is left, and a read-only file still reports PermissionDenied.
/// The same scenario against the write itself is shared atomic_file.rs `integrations_dotfile_write_is_atomic`.
#[cfg(unix)]
#[test]
fn integrations_dotfile_write_is_atomic_install() {
    use std::os::unix::fs::PermissionsExt;
    let temp_dir = TempDir::new("atomic");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    let real = dir.join("dotfiles").join("zshrc");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::create_dir_all(c.home()).unwrap();
    std::fs::write(&real, "export A=1\n").unwrap();
    std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o600)).unwrap();
    let zshrc = c.home().join(".zshrc");
    std::os::unix::fs::symlink(&real, &zshrc).unwrap();
    let old = dir.join("old-zshrc");
    std::fs::hard_link(&real, &old).unwrap(); // the old inode, as a concurrent reader sees it
    let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
    rc.install(&c).unwrap();
    assert_eq!(
        std::fs::read_to_string(&old).unwrap(),
        "export A=1\n",
        "rewritten in place"
    );
    assert!(std::fs::symlink_metadata(&zshrc)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
    assert_eq!(
        std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::read_dir(real.parent().unwrap()).unwrap().count(),
        1,
        "temp file left behind"
    );
    if !is_root() {
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert!(matches!(
            rc.uninstall(&c),
            Err(ShellIntegrationError::PermissionDenied(_))
        ));
        assert!(std::fs::read_to_string(&real).unwrap().contains(PRE_MARKER));
    }
    // A dangling relative symlink (dotfiles checkout not populated yet): the link stays, its target is created.
    let bashrc = c.home().join(".bashrc");
    std::os::unix::fs::symlink("../dotfiles/bashrc", &bashrc).unwrap();
    let rc = Integration::dotfile(Shell::Bash, bashrc.clone());
    rc.install(&c).unwrap();
    assert!(std::fs::symlink_metadata(&bashrc)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(std::fs::read_to_string(dir.join("dotfiles/bashrc"))
        .unwrap()
        .contains(PRE_MARKER));
    let _ = std::fs::remove_dir_all(dir);
}

/// `integrations uninstall` removes integrations whose scripts name another binary (it moved): is_installed
/// fails for all of them. With nothing left it reports nothing done.
#[test]
fn integrations_uninstall_after_binary_moved() {
    let temp_dir = TempDir::new("moved");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    let shells = [Shell::Bash, Shell::Fish];
    install_dotfiles(&c, &shells).unwrap();
    let moved = c.clone().with_binary("/elsewhere/tabsmith".into());
    let all: Vec<Integration> = shells
        .iter()
        .flat_map(|s| integrations(&moved, *s).unwrap())
        .collect();
    assert!(all.iter().all(|i| i.is_installed(&moved).is_err()));
    assert!(uninstall_dotfiles(&moved, &shells).unwrap());
    assert!(!std::fs::read_to_string(c.home().join(".bashrc"))
        .unwrap()
        .contains("tabsmith"));
    assert!(all.iter().all(|i| !i.has_traces(&moved)), "{all:?}");
    assert!(!uninstall_dotfiles(&moved, &shells).unwrap());
    let _ = std::fs::remove_dir_all(dir);
}

/// The manual-install lines setup prints are the ones is_installed and uninstall recognise.
#[test]
fn integrations_manual_lines_are_recognised() {
    let temp_dir = TempDir::new("manual");
    let dir = temp_dir.path().to_path_buf();
    let c = ctx(&dir);
    std::fs::create_dir_all(c.home()).unwrap();
    let text = manual_text(&c, &[Shell::Bash, Shell::Zsh]);
    let lines: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("zsh:"))
        .map(str::trim)
        .collect();
    assert_eq!(lines.len(), 2, "{text}");
    let zshrc = c.home().join(".zshrc");
    std::fs::write(&zshrc, format!("{}\nexport A=1\n{}\n", lines[0], lines[1])).unwrap();
    let rc = Integration::dotfile(Shell::Zsh, zshrc.clone());
    rc.write_scripts(&c).unwrap(); // what setup's manual path writes
    rc.is_installed(&c).unwrap();
    rc.uninstall(&c).unwrap();
    assert_eq!(std::fs::read_to_string(&zshrc).unwrap(), "export A=1\n");
    assert!(
        !text.contains("pwsh")
            && manual_text(&c, &[Shell::Pwsh]).contains("pwsh ($PROFILE): if (Test-Path")
    );
    let _ = std::fs::remove_dir_all(dir);
}
