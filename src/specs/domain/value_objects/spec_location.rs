use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SpecLocation {
    /// A bundled/user spec by global name, e.g. `git`, `aws/s3`.
    Global { name: String },
    /// `<dir>/.fig/autocomplete/build/<name>.js`.
    Local { dir: PathBuf, name: String },
}

impl SpecLocation {
    /// Cache key `type,path,name`. Caches key on it, never on `SpecLocation`: `PathBuf` equality merges `/p/`
    /// and `/p`, while the key keeps the trailing slash.
    pub(crate) fn cache_key(&self) -> String {
        match self {
            SpecLocation::Global { name } => format!("global,,{name}"),
            SpecLocation::Local { dir, name } => format!("local,{},{name}", dir.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_key_a_global_location_by_its_name() {
        let location = SpecLocation::Global { name: "git".into() };

        let key = location.cache_key();

        assert_eq!(key, "global,,git");
    }

    #[test]
    fn should_keep_the_trailing_slash_of_a_local_dir_in_its_key() {
        let location = SpecLocation::Local {
            dir: "/cwd/dir/".into(),
            name: "test".into(),
        };

        let key = location.cache_key();

        assert_eq!(key, "local,/cwd/dir/,test");
    }
}
