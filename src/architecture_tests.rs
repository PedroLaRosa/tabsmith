//! Architecture fitness test (plan section 1): every registered module keeps the hexagonal layer rules.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Modules that follow the hexagonal layout; each phase registers the module it finishes. `bootstrap` is the
/// composition root and is never registered.
const REGISTERED_MODULES: &[&str] = &[
    "completion", "diagnostics", "dropdown", "ghost_text", "history", "settings", "shared", "shell_integration", "specs",
    "terminal_session",
];

/// The flat files the refactor is emptying (`src/<name>.rs`). Domain and application never use them: they reach
/// serde, I/O and the QuickJS runtime by the back door.
// ponytail: a name stays listed after its file is deleted (harmless, nothing can use it then). `specs` is not listed:
// it is the registered module now, not the legacy `specs.rs` it replaced in Phase 5.
const LEGACY_MODULES: &[&str] = &[
    "cli",
    "doctor",
    "engine",
    "icons",
    "inline",
    "integrations",
    "js",
    "keys",
    "menu",
    "parser",
    "pty",
    "render",
    "shell_parser",
    "spec",
    "suggest",
    "tracker",
    "types",
];

/// External crates the inner layers may use. std, core and alloc are not in Cargo.toml, so they are never flagged.
const ALLOWED_EXTERNAL_CRATES: &[&str] = &["unicode_width"];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    DomainLayer,
    ApplicationLayer,
    ModuleBoundary,
    SharedKernel,
    ModuleDeclarations,
    SplicedCode,
}

impl Rule {
    fn description(self) -> &'static str {
        match self {
            Rule::DomainLayer => {
                "domain uses application, infrastructure, bootstrap, a legacy module, an external crate, an output macro or the \
                 logger"
            }
            Rule::ApplicationLayer => {
                "application uses infrastructure, bootstrap, a legacy module, an external crate or an output macro (print!, \
                 dbg!, ..)"
            }
            Rule::ModuleBoundary => "uses another module's use case (or use case container) or infrastructure, or bootstrap",
            Rule::SharedKernel => "shared uses a crate module other than shared",
            Rule::ModuleDeclarations => "a mod.rs imports, re-exports or holds an item; it only declares its children",
            Rule::SplicedCode => {
                "compiles in a file the rules never read (#[path], include!) or hands a macro to other modules \
                 (#[macro_export], #[macro_use])"
            }
        }
    }
}

#[derive(Debug, PartialEq)]
struct Violation {
    line: usize,
    rule: Rule,
}

/// One `src/<file>:<line>: <rule>: <code>` entry per violation in the registered modules under `src`.
fn registered_module_violations(src: &Path, external_crates: &[String]) -> Vec<String> {
    let mut report = Vec::new();
    for module in REGISTERED_MODULES {
        for path in misplaced_files(src, module) {
            report.push(format!("src/{path}: {MISPLACED_FILE}"));
        }
        for file in rust_files(&src.join(module)) {
            let path = relative_path(src, &file);
            let source = fs::read_to_string(&file).unwrap_or_else(|err| panic!("read {path}: {err}"));
            let lines: Vec<&str> = source.lines().collect();
            for Violation { line, rule } in violations(&path, &source, external_crates) {
                let code = lines[line - 1].trim();
                report.push(format!("src/{path}:{line}: {}: {code}", rule.description()));
            }
        }
    }
    report
}

const MISPLACED_FILE: &str =
    "outside a layer folder, where the layer rules cannot read its layer: a module is src/<module>/mod.rs plus its layers";

/// The files of `module` that the layer rules, which read the layer from the folder name, would never check. The
/// plan's layout keeps a module in `src/<module>/`, with only `mod.rs` outside a layer folder.
// ponytail: that `mod.rs` gets rules 3-4 and 6 only, which is enough because rule 6 lets any mod.rs hold only
// attributes and child declarations.
fn misplaced_files(src: &Path, module: &str) -> Vec<String> {
    let root_file = src.join(format!("{module}.rs"));
    let module_files = rust_files(&src.join(module));
    let misplaced = root_file.exists().then_some(root_file).into_iter().chain(module_files);
    misplaced
        .map(|file| relative_path(src, &file))
        .filter(|path| !in_the_module_layout(path))
        .collect()
}

const LAYERS: &[&str] = &["domain", "application", "infrastructure"];

/// `path` (relative to `src`) is `<module>/mod.rs` or a file somewhere inside `<module>/<layer>/`.
fn in_the_module_layout(path: &str) -> bool {
    let mut parts = path.split('/').skip(1);
    match (parts.next(), parts.next()) {
        (Some("mod.rs"), None) => true,
        (Some(folder), Some(_)) => LAYERS.contains(&folder),
        _ => false,
    }
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| panic!("read {}: {err}", dir.display()));
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            files.extend(rust_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// `file` relative to `src`, `/`-separated on every platform.
fn relative_path(src: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(src).expect("file under src");
    let components: Vec<_> = relative.components().map(|part| part.as_os_str().to_string_lossy()).collect();
    components.join("/")
}

/// The rules `source` breaks, line by line; `path` is relative to `src` and `/`-separated (`<module>/<layer>/..`).
/// Test-only code keeps the shared kernel and mod.rs rules only.
fn violations(path: &str, source: &str, external_crates: &[String]) -> Vec<Violation> {
    let mut components = path.split('/');
    let module = components.next().unwrap_or_default();
    let layer = components.next();
    // ponytail: a `tests.rs` or `<what>_tests.rs` file below a module is taken to be the body of a `#[cfg(test)]
    // mod ...;` (the plan's sibling test file); read the parent's declaration if a production file is ever named so.
    let test_file = path.ends_with("/tests.rs") || (path.contains('/') && path.ends_with("_tests.rs"));
    let mut open_braces = Vec::new();
    let mut test_only_item = TestOnlyItem::default();
    let imported_names = ImportedNames::in_source(source);
    let mut found = Vec::new();
    for (index, line) in source.lines().enumerate() {
        let code = without_line_comment(line);
        test_only_item.open_at(code, open_braces.len());
        let paths = paths_in(code, &mut open_braces);
        let production = !test_only_item.holds(code, open_braces.len()) && !test_file;
        let resolved = imported_names.resolve(&paths);
        let checks = [
            (
                Rule::DomainLayer,
                production && layer == Some("domain") && breaks_domain_layer(code, &paths, external_crates),
            ),
            (
                Rule::ApplicationLayer,
                production && layer == Some("application") && breaks_application_layer(code, &paths, external_crates),
            ),
            (
                Rule::ModuleBoundary,
                production
                    && (crosses_module_boundary(module, &resolved) || reaches_the_composition_root(layer, &resolved)),
            ),
            (Rule::SharedKernel, module == "shared" && leaves_the_shared_kernel(&paths)),
            (Rule::ModuleDeclarations, path.ends_with("/mod.rs") && !only_declares_children(code)),
            (Rule::SplicedCode, splices_in_unread_code(code)),
        ];
        let broken = checks.into_iter().filter(|(_, is_broken)| *is_broken);
        found.extend(broken.map(|(rule, _)| Violation { line: index + 1, rule }));
    }
    found
}

/// The `#[cfg(test)]` item being read: from its attribute to the brace that closes its body, or, for a bodyless item
/// (a field, a variant, a match arm, a statement), to its `;` or `,` or the brace that closes the block around it.
// ponytail: a test-only fn with a multi-line `where` clause ends at the clause's first `,`; skip `where` clauses
// here if one ever shows up.
#[derive(Default)]
struct TestOnlyItem {
    /// Brace depth at the attribute; `None` outside a test-only item.
    depth: Option<usize>,
    has_body: bool,
    /// `(` and `[` opened in the item and not closed yet: a `,` inside a multi-line signature does not end it.
    open_groups: isize,
}

impl TestOnlyItem {
    /// Before the line's braces are read: a test-only attribute opens an item at the current brace depth.
    fn open_at(&mut self, code: &str, depth: usize) {
        if self.depth.is_none() && is_test_only_attribute(code) {
            *self = TestOnlyItem { depth: Some(depth), ..Self::default() };
        }
    }

    /// After the line's braces are read: whether the line belongs to the item. Its last line closes it.
    fn holds(&mut self, code: &str, depth: usize) -> bool {
        let Some(start) = self.depth else {
            return false;
        };
        self.has_body |= code.contains('{');
        self.open_groups += code.matches(['(', '[']).count() as isize - code.matches([')', ']']).count() as isize;
        let ends = if self.has_body {
            depth <= start
        } else {
            depth < start || (self.open_groups <= 0 && code.trim_end().ends_with([';', ',']))
        };
        if ends {
            self.depth = None;
        }
        true
    }
}

/// The short names crate paths bring in: after `use crate::history;`, `use crate::history::{self, ..}` or
/// `use crate::history::application;`, `history::..` and `application::..` read as the crate paths they stand for.
// ponytail: every crate path's last segment is recorded, not only those on `use` lines, file-wide rather than per
// scope (a name imported twice reads as its last import everywhere); `as` renames and glob imports are not followed.
#[derive(Default)]
struct ImportedNames<'a> {
    full_paths: HashMap<&'a str, Vec<&'a str>>,
}

impl<'a> ImportedNames<'a> {
    /// The names `source` imports, read before any line is checked: a `use` may come after the code that uses it.
    fn in_source(source: &'a str) -> Self {
        let mut imported_names = Self::default();
        let mut open_braces = Vec::new();
        for line in source.lines() {
            imported_names.record(&paths_in(without_line_comment(line), &mut open_braces));
        }
        imported_names
    }

    fn record(&mut self, paths: &[Vec<&'a str>]) {
        for path in paths.iter().filter(|path| path.first() == Some(&"crate")) {
            let full_path = path.strip_suffix(&["self"]).unwrap_or(path);
            if let Some(name) = full_path.last() {
                self.full_paths.insert(name, full_path.to_vec());
            }
        }
    }

    /// `paths` with an imported name at the start replaced by the crate path it stands for.
    fn resolve(&self, paths: &[Vec<&'a str>]) -> Vec<Vec<&'a str>> {
        let resolve = |path: &Vec<&'a str>| match self.full_paths.get(path[0]) {
            Some(full_path) => full_path.iter().chain(&path[1..]).copied().collect(),
            None => path.clone(),
        };
        paths.iter().map(resolve).collect()
    }
}

/// `line` up to its `//` comment; the `//` of a `scheme://` URL does not start one.
fn without_line_comment(line: &str) -> &str {
    let comment = line.match_indices("//").find(|(start, _)| !line[..*start].ends_with(':'));
    comment.map_or(line, |(start, _)| &line[..start])
}

/// `#[cfg(test)]` or `#[cfg(all(test, ..))]`: the item it marks is compiled only for tests.
fn is_test_only_attribute(code: &str) -> bool {
    let code = code.trim_start();
    code.starts_with("#[cfg(test)]") || code.starts_with("#[cfg(all(test,")
}

/// The segments of every `a::b::c` path in `code`; `::serde::X` and `serde::{` both start at `serde`. A member of
/// a `use a::{..}` group gets the `a` prefix, and the name right after `use` or `extern crate` is a path even
/// without `::`.
/// `open_braces` holds each open brace's prefix across lines (empty for a block).
// ponytail: a line scanner, not a Rust parser. String literals are read as code (a `//` in one cuts the line unless
// it follows `:`, and an unpaired brace, bracket or parenthesis in one shifts the use-group and test-only item
// tracking), only `//` comments are skipped, and `super::`-relative paths are not resolved. Switch to a real parser
// if a false hit or a miss ever shows up.
fn paths_in<'a>(code: &'a str, open_braces: &mut Vec<Vec<&'a str>>) -> Vec<Vec<&'a str>> {
    let mut paths = Vec::new();
    let mut previous_words = ("", "");
    for piece in code.split_inclusive(|c: char| !is_path_char(c)) {
        let chunk = piece.trim_end_matches(|c: char| !is_path_char(c));
        let imported = matches!(previous_words, (_, "use") | ("extern", "crate"));
        if !chunk.is_empty() {
            previous_words = (previous_words.1, chunk);
        }
        let segments: Vec<&str> = chunk.split("::").filter(|segment| !segment.is_empty()).collect();
        if piece.ends_with("::{") {
            open_braces.push(segments);
            continue;
        }
        let in_use_group = open_braces.iter().any(|prefix| !prefix.is_empty());
        if !segments.is_empty() && (chunk.contains("::") || in_use_group || imported) {
            paths.push(open_braces.concat().into_iter().chain(segments).collect());
        }
        if piece.ends_with('{') {
            open_braces.push(Vec::new());
        } else if piece.ends_with('}') {
            open_braces.pop();
        }
    }
    paths
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == ':'
}

/// The domain keeps the application rule and uses neither the application layer nor the logger.
fn breaks_domain_layer(code: &str, paths: &[Vec<&str>], external_crates: &[String]) -> bool {
    breaks_application_layer(code, paths, external_crates)
        || paths.iter().any(|path| path.contains(&"application") || path.contains(&"logger"))
}

/// std plus unicode-width: no infrastructure, no composition root, no legacy module, no external crate, no output.
fn breaks_application_layer(code: &str, paths: &[Vec<&str>], external_crates: &[String]) -> bool {
    writes_output(code)
        || paths.iter().any(|path| {
            path.contains(&"infrastructure")
                || is_the_composition_root(path)
                || is_a_legacy_module(path)
                || is_forbidden_external_crate(path[0], external_crates)
        })
}

fn is_a_legacy_module(path: &[&str]) -> bool {
    matches!(path, ["crate", root, ..] if LEGACY_MODULES.contains(root))
}

/// `print!`, `println!`, `eprint!`, `eprintln!` or `dbg!`, bare or by path (`std::println!`).
// ponytail: macro names only. std is allowed, so output through a `std::io` handle (`writeln!(std::io::stderr(),
// ..)`, `stdout().write_all(..)`) passes; flag `io::stdout`/`io::stderr` paths if domain or application writes one.
fn writes_output(code: &str) -> bool {
    macro_names(code).any(|name| matches!(name, "print" | "println" | "eprint" | "eprintln" | "dbg"))
}

/// The name of every macro `code` calls, without its path (`std::println!` is `println`).
fn macro_names(code: &str) -> impl Iterator<Item = &str> {
    let calls = code.split_inclusive(|c: char| !is_path_char(c)).filter_map(|piece| piece.strip_suffix('!'));
    calls.map(|path| path.rsplit("::").next().unwrap_or(path))
}

/// Nothing depends on bootstrap (plan section 2). Rules 1-2 already report it from domain and application files.
fn reaches_the_composition_root(layer: Option<&str>, paths: &[Vec<&str>]) -> bool {
    !matches!(layer, Some("domain" | "application")) && paths.iter().any(|path| is_the_composition_root(path))
}

fn is_the_composition_root(path: &[&str]) -> bool {
    path.starts_with(&["crate", "bootstrap"])
}

fn is_forbidden_external_crate(root: &str, external_crates: &[String]) -> bool {
    !ALLOWED_EXTERNAL_CRATES.contains(&root) && external_crates.iter().any(|name| name == root)
}

/// Another module's use cases and adapters are reached only through bootstrap. The shared kernel is open to every
/// module; rules 1-2 still keep its infrastructure out of domain and application files. A use case is its file
/// (`*_use_case`, `*_use_cases`) or its type (`*UseCase`, `*UseCases`, the container of a module's use cases)
/// anywhere under another module, so a use case re-exported under its own
/// type name is caught at the module root (`crate::specs::InstallSpecBundleUseCase`) or in a legacy shim
/// (`crate::types::InstallSpecBundleUseCase`) too. An adapter has no name suffix, so only its `infrastructure`
/// segment gives it away.
// ponytail: a use case re-exported under another name (`type Installer = ..UseCase;`, a wrapper fn), an `as` rename,
// a `super::` path, and an adapter re-exported by a legacy file (`crate::types::X` from a
// `pub(crate) use crate::specs::infrastructure::..` in a flat file such as the retired src/types.rs) still pass; read every file of the module for
// those, as rule 6 reads mod.rs, and the legacy files for `pub(crate) use crate::<module>::..`, if one shows up.
fn crosses_module_boundary(own_module: &str, paths: &[Vec<&str>]) -> bool {
    paths.iter().any(|path| match path.as_slice() {
        ["crate", "shared", ..] => false,
        ["crate", module, "infrastructure", ..] => *module != own_module,
        ["crate", module, rest @ ..] => {
            *module != own_module
                && rest.iter().any(|segment| segment.contains("_use_case") || segment.contains("UseCase"))
        }
        _ => false,
    })
}

/// The kernel never imports a feature or legacy module.
fn leaves_the_shared_kernel(paths: &[Vec<&str>]) -> bool {
    paths.iter().any(|path| matches!(path.as_slice(), ["crate", module, ..] if *module != "shared"))
}

/// Blank, attributes, or a child declaration after them (`mod x;`, `pub(crate) mod x;`): a mod.rs that re-exports,
/// aliases (`type X = ..;`) or wraps (`fn`) an item gives it a path without the file rule 3 reads.
// ponytail: one-line attributes only; a multi-line `#[cfg(..)]` reads as an item, so join its lines if one lands here.
fn only_declares_children(code: &str) -> bool {
    let code = code.trim();
    let item = match code.strip_prefix('#') {
        Some(_) => code.rsplit_once(']').map_or("", |(_, item)| item.trim_start()),
        None => code,
    };
    let declaration = item.strip_prefix("pub(crate) ").unwrap_or(item);
    let child = declaration.strip_prefix("mod ").and_then(|rest| rest.strip_suffix(';'));
    item.is_empty() || child.is_some_and(|name| name.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// `#[path = ..]` and `include!` compile a file the rules never read into the module; `#[macro_export]` and
/// `#[macro_use]` hand a macro to modules that call it as `crate::name!` or `name!`, a path rules 1-3 cannot read.
// ponytail: only the registered modules and main.rs are read, so a macro exported from bootstrap or a legacy file
// still passes, and so do `#[cfg_attr(.., path = ..)]` and a space after `#[` or before `!`. Read every file under
// src for these if one shows up.
fn splices_in_unread_code(code: &str) -> bool {
    let attributes = ["#[path", "#[macro_export", "#[macro_use"];
    attributes.iter().any(|attribute| code.contains(attribute)) || macro_names(code).any(|name| name == "include")
}

/// The lines of `main_rs` every module would see as `crate::<name>`, a path rules 1-3 cannot read as the shared
/// infrastructure or bootstrap: a crate-root item other than a child declaration or `fn main` (a `type` alias, a
/// wrapper fn), and a `use` or a splice (`#[path]`, `include!`, a macro handed on) anywhere.
// ponytail: braces are counted per line, so an unpaired brace in a string literal shifts what counts as the crate root.
fn crate_root_back_doors(main_rs: &str) -> Vec<&str> {
    let mut depth = 0;
    let mut found = Vec::new();
    for line in main_rs.lines() {
        let code = without_line_comment(line);
        let root_item = depth == 0 && !only_declares_children(code) && !code.starts_with("fn main(");
        if root_item || imports_or_re_exports(code) || splices_in_unread_code(code) {
            found.push(line);
        }
        depth += code.matches('{').count() as isize - code.matches('}').count() as isize;
    }
    found
}

/// A `use` item (`use`, `pub(crate) use`, a glob).
fn imports_or_re_exports(code: &str) -> bool {
    code.split(|c: char| !is_path_char(c)).any(|word| word == "use")
}

/// Crate names (`-` read as `_`) from `[dependencies]` and every `[target.*.dependencies]` table.
fn external_crates(cargo_toml: &str) -> Vec<String> {
    let mut in_dependency_table = false;
    let mut names = Vec::new();
    for line in cargo_toml.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependency_table = line == "[dependencies]"
                || (line.starts_with("[target.") && line.ends_with(".dependencies]"));
        } else if let Some((name, _)) = line.split_once('=').filter(|_| in_dependency_table) {
            names.push(name.trim().replace('-', "_"));
        }
    }
    names
}

mod tests {
    use super::*;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;

    fn violations_in(path: &str, source: &str) -> Vec<Violation> {
        let external_crates = ["anyhow", "serde", "serde_json", "unicode_width"].map(String::from);
        violations(path, source, &external_crates)
    }

    /// A `src` folder holding an empty file at each of `files` (relative, `/`-separated).
    fn source_tree(files: &[&str]) -> TempDir {
        let src = TempDir::new("architecture");
        for file in files {
            let path = src.path().join(file);
            fs::create_dir_all(path.parent().expect("file under src")).expect("create folder");
            fs::write(path, "").expect("write file");
        }
        src
    }

    #[test]
    fn should_keep_every_registered_module_inside_its_layer_rules() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");

        let report = registered_module_violations(&root.join("src"), &external_crates(&cargo_toml));

        assert!(report.is_empty(), "architecture rules broken:\n{}", report.join("\n"));
    }

    /// Every module sees the crate root's private items as `crate::<name>`: a `use crate::shared::..::logger;` or a
    /// `type Cli = ..;` there would reach the shared infrastructure or the composition root by a path rules 1-3 cannot
    /// read as either.
    #[test]
    fn should_keep_use_items_out_of_the_crate_root() {
        let main_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs");
        let source = fs::read_to_string(main_rs).expect("read src/main.rs");

        let back_doors = crate_root_back_doors(&source);

        let message = "src/main.rs holds more than its module declarations and fn main, so every module can use these";
        assert!(back_doors.is_empty(), "{message}:\n{}", back_doors.join("\n"));
    }

    /// A crate-root `type` alias or wrapper fn is a `crate::<name>` path rules 1-3 cannot read, as a `use` item is.
    #[test]
    fn should_flag_a_crate_root_item_other_than_a_module_declaration_or_main() {
        let main_rs = "//! tabsmith.\n\
                       #![allow(dead_code)]\n\
                       \n\
                       #[cfg(test)]\n\
                       mod architecture_tests;\n\
                       mod settings;\n\
                       type SetSetting = settings::application::set_setting_use_case::SetSettingUseCase;\n\
                       fn logger() {}\n\
                       \n\
                       fn main() {\n    \
                       let code = run(crate::shared::infrastructure::logger::init());\n    \
                       use crate::bootstrap::cli::Cli;\n\
                       }\n";

        let found = crate_root_back_doors(main_rs);

        assert_eq!(
            found,
            [
                "type SetSetting = settings::application::set_setting_use_case::SetSettingUseCase;",
                "fn logger() {}",
                "    use crate::bootstrap::cli::Cli;",
            ]
        );
    }

    /// Rule 6 lets a child declaration's attributes through, so the crate root needs the splice check of its own.
    #[test]
    fn should_flag_a_crate_root_line_that_splices_in_a_file_the_rules_never_read() {
        let main_rs = "mod settings;\n\
                       #[path = \"leak.rs\"]\n\
                       mod leak;\n\
                       #[macro_use]\n\
                       mod shared;\n\
                       \n\
                       fn main() {\n    \
                       include!(\"leak.rs\");\n\
                       }\n";

        let found = crate_root_back_doors(main_rs);

        assert_eq!(found, ["#[path = \"leak.rs\"]", "#[macro_use]", "    include!(\"leak.rs\");"]);
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_another_layer() {
        let source = "use super::setting_key::SettingKey;\n\
                      use crate::shared::application::ports::clock::Clock;\n\
                      use crate::settings::infrastructure::cli::SettingsArgs;\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_the_composition_root() {
        let source = "use crate::bootstrap::cli::Cli;\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_flag_a_domain_file_that_uses_an_external_crate() {
        let source = "use serde::Serialize;\n\
                      let width = text.len();\n\
                      let text = serde_json::to_string(&value);\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_domain_file_that_prints_or_logs() {
        let source = "println!(\"{key}\");\n\
                      let key = SettingKey::new(text)?;\n\
                      eprintln!(\"{err}\");\n\
                      crate::logger::warn(\"unknown key\");\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::DomainLayer },
                Violation { line: 3, rule: Rule::DomainLayer },
                Violation { line: 4, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_ignore_references_inside_line_comments() {
        let source = "//! Built by crate::bootstrap, printed with println!.\n\
                      /// Same rules as crate::settings::application::set_setting_use_case.\n\
                      let width = text.len(); // serde::Serialize would leak here\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_read_each_member_of_a_use_group_under_the_group_prefix() {
        let source = "use crate::{\n    \
                      shared::domain::value_objects::shell::Shell,\n    \
                      settings::domain::settings::Settings,\n\
                      };\n\
                      fn render(shell: Shell) {\n\
                      }\n";

        let found = violations_in("shared/infrastructure/json_document_store.rs", source);

        assert_eq!(found, vec![Violation { line: 3, rule: Rule::SharedKernel }]);
    }

    #[test]
    fn should_match_whole_path_segments_only() {
        let source = "use crate::settings::domain::application_state::ApplicationState;\n\
                      let text = my_serde::to_text(&state);\n\
                      let probe = crate::bootstrapping::Probe;\n\
                      let name = event_logger::name();\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_domain_file_that_uses_std_or_unicode_width() {
        let source = "use std::fmt;\n\
                      use unicode_width::UnicodeWidthStr;\n\
                      let width = unicode_width::UnicodeWidthChar::width(c);\n";

        let found = violations_in("dropdown/domain/menu.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_an_application_file_that_uses_infrastructure_or_the_composition_root() {
        let source = "use crate::settings::domain::settings::Settings;\n\
                      use crate::settings::infrastructure::cli::SettingsArgs;\n\
                      use crate::bootstrap::cli::Cli;\n";

        let found = violations_in("settings/application/set_setting_use_case.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::ApplicationLayer },
                Violation { line: 3, rule: Rule::ApplicationLayer },
            ]
        );
    }

    #[test]
    fn should_flag_an_application_file_that_uses_an_external_crate_other_than_unicode_width() {
        let source = "use anyhow::Result;\n\
                      use unicode_width::UnicodeWidthStr;\n\
                      let value = serde_json::json!({});\n";

        let found = violations_in("settings/application/set_setting_use_case.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 1, rule: Rule::ApplicationLayer },
                Violation { line: 3, rule: Rule::ApplicationLayer },
            ]
        );
    }

    #[test]
    fn should_flag_a_file_that_uses_another_modules_use_case_or_infrastructure() {
        let source = "use crate::completion::domain::suggestion::Suggestion;\n\
                      use crate::completion::application::rank_suggestions_use_case::RankSuggestionsUseCase;\n\
                      let history = crate::history::infrastructure::jsonl_history_repository::open(path);\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::ModuleBoundary },
                Violation { line: 3, rule: Rule::ModuleBoundary },
            ]
        );
    }

    #[test]
    fn should_flag_a_shared_file_that_uses_a_feature_or_legacy_module() {
        let source = "use crate::shared::application::ports::clock::Clock;\n\
                      use crate::settings::domain::settings::Settings;\n\
                      let now = crate::types::now_ms();\n";

        let found = violations_in("shared/infrastructure/system_clock.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::SharedKernel },
                Violation { line: 3, rule: Rule::SharedKernel },
            ]
        );
    }

    #[test]
    fn should_read_external_crate_names_from_every_dependency_table() {
        let cargo_toml = "[package]\nname = \"tabsmith\"\n\n\
                          [features]\ndefault = [\"embed-specs\"]\n\n\
                          [dependencies]\nanyhow = \"1\"\nunicode-width = \"0.2\"\n\n\
                          [target.'cfg(unix)'.dependencies]\nsignal-hook = \"0.3\"\n\n\
                          [target.'cfg(windows)'.dependencies]\n\
                          windows-sys = { version = \"0.59\", features = [\"Win32_Foundation\"] }\n\n\
                          [profile.release]\nlto = true\n";

        let names = external_crates(cargo_toml);

        assert_eq!(names, ["anyhow", "unicode_width", "signal_hook", "windows_sys"]);
    }

    #[test]
    fn should_not_flag_an_application_file_that_uses_the_domain_and_ports() {
        let source = "use super::ports::spec_bundle_installer::SpecBundleInstaller;\n\
                      use crate::settings::domain::settings::Settings;\n\
                      use crate::shared::application::ports::clock::Clock;\n\
                      use std::rc::Rc;\n";

        let found = violations_in("shell_integration/application/install_integration_use_case.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_another_modules_use_case_imported_as_a_module() {
        let source = "use crate::specs::application::install_spec_bundle_use_case;\n";

        let found = violations_in("shell_integration/infrastructure/shell_integration_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_a_module_that_uses_another_modules_use_case_container() {
        let source = "use crate::terminal_session::application::session_use_cases::SessionUseCases;\n";

        let found = violations_in("completion/infrastructure/completion_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_use_case_re_exported_by_its_type_name_outside_its_file() {
        let source = "use crate::specs::application::installers::InstallSpecBundleUseCase;\n";

        let found = violations_in("shell_integration/infrastructure/shell_integration_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_use_case_re_exported_by_its_type_name_outside_its_application_layer() {
        let source = "use crate::specs::InstallSpecBundleUseCase;\n";

        let found = violations_in("shell_integration/infrastructure/shell_integration_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_infrastructure_reached_through_its_module_imported_as_self() {
        let source = "use crate::history::{self, domain::history_entry::HistoryEntry};\n\
                      let repo = history::infrastructure::jsonl_history_repository::JsonlHistoryRepository::new(p);\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![Violation { line: 2, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_use_case_reached_through_its_imported_module() {
        let source = "use crate::history;\n\
                      let uc = history::application::record_command_use_case::RecordCommandUseCase::new(r);\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![Violation { line: 2, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_use_case_reached_through_its_imported_application_layer() {
        let source = "use crate::history::application;\n\
                      let uc = application::record_command_use_case::RecordCommandUseCase::new(r);\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![Violation { line: 2, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_another_modules_infrastructure_reached_by_a_short_name_imported_below_it() {
        let source = "let repo = history::infrastructure::jsonl_history_repository::JsonlHistoryRepository::new(p);\n\
                      use crate::history;\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_not_flag_its_own_adapters_or_the_shared_kernel_reached_through_an_imported_module() {
        let source = "use crate::ghost_text::{self, domain::ghost_text::GhostText};\n\
                      use crate::shared;\n\
                      let cache = ghost_text::infrastructure::ghost_text_cache::GhostTextCache::new();\n\
                      shared::infrastructure::logger::warn(\"cache miss\");\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_file_that_uses_its_own_use_cases_or_another_modules_domain_and_ports() {
        let source = "use crate::ghost_text::application::suggest_ghost_text_use_case::SuggestGhostTextUseCase;\n\
                      use crate::ghost_text::infrastructure::ghost_text_cache::GhostTextCache;\n\
                      use crate::completion::domain::suggestion::Suggestion;\n\
                      use crate::history::application::ports::history_source::HistorySource;\n";

        let found = violations_in("ghost_text/infrastructure/ghost_text_module.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_shared_file_that_uses_the_kernel_or_an_external_crate() {
        let source = "use crate::shared::domain::value_objects::data_directory::DataDirectory;\n\
                      use serde_json::Value;\n\
                      let text = std::fs::read_to_string(path)?;\n";

        let found = violations_in("shared/infrastructure/json_document_store.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_flag_a_modules_infrastructure_that_uses_the_shared_infrastructure() {
        let source = "use crate::shared::infrastructure::json_document_store::JsonDocumentStore;\n\
                      crate::shared::infrastructure::logger::warn(\"unknown key\");\n";

        let found = violations_in("settings/infrastructure/json_file_settings_repository.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_a_domain_or_application_file_that_uses_the_shared_infrastructure() {
        let domain = "use crate::shared::infrastructure::atomic_file::AtomicFile;\n";
        let application = "use crate::shared::infrastructure::logger;\n";

        let found = [
            violations_in("settings/domain/settings.rs", domain),
            violations_in("settings/application/set_setting_use_case.rs", application),
        ];

        assert_eq!(
            found,
            [
                vec![Violation { line: 1, rule: Rule::DomainLayer }],
                vec![Violation { line: 1, rule: Rule::ApplicationLayer }],
            ]
        );
    }

    #[test]
    fn should_flag_a_domain_or_application_file_that_uses_a_legacy_module() {
        let domain = "use crate::shared::domain::value_objects::shell::Shell;\n\
                      use crate::types::SessionCtx;\n";
        let application = "use crate::completion::domain::suggestion::Suggestion;\n\
                           let engine = crate::engine::spawn(dependencies);\n";

        let found = [
            violations_in("history/domain/entities/history_entry.rs", domain),
            violations_in("history/application/record_command_use_case.rs", application),
        ];

        assert_eq!(
            found,
            [
                vec![Violation { line: 2, rule: Rule::DomainLayer }],
                vec![Violation { line: 2, rule: Rule::ApplicationLayer }],
            ]
        );
    }

    #[test]
    fn should_not_apply_the_layer_rules_inside_a_test_only_item() {
        let source = "pub(crate) fn width(text: &str) -> usize { text.len() }\n\
                      #[cfg(test)]\n\
                      mod tests {\n    \
                      use serde_json::json;\n    \
                      use crate::completion::infrastructure::spec_loader::SpecLoader;\n    \
                      fn fixture() { println!(\"{}\", json!({})); }\n\
                      }\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_apply_the_layer_rules_again_once_a_test_only_item_ends() {
        let source = "#[cfg(test)]\n\
                      use serde_json::json;\n\
                      use serde::Serialize;\n\
                      #[cfg(all(test, unix))]\n\
                      fn fixture() -> Value { serde_json::json!({}) }\n\
                      let text = serde_json::to_string(&value);\n\
                      #[cfg(test)]\n\
                      #[derive(Default)]\n\
                      struct Recorder {\n    \
                      seen: Vec<serde_json::Value>,\n\
                      }\n\
                      println!(\"{text}\");\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 3, rule: Rule::DomainLayer },
                Violation { line: 6, rule: Rule::DomainLayer },
                Violation { line: 12, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_apply_the_layer_rules_again_after_a_test_only_field() {
        let source = "pub(crate) struct Settings {\n    \
                      #[cfg(test)]\n    \
                      probe: u8,\n    \
                      cache: serde_json::Value,\n\
                      }\n\
                      fn load() -> serde_json::Value { serde_json::Value::Null }\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 4, rule: Rule::DomainLayer },
                Violation { line: 6, rule: Rule::DomainLayer },
            ]
        );
    }

    #[test]
    fn should_apply_the_layer_rules_again_after_a_test_only_variant() {
        let source = "pub(crate) enum MenuEvent {\n    \
                      #[cfg(test)]\n    \
                      Probe,\n    \
                      Open(serde_json::Value),\n\
                      }\n";

        let found = violations_in("dropdown/domain/menu_event.rs", source);

        assert_eq!(found, vec![Violation { line: 4, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_apply_the_layer_rules_again_once_the_block_around_a_test_only_variant_closes() {
        let source = "pub(crate) enum MenuEvent {\n    \
                      #[cfg(test)]\n    \
                      Probe\n\
                      }\n\
                      fn load() -> serde_json::Value { serde_json::Value::Null }\n";

        let found = violations_in("dropdown/domain/menu_event.rs", source);

        assert_eq!(found, vec![Violation { line: 5, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_not_apply_the_layer_rules_inside_a_multi_line_test_only_signature() {
        let source = "#[cfg(test)]\n\
                      fn fixture(\n    \
                      expected: serde_json::Value,\n    \
                      actual: serde_json::Value,\n\
                      ) -> bool {\n    \
                      expected == actual\n\
                      }\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_apply_the_layer_rules_to_the_sibling_tests_file_of_a_test_module() {
        let source = "use super::*;\n\
                      use serde_json::json;\n\
                      use crate::settings::infrastructure::json_file_settings_repository::JsonFileSettingsRepository;\n";

        let found = violations_in("settings/domain/settings/tests.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_not_apply_the_layer_rules_to_a_second_sibling_tests_file_named_after_what_it_tests() {
        let source = "use super::*;\n\
                      use serde_json::json;\n\
                      use crate::settings::infrastructure::json_file_settings_repository::JsonFileSettingsRepository;\n";

        let found = violations_in("settings/domain/settings/execute_tests.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_keep_the_shared_kernel_rule_in_test_only_code() {
        let module = "#[cfg(test)]\n\
                      mod tests {\n    \
                      use crate::settings::Settings;\n\
                      }\n";
        let tests_file = "use crate::integrations::Ctx;\n";

        let found = [
            violations_in("shared/infrastructure/json_document_store.rs", module),
            violations_in("shared/infrastructure/json_document_store/tests.rs", tests_file),
        ];

        assert_eq!(
            found,
            [
                vec![Violation { line: 3, rule: Rule::SharedKernel }],
                vec![Violation { line: 1, rule: Rule::SharedKernel }],
            ]
        );
    }

    #[test]
    fn should_flag_an_infrastructure_file_that_uses_the_composition_root() {
        let source = "use crate::shared::infrastructure::logger;\n\
                      let help = <crate::bootstrap::cli::Cli as clap::Parser>::try_parse_from([\"tabsmith\"]);\n";

        let found = violations_in("settings/infrastructure/cli/settings_command.rs", source);

        assert_eq!(found, vec![Violation { line: 2, rule: Rule::ModuleBoundary }]);
    }

    #[test]
    fn should_flag_a_domain_or_application_file_that_writes_to_stdout_or_stderr() {
        let domain = "print!(\"{key}\");\n\
                      eprint!(\"{err}\");\n\
                      let width = dbg!(text.len());\n\
                      std::println!(\"{width}\");\n";
        let application = "println!(\"{key}\");\n\
                           eprintln!(\"{err}\");\n";

        let found = [
            violations_in("settings/domain/settings.rs", domain),
            violations_in("settings/application/set_setting_use_case.rs", application),
        ];

        assert_eq!(
            found,
            [
                vec![
                    Violation { line: 1, rule: Rule::DomainLayer },
                    Violation { line: 2, rule: Rule::DomainLayer },
                    Violation { line: 3, rule: Rule::DomainLayer },
                    Violation { line: 4, rule: Rule::DomainLayer },
                ],
                vec![
                    Violation { line: 1, rule: Rule::ApplicationLayer },
                    Violation { line: 2, rule: Rule::ApplicationLayer },
                ],
            ]
        );
    }

    #[test]
    fn should_not_take_a_function_or_a_comparison_for_an_output_macro() {
        let source = "fn print(report: &Report) -> String { report.to_string() }\n\
                      let changed = !reprint!(value) != dbg;\n\
                      let text = sprint!(\"{key}\");\n";

        let found = violations_in("settings/application/set_setting_use_case.rs", source);

        assert_eq!(found, vec![]);
    }

    #[test]
    fn should_flag_an_external_crate_imported_under_another_name() {
        let source = "use serde_json as json;\n\
                      let text = json::to_string(&value);\n\
                      use unicode_width;\n";

        let found = violations_in("settings/domain/settings.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_flag_an_external_crate_linked_under_another_name() {
        let source = "extern crate serde as probe_serde;\n\
                      let value: Option<probe_serde::de::IgnoredAny> = None;\n\
                      extern crate unicode_width as width;\n";

        let found = violations_in("shell_integration/domain/value_objects/init_script.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_not_take_the_slashes_of_a_url_for_a_line_comment() {
        let source = "let docs = \"https://fig.io/docs\"; let value = serde_json::Value::Null;\n\
                      let docs = \"https://fig.io/docs\"; // serde_json::Value::Null\n";

        let found = violations_in("specs/domain/spec.rs", source);

        assert_eq!(found, vec![Violation { line: 1, rule: Rule::DomainLayer }]);
    }

    #[test]
    fn should_not_flag_an_infrastructure_file_that_uses_the_application_layer() {
        let source = "use crate::settings::application::set_setting_use_case::SetSettingUseCase;\n";

        let found = violations_in("settings/infrastructure/settings_module.rs", source);

        assert_eq!(found, vec![]);
    }

    /// A re-export would let another module import a use case by a path without its `*_use_case` file (rule 3).
    #[test]
    fn should_flag_a_mod_rs_that_imports_or_re_exports() {
        let source = "pub(crate) mod record_command_use_case;\n\
                      pub(crate) use record_command_use_case::RecordCommandUseCase;\n\
                      pub use adapters::*;\n\
                      use std::rc::Rc;\n";

        let found = violations_in("history/application/mod.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 2, rule: Rule::ModuleDeclarations },
                Violation { line: 3, rule: Rule::ModuleDeclarations },
                Violation { line: 4, rule: Rule::ModuleDeclarations },
            ]
        );
    }

    /// A type alias or a wrapper fn names a use case by another path, as a re-export does.
    #[test]
    fn should_flag_a_mod_rs_that_holds_an_item_other_than_a_child_declaration() {
        let source = "//! The settings application layer.\n\
                      #[cfg(test)]\n\
                      pub(crate) mod testing;\n\
                      mod set_setting_use_case;\n\
                      pub(crate) type SetSetting = set_setting_use_case::SetSettingUseCase;\n\
                      pub(crate) fn set_setting() {}\n\
                      #[cfg(test)] pub(crate) use set_setting_use_case::SetSettingUseCase;\n";

        let found = violations_in("settings/application/mod.rs", source);

        assert_eq!(
            found,
            vec![
                Violation { line: 5, rule: Rule::ModuleDeclarations },
                Violation { line: 6, rule: Rule::ModuleDeclarations },
                Violation { line: 7, rule: Rule::ModuleDeclarations },
            ]
        );
    }

    /// `#[path]` and `include!` compile `src/leak.rs` into the domain, a file outside every module the rules read.
    #[test]
    fn should_flag_a_file_that_splices_in_a_file_the_rules_never_read() {
        let declaration = "#[path = \"../../leak.rs\"]\n\
                           pub(crate) mod leak;\n";
        let splice = "pub(crate) fn decide() {}\n\
                      include!(\"../../leak.rs\");\n";

        let found = [
            violations_in("shell_integration/domain/mod.rs", declaration),
            violations_in("shell_integration/domain/spliced.rs", splice),
        ];

        assert_eq!(
            found,
            [
                vec![Violation { line: 1, rule: Rule::SplicedCode }],
                vec![Violation { line: 2, rule: Rule::SplicedCode }],
            ]
        );
    }

    /// The domain would call the macro as `crate::warn_user!` or `warn_user!`, a path that names no logger.
    #[test]
    fn should_flag_a_file_that_hands_a_macro_to_other_modules() {
        let exported = "#[macro_export]\n\
                        macro_rules! warn_user {\n    \
                        ($m:expr) => { $crate::shared::infrastructure::logger::warn($m) };\n\
                        }\n";
        let in_scope = "#[macro_use]\n\
                        pub(crate) mod user_log;\n";

        let found = [
            violations_in("shared/infrastructure/user_log.rs", exported),
            violations_in("shared/infrastructure/mod.rs", in_scope),
        ];

        assert_eq!(
            found,
            [
                vec![Violation { line: 1, rule: Rule::SplicedCode }],
                vec![Violation { line: 1, rule: Rule::SplicedCode }],
            ]
        );
    }

    #[test]
    fn should_flag_a_module_root_file_next_to_the_module_folder() {
        let src = source_tree(&["settings.rs", "settings/domain/settings.rs"]);

        let misplaced = misplaced_files(src.path(), "settings");

        assert_eq!(misplaced, ["settings.rs"]);
    }

    #[test]
    fn should_flag_a_file_directly_under_the_module_folder_other_than_mod_rs() {
        let src = source_tree(&["settings/mod.rs", "settings/helpers.rs", "settings/domain/settings.rs"]);

        let misplaced = misplaced_files(src.path(), "settings");

        assert_eq!(misplaced, ["settings/helpers.rs"]);
    }

    /// `settings/domain.rs` next to `settings/domain/` is legal Rust, but its layer would read as `domain.rs`.
    #[test]
    fn should_flag_a_layer_root_file_next_to_its_layer_folder() {
        let src = source_tree(&["settings/mod.rs", "settings/domain.rs", "settings/domain/settings.rs"]);

        let misplaced = misplaced_files(src.path(), "settings");

        assert_eq!(misplaced, ["settings/domain.rs"]);
    }

    /// A misspelled layer or a folder that is no layer at all would get rules 3-4 only.
    #[test]
    fn should_flag_a_file_in_a_module_folder_that_is_not_a_layer() {
        let src = source_tree(&[
            "settings/mod.rs",
            "settings/model/leak.rs",
            "settings/aplication/leak.rs",
            "settings/domain/mod.rs",
        ]);

        let misplaced = misplaced_files(src.path(), "settings");

        assert_eq!(misplaced, ["settings/aplication/leak.rs", "settings/model/leak.rs"]);
    }

    #[test]
    fn should_not_flag_the_mod_rs_layout_at_any_depth() {
        let src = source_tree(&[
            "settings/mod.rs",
            "settings/domain/mod.rs",
            "settings/domain/value_objects/mod.rs",
            "settings/domain/value_objects/setting_key.rs",
            "settings/infrastructure/cli/settings_command.rs",
        ]);

        let misplaced = misplaced_files(src.path(), "settings");

        assert_eq!(misplaced, Vec::<String>::new());
    }
}
