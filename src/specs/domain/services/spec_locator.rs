use crate::specs::domain::value_objects::spec_location::SpecLocation;
use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;

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
            spec_location("@withfig/autocomplete-tools", cwd, Some(false)),
            g("@withfig/autocomplete-tools")
        );
        assert_eq!(
            spec_location("@withfig/autocomplete-tools", cwd, None),
            l("test_cwd/@withfig/", "autocomplete-tools")
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
