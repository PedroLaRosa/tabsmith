use crate::modules::specs::domain::value_objects::spec_suggestion::SpecSuggestion;
#[cfg(test)]
use std::collections::HashMap;
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

/// The native `filepaths`/`folders` template listing (SPEC §6.6.4).
pub(crate) trait DirectoryLister {
    /// The entries of `dir` with `ls -1ApL` semantics: folders end in `/`, symlinks are followed, `.DS_Store` and
    /// names holding a control char are skipped, dot names sort last, and `../` comes after them; `files: false`
    /// keeps only the folders. An unreadable dir lists nothing, not even `../`. Synchronous: the rows are ready in
    /// the same call, so template rows reach the Parse reply.
    fn list(&self, dir: &Path, files: bool, is_dangerous: bool) -> Vec<SpecSuggestion>;
}

/// Test double: lists the rows it was given for a directory, as given. It ignores `files` and `is_dangerous`; that
/// policy belongs to the adapter.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct InMemoryDirectoryLister {
    listings: HashMap<PathBuf, Vec<SpecSuggestion>>,
}

#[cfg(test)]
impl InMemoryDirectoryLister {
    pub(crate) fn with_listing(mut self, dir: impl Into<PathBuf>, rows: Vec<SpecSuggestion>) -> Self {
        self.listings.insert(dir.into(), rows);
        self
    }
}

#[cfg(test)]
impl DirectoryLister for InMemoryDirectoryLister {
    fn list(&self, dir: &Path, _files: bool, _is_dangerous: bool) -> Vec<SpecSuggestion> {
        self.listings.get(dir).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_list_the_entries_it_was_given_for_a_directory() {
        let rows = vec![
            SpecSuggestion::default().with_names(vec!["src/".into()]),
            SpecSuggestion::default().with_names(vec!["../".into()]),
        ];
        let lister = InMemoryDirectoryLister::default().with_listing("/work", rows.clone());

        let listed = lister.list(Path::new("/work"), true, false);

        assert_eq!(listed, rows);
    }

    #[test]
    fn should_list_nothing_for_a_directory_it_was_not_given() {
        let lister = InMemoryDirectoryLister::default().with_listing(
            "/work",
            vec![SpecSuggestion::default().with_names(vec!["src/".into()])],
        );

        let rows = lister.list(Path::new("/elsewhere"), true, false);

        assert_eq!(rows, Vec::<SpecSuggestion>::new());
    }
}
