use std::path::PathBuf;

// ponytail: not the lookup js::find_exe does (that one takes cwd and env, checks the exec bit and orders PATHEXT
// differently); merging them changes either the setup/doctor shell checks or generator exec.
/// First `name` executable on PATH (Windows: tries PATHEXT extensions).
pub(crate) fn which(name: &str) -> Option<PathBuf> {
    let exts: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into())
            .split(';')
            .map(String::from)
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&std::env::var_os("PATH")?).find_map(|dir| {
        exts.iter()
            .map(|e| dir.join(format!("{name}{e}")))
            .find(|p| p.is_file())
    })
}
