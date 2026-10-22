//! The embedded Fig spec bundle: extraction, index and name → file resolution (SPEC §9.1, §6.3).
//! OWNER: host agent.
use crate::settings::Settings;
use anyhow::Context;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The solid `tar | zstd -19` of `@withfig/autocomplete/build` (SPEC D11).
#[cfg(feature = "embed-specs")]
static ARCHIVE: &[u8] = include_bytes!("../assets/specs.tar.zst");

/// Version of the bundled `@withfig/autocomplete` package.
pub fn version() -> &'static str {
    include_str!("../assets/SPECS_VERSION").trim()
}

/// `~/.tabsmith/specs/<ver>/`, extracting the embedded archive first if `index.json` is missing (atomic rename;
/// removes other versions afterwards). Without the `embed-specs` feature the directory must already exist.
pub fn ensure_extracted() -> anyhow::Result<PathBuf> {
    let root = crate::settings::home()?.join("specs");
    let dir = root.join(version());
    if dir.join("index.json").is_file() {
        return Ok(dir);
    }
    #[cfg(feature = "embed-specs")]
    return extract_to(ARCHIVE, &root, version());
    #[cfg(not(feature = "embed-specs"))]
    anyhow::bail!("no specs in {} (built without the `embed-specs` feature)", dir.display())
}

/// Unpack `archive` (tar.zst) into `<root>/<ver>.tmp-<pid>/`, rename it to `<root>/<ver>/` and remove other
/// versions (SPEC §9.1). A concurrent extractor that loses the rename deletes its temp dir and uses the winner's.
pub(crate) fn extract_to(archive: &[u8], root: &Path, ver: &str) -> anyhow::Result<PathBuf> {
    let dir = root.join(ver);
    let tmp = root.join(format!("{ver}.tmp-{}", std::process::id()));
    std::fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = unpack(archive, &tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e).with_context(|| format!("extracting specs into {}", tmp.display()));
    }
    if dir.exists() && !dir.join("index.json").is_file() {
        let _ = std::fs::remove_dir_all(&dir); // leftover without an index: replace it
    }
    if let Err(e) = std::fs::rename(&tmp, &dir) {
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join("index.json").is_file() {
            return Err(e).with_context(|| format!("renaming specs into {}", dir.display()));
        }
    }
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // ponytail: other extractors' `.tmp-<pid>` dirs are kept (may be live); a crash mid-extraction leaks one.
        if name != ver && name != "user" && !name.contains(".tmp-") && entry.path().is_dir() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    Ok(dir)
}

/// Unpack the regular files of a tar.zst into `dst` (parents created on demand). Skips macOS AppleDouble
/// `._*` entries: the asset was tarred by bsdtar with xattrs, so it holds a `._x` twin for every file.
fn unpack(archive: &[u8], dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let mut ar = tar::Archive::new(zstd::Decoder::new(archive)?);
    for entry in ar.entries()? {
        let mut entry = entry?;
        let apple_double = entry.path()?.file_name().is_some_and(|n| n.to_string_lossy().starts_with("._"));
        if entry.header().entry_type().is_file() && !apple_double {
            entry.unpack_in(dst)?;
        }
    }
    Ok(())
}

/// `~/.tabsmith/specs/user/` (user overrides, same layout as the npm build/ dir).
pub fn user_dir() -> anyhow::Result<PathBuf> {
    Ok(crate::settings::home()?.join("specs").join("user"))
}

/// `index.json` of the bundled set ∪ `*.js` stems in the user dir.
#[derive(Clone, Debug, Default)]
pub struct SpecIndex {
    /// Root spec names (e.g. `git`, `aws`, `@withfig/autocomplete-tools`).
    pub completions: Vec<String>,
    /// Diff-versioned spec names (`az`, `heroku`, …) → load `<name>/index.js`.
    pub diff_versioned: HashSet<String>,
}

impl SpecIndex {
    pub fn load() -> anyhow::Result<SpecIndex> {
        SpecIndex::load_from(&ensure_extracted()?, &user_dir()?)
    }

    /// `bundle/index.json` (`{completions, diffVersionedCompletions}`) plus the top-level `*.js` stems of `user`.
    pub(crate) fn load_from(bundle: &Path, user: &Path) -> anyhow::Result<SpecIndex> {
        #[derive(serde::Deserialize)]
        struct Raw {
            completions: Vec<String>,
            #[serde(rename = "diffVersionedCompletions", default)]
            diff_versioned: Vec<String>,
        }
        let path = bundle.join("index.json");
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let raw: Raw = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        let mut completions = raw.completions;
        for entry in std::fs::read_dir(user).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "js") && p.is_file() {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    if !completions.iter().any(|c| c == stem) {
                        completions.push(stem.to_string());
                    }
                }
            }
        }
        Ok(SpecIndex { completions, diff_versioned: raw.diff_versioned.into_iter().collect() })
    }
}

/// Resolve a global spec name to a file: dev folder (when `autocomplete.developerMode`), then the user dir, then
/// the extracted bundle; `<name>.js`, else `<name>/index.js`; diff-versioned names go to `<name>/index.js`.
pub fn resolve(name: &str, settings: &Settings, index: &SpecIndex) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if settings.bool("autocomplete.developerMode", false) {
        roots.extend(settings.string("autocomplete.devCompletionsFolder").filter(|d| !d.is_empty()).map(PathBuf::from));
    }
    let home = crate::settings::home().ok()?;
    roots.push(home.join("specs").join("user"));
    roots.push(home.join("specs").join(version()));
    resolve_in(name, &roots, index)
}

/// [`resolve`] over explicit roots (first wins). Absolute names and `..` components are rejected.
pub(crate) fn resolve_in(name: &str, roots: &[PathBuf], index: &SpecIndex) -> Option<PathBuf> {
    if name.is_empty() || Path::new(name).has_root() || name.split(['/', '\\']).any(|c| c == "..") {
        return None;
    }
    let diff = index.diff_versioned.contains(name);
    roots.iter().find_map(|root| {
        let file = root.join(format!("{name}.js"));
        if !diff && file.is_file() {
            return Some(file);
        }
        Some(root.join(name).join("index.js")).filter(|f| f.is_file())
    })
}

/// Test helpers shared with `js` tests: unique temp dirs and partial extraction of the real bundle.
#[cfg(test)]
pub(crate) mod testutil {
    use std::path::{Path, PathBuf};

    /// A unique temp dir, removed on drop.
    pub struct TmpDir(pub PathBuf);

    impl TmpDir {
        pub fn new(tag: &str) -> TmpDir {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let p = std::env::temp_dir().join(format!(
                "tabsmith-test-{tag}-{}-{nanos}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            TmpDir(p)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Unpack only `files` (paths inside the bundle, e.g. `git.js`) of the embedded archive into a temp dir.
    #[cfg(feature = "embed-specs")]
    pub fn bundle_subset(files: &[&str]) -> TmpDir {
        let dir = TmpDir::new("bundle");
        let mut ar = tar::Archive::new(zstd::Decoder::new(super::ARCHIVE).unwrap());
        for entry in ar.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().into_owned();
            let rel = path.strip_prefix(".").unwrap_or(&path);
            if files.iter().any(|f| rel == Path::new(f)) {
                assert!(entry.unpack_in(dir.path()).unwrap());
            }
        }
        dir
    }

    /// The whole embedded bundle, extracted with the production code path into `<tmp>/x/`.
    #[cfg(feature = "embed-specs")]
    pub fn full_bundle() -> TmpDir {
        let root = TmpDir::new("bundle-all");
        super::extract_to(super::ARCHIVE, root.path(), "x").unwrap();
        root
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::TmpDir;
    use super::*;

    fn tar_zst(files: &[(&str, &str)]) -> Vec<u8> {
        let mut b = tar::Builder::new(Vec::new());
        for (path, body) in files {
            let mut h = tar::Header::new_gnu();
            h.set_size(body.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            b.append_data(&mut h, path, body.as_bytes()).unwrap();
        }
        zstd::encode_all(&b.into_inner().unwrap()[..], 3).unwrap()
    }

    #[test]
    fn extract_renames_and_removes_other_versions() {
        let root = TmpDir::new("extract");
        std::fs::create_dir_all(root.path().join("0.9.0")).unwrap();
        std::fs::create_dir_all(root.path().join("user")).unwrap();
        let archive =
            tar_zst(&[("./index.json", r#"{"completions":["git"]}"#), ("./aws/s3.js", "x"), ("./aws/._s3.js", "\u{0}")]);
        let dir = extract_to(&archive, root.path(), "1.0.0").unwrap();
        assert_eq!(dir, root.path().join("1.0.0"));
        assert!(dir.join("index.json").is_file() && dir.join("aws/s3.js").is_file());
        assert!(!dir.join("aws/._s3.js").exists(), "AppleDouble entries skipped");
        assert!(!root.path().join("0.9.0").exists(), "old version removed");
        assert!(root.path().join("user").is_dir(), "user dir kept");
        // Loser of a concurrent extraction: the winner's dir stays, the temp dir is gone.
        std::fs::write(dir.join("marker"), "winner").unwrap();
        assert_eq!(extract_to(&archive, root.path(), "1.0.0").unwrap(), dir);
        assert!(dir.join("marker").is_file());
        let names: Vec<_> = std::fs::read_dir(root.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }

    #[test]
    fn extract_error_cleans_temp() {
        let root = TmpDir::new("extract-bad");
        assert!(extract_to(b"not zstd", root.path(), "1.0.0").is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn index_merges_user_stems() {
        let bundle = TmpDir::new("index");
        let user = TmpDir::new("index-user");
        std::fs::write(
            bundle.path().join("index.json"),
            r#"{"completions":["git","az"],"diffVersionedCompletions":["az"]}"#,
        )
        .unwrap();
        std::fs::write(user.path().join("git.js"), "").unwrap();
        std::fs::write(user.path().join("mytool.js"), "").unwrap();
        std::fs::write(user.path().join("notes.txt"), "").unwrap();
        let idx = SpecIndex::load_from(bundle.path(), user.path()).unwrap();
        assert_eq!(idx.completions, ["git", "az", "mytool"]);
        assert!(idx.diff_versioned.contains("az") && idx.diff_versioned.len() == 1);
        // Missing user dir is fine.
        assert_eq!(SpecIndex::load_from(bundle.path(), &user.path().join("nope")).unwrap().completions.len(), 2);
    }

    #[test]
    fn resolve_order() {
        let t = TmpDir::new("resolve");
        let (dev, user, bundle) = (t.path().join("dev"), t.path().join("user"), t.path().join("bundle"));
        for (dir, file) in [
            (&user, "git.js"),
            (&bundle, "git.js"),
            (&bundle, "az.js"),
            (&bundle, "az/index.js"),
            (&bundle, "aws/s3.js"),
            (&bundle, "foo/index.js"),
            (&dev, "foo.js"),
        ] {
            let p = dir.join(file);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "").unwrap();
        }
        let idx = SpecIndex { completions: vec![], diff_versioned: ["az".to_string()].into() };
        let roots = [dev.clone(), user.clone(), bundle.clone()];
        assert_eq!(resolve_in("git", &roots, &idx), Some(user.join("git.js")));
        assert_eq!(resolve_in("az", &roots, &idx), Some(bundle.join("az/index.js")));
        assert_eq!(resolve_in("aws/s3", &roots, &idx), Some(bundle.join("aws/s3.js")));
        assert_eq!(resolve_in("foo", &roots, &idx), Some(dev.join("foo.js")));
        assert_eq!(resolve_in("foo", &roots[1..], &idx), Some(bundle.join("foo/index.js")));
        assert_eq!(resolve_in("gcloud/alpha", &roots, &idx), None);
        assert_eq!(resolve_in("../bundle/git", &roots, &idx), None);
        assert_eq!(resolve_in("/etc/passwd", &roots, &idx), None);
    }

    #[cfg(feature = "embed-specs")]
    #[test]
    fn embedded_index_shape() {
        let b = testutil::bundle_subset(&["index.json"]);
        let idx = SpecIndex::load_from(b.path(), &b.path().join("user")).unwrap();
        assert_eq!(idx.completions.len(), 1484);
        assert_eq!(idx.diff_versioned.len(), 6);
        assert!(idx.diff_versioned.contains("heroku"));
    }
}
