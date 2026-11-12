use crate::modules::specs::domain::value_objects::spec_index::SpecIndex;
use crate::modules::specs::domain::value_objects::spec_location::SpecLocation;
use std::path::{Component, Path, PathBuf};

/// Where the spec for `name` lives (SPEC §6.3 `spec_location`). `is_script`: `None` for the first token,
/// `Some(false)` for `isCommand` args, `Some(true)` for `isScript` args.
pub(crate) fn spec_location(name: &str, cwd: &Path, is_script: Option<bool>) -> SpecLocation {
    let (path, base) = name.split_at(name.rfind('/').map_or(0, |i| i + 1));
    let global = match is_script {
        None if name == "bin/console" || name.ends_with("/bin/console") => {
            return SpecLocation::Global {
                name: "php/bin-console".into(),
            }
        }
        None => !path.contains('/'),
        Some(false) => !["/", "./", "~/"].iter().any(|p| path.starts_with(p)),
        Some(true) => false,
    };
    if global {
        return SpecLocation::Global { name: name.into() };
    }
    let dir = if path.starts_with('/') || path.starts_with("~/") {
        PathBuf::from(path)
    } else {
        cwd.join(path.strip_prefix("./").unwrap_or(path))
    };
    SpecLocation::Local {
        dir,
        name: base.into(),
    }
}

/// The file of a global spec name in `roots` (first wins): `<name>.js`, else `<name>/index.js`, in one root before
/// the next; a diff-versioned name only ever loads `<name>/index.js`. Only plain relative names: absolute,
/// drive-relative (`C:foo`), `.` and `..` components are rejected. `is_file` is the file-system probe.
pub(crate) fn find_global_spec(
    name: &str,
    roots: &[PathBuf],
    index: &SpecIndex,
    is_file: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if name.is_empty()
        || !Path::new(name)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let diff = index.diff_versioned().contains(name);
    roots.iter().find_map(|root| {
        let file = root.join(format!("{name}.js"));
        if !diff && is_file(&file) {
            return Some(file);
        }
        Some(root.join(name).join("index.js")).filter(|f| is_file(f))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::slice;

    /// An `is_file` probe that answers yes for exactly `paths`.
    fn files(paths: &[PathBuf]) -> impl Fn(&Path) -> bool {
        let planted: HashSet<PathBuf> = paths.iter().cloned().collect();
        move |path| planted.contains(path)
    }

    #[test]
    fn should_look_through_one_root_before_the_next() {
        let (user, bundle) = (PathBuf::from("user"), PathBuf::from("bundle"));
        let is_file = files(&[user.join("foo").join("index.js"), bundle.join("foo.js")]);

        let found = find_global_spec(
            "foo",
            &[user.clone(), bundle],
            &SpecIndex::default(),
            is_file,
        );

        assert_eq!(found, Some(user.join("foo").join("index.js")));
    }

    #[test]
    fn should_find_no_spec_for_an_empty_a_parent_or_an_absolute_name() {
        let root = PathBuf::from("bundle");
        let is_file = files(&[
            root.join(".js"),
            root.join("../x.js"),
            PathBuf::from("/x.js"),
        ]);
        let roots = [root];

        let found = ["", "../x", "/x"]
            .map(|name| find_global_spec(name, &roots, &SpecIndex::default(), &is_file));

        assert_eq!(found, [None, None, None]);
    }

    #[test]
    fn should_load_a_diff_versioned_name_from_its_index_file_beside_a_plain_file() {
        let bundle = PathBuf::from("bundle");
        let is_file = files(&[bundle.join("az.js"), bundle.join("az").join("index.js")]);
        let index = SpecIndex::new(vec![], ["az".to_string()].into());

        let found = find_global_spec("az", slice::from_ref(&bundle), &index, is_file);

        assert_eq!(found, Some(bundle.join("az").join("index.js")));
    }

    #[test]
    fn should_never_probe_the_plain_file_of_a_diff_versioned_name() {
        let (user, bundle) = (PathBuf::from("user"), PathBuf::from("bundle"));
        let index = SpecIndex::new(vec![], ["az".to_string()].into());
        let probed = RefCell::new(Vec::new());

        find_global_spec(
            "az",
            &[user.clone(), bundle.clone()],
            &index,
            |path: &Path| {
                probed.borrow_mut().push(path.to_path_buf());
                false
            },
        );

        assert_eq!(
            probed.into_inner(),
            [
                user.join("az").join("index.js"),
                bundle.join("az").join("index.js")
            ]
        );
    }

    #[test]
    fn should_take_the_spec_of_the_first_root_that_holds_it() {
        let (user, bundle) = (PathBuf::from("user"), PathBuf::from("bundle"));
        let is_file = files(&[user.join("git.js"), bundle.join("git.js")]);

        let found = find_global_spec(
            "git",
            &[user.clone(), bundle],
            &SpecIndex::default(),
            is_file,
        );

        assert_eq!(found, Some(user.join("git.js")));
    }

    #[test]
    fn should_prefer_the_plain_file_to_the_index_file_of_one_root() {
        let bundle = PathBuf::from("bundle");
        let is_file = files(&[bundle.join("foo.js"), bundle.join("foo").join("index.js")]);

        let found = find_global_spec(
            "foo",
            slice::from_ref(&bundle),
            &SpecIndex::default(),
            is_file,
        );

        assert_eq!(found, Some(bundle.join("foo.js")));
    }

    #[test]
    fn should_find_a_multi_segment_name() {
        let bundle = PathBuf::from("bundle");
        let is_file = files(&[bundle.join("aws/s3.js")]);

        let found = find_global_spec(
            "aws/s3",
            slice::from_ref(&bundle),
            &SpecIndex::default(),
            is_file,
        );

        assert_eq!(found, Some(bundle.join("aws/s3.js")));
    }

    #[test]
    fn should_find_nothing_when_no_root_holds_the_spec() {
        let is_file = files(&[PathBuf::from("bundle").join("git.js")]);

        let found = find_global_spec(
            "gcloud/alpha",
            &[PathBuf::from("bundle")],
            &SpecIndex::default(),
            is_file,
        );

        assert_eq!(found, None);
    }

    #[test]
    fn spec_location_table() {
        let cwd = Path::new("test_cwd");
        let g = |n: &str| SpecLocation::Global { name: n.into() };
        let l = |d: &str, n: &str| SpecLocation::Local {
            dir: d.into(),
            name: n.into(),
        };
        assert_eq!(spec_location("git", cwd, None), g("git"));
        assert_eq!(
            spec_location("@tabsmith/autocomplete-tools", cwd, Some(false)),
            g("@tabsmith/autocomplete-tools")
        );
        assert_eq!(
            spec_location("@tabsmith/autocomplete-tools", cwd, None),
            l("test_cwd/@tabsmith/", "autocomplete-tools")
        );
        assert_eq!(spec_location("./test", cwd, None), l("test_cwd/", "test"));
        assert_eq!(spec_location("~/test", cwd, None), l("~/", "test"));
        assert_eq!(spec_location("/test", cwd, None), l("/", "test"));
        assert_eq!(spec_location("/dir/test", cwd, None), l("/dir/", "test"));
        assert_eq!(spec_location("~/dir/test", cwd, None), l("~/dir/", "test"));
        assert_eq!(
            spec_location("./dir/test", cwd, None),
            l("test_cwd/dir/", "test")
        );
        assert_eq!(
            spec_location("bin/console", cwd, None),
            g("php/bin-console")
        );
        assert_eq!(
            spec_location("vendor/bin/console", cwd, None),
            g("php/bin-console")
        );
        assert_eq!(
            spec_location("./run.sh", cwd, Some(false)),
            l("test_cwd/", "run.sh")
        );
        assert_eq!(
            spec_location("script", cwd, Some(true)),
            l("test_cwd", "script")
        );
    }
}
