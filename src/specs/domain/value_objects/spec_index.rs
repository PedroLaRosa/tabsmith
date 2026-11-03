use std::collections::HashSet;

/// `index.json` of the bundled set ∪ `*.js` stems in the user dir.
#[derive(Clone, Debug, Default)]
pub(crate) struct SpecIndex {
    /// Root spec names (e.g. `git`, `aws`, `@withfig/autocomplete-tools`).
    completions: Vec<String>,
    /// Diff-versioned spec names (`az`, `heroku`, …) → load `<name>/index.js`.
    diff_versioned: HashSet<String>,
}

impl SpecIndex {
    pub(crate) fn new(completions: Vec<String>, diff_versioned: HashSet<String>) -> Self {
        SpecIndex {
            completions,
            diff_versioned,
        }
    }

    pub(crate) fn completions(&self) -> &[String] {
        &self.completions
    }

    pub(crate) fn diff_versioned(&self) -> &HashSet<String> {
        &self.diff_versioned
    }
}
