//! The embedded Fig spec bundle: extraction, index and name → file resolution (SPEC §9.1, §6.3).
//! OWNER: host agent.
use crate::settings::Settings;
use anyhow::Context;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The solid `tar | zstd -19` of `@withfig/autocomplete/build` (SPEC D11).
#[cfg(feature = "embed-specs")]
static ARCHIVE: &[u8] = include_bytes!("../assets/specs.tar.zst");

/// Version of the bundled `@withfig/autocomplete` package.
pub fn version() -> &'static str {
    include_str!("../assets/SPECS_VERSION").trim()
}

/// Touched in `<ver>/` by every engine start, so other binaries' cleanup sees the version is in use.
const LAST_USED: &str = ".last-used";
/// A version dir unused this long is removed (sessions of an older binary may still read theirs).
const VERSION_STALE: Duration = Duration::from_secs(7 * 86400);
/// A `.tmp-<pid>` dir untouched this long is an extraction that died (one takes seconds).
const TMP_STALE: Duration = Duration::from_secs(600);

/// `~/.tabsmith/specs/<ver>/`, extracting the embedded archive first if `index.json` is missing (atomic rename).
/// Marks the version used and removes stale versions/temp dirs. Without `embed-specs` the dir must already exist.
pub fn ensure_extracted() -> anyhow::Result<PathBuf> {
    let root = crate::settings::home()?.join("specs");
    let dir = root.join(version());
    if dir.join("index.json").is_file() {
        touch(&dir);
        cleanup(&root, version());
        return Ok(dir);
    }
    #[cfg(feature = "embed-specs")]
    return extract_to(ARCHIVE, &root, version());
    #[cfg(not(feature = "embed-specs"))]
    anyhow::bail!(
        "no specs in {} (built without the `embed-specs` feature)",
        dir.display()
    )
}

/// Unpack `archive` (tar.zst) into `<root>/<ver>.tmp-<pid>/`, rename it to `<root>/<ver>/` and [`cleanup`]
/// (SPEC §9.1). A concurrent extractor that loses the rename deletes its temp dir and uses the winner's.
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
    touch(&dir);
    cleanup(root, ver);
    Ok(dir)
}

fn touch(dir: &Path) {
    let _ =
        std::fs::File::create(dir.join(LAST_USED)).and_then(|f| f.set_modified(SystemTime::now()));
}

/// Remove other version dirs unused for [`VERSION_STALE`] (age of `.last-used`, else of the dir; a version a
/// running session uses was touched at its start) and `<ver>.tmp-` dirs older than [`TMP_STALE`]. Only those:
/// `user/` or any other dir there belongs to the user.
/// ponytail: a session running longer than a week without a restart may lose its version to a newer binary.
fn cleanup(root: &Path, ver: &str) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let (name, p) = (
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        );
        let (stem, max) = match name.split_once(".tmp-") {
            Some((stem, _)) => (stem, TMP_STALE),
            None => (name.as_str(), VERSION_STALE),
        };
        // ponytail: npm versions are `1.2.3`; a pre-release suffix would make its dirs look like the user's (kept)
        let ours = stem
            .split('.')
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        let age = std::fs::metadata(p.join(LAST_USED))
            .or_else(|_| std::fs::metadata(&p))
            .and_then(|m| m.modified());
        if ours
            && name != ver
            && p.is_dir()
            && age.is_ok_and(|t| t.elapsed().unwrap_or_default() > max)
        {
            let _ = std::fs::remove_dir_all(&p);
        }
    }
}

/// Unpack the regular files of a tar.zst into `dst` (parents created on demand). Skips macOS AppleDouble
/// `._*` entries: the asset was tarred by bsdtar with xattrs, so it holds a `._x` twin for every file.
fn unpack(archive: &[u8], dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let mut ar = tar::Archive::new(zstd::Decoder::new(archive)?);
    for entry in ar.entries()? {
        let mut entry = entry?;
        let apple_double = entry
            .path()?
            .file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with("._"));
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
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let raw: Raw =
            serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
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
        Ok(SpecIndex {
            completions,
            diff_versioned: raw.diff_versioned.into_iter().collect(),
        })
    }
}

/// Resolve a global spec name to a file: dev folder (when `autocomplete.developerMode`), then the user dir, then
/// the extracted bundle; `<name>.js`, else `<name>/index.js`; diff-versioned names go to `<name>/index.js`.
pub fn resolve(name: &str, settings: &Settings, index: &SpecIndex) -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = dev_folder(settings).into_iter().collect();
    let home = crate::settings::home().ok()?;
    roots.push(home.join("specs").join("user"));
    roots.push(home.join("specs").join(version()));
    resolve_in(name, &roots, index)
}

/// `autocomplete.devCompletionsFolder` (leading `~` = the user's home) when `autocomplete.developerMode` is on.
fn dev_folder(settings: &Settings) -> Option<PathBuf> {
    let d = settings
        .string("autocomplete.devCompletionsFolder")
        .filter(|d| !d.is_empty())?;
    if !settings.bool("autocomplete.developerMode", false) {
        return None;
    }
    Some(match Path::new(d).strip_prefix("~") {
        Ok(rest) => {
            crate::settings::user_home().map_or_else(|_| PathBuf::from(d), |h| h.join(rest))
        }
        Err(_) => PathBuf::from(d),
    })
}

/// [`resolve`] over explicit roots (first wins). Only plain relative names: absolute, drive-relative (`C:foo`),
/// `.` and `..` components are rejected.
pub(crate) fn resolve_in(name: &str, roots: &[PathBuf], index: &SpecIndex) -> Option<PathBuf> {
    if name.is_empty()
        || !Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
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

/// Test helpers shared with `js` tests: partial extraction of the real bundle into temp dirs.
#[cfg(all(test, feature = "embed-specs"))]
pub(crate) mod testutil {
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use std::path::Path;

    /// Unpack only `files` (paths inside the bundle, e.g. `git.js`) of the embedded archive into a temp dir.
    #[cfg(feature = "embed-specs")]
    pub fn bundle_subset(files: &[&str]) -> TempDir {
        let dir = TempDir::new("bundle");
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
    pub fn full_bundle() -> TempDir {
        let root = TempDir::new("bundle-all");
        super::extract_to(super::ARCHIVE, root.path(), "x").unwrap();
        root
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
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
        let root = TempDir::new("extract");
        std::fs::create_dir_all(root.path().join("0.9.0")).unwrap();
        last_used(&root.path().join("0.9.0"), Duration::from_secs(8 * 86400));
        std::fs::create_dir_all(root.path().join("user")).unwrap();
        last_used(&root.path().join("user"), Duration::from_secs(8 * 86400));
        let archive = tar_zst(&[
            ("./index.json", r#"{"completions":["git"]}"#),
            ("./aws/s3.js", "x"),
            ("./aws/._s3.js", "\u{0}"),
        ]);
        let dir = extract_to(&archive, root.path(), "1.0.0").unwrap();
        assert_eq!(dir, root.path().join("1.0.0"));
        assert!(dir.join("index.json").is_file() && dir.join("aws/s3.js").is_file());
        assert!(
            !dir.join("aws/._s3.js").exists(),
            "AppleDouble entries skipped"
        );
        assert!(
            !root.path().join("0.9.0").exists(),
            "version unused for a week removed"
        );
        assert!(dir.join(".last-used").is_file(), "new version marked used");
        assert!(root.path().join("user").is_dir(), "user dir kept");
        // Loser of a concurrent extraction: the winner's dir stays, the temp dir is gone.
        std::fs::write(dir.join("marker"), "winner").unwrap();
        assert_eq!(extract_to(&archive, root.path(), "1.0.0").unwrap(), dir);
        assert!(dir.join("marker").is_file());
        let names: Vec<_> = std::fs::read_dir(root.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }

    /// Make `dir` look last used `ago`.
    fn last_used(dir: &Path, ago: Duration) {
        let f = std::fs::File::create(dir.join(LAST_USED)).unwrap();
        f.set_modified(SystemTime::now() - ago).unwrap();
    }

    fn names(root: &Path) -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        n.sort();
        n
    }

    /// A version an older, still-running binary uses is kept; one unused for a week goes.
    #[test]
    fn extract_keeps_versions_in_use() {
        let root = TempDir::new("extract-live");
        for d in ["0.8.0", "0.9.0", "dev", "notes.tmp-1"] {
            std::fs::create_dir_all(root.path().join(d)).unwrap();
        }
        last_used(&root.path().join("0.8.0"), Duration::from_secs(8 * 86400));
        last_used(&root.path().join("0.9.0"), Duration::from_secs(3600));
        // specs-cleanup-deletes-user-dirs: a stale dir tabsmith did not create (dev specs, notes) is the user's
        for d in ["dev", "notes.tmp-1"] {
            last_used(&root.path().join(d), Duration::from_secs(8 * 86400));
        }
        extract_to(&tar_zst(&[("./index.json", "{}")]), root.path(), "1.0.0").unwrap();
        assert_eq!(names(root.path()), ["0.9.0", "1.0.0", "dev", "notes.tmp-1"]);
    }

    /// The temp dir of an extraction that died is removed; a live extractor's (recent) one is kept.
    #[test]
    fn extract_removes_abandoned_temp_dirs() {
        let root = TempDir::new("extract-tmp");
        for d in ["1.0.0.tmp-99999", "1.0.0.tmp-11111"] {
            std::fs::create_dir_all(root.path().join(d)).unwrap();
        }
        last_used(
            &root.path().join("1.0.0.tmp-99999"),
            Duration::from_secs(3600),
        );
        extract_to(&tar_zst(&[("./index.json", "{}")]), root.path(), "1.0.0").unwrap();
        assert_eq!(names(root.path()), ["1.0.0", "1.0.0.tmp-11111"]);
    }

    #[test]
    fn extract_error_cleans_temp() {
        let root = TempDir::new("extract-bad");
        assert!(extract_to(b"not zstd", root.path(), "1.0.0").is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn index_merges_user_stems() {
        let bundle = TempDir::new("index");
        let user = TempDir::new("index-user");
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
        assert_eq!(
            SpecIndex::load_from(bundle.path(), &user.path().join("nope"))
                .unwrap()
                .completions
                .len(),
            2
        );
    }

    #[test]
    fn resolve_order() {
        let t = TempDir::new("resolve");
        let (dev, user, bundle) = (
            t.path().join("dev"),
            t.path().join("user"),
            t.path().join("bundle"),
        );
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
        let idx = SpecIndex {
            completions: vec![],
            diff_versioned: ["az".to_string()].into(),
        };
        let roots = [dev.clone(), user.clone(), bundle.clone()];
        assert_eq!(resolve_in("git", &roots, &idx), Some(user.join("git.js")));
        assert_eq!(
            resolve_in("az", &roots, &idx),
            Some(bundle.join("az/index.js"))
        );
        assert_eq!(
            resolve_in("aws/s3", &roots, &idx),
            Some(bundle.join("aws/s3.js"))
        );
        assert_eq!(resolve_in("foo", &roots, &idx), Some(dev.join("foo.js")));
        assert_eq!(
            resolve_in("foo", &roots[1..], &idx),
            Some(bundle.join("foo/index.js"))
        );
        assert_eq!(resolve_in("gcloud/alpha", &roots, &idx), None);
        assert_eq!(resolve_in("../bundle/git", &roots, &idx), None);
        assert_eq!(resolve_in("/etc/passwd", &roots, &idx), None);
    }

    /// `C:name` joined to a root replaces it with a path relative to drive C's cwd (here: the process cwd).
    #[cfg(windows)]
    #[test]
    fn resolve_rejects_drive_relative_names() {
        let cwd = std::env::current_dir().unwrap();
        let drive = cwd.to_string_lossy().chars().next().unwrap();
        let stem = format!("tabsmith-drive-rel-{}", std::process::id());
        let planted = cwd.join(format!("{stem}.js"));
        std::fs::write(&planted, "").unwrap();
        let root = TempDir::new("drive");
        let r = resolve_in(
            &format!("{drive}:{stem}"),
            &[root.path().to_path_buf()],
            &SpecIndex::default(),
        );
        let _ = std::fs::remove_file(&planted);
        assert_eq!(r, None);
    }

    #[test]
    fn dev_folder_expands_tilde() {
        let s = |dev: bool, d: &str| {
            let m = [
                ("autocomplete.developerMode", dev.into()),
                ("autocomplete.devCompletionsFolder", d.into()),
            ];
            Settings::from_map(
                m.into_iter()
                    .map(|(k, v): (&str, serde_json::Value)| (k.to_string(), v))
                    .collect(),
            )
        };
        let home = crate::settings::user_home().unwrap();
        assert_eq!(
            dev_folder(&s(true, "~/code/build")),
            Some(home.join("code").join("build"))
        );
        assert_eq!(dev_folder(&s(true, "~")), Some(home));
        assert_eq!(
            dev_folder(&s(true, "~user/x")),
            Some(PathBuf::from("~user/x")),
            "only a whole `~` component"
        );
        assert_eq!(
            dev_folder(&s(true, "/abs/dev")),
            Some(PathBuf::from("/abs/dev"))
        );
        assert_eq!(dev_folder(&s(false, "/abs/dev")), None);
        assert_eq!(dev_folder(&s(true, "")), None);
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
