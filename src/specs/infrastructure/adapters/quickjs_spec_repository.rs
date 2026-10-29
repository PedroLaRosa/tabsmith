//! Specs loaded from the embedded bundle and from local `.fig/autocomplete/build` folders through the QuickJS
//! runtime, with diff versions applied and the CLI version detected (SPEC §6.3).
use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_DEVELOPER_MODE;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::infrastructure::data_directory_from_env::user_home_from_env;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::entities::subcommand::Subcommand;
use crate::specs::domain::repositories::spec_repository::SpecRepository;
use crate::specs::domain::spec_error::SpecError;
use crate::specs::domain::value_objects::exec_request::ExecRequest;
use crate::specs::domain::value_objects::generator_context::GeneratorContext;
use crate::specs::domain::value_objects::semantic_version::semver_clean;
use crate::specs::domain::value_objects::spec_location::SpecLocation;
use crate::specs::domain::value_objects::spec_lookup::SpecLookup;
use crate::specs::domain::value_objects::task_id::TaskId;
use crate::specs::infrastructure::adapters::embedded_spec_bundle::EmbeddedSpecBundle;
use crate::specs::infrastructure::adapters::fig_spec_mapper::{self, normalise};
use crate::specs::infrastructure::adapters::fig_version_diff::apply_versions;
use crate::specs::infrastructure::adapters::quickjs_spec_runtime::{JsArg, QuickJsSpecRuntime};
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Deadline for one spec load / spec callback (SPEC §6.3).
/// The parser keeps its own 5 s for spec callbacks; neither is the runtime's module `LOAD_TIMEOUT`.
const TIMEOUT: Duration = Duration::from_secs(5);

/// The specs a [`QuickJsSpecRuntime`] evaluates. It holds the concrete runtime: finding a spec reads module exports
/// and a versioned spec's raw `default()` result as JSON.
pub(crate) struct QuickJsSpecRepository {
    runtime: Rc<QuickJsSpecRuntime>,
    bundle: EmbeddedSpecBundle,
    loader: RefCell<Loader>,
}

impl QuickJsSpecRepository {
    /// Reads nothing and runs no JS until the first find.
    pub(crate) fn new(runtime: Rc<QuickJsSpecRuntime>, bundle: EmbeddedSpecBundle) -> Self {
        QuickJsSpecRepository {
            runtime,
            bundle,
            loader: RefCell::new(Loader::default()),
        }
    }
}

impl SpecRepository for QuickJsSpecRepository {
    fn find(
        &self,
        location: &SpecLocation,
        lookup: &SpecLookup<'_>,
    ) -> Result<Option<Rc<Subcommand>>, SpecError> {
        // One borrow for the whole find: nothing it calls re-enters the repository (the runtime's JS host functions
        // touch only its task registry).
        self.loader
            .borrow_mut()
            .load(location, lookup, &self.runtime, &self.bundle)
    }

    fn forget_loaded_specs(&self) {
        self.loader.borrow_mut().specs.clear();
    }
}

fn dev_mode(settings: &Settings) -> bool {
    settings.bool(AUTOCOMPLETE_DEVELOPER_MODE)
}

#[derive(Default)]
struct Loader {
    /// Spec cache by `SpecLocation::cache_key` (None = missing, cached too).
    specs: HashMap<String, Option<Rc<Subcommand>>>,
    /// `cliVersion-<name>` memo (successful detections only).
    versions: HashMap<String, String>,
    /// Version detections still running after an aborted parse, reused by the next one (no respawn per keystroke).
    inflight: HashMap<String, TaskId>,
    /// Last normalised spec per cache key with the JSON it came from: an unchanged spec keeps its `Rc` through
    /// developer-mode reloads and `forget_loaded_specs` (the engine keys arg/generator identity on the address).
    by_json: HashMap<String, (Value, Rc<Subcommand>)>,
}

impl Loader {
    fn load(
        &mut self,
        loc: &SpecLocation,
        lookup: &SpecLookup,
        js: &QuickJsSpecRuntime,
        bundle: &EmbeddedSpecBundle,
    ) -> Result<Option<Rc<Subcommand>>, SpecError> {
        let (SpecLocation::Global { name } | SpecLocation::Local { name, .. }) = loc;
        if lookup.disables(loc) {
            return Ok(None);
        }
        let dev = dev_mode(lookup.settings());
        let key = loc.cache_key();
        if let Some(spec) = self.specs.get(&key).filter(|_| !dev) {
            return Ok(spec.clone());
        }
        let Some(path) = spec_file(loc, lookup, bundle) else {
            self.specs.insert(key, None);
            return Ok(None);
        };
        let exports = js.load_module(&path)?;
        let json = match exports.get("default") {
            Some(v) if fig_spec_mapper::fn_id(v).is_some() => {
                self.versioned(&exports, name, &path, dev, js, lookup.generator_context())?
            }
            Some(v) if v.is_object() => Some(v.clone()),
            _ => None,
        };
        let spec = match json {
            Some(j) => match self.by_json.get(&key).filter(|(old, _)| *old == j) {
                Some((_, rc)) => Some(rc.clone()),
                None => {
                    let rc = Rc::new(normalise(&j).map_err(|e| SpecError::Invalid(format!("{name}: {e}")))?);
                    self.by_json.insert(key.clone(), (j, rc.clone()));
                    Some(rc)
                }
            },
            None => None,
        };
        self.specs.insert(key, spec.clone());
        Ok(spec)
    }

    /// `default` is `async (version) => Subcommand | {versionedSpecPath, version}` (SPEC §6.3).
    fn versioned(
        &mut self,
        exports: &Map<String, Value>,
        name: &str,
        path: &Path,
        dev: bool,
        js: &QuickJsSpecRuntime,
        generator: &GeneratorContext,
    ) -> Result<Option<Value>, SpecError> {
        let memo = format!("cliVersion-{name}");
        let version = match self.versions.get(&memo).filter(|_| !dev) {
            Some(v) => Some(v.clone()),
            None => cli_version(exports, name, js, generator, &mut self.inflight)?,
        };
        if let Some(v) = &version {
            self.versions.insert(memo, v.clone());
        }
        let default = exports
            .get("default")
            .and_then(fig_spec_mapper::fn_id)
            .ok_or_else(|| SpecError::Invalid("default is not a function".into()))?;
        let arg = version.map_or(Value::Null, Value::String);
        let r = js.call_json(default, vec![JsArg::Json(arg)], Some(generator), TIMEOUT)?;
        let Some(vsp) = r.get("versionedSpecPath").and_then(Value::as_str) else {
            return Ok(Some(r).filter(Value::is_object));
        };
        // ponytail: the file sits next to `<name>/index.js`; true for all 6 bundled diff-versioned specs.
        let base = vsp.rsplit('/').next().unwrap_or(vsp);
        let file = path.with_file_name(format!("{base}.js"));
        let ex = js.load_module(&file)?;
        match (
            ex.get("default"),
            ex.get("versions").and_then(Value::as_object),
        ) {
            (Some(spec), Some(diffs)) => Ok(Some(apply_versions(
                spec.clone(),
                diffs,
                r.get("version").and_then(Value::as_str),
            ))),
            _ => Err(SpecError::Invalid(format!(
                "{}: invalid versioned spec file",
                file.display()
            ))),
        }
    }
}

/// The file a location loads from: [`EmbeddedSpecBundle::resolve`] for globals, `<dir>/.fig/autocomplete/build/<name>.js` (else
/// `<name>/index.js`) for locals.
fn spec_file(loc: &SpecLocation, lookup: &SpecLookup, bundle: &EmbeddedSpecBundle) -> Option<PathBuf> {
    match loc {
        SpecLocation::Global { name } => bundle.resolve(name, lookup.settings(), lookup.index()),
        SpecLocation::Local { dir, name } => {
            let dir = match dir.strip_prefix("~") {
                Ok(rest) => user_home_from_env().ok()?.join(rest),
                Err(_) => dir.clone(),
            };
            let build = dir.join(".fig").join("autocomplete").join("build");
            [
                build.join(format!("{name}.js")),
                build.join(name).join("index.js"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        }
    }
}

/// `getVersionCommand(exec)` if exported, else `semver_clean(<name> --version)`; failures → None. An aborted
/// wait leaves the detection running in `inflight`, and the next parse waits on that same task.
fn cli_version(
    exports: &Map<String, Value>,
    name: &str,
    js: &QuickJsSpecRuntime,
    generator: &GeneratorContext,
    inflight: &mut HashMap<String, TaskId>,
) -> Result<Option<String>, SpecError> {
    if name == "@usermn/sdc" {
        return Ok(None); // its version command runs `npx`, which may download a package
    }
    let f = exports
        .get("getVersionCommand")
        .and_then(fig_spec_mapper::fn_id);
    let id = *inflight.entry(name.to_string()).or_insert_with(|| match f {
        Some(f) => js.start_call(f, vec![JsArg::Exec], Some(generator)),
        None => js.start_exec(
            ExecRequest::new(name.into(), vec!["--version".into()], None, HashMap::new(), None),
            generator,
        ),
    });
    let r = js.wait_json(&[id], Instant::now() + TIMEOUT);
    if matches!(r, Err(SpecError::Aborted)) {
        return Err(SpecError::Aborted);
    }
    inflight.remove(name);
    js.forget(&[id]);
    let Ok(Some(Ok(v))) = r.map(|mut r| r.pop().flatten()) else {
        return Ok(None);
    };
    Ok(match f {
        Some(_) => v.as_str().map(String::from),
        None => v
            .get("stdout")
            .and_then(Value::as_str)
            .and_then(semver_clean),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::domain::services::settings_catalog::AUTOCOMPLETE_DISABLE_FOR_COMMANDS;
    use crate::settings::domain::value_objects::setting_value::SettingValue;
    use crate::shared::infrastructure::data_directory_from_env::data_directory_from_env;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::specs::domain::value_objects::spec_index::SpecIndex;

    const MK: &str = r#"var s={name:"mk",args:{name:"t"}};export{s as default};"#;

    /// A repository over a fresh runtime, with the bundle this process's env names.
    fn repository() -> QuickJsSpecRepository {
        use crate::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
        let runtime =
            QuickJsSpecRuntime::new(Rc::new(ChannelRuntimeEventSource::new()), &Settings::default())
                .unwrap();
        QuickJsSpecRepository::new(
            Rc::new(runtime),
            EmbeddedSpecBundle::new(data_directory_from_env()),
        )
    }

    /// Writes `source` as the local spec `mk` of `dir`.
    fn write_mk(dir: &TempDir, source: &str) {
        let build = dir.path().join(".fig").join("autocomplete").join("build");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join("mk.js"), source).unwrap();
    }

    /// Finds the local spec `mk` of `dir` under `settings`, with an empty spec index and generator context.
    fn find_mk(
        repository: &QuickJsSpecRepository,
        dir: &TempDir,
        settings: &Settings,
    ) -> Result<Option<Rc<Subcommand>>, SpecError> {
        let (index, generator_context) = (SpecIndex::default(), GeneratorContext::default());
        let location = SpecLocation::Local {
            dir: dir.path().to_path_buf(),
            name: "mk".into(),
        };
        repository.find(
            &location,
            &SpecLookup::new(settings, &index, &generator_context),
        )
    }

    fn disabling_mk() -> Settings {
        let disabled = SettingValue::List(vec![SettingValue::Text("mk".into())]);
        Settings::default().with(AUTOCOMPLETE_DISABLE_FOR_COMMANDS, disabled)
    }

    /// Disabled first (nothing cached), then loaded and cached, then disabled again (checked before the cache).
    #[test]
    fn should_find_nothing_for_a_disabled_command_and_cache_nothing() {
        let dir = TempDir::new("repo-disabled");
        write_mk(&dir, MK);
        let repository = repository();
        let (enabled, disabled) = (Settings::default(), disabling_mk());

        let found = [&disabled, &enabled, &disabled]
            .map(|settings| find_mk(&repository, &dir, settings).unwrap().is_some());

        assert_eq!(found, [false, true, false]);
    }

    #[test]
    fn should_keep_a_missing_spec_missing_until_loaded_specs_are_forgotten() {
        let dir = TempDir::new("repo-missing");
        let repository = repository();
        let settings = Settings::default();
        let before = find_mk(&repository, &dir, &settings).unwrap();
        write_mk(&dir, MK);
        let cached = find_mk(&repository, &dir, &settings).unwrap();

        repository.forget_loaded_specs();
        let after = find_mk(&repository, &dir, &settings).unwrap();

        assert_eq!(
            [before.is_some(), cached.is_some(), after.is_some()],
            [false, false, true]
        );
    }

    #[test]
    fn should_not_cache_a_spec_that_failed_to_load() {
        let dir = TempDir::new("repo-failed");
        write_mk(&dir, r#"throw new Error("boom");var s={name:"mk"};export{s as default};"#);
        let repository = repository();
        let settings = Settings::default();
        let failed = find_mk(&repository, &dir, &settings);
        write_mk(&dir, MK);

        let found = find_mk(&repository, &dir, &settings);

        assert!(matches!(failed, Err(SpecError::Invalid(_))));
        assert!(matches!(found, Ok(Some(_))));
    }

    #[test]
    fn cli_version_detection_survives_an_aborted_parse() {
        // cli-version-respawn-per-keystroke: an abort keeps the detection running; the next parse reuses it
        use crate::specs::application::ports::runtime_event_source::ChannelRuntimeEventSource;
        let events = Rc::new(ChannelRuntimeEventSource::new());
        let js = QuickJsSpecRuntime::new(events.clone(), &Settings::default()).unwrap();
        let dir = crate::shared::infrastructure::testing::temp_dir::TempDir::new("cliver");
        let p = dir.path().join("v.js");
        let src = "var v=async()=>{await new Promise(r=>setTimeout(r,200));globalThis.n=(globalThis.n||0)+1;return `${globalThis.n}.0.0`};export{v as getVersionCommand};";
        std::fs::write(&p, src).unwrap();
        let exports = js.load_module(&p).unwrap();
        let (generator, mut inflight) = (GeneratorContext::default(), HashMap::new());
        events.send_newer_request();
        assert!(matches!(
            cli_version(&exports, "az", &js, &generator, &mut inflight),
            Err(SpecError::Aborted)
        ));
        assert_eq!(inflight.len(), 1);
        assert!(events.take_newer_request());
        assert_eq!(
            cli_version(&exports, "az", &js, &generator, &mut inflight)
                .unwrap()
                .as_deref(),
            Some("1.0.0")
        );
        assert!(inflight.is_empty());
    }
}
