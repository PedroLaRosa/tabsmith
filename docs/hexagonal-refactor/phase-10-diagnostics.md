# Phase 10 design: `diagnostics` (doctor, diagnostic report, debug logs)

Status: DESIGN, not started (written at `b0fe04d`, the tip of PR #1 plus Phases 8-9; branch `refactor/hexagonal-diagnostics`).
Inputs: the plan (sections 1, 2, 4 `diagnostics`, 5 "Phase 10", 9 hand-offs), `agent-brief.md` (D1-D9, binding),
`handoff.md`, the glossary (`diagnostics` section), `source-map/doctor.md` and `source-map/cli.md` (written at `af1b304`,
re-verified against the CURRENT `src/doctor.rs` 927 lines and `src/cli.rs` 200 lines), and four recon notes
(`target/p10/recon-{behaviour,seams,architecture,verification}.md`, git-ignored). Line numbers below are of `b0fe04d`:
read the code with `git show b0fe04d:src/doctor.rs` (tag `phase10-start`, created in 10.0) because 10.13a edits `doctor.rs`.

## 1. Scope

Moves: all of `src/doctor.rs` and the diagnostic / debug-logs parts of `src/cli.rs` into a new registered module
`diagnostics` (domain / application / infrastructure). `doctor.rs` and `cli.rs` are deleted. Also: `settings::infrastructure::
load_or_defaults` (its last caller is doctor.rs:526) is deleted after diagnostics gets its own lenient settings adapter;
`RestartReminderRepository` gains two methods; `Integration` gains `PartialEq, Eq`. Bootstrap gains three bridges and
`diagnostics_module()`.

Does NOT change: stdout, stderr, exit codes, files written (bytes, modes), process spawns (count, argv, order, timing),
clap identifiers (D4), the `logs/{cli,pty}.log` contract, `types.rs` (Phase 11), `docs/SPEC.md` (Phase 11).

Behaviour oracle (besides the gate): `scripts/hexagonal-refactor/doctor-diff.sh` (committed in 10.0): 470 commands in 61
scenarios (fix / re-run / abort, spawn log, restart reminder, strict, root, no HOME, pty colours, spinner, Ctrl-C,
diagnostic env branches, debug logs tails). Baseline transcript of `af1b304` is `target/p10/doctor-diff.baseline.txt`
(identical to the Phase 9 tip). Run it on `target/debug/tabsmith` after `cargo build --locked` and `cmp` with the baseline
(about 3.5 min): after 10.17, 10.18a and 10.20. It is the only net for the non-`--all` flow. Never re-baseline.

## 2. The one hard problem: doctor is LAZY, the domain must be pure

Today every group is built and probed lazily (`r.group(.., dotfile_checks) && r.group(.., setup_checks) && r.group(..,
env_checks)`, doctor.rs:210-219), a fix is followed by a re-probe of just that check, skipped and after-abort checks are never
probed, and some probes spawn processes. The plan's literal `SystemFacts` snapshot would add spawns an aborted run never made,
move the corrupt-settings warn log, and leave a fixed check reporting Err. Decision (deviation 2):

* **A `Check` is DATA**: `{ name, severity (decided when the group is built), subject }`. No closure, no I/O.
* **`DoctorCheckCatalog`** (domain service, free functions) holds ALL rules: which checks a group has, their names and
  headers, the severity rules, and, per check, a pure function `facts -> CheckOutcome`. Where a rule needs a fact only
  conditionally (the OS rule needs `sw_vers` on macOS only; the dotfile severity needs PATH only for a non-current shell),
  the function takes a thunk (`impl FnOnce() -> T`), so the catalog decides what is asked and a test can make the thunk
  panic.
* **Two application services gather facts on demand through ports** (they are services, not use cases, not ports):
  `CheckGroupBuilder` does the CONSTRUCTION-time I/O of one group when asked (the `which()` calls, the bash login-file stats and
  the pwsh `$PROFILE` spawn through `IntegrationProbe::list`, the settings load plus its warn log, `which("fish")`);
  `CheckEvaluator` asks the ports for the facts of ONE check at the moment that check is evaluated, then calls the catalog.
  Evaluating again after a fix re-asks (live disk state).
* **`RunDoctorUseCase` orders everything**: it builds a group only after the previous group returned true, builds the group
  BEFORE printing its header (doctor.rs builds the argument of `Run::group` first, so with `--all` the construction I/O and
  its stderr warnings precede the dim header), evaluates check by check, and re-evaluates after a successful fix.
* Immutable facts are hoisted into `SystemFacts` (the env group's snapshot, read when that group is built) and into the use
  case (paths, parent exe, current shell). File reads, spawns and the settings snapshot are never hoisted.

How each lazy trap is kept:

| Trap | Mechanism | Pinned by |
|---|---|---|
| groups built only after the previous passed, after its fixes | `execute` calls `builder.<group>()` inside `a && b && c` | `should_build_no_later_group_after_an_abort` (probe panics on group 2 build) + doctor-diff b9/c1/k3 spawn logs |
| settings loaded ONCE at env-group build, after setup's reset fix; `--all` loads (and warns) too | `CheckGroupBuilder::environment` calls `SettingsSnapshot::load` once; evaluators never load | builder test counts loads; doctor-diff e1/o (`-v` interleaving) |
| pwsh `$PROFILE` spawn at dotfile-group build, only if pwsh is current or on PATH; a second spawn inside `install_missing` | builder calls `IntegrationProbe::list` per kept shell only; fixer gets the ORIGINAL paths (pwsh_profile None) | builder test (list never asked for an off-PATH shell), bridge test, doctor-diff b9 |
| re-run after a fix re-probes | evaluator asks the ports again; the dotfile LIST is not re-listed (snapshot) | `should_ask_the_probe_again_after_a_fix`, `should_not_list_the_integrations_again_after_a_fix` |
| Skip never evaluated, after-abort never evaluated | `run_group` filters Skip and returns at the first abort | `doctor_group_soft_fix_abort_flow` with panicking probes |
| `sw_vers` / bash / fish version spawns only when their turn comes | evaluator + thunks; strict bash Err aborts before the fish probe | evaluator tests, `should_not_probe_fish_after_a_strict_bash_error` |
| header after construction I/O | `execute`: build, then `group_header` | `should_build_each_group_before_printing_its_header` |

## 3. Final file tree (one type per snake_case file; `mod.rs` only declares children)

```
src/diagnostics/
  mod.rs
  domain/
    mod.rs
    diagnostics_error.rs         DiagnosticsError
    entities/{mod.rs, doctor_run.rs}
    value_objects/{mod.rs,
      check.rs  check_group.rs  check_subject.rs  environment_check.rs  system_check.rs
      check_severity.rs  check_outcome.rs  check_fix.rs  info_line.rs
      system_facts.rs  terminal_environment.rs  operating_system.rs  tool_version.rs
      diagnostic_report.rs  diagnostic_section.rs  diagnostic_value.rs}
    services/{mod.rs, doctor_check_catalog.rs (+ doctor_check_catalog/tests.rs), username_scrubber.rs}
  application/
    mod.rs
    check_group_builder.rs (+ check_group_builder/tests.rs if > 500 lines)
    check_evaluator.rs
    run_doctor_use_case.rs (+ run_doctor_use_case/tests.rs)
    collect_diagnostic_report_use_case.rs
    show_recent_logs_use_case.rs
    doctor_request.rs  doctor_verdict.rs  log_paths.rs          (DTOs)
    test_fixtures.rs                                            (#[cfg(test)] EventLog + Recording/InMemory harness + LoggingRestartReminder, each item with its own #[cfg(test)])
    ports/{mod.rs, system_facts_probe.rs, integration_probe.rs, bundled_specs_probe.rs, settings_snapshot.rs,
           check_fixer.rs, doctor_reporter.rs, log_reader.rs, diagnostic_host.rs}
  infrastructure/
    mod.rs
    diagnostics_module.rs
    adapters/{mod.rs, os_system_facts_probe.rs, lenient_settings.rs, file_repairs.rs, terminal_doctor_reporter.rs,
              spinner.rs, cursor_restorer.rs, file_log_reader.rs, os_diagnostic_host.rs}
    cli/{mod.rs, doctor_command.rs, diagnostic_command.rs, debug_logs_command.rs}
```
About 62 files. The plan's section 4 lists 8 VOs and 4 ports; the extras are required by the lazy design (deviations 2, 5, 6).
Edges: diagnostics -> shared, settings, shell_integration, ghost_text (domain only, plus the `RestartReminderRepository` trait).
NOT specs, NOT terminal_session (the bridges in bootstrap make them unnecessary): `phase10-greps.sh` B2.

## 4. Domain

All `#[derive(Clone, Debug, PartialEq)]` unless stated; fields private with accessors; std only (+ `unicode_width` not needed);
no cfg, no I/O, no `println!`, no anyhow/serde/clap.

```rust
// value_objects/check_severity.rs     Copy, Eq too (the locked test asserts on it)
pub(crate) enum CheckSeverity { Normal, Soft, Skip }

// value_objects/info_line.rs          one line of an Err's info; the reporter styles the Command span
pub(crate) enum InfoLine { Text(String), Command { before: String, command: String, after: String } }
//   reinstall hint = Command { "Run ", "tabsmith integrations install dotfiles {shell}", " to reinstall shell integrations for {shell}" }
//   rebuilt as format!("{before}{}{after}", paint("35", command)) at print time (paint is tty dependent: never freeze it in the domain)

// value_objects/check_fix.rs          data; the glossary's ReinstallIntegrations is renamed (deviation 1)
pub(crate) enum CheckFix {
    CreateDataDirectory(PathBuf),
    InstallIntegration(Integration),              // needs Integration: PartialEq, Eq (10.1)
    InstallMissingIntegrations(Vec<Shell>),       // = IntegrationInstaller::install_missing, NOT ReinstallIntegrationsUseCase
    ResetSettings(PathBuf),
    InstallSpecBundle,
}
impl CheckFix {
    // true: CreateDataDirectory false, InstallIntegration true, InstallMissingIntegrations true, ResetSettings false,
    // InstallSpecBundle false (explicit arms, D8; trap T18, doctor.rs:351-355 and 432-438). A domain rule under a pure test.
    pub(crate) fn marks_restart_reminder(&self) -> bool;
}

// value_objects/check_outcome.rs
pub(crate) enum CheckOutcome { Ok, Warn(String), Err { reason: String, info: Vec<InfoLine>, fix: Option<CheckFix> } }
impl CheckOutcome { pub(crate) fn err(reason: impl Into<String>) -> Self /* empty info, no fix */; is_ok(); is_err(); }

// value_objects/tool_version.rs
pub(crate) struct ToolVersion(Vec<u32>);   // non-empty by construction
impl ToolVersion {
    pub(crate) fn parse(text: &str) -> Option<ToolVersion>;     // doctor.rs:639-647 verbatim
    pub(crate) fn components(&self) -> &[u32];
    pub(crate) fn major(&self) -> u32;  pub(crate) fn minor_or_zero(&self) -> u32;   // bash message: v[0], v.get(1).unwrap_or(&0)
    pub(crate) fn is_below(&self, bound: &[u32]) -> bool;       // LEXICOGRAPHIC slice compare, NO zero padding: [3,3] < [3,3,0], [4] < [4,4]
}
impl Display for ToolVersion  // components joined by '.'

// value_objects/operating_system.rs
pub(crate) enum OperatingSystem { Linux, Macos, Windows, Other(String) }
impl OperatingSystem { pub(crate) fn from_name(os: &str) -> Self }   // "linux" | "macos" | "windows" | other (the probe passes consts::OS)

// value_objects/terminal_environment.rs   the env facts the terminal rule reads, read once when the env group is built
pub(crate) struct TerminalEnvironment {
    term_program: Option<String>, bundle_identifier: Option<String> /* __CFBundleIdentifier */, tabsmith_term: Option<String>,
    inside_emacs: bool /* PRESENCE, even empty */, launched_by_tabsmith: bool /* PRESENCE */, in_ci: bool /* PRESENCE */,
    parent_is_cmd: bool, tabsmith_version: String /* env!("CARGO_PKG_VERSION") from the probe: a fact, so tests can fix it */ }

// value_objects/system_facts.rs           the ENV GROUP's immutable snapshot (glossary redefinition, deviation 2)
pub(crate) struct SystemFacts { terminal: TerminalEnvironment, session_id: Option<String>, settings: Settings }

// value_objects/environment_check.rs      TerminalIntegrations | Session | AutocompleteEnabled | DeveloperMode | Inline
// value_objects/system_check.rs           Os | Bash { program: String } | Fish
// value_objects/check_subject.rs          WHAT a check evaluates (data):
pub(crate) enum CheckSubject {
    Dotfile(Integration), ListingFailure(String), DataDirectory, ScriptsPointAtBinary, SettingsCorruption(PathBuf),
    SpecsExtracted, WindowsConsole, WindowsVersion, Environment(EnvironmentCheck, Rc<SystemFacts>), System(SystemCheck),
}
// value_objects/check.rs                  Check { name: String, severity: CheckSeverity, subject: CheckSubject }
// value_objects/check_group.rs            CheckGroup { header: String, checks: Vec<Check> }
```

```rust
// entities/doctor_run.rs  DoctorRun: policy and counts of one run (the spinner field stays out)
impl DoctorRun {
    pub(crate) fn new(all: bool, strict: bool) -> Self;  all(); strict();
    pub(crate) fn effective(&self, severity: CheckSeverity, outcome: CheckOutcome) -> CheckOutcome;  // Soft Err -> Warn(reason) unless strict (drops info+fix); explicit arms (D8)
    pub(crate) fn count(&mut self, outcome: &CheckOutcome);        // Ok nothing, Warn -> warnings+1, Err -> errors+1
    pub(crate) fn abort(&mut self);                                // errors += 1 DIRECTLY (an unfixed abort)
    pub(crate) fn counts(&self) -> (usize, usize);  pub(crate) fn restore(&mut self, counts: (usize, usize));   // the system group
    pub(crate) fn failed(&self) -> bool;                           // errors > 0 || (strict && warnings > 0)
}
```

`DoctorCheckCatalog` (`services/doctor_check_catalog.rs`, free `pub(crate) fn`s; tests in `doctor_check_catalog/tests.rs`):

```rust
// -- what a group has (data) --
const headers: DOTFILES_HEADER "Let's check your dotfiles...", SETUP_HEADER "Let's make sure tabsmith is set up correctly...",
               ENVIRONMENT_HEADER "Let's check if your environment is supported...", SYSTEM_HEADER "Let's check if your system is compatible..."
fn dotfile_severity(shell, current: Option<Shell>, on_path: impl FnOnce() -> bool) -> Option<CheckSeverity>   // current => Normal WITHOUT asking; on PATH => Soft; else None
fn dotfile_check(home: &Path, integration: Integration, severity) -> Check      // name "{shell} {short_path} integration check"
fn listing_failure_check(shell, severity, reason: String) -> Check              // name "{shell} integration check", reason = error text
fn setup_checks(settings_file: PathBuf, windows: bool) -> Vec<Check>            // order: data dir, scripts, settings, specs, [Windows Console Check, Windows version]
fn environment_checks(facts: SystemFacts, parent: Option<&str>, tabsmith_shell: Option<&str>) -> Vec<Check>   // Session severity = session_kind; Inline = Normal if inline.enabled else Soft
fn system_checks(current: Option<Shell>, parent: Option<&str>, fish_on_path: bool) -> Vec<Check>              // Bash program = parent when current==Bash else "bash"
fn short_path(home: &Path, p: &Path) -> String                                   // doctor.rs:273-278 verbatim
fn session_kind(sid: Option<&str>, parent: Option<&str>, tabsmith_shell: Option<&str>) -> CheckSeverity        // name and signature kept (locked test)
// -- what a check says (pure) --
fn dotfile_outcome(integration: &Integration, result: Result<(), ShellIntegrationError>) -> CheckOutcome       // five explicit arms; PermissionDenied: info, NO fix
fn data_directory_outcome(data_root: &Path, exists: bool) -> CheckOutcome
fn scripts_outcome(binary: &str, script_binaries: &[(Shell, String)]) -> CheckOutcome                          // stale = bin != binary; reason uses the FIRST; shells = Vec::dedup (consecutive only)
fn settings_outcome(settings_file: &Path, loads: bool) -> CheckOutcome
fn specs_outcome(extracted: bool) -> CheckOutcome
fn windows_console_outcome(attached: bool) -> CheckOutcome                                                     // non-cfg; the 8 info lines, the third is ""
fn windows_version_setup_outcome(build: Option<u32>) -> CheckOutcome                                           // < 17763 => Err (setup group)
fn terminal_outcome(t: &TerminalEnvironment) -> CheckOutcome                                                   // 7 rules, first match wins
fn session_outcome(run_file_exists: bool) -> CheckOutcome
fn autocomplete_enabled_outcome(s: &Settings) -> CheckOutcome;  fn developer_mode_outcome(s: &Settings) -> CheckOutcome
fn inline_outcome(s: &Settings, env_is_set: impl Fn(&str) -> bool) -> CheckOutcome                             // + is_local; explicit arms (D8)
fn os_outcome(os: &OperatingSystem, macos_version: impl FnOnce() -> Option<ToolVersion>, windows_build: impl FnOnce() -> Option<u32>) -> CheckOutcome
fn bash_outcome(v: Option<ToolVersion>) -> CheckOutcome;  fn fish_outcome(v: Option<ToolVersion>) -> CheckOutcome
```

`UsernameScrubber` (`services/username_scrubber.rs`): `new(user: String)`, `clean(&self, s: &str) -> String` =
empty user => identity; else `s.replace("/{user}", "/USER").replace("\\{user}", "\\USER")` in that order, raw substrings.
`DiagnosticReport { sections: Vec<DiagnosticSection> }`, `DiagnosticSection { name: String, rows: Vec<(String, DiagnosticValue)> }`
(insertion order), `DiagnosticValue { Text(String), Flag(bool), Missing }` (the domain cannot hold `serde_json::Value`).
`DiagnosticsError` (`diagnostics_error.rs`): `IntegrationPaths(ShellIntegrationError)` and `DataDirectory(SharedError)`; `Display`
writes the wrapped text byte for byte; `impl std::error::Error` WITHOUT `source()` (anyhow's `{:#}` would print the text twice).

## 5. Application

Ports (traits; every one gets an `InMemory*` / `Recording*` / `Panicking*` double in its own file under `#[cfg(test)]`, tests in the same file):

```rust
trait SystemFactsProbe {            // infra OsSystemFactsProbe (own module only: OS, env, fs, spawns)
    fn parent_executable(&self) -> Option<String>;                // process_info::parent_exe()
    fn is_on_path(&self, program: &str) -> bool;                  // which(..).is_some()
    fn env_text(&self, name: &str) -> Option<String>;             // std::env::var(k).ok()  (non-UTF-8 = unset; EMPTY is kept)
    fn env_is_set(&self, name: &str) -> bool;                     // var_os(k).is_some()    (doctor's `has`)
    fn env_is_non_empty(&self, name: &str) -> bool;               // var_os(k).is_some_and(!empty)  (the inline API key)
    fn tabsmith_version(&self) -> String;                         // env!("CARGO_PKG_VERSION")
    fn operating_system(&self) -> OperatingSystem;                // OperatingSystem::from_name(consts::OS)
    fn directory_exists(&self, path: &Path) -> bool;  fn file_exists(&self, path: &Path) -> bool;
    fn settings_file(&self, data_root: &Path) -> PathBuf;         // data_directory.settings_file().unwrap_or_else(|_| data_root/settings.json)
    fn settings_file_loads(&self, path: &Path) -> bool;           // JsonDocumentStore::load(path).is_ok()
    fn tool_version(&self, program: &str, args: &[&str]) -> Option<ToolVersion>;   // output_with_timeout 5 s, stdout only, failure/timeout/no version = None
    fn console_attached(&self) -> bool;                           // NO cfg on the trait: the Windows body is inside the adapter
    fn windows_build(&self) -> Option<u32>;                       // `cmd /C ver` third number; None off Windows; NOT cached (two spawns per run)
}
trait IntegrationProbe {            // bootstrap IntegrationProbeBridge (only bootstrap may build IntegrationInstaller)
    fn locate_paths(&self) -> Result<IntegrationPaths, ShellIntegrationError>;                    // integration_paths_from(&data_directory)
    fn list(&self, p: &IntegrationPaths, shell: Shell) -> Result<Vec<Integration>, ShellIntegrationError>;   // spawns pwsh when shell==Pwsh && p.pwsh_profile()==None, EVERY call
    fn check(&self, p: &IntegrationPaths, i: &Integration) -> Result<(), ShellIntegrationError>;
    fn script_binaries(&self, p: &IntegrationPaths) -> Vec<(Shell, String)>;                      // script_bins moved verbatim
}
trait BundledSpecsProbe { fn is_extracted(&self, data_root: &Path) -> bool; }   // bootstrap: data_root/specs/<embedded_spec_bundle::version()>/index.json is_file
trait SettingsSnapshot { fn load(&self) -> Settings; }                         // infra LenientSettings: warn `settings: <chain>` + defaults
trait CheckFixer { fn fix(&self, fix: &CheckFix, paths: &IntegrationPaths) -> Result<(), String>; }   // Err = the text after `Failed to fix: `, pre-rendered `{e:#}`
trait DoctorReporter {              // infra TerminalDoctorReporter; every method is ONE println!/eprint of today
    fn start_progress(&self); fn stop_progress(&self) /* idempotent */;
    fn group_header(&self, header: &str);                                       // paint("2", header)
    fn outcome(&self, name: &str, outcome: &CheckOutcome);                      // print_outcome, doctor.rs:94-108
    fn fixing(&self);      /* "Attempting to fix automatically..." */   fn fix_failed(&self, reason: &str); /* "Failed to fix: {reason}" */
    fn rechecking(&self);  /* "Re-running check..." */                  fn blank_line(&self);
    fn doctor_found_errors(&self);  /* "{paint 31 ✘} Doctor found errors. Please fix them and try again." */
    fn everything_looks_good(&self); /* "{paint 32 ✔} Everything looks good!" */
    fn support_hint(&self);          /* "  tabsmith still not working? Check ~/.tabsmith/logs/pty.log" (hard-coded ~) */
    fn restart_notice(&self);        /* "  {paint 1 PS. Autocomplete won't work in any existing terminal sessions, only new ones.}" + "  (You might want to restart your terminal emulator)" */
}
trait LogReader { fn read(&self, path: &Path) -> Option<Vec<u8>>; }            // fs::read(path).ok()
trait DiagnosticHost {              // infra OsDiagnosticHost: cli.rs:41-132 minus the rules
    fn user(&self) -> String;                       // USERNAME on Windows (cfg!(windows)), else USER; unset/non-UTF-8 => ""
    fn env_text(&self, name: &str) -> Option<String>;   // NON-EMPTY UTF-8 (cli's `env`; differs from doctor's presence rules ON PURPOSE)
    fn environment_variables(&self) -> Vec<(String, String)>;   // vars_os, non-UTF-8 keys dropped, values lossy, unfiltered, unsorted
    fn shell_version(&self, shell: &str) -> Option<String>;     // `$SHELL --version`, Command::output (NO timeout), stdout only, first line trimmed; blank first line = Some("")
    fn current_dir(&self) -> Option<String>;  fn current_exe(&self) -> Option<String>;   // display() (lossy)
    fn in_wsl(&self) -> bool;                       // cfg!(target_os="linux") && (WSL_DISTRO_NAME non-empty || /proc/sys/kernel/osrelease lowercased contains "microsoft"), verbatim
    fn version(&self) -> String;  fn platform(&self) -> String;   // env!("CARGO_PKG_VERSION"); "{OS} {ARCH}"
}
// RestartReminderRepository (shell_integration domain trait, legal import) gains is_pending() / clear_pending() in 10.1.
```

Services and use cases (constructed with `Rc<dyn Port>`; single thread):

```rust
CheckGroupBuilder { system, integrations, settings }                       // construction-time I/O of ONE group, on demand
    fn dotfiles(&self, paths: &IntegrationPaths, current: Option<Shell>) -> CheckGroup
    fn setup(&self, paths: &IntegrationPaths) -> CheckGroup                // settings_file(paths.data_root()); Windows checks iff operating_system()==Windows
    fn environment(&self, parent: Option<&str>) -> CheckGroup              // reads the env, loads settings ONCE, builds SystemFacts
    fn system(&self, current: Option<Shell>, parent: Option<&str>) -> CheckGroup   // is_on_path("fish") here
CheckEvaluator { system, integrations, bundled_specs }
    fn evaluate(&self, check: &Check, paths: &IntegrationPaths) -> CheckOutcome   // one match over CheckSubject, explicit arms; asks ONLY this check's facts
RunDoctorUseCase { builder, evaluator, fixer, reporter, system, integrations, restart_reminder }
    fn execute(&self, request: DoctorRequest) -> Result<DoctorVerdict, DiagnosticsError>
CollectDiagnosticReportUseCase { host }      fn execute(&self) -> DiagnosticReport
ShowRecentLogsUseCase { data_directory: DataDirectory, reader }
    fn paths(&self) -> Result<LogPaths, DiagnosticsError>        // logs_dir() first; no I/O besides the path
    fn pty_tail(&self, paths: &LogPaths) -> Option<Vec<String>>  // reader.read(pty_log) + last 50 lines
//   DTOs: DoctorRequest { all, strict }; DoctorVerdict { Passed, Failed }; LogPaths { cli_log: PathBuf, pty_log: PathBuf }
```

`CheckEvaluator` dispatch (the facts each subject asks, nothing else):
Dotfile -> `integrations.check(paths, i)`; ListingFailure -> none; DataDirectory -> `directory_exists(data_root)`;
ScriptsPointAtBinary -> `script_binaries(paths)`; SettingsCorruption(p) -> `settings_file_loads(p)`; SpecsExtracted -> `bundled_specs.is_extracted(data_root)`;
WindowsConsole -> `console_attached()`; WindowsVersion -> `windows_build()`; Environment(Session) -> `file_exists(data_root/run/{sid}.json)` (sid `unwrap_or_default`);
Environment(Inline) -> `env_is_non_empty(key)` through the thunk; the other env checks need no probe; System(Os) -> thunks to `tool_version("sw_vers", ["-productVersion"])` / `windows_build()`;
System(Bash{program}) -> `tool_version(program, ["--version"])`; System(Fish) -> `tool_version("fish", ["--version"])`.

### `ShowRecentLogsUseCase`
Two methods, because `cli.rs:159-163` prints the two path lines BEFORE `std::fs::read(&pty)` and the command keeps that order (it matters when `pty.log` is a FIFO, a device or huge).
`paths()`: `data_directory.logs_dir()?` FIRST (error before any print, `DiagnosticsError::DataDirectory`), then `cli_log = dir/cli.log`, `pty_log = dir/pty.log`; no I/O besides the path.
`pty_tail(&paths)`: `reader.read(pty_log)`: `Some(bytes)` => `Some(last 50 of String::from_utf8_lossy(&bytes).lines())` (a blank line is printed even for an EMPTY file); `None` => `None`.
`debug_logs_command`: `paths()?` -> `println!("{cli}\n{pty}")` -> `pty_tail()` -> only for `Some(tail)`, `println!()` and each line.

## 6. Infrastructure

* `OsSystemFactsProbe::new(data_directory: DataDirectory)`: the bodies of section 5 over `process_info`, `which`, `JsonDocumentStore`,
  `output_with_timeout(Command::new(prog).args(args), 5 s).ok()` then `ToolVersion::parse`. `console_attached` / `windows_build` have a
  `#[cfg(windows)]` body (moved verbatim: `is_console(stdin.as_raw_handle()) && is_console(stdout.as_raw_handle())`, `version_from("cmd", ["/C","ver"])?.get(2)`) and a
  `#[cfg(not(windows))]` body (`true` / `None`), in the SAME file as `version_from`.
* `LenientSettings::new(Rc<dyn SettingsRepository>)` implements `SettingsSnapshot` with the body of `LenientLiveSettings`:
  `repository.load().unwrap_or_else(|e| { logger::warn(&format!("settings: {}", display_chain(&e))); Settings::default() })`. Injected, so diagnostics never names
  `JsonFileSettingsRepository`. The three `load_or_defaults` tests are re-homed here (10.18b).
* `FileRepairs::new(clock: Arc<dyn Clock + Send + Sync>)`: `create_data_directory(&Path) -> io::Result<()>` = `std::fs::create_dir_all` (default umask, no 0700);
  `reset_settings(&Path) -> io::Result<()>` = `std::fs::copy(path, path.with_extension(format!("json.bak-{}", clock.now_secs())))?` then `std::fs::write(path, "{}\n")`
  (extension REPLACED; not atomic; follows symlinks; keeps the mode; NOT `SettingsRepository`/`AtomicFile`). `Clock::now_secs` equals `types::now_secs`.
* `TerminalDoctorReporter`: pure `*_lines` functions take a `Painter { styled: bool }` (`sgr(s, code, styled)`), production passes `Painter { styled: stdout_tty() }` at call time, so SGR bytes are unit-tested;
  `println!` once per line exactly as today; owns `RefCell<Option<Spinner>>`; `start_progress` = `Spinner::start()` (None unless stderr is a tty), `stop_progress` = take + drop.
* `Spinner` (`spinner.rs`) and `restore_cursor_on_ctrl_c` (`cursor_restorer.rs`, both cfg variants in ONE file) are MOVED verbatim from doctor.rs:734-797 in 10.13a.
  Drop order: stop flag, join, then `eprint!("\r\x1b[2K\x1b[?25h")`. The SIGINT thread is installed once per `Spinner::start`, never removed, prints `\x1b[?25h` and exits 1.
* `FileLogReader` = `std::fs::read(path).ok()`. `OsDiagnosticHost` = cli.rs:42-125 bodies; the stdout handling of `shell_version` is the pure helper `first_line_of(stdout: &[u8]) -> Option<String>` (lossy UTF-8, `lines().next()`, trim; empty stdout `None`, blank first line `Some("")`), unit-tested without a process. No test execs a freshly written script (ETXTBSY flake, rust-lang/rust#114554); the process test is: a missing program gives `None`.
* `cli/doctor_command.rs`: `DoctorArgs` (clap, copied byte for byte: the two doc comments, `#[arg(long, short)]`), `run_doctor_command(args, build_module: impl FnOnce() -> DiagnosticsModule) -> anyhow::Result<i32>`:
  1. `process_info::is_root()` FIRST: `eprintln!("{}", epaint("1;31", "Running doctor as root is not supported."))`; without `--all` also the second line (`epaint("31", ...)`) and `return Ok(1)`; with `--all` continue. No path, no state touched.
  2. only now `build_module()` (root without `--all` constructs nothing, by construction), then `module.run_doctor.execute(DoctorRequest { all, strict })?` then `Ok(i32::from(verdict == Failed))`.
  `cli/diagnostic_command.rs`: `DiagnosticArgs { #[arg(long, short, value_enum, default_value_t)] pub(crate) format: OutputFormat }`, `run_diagnostic_command` (one `println!` of `args.format.render(|| to_toml(&report), || json(&report))`, `Ok(0)`),
  `to_toml` (verbatim over `DiagnosticValue`: `Missing` rows dropped, key bare iff all chars ASCII alphanumeric / `_` / `-` (an EMPTY key is bare), else `Value::String(k).to_string()`, value = `Value::String(s).to_string()` or `true`/`false`,
  `out.trim_end()`), `json` (a `serde_json::Map` => keys ALPHABETICAL, nulls kept). `cli/debug_logs_command.rs`: `run_debug_logs_command(&DiagnosticsModule)` (order: see section 5).
* `DiagnosticsModule { run_doctor, collect_diagnostic_report, show_recent_logs }` and `build_diagnostics_module(dependencies, settings: Rc<dyn SettingsRepository>, restart_reminder: Rc<dyn RestartReminderRepository>, integrations: Rc<dyn IntegrationProbe>, bundled_specs: Rc<dyn BundledSpecsProbe>, fixer: Rc<dyn CheckFixer>)`.
  It ONLY builds (D3): no env read, no `which`, no pwsh, no file.

## 7. Bootstrap

`bootstrap/bridges.rs` (or a sibling `bootstrap/diagnostics_bridges.rs` with a line in `bootstrap/mod.rs` if it passes about 250 lines; bootstrap is not fitness-checked) gains the only code that may name
`IntegrationInstaller`, `integration_failures_error`, `embedded_spec_bundle::version`, `InstallSpecBundleUseCase`, `FileDotfileRepository`:

* `IntegrationProbeBridge { installer: Rc<IntegrationInstaller>, data_directory }`: `locate_paths` = `integration_paths_from(&data_directory)`; `list` = `installer.integrations`; `check` = `installer.check`;
  `script_binaries` = `script_bins` VERBATIM (doctor.rs:367-395: profile patched to `Some(PathBuf::new())` so pwsh is never spawned, `Shell::ALL` filtered by `which`, `integrations(..).unwrap_or_default()`,
  `read_to_string(p).ok()?.split('\'').nth(1)?`).
* `BundledSpecsBridge`: `is_extracted(data_root)` = `data_root.join("specs").join(version()).join("index.json").is_file()`.
* `CheckFixerBridge { installer, restart_reminder: Rc<dyn RestartReminderRepository>, spec_bundle: InstallSpecBundleUseCase, repairs: FileRepairs }`, FIVE explicit arms (D8, no catch-all) that apply the fix; after an `Ok` the bridge calls `restart_reminder.save_pending()` iff `fix.marks_restart_reminder()` (the policy is the domain rule, the bridge only delegates). `fn rendered(e: impl Into<anyhow::Error>) -> String { format!("{:#}", e.into()) }`:

| Fix | Body | Err text (after `Failed to fix: `) |
|---|---|---|
| `CreateDataDirectory(p)` | `repairs.create_data_directory(p)` | io Display, no path (`Permission denied (os error 13)`) |
| `InstallIntegration(i)` | `installer.install(paths, i)?` then (after the `Ok`) `save_pending()` through `marks_restart_reminder()` (true) | `ShellIntegrationError` Display (no source) |
| `InstallMissingIntegrations(shells)` | `installer.install_missing(paths, shells).map_err(integration_failures_error)?` then `save_pending()` through `marks_restart_reminder()` (true, even for `Ok(false)`) | `"\n\n"` + failure paragraphs, description bold iff stdout is a tty AT FIX TIME |
| `ResetSettings(p)` | `repairs.reset_settings(p)`; `marks_restart_reminder()` false: NO restart mark | io Display |
| `InstallSpecBundle` | `spec_bundle.execute()` (built from `build_specs_module(&deps).install_spec_bundle` in `diagnostics_module()`) | `SpecError` Display via `rendered` |

`paths` handed to `fix` is the ORIGINAL `IntegrationPaths` (pwsh_profile None): `install_missing` re-lists, so pwsh is spawned again exactly as today.
`bootstrap/cli.rs`: drop `use crate::{cli, doctor};`; `Doctor(DoctorArgs)`, `Diagnostic(DiagnosticArgs)` (doc comments, `#[command(alias = "diagnostics")]`, variant order frozen, D4); arms
`Some(Command::Doctor(a)) => run_doctor_command(a, diagnostics_module)`, `Some(Command::Diagnostic(a)) => run_diagnostic_command(a, &diagnostics_module())`, `DebugCmd::Logs => run_debug_logs_command(&diagnostics_module())`;
`fn diagnostics_module() -> DiagnosticsModule` next to `shell_integration_module()`:

```rust
let deps = SharedDependencies::from_env();
let settings = Rc::new(JsonFileSettingsRepository::new(deps.data_directory.clone()));
let restart: Rc<dyn RestartReminderRepository> = Rc::new(StateRestartReminderRepository::new(deps.data_directory.clone()));
let installer = Rc::new(IntegrationInstaller::new(Rc::new(FileDotfileRepository), Rc::new(FileBackupRepository), deps.clock.clone(), Rc::new(OsShellEnvironment)));
let spec_bundle = build_specs_module(&deps).install_spec_bundle;
build_diagnostics_module(&deps, settings, restart.clone(),
    Rc::new(IntegrationProbeBridge::new(installer.clone(), deps.data_directory.clone())), Rc::new(BundledSpecsBridge),
    Rc::new(CheckFixerBridge::new(installer, restart, spec_bundle, FileRepairs::new(deps.clock.clone()))))
```
`main.rs`: `mod cli;` and `mod doctor;` replaced by `mod diagnostics;`. `architecture_tests.rs`: `"diagnostics"` added to `REGISTERED_MODULES` (10.0); `cli` / `doctor` STAY in `LEGACY_MODULES` (precedent `pty`, `tracker`; Phase 11 empties it).

## 8. `RunDoctorUseCase::execute` (event order = doctor.rs:183-270 + `Run::group` 138-180)

```
paths   = integrations.locate_paths()?                       // Err => DiagnosticsError::IntegrationPaths; no summary, no epilogue (main prints `error: ...`)
parent  = system.parent_executable();  current = parent.as_deref().and_then(Shell::from_name)
run     = DoctorRun::new(all, strict);  if !all { reporter.start_progress() }
ok      = run_group(builder.dotfiles(..)) && run_group(builder.setup(..)) && run_group(builder.environment(..))      // each BUILT only if the previous passed
if ok { counts = run.counts(); run_group(builder.system(..)); run.restore(counts) }
reporter.stop_progress()
failed  = run.failed()
failed ? { blank; doctor_found_errors; blank } : { if !all { everything_looks_good }; blank; support_hint; blank }
if restart_reminder.is_pending() { reporter.restart_notice(); restart_reminder.clear_pending() }     // after a failed run and after --all; NOT after the root return or a paths error
Ok(verdict)

run_group(group):   // group is BUILT before this call, i.e. before its header
  if all { reporter.group_header(group.header()) }
  for check in checks where severity != Skip:
      out = run.effective(severity, evaluator.evaluate(check, paths))
      if all || !out.is_ok() { reporter.stop_progress(); reporter.outcome(name, &out) }
      if all { run.count(&out); continue }
      let Err{fix,..} = out else { run.count(&out); continue }
      if let Some(fix) = fix {
          reporter.fixing()
          match fixer.fix(&fix, paths) {
              Err(text) => reporter.fix_failed(&text),
              Ok(()) => { reporter.rechecking(); reporter.blank_line();
                          again = run.effective(severity, evaluator.evaluate(check, paths)); reporter.outcome(name, &again);
                          if !again.is_err() { run.count(&again); continue } }       // the original error is NOT counted; a still-Err re-run has no second fix
          }
      }
      reporter.blank_line(); run.abort(); return false
  if all { reporter.blank_line() }
  true
```

## 9. Check catalogue (every check; severity decided at BUILD, `Soft -> Warn` at EVALUATION incl. the re-run unless `--strict`)

| Group / check name | Severity | Subject | Facts (lazy, per evaluation) | Outcome / fix |
|---|---|---|---|---|
| 1 `{shell} {short_path} integration check` (one per integration, `Shell::ALL` order) | current shell (PARENT exe only) => Normal even off PATH; else Soft if `is_on_path`; else no check and NO list/spawn | `Dotfile(Integration)` | `IntegrationProbe::check` | Ok; NotInstalled/Improper => message with the prefix stripped; FileDoesNotExist/PermissionDenied/Io/Environment/SpecBundle => `to_string()`; info = hint `InfoLine::Command`; fix `InstallIntegration` except PermissionDenied (no fix) |
| 1 `{shell} integration check` (listing failed) | as above | `ListingFailure(text)` | none | Err(text) (`could not ask pwsh for $PROFILE: timed out after 5s` / `<io>` / `pwsh printed an empty $PROFILE`), no info, no fix; never re-run |
| 2 `tabsmith data dir exists` | Normal | `DataDirectory` | `directory_exists(data_root)` | Err `No directory at path {data_root}`; fix `CreateDataDirectory` |
| 2 `Integration scripts point at this binary` | Normal | `ScriptsPointAtBinary` | `script_binaries(paths)` | Err `Integration scripts reference {first stale bin}`; fix `InstallMissingIntegrations(stale shells, Vec::dedup)` |
| 2 `tabsmith settings corruption` | Normal | `SettingsCorruption(settings_file)` | `settings_file_loads(path)` | Err `tabsmith settings file is corrupted`; fix `ResetSettings(path)` |
| 2 `Completion specs installed` | Normal | `SpecsExtracted` | `BundledSpecsProbe::is_extracted(data_root)` | Err `Completion specs are not extracted`; fix `InstallSpecBundle` |
| 2 `Windows Console Check` (only when `operating_system()==Windows`) | Normal | `WindowsConsole` | `console_attached()` | Err `Windows Console APIs are not supported in this terminal` + 8 info lines (third is `""`, prints as two spaces), no fix |
| 2 `Windows version` (same condition) | Normal | `WindowsVersion` | `windows_build()` | Err (build < 17763) `Windows 10 1809 or newer is required (ConPTY)`; None/>= => Ok |
| 3 `tabsmith terminal integrations` | Normal | `Environment(TerminalIntegrations, facts)` | none (snapshot) | first match: Warp, cmd, `INSIDE_EMACS` (presence), SecureCRT, `PROCESS_LAUNCHED_BY_TABSMITH` (presence), `CI` (presence), then `TABSMITH_TERM` vs version: Ok / unset Err / other Err with info `TABSMITH_TERM={v}` |
| 3 `Session check` | `session_kind`: Normal iff session id is Some (even "") and the shell (parent, else `TABSMITH_SHELL`) is not pwsh; else Skip | `Environment(Session, facts)` | `file_exists(data_root/run/{sid}.json)` | Ok or `Warn("Shell has not reported its environment yet (tabsmith _ pre-cmd did not run)")`, never Err |
| 3 `Autocomplete is enabled` | Normal | `Environment(AutocompleteEnabled, facts)` | none | `autocomplete.disable` true => Err `Autocomplete disabled.` + info `To fix run: tabsmith settings autocomplete.disable false` |
| 3 `Autocomplete dev mode` | Normal | `Environment(DeveloperMode, facts)` | none | true => `Warn("autocomplete.developerMode is enabled")` |
| 3 `Inline` | Normal if `inline.enabled` else Soft (the Soft is dead: the outcome is Warn) | `Environment(Inline, facts)` | `env_is_non_empty(api_key_env)` via thunk | disabled => Warn; invalid source => Err `Inline source '{s}' is not a valid profile`; non-local profile with unset/empty key => Err `{KEY} is not set`; else Ok |
| 4 `OS is supported` | Normal | `System(Os)` | linux: none; macos: `tool_version("sw_vers", ["-productVersion"])`; windows: `windows_build()` | Warn only (`macOS {v} is not supported (needs macOS 11 or newer)`, `Windows 10 1809 or newer is required (ConPTY)`, `{os} is not supported`) |
| 4 `Bash is new enough` | Soft if current==Bash else Skip | `System(Bash{program})`, program = PARENT exe path | `tool_version(program, ["--version"])` | `v.is_below([4,4])` => Err `Bash {major}.{minor_or_zero} is not supported (needs >= 4.4 for PS0); install a newer bash or switch to zsh` |
| 4 `Fish is up to date` | Normal if `is_on_path("fish")` else Skip | `System(Fish)` | `tool_version("fish", ["--version"])` (the `fish` on PATH) | `v.is_below([3,3,0])` => Err `your fish version is outdated (need at least 3.3.0, found {v})` |

The system group never changes the exit code (counts restored) but still prints, aborts on a Normal Err, and `Everything looks good!` is still printed.

## 10. `cfg(windows)` and platform code

No `cfg` in domain or application (grep B15). The Windows rules are plain functions over `Option`/`bool` facts and are unit-tested on Linux; list membership uses `OperatingSystem::Windows`
(on a Windows build `consts::OS == "windows"`, on others never: identical to the old `#[cfg(windows)]` block). cfg lines after the phase: `os_system_facts_probe.rs` (the two Windows method bodies and their `not(windows)` twins),
`cursor_restorer.rs` (`cfg(unix)` / `cfg(windows)` pair, verbatim), nothing else. Removed: `#[cfg_attr(not(windows), allow(unused_mut))]` and the `#[cfg(windows)] "windows" =>` arm of `os_outcome`.
`gate.sh` prints NOTHING about Windows when the target or mingw is missing: 10.0 installs them (`rustup target add x86_64-pc-windows-gnu`, `apt-get install -y gcc-mingw-w64-x86-64`) and runs
`cargo check --locked --all-targets --target x86_64-pc-windows-gnu` once at `b0fe04d`; if that is impossible, every moved cfg hunk is hand-checked with the list of the recon (use paths, `windows_sys` names,
`unsafe extern "system" fn handler(_ctrl: u32) -> i32`, `// SAFETY:` comments, imports inside the Windows blocks) and the close report says Windows was not compiled.

## 11. Trap -> mechanism -> test (source-map doctor.md / cli.md re-verified, plus new ones)

| # | Trap | Mechanism | Pinned by |
|---|---|---|---|
| T1-T3 | lazy groups, re-run re-probes, Skip/after-abort never evaluated | section 2 | `doctor_group_soft_fix_abort_flow`, builder/use-case tests, doctor-diff |
| T4 | Soft Err -> Warn drops info and fix, also at the re-run; `--strict` keeps Err | `DoctorRun::effective` | `doctor_group_soft_fix_abort` (policy half) |
| T5 | print order, fix messages, `Re-running check...` then blank then outcome even if Ok, abort = blank + `errors += 1` directly | `run_group` + `Recording` reporter events | use-case tests, doctor-diff a-e |
| T6 | `--all`: header only here, blank after group, never fixes / aborts / re-runs, no spinner | `run_group` | use-case tests |
| T7 | `Warn` prints without the check name; info block = blank + `  {line}` (empty line = two spaces) | reporter `outcome_lines` | reporter styled / plain tests |
| T8 | system group counts restored | `counts()` / `restore()` | use-case tests, doctor-diff k2 |
| T9 | exit code `errors>0 \|\| (strict && warnings>0)` | `DoctorRun::failed` | entity tests |
| T10 | root check BEFORE path resolution; `--all` continues | `doctor_command` | doctor-diff m, r4 (run as root) |
| T11 | hard-coded `~/.tabsmith/logs/pty.log` hint | reporter literal | reporter test |
| T12 | epilogue: only `as_bool()==Some(true)`, two lines (second unstyled), insert `false` KEEPING the key, other keys kept, errors ignored, corrupt state skipped, after failed runs and `--all`, not after root / paths error | `is_pending` / `clear_pending`, use case order read-print-write | 10.1 repository tests, use-case tests, doctor-diff l1-l6 |
| T13 | dotfile severity from the PARENT only; `which` only for a non-current shell; Shell::ALL order | `dotfile_severity` + builder | catalog + builder tests |
| T14 | listing error check name and text | `listing_failure_check` | builder test |
| T15 | name `{shell} {short_path}`; mixed separators on Windows; `short_path(home, home) == "~/"` | `short_path` | catalog tests |
| T16 | reason prefix stripped for NotInstalled/Improper; PermissionDenied no fix; five explicit arms (D8) | `dotfile_outcome` | catalog tests |
| T17 | magenta span inside an info line, tty dependent | `InfoLine::Command` | reporter styled test |
| T18 | both integration fixes mark the reminder UNCONDITIONALLY (even `Ok(false)`); the settings fix does not | `CheckFix::marks_restart_reminder` (domain rule), `CheckFixerBridge` delegates | pure domain test, bridge tests |
| T19 | `script_bins`: profile `Some("")` (no 2nd pwsh spawn), PATH filter, errors swallowed, text between the first two `'` (a `'` in the path stays stale) | bridge `script_binaries` verbatim | bridge tests, doctor-diff d7 |
| T20 | `shells.dedup()` removes CONSECUTIVE duplicates only | `scripts_outcome` | catalog test `[(Bash,a),(Zsh,b),(Bash,c)]` |
| T21-T23 | settings predicate; settings fix (`with_extension` REPLACES, non-atomic `{}\n`, follows symlinks, keeps mode); data dir fix (default umask) | `FileRepairs`, `OsSystemFactsProbe` | file tests with `FixedClock`, doctor-diff f |
| T24 | specs check path vs fix | `BundledSpecsBridge`, bridge arm | bridge tests |
| T25 | `Failed to fix: {e:#}` texts per variant | `rendered()` | bridge tests per variant (with and without a tty for the bold span) |
| T26 | terminal rule ORDER (7 rules), presence vs UTF-8 reads | `terminal_outcome`, `TerminalEnvironment` | catalog tests (precedence, empty `CI=`) |
| T27-T29 | `session_kind`; inline outcome, `is_local` substring; getter defaults (`autocomplete.disable` false, `developerMode` false) | catalog | catalog tests; `doctor_session_check_skips_pwsh` |
| T30 | version parsing and LEXICOGRAPHIC compare (`3.` -> [3], overflow -> None, first dotted token, `[3,3]` outdated, `[4]` old) | `ToolVersion` | `doctor_version_parsing` + new tests |
| T31 | bash probed via the PARENT path, fish via PATH | `SystemCheck::Bash{program}` | builder + evaluator tests |
| T32-T33 | OS rules (Warn only); Windows version Err in setup, Warn in system; `windows_build` twice; console needs stdin AND stdout | catalog, probe | catalog tests, hand check |
| T34-T35 | spinner bytes / drop order; SIGINT exit 1, installed once, never removed | `Spinner`, `restore_cursor_on_ctrl_c` moved verbatim | doctor-diff r2/r3 (pty) |
| T36 | order: root, paths (`?`), parent, spinner, groups | `execute` | event-order test |
| T37 | header AFTER group construction (`--all`) | `execute` | `should_build_each_group_before_printing_its_header` |
| T38 | the dotfile list is a snapshot | builder / evaluator | `should_not_list_the_integrations_again_after_a_fix` |
| T39 | `# tabsmith ignore` passes without reading scripts | already in `IntegrationInstaller::check` | nothing to do |
| T40 | io fix errors print the BARE io Display (no path) | `rendered` | bridge / repairs tests |
| T41 | paths error propagates as `error: ...`; `DiagnosticsError` has no `source()` | `DiagnosticsError` | error test (`format!("{e:#}")` equals `to_string()`) |
| T42 | `Inline` Soft is dead; session id `Some("")` checks `run/.json` | catalog | catalog tests |
| C1 | JSON keys alphabetical + nulls kept; TOML insertion order + nulls dropped | `diagnostic_command` (Map vs rows) | `diagnostic_toml` + new JSON tests |
| C2 | scrubbing: `/user` then `\user` raw substrings, not version/os/env-var KEYS/booleans, empty user = none (`USER=sb` rewrites `/usr/sbin`) | `UsernameScrubber`, use case | scrubber + use-case tests |
| C3 | cli `env()` = non-empty UTF-8, doctor `has()` = presence | two ports | host / probe tests |
| C4 | `$SHELL --version`: no timeout, stdout only, first line trimmed, blank first line = `Some("")` (JSON `""`) | `OsDiagnosticHost` | host test with a script, use-case test |
| C5-C7 | terminal fallback chain; `in_wsl` gating; env-vars filter / sort byte order (`TERM` after `TABSMITH_*`), empty values kept, non-UTF-8 key dropped | use case + host | use-case tests, doctor-diff p |
| C8 | TOML bare keys / quoting / trim | `to_toml` | `diagnostic_toml` + new |
| C9 | debug logs: paths printed even if missing, and BEFORE the pty.log read; HOME error before any print; last 50 lines; blank line even for an empty file | `ShowRecentLogsUseCase::paths` / `pty_tail`, command | use-case tests, golden, doctor-diff q |
| C10 | D3: missing HOME fails at the same moment with the same text | `diagnostics_module()` only builds | module test, golden no-HOME lines |
| C11 | `now_secs` -> `Clock` | `FileRepairs` | backup-name test |

## 12. Deviations from the plan's letter (D1; report each in the final answer)

1. `CheckFix::ReinstallIntegrations` becomes `InstallMissingIntegrations` (the fix is `install_missing`, install-if-not-installed with backups; the reinstall use case uninstalls first). Glossary updated.
2. `SystemFacts` is the env group's snapshot, not a monolith; per-check facts are gathered through ports by `CheckGroupBuilder` / `CheckEvaluator` (lazy groups, re-probe after a fix).
3. `RestartReminderRepository` gains `is_pending` / `clear_pending` (insert `false`, keep the key) instead of the plan's `find` / `delete` (delete would change state.json bytes). The epilogue reads state.json twice (is_pending, then clear_pending reloads): unobservable.
4. The settings reset and data-dir creation fixes are plain `std::fs` in `FileRepairs`, not the settings repository (backup copy, non-atomic write, mode).
5. More ports than the plan's four: `IntegrationProbe`, `BundledSpecsProbe` (bootstrap bridges), `SettingsSnapshot`, `DiagnosticHost`; and more bridges than `CheckFixer`.
6. New VOs: `InfoLine`, `DiagnosticValue`, `CheckSubject`, `EnvironmentCheck`, `SystemCheck`, `TerminalEnvironment`, `OperatingSystem`; `DiagnosticsError`; services `CheckGroupBuilder`, `CheckEvaluator`.
7. `Integration` derives `PartialEq, Eq` (shell_integration, no behaviour).
8. No `cfg(windows)` on port methods or in domain/application; Windows list membership via `OperatingSystem::Windows`.
9. `doctor_script_bins_skip_orphans` moves to the bootstrap bridge tests (the code under test lives there); `doctor_group_soft_fix_abort` is split (policy half keeps the name in `doctor_run.rs`, flow half is `doctor_group_soft_fix_abort_flow`).
10. `LEGACY_MODULES` keeps `cli` and `doctor` (precedent `pty`, `tracker`).
11. The plan's "launch rules reuse `LaunchPolicy`" is NOT done (doctor's terminal rules differ in order and messages: source-map "Plan gaps").
12. `diagnostics` reaches `specs` and the installer only through bootstrap bridges, so the plan's allowed edges `specs` and `terminal_session` are unused.

## 13. Test plan

**The five locked tests, plus the flow half of the split one** (names kept; each is created at its destination in 10.18a, in the SAME commit that deletes `doctor.rs` / `cli.rs`, so no name exists twice at any commit; earlier steps use other names):

| Locked test | Destination | Change |
|---|---|---|
| `doctor_group_soft_fix_abort` | `domain/entities/doctor_run.rs` | the three scenarios drive `DoctorRun` (`effective`, `count`, `abort`) through the same sequence; asserts `(1,1)`, `(1,0)`, `(2,1)` |
| `doctor_group_soft_fix_abort_flow` (the other half, D6) | `application/run_doctor_use_case/tests.rs` | group returns false / false / true, the fixer ran (true) / not (false); `run.counts()` == (1,1), (1,0), (2,1) via the `DoctorRun` passed to `run_group`, plus the probe `EventLog`; skipped and after-abort checks are subjects whose probe PANICS |
| `doctor_script_bins_skip_orphans` | `bootstrap/bridges.rs` tests | same fixtures (`TempDir`, `IntegrationPaths::new`, `install_dotfiles`, `with_binary`), calls the bridge's `script_binaries`; keeps `if which("bash").is_some()` |
| `doctor_session_check_skips_pwsh` | catalog tests | `Kind` -> `CheckSeverity`; the five `assert_eq!` unchanged; `session_kind` keeps its signature |
| `doctor_version_parsing` | `tool_version.rs` | `version_of(..)` -> `ToolVersion::parse(..)` compared through `components()`; the last slice-order assert kept |
| `diagnostic_toml` | `diagnostic_command.rs` | rows `Text("0.1.0")`, `Missing`, `Text("/a:\"b\"")`, `Flag(false)`; identical expected string |

**Re-homed** (10.18b, same names, same commit as the deletion of `load_or_defaults.rs`): `should_give_the_settings_the_repository_loads`, `should_word_the_warning_as_settings_then_the_error_and_each_cause`,
`should_fall_back_to_the_default_settings_when_they_cannot_be_loaded` -> `lenient_settings.rs` (`load_or_defaults(&repository)` becomes `LenientSettings::new(Rc::new(repository)).load()`). The locked name
`should_word_the_session_settings_warning_like_the_load_or_defaults_one` stays where it is.

**New tests** (names `should_...`, AAA, no mocks; check `git grep -n "fn <name>\b"` first): every row of section 9 from the facts (the `case_` rows of the doctor-diff transcript are the oracle: one fact set + its exact output);
`ToolVersion` traps; `UsernameScrubber`; `DoctorRun`; builder (severity matrix, no list for an off-PATH shell, settings loaded once, header consts); evaluator (laziness with `Panicking` probes); use case (R-quiet-ok, R-all-ok,
R-fix-ok / fail / still-err, R-no-fix-abort, R-system-ignored, R-exit, R-ctx-error, R-header-after-prepare, R-reminder, event order); reporter (plain and styled bytes); bridge fixer strings; `CheckFix::marks_restart_reminder` (the five arms); `first_line_of` (empty stdout, blank first line, lossy UTF-8, trimmed first line) and a missing program gives `None`; use-case Soft + `--strict` + a fix (fixed then Ok; still failing aborts with `errors == 1` and no second `Fixing`);
epilogue order tests through the test-only decorator `LoggingRestartReminder` (`application/test_fixtures.rs`: wraps a `RestartReminderRepository` and pushes `reminder.is_pending` / `reminder.clear_pending` into the `EventLog`); `doctor_group_soft_fix_abort_flow` MUST assert `run.counts()` == (1,1), (1,0), (2,1)
via the `DoctorRun` it passes to `run_group`, plus the probe `EventLog` (the `DoctorRun` replay in `doctor_run.rs` is the arithmetic half only); `FileRepairs` (backup name with `FixedClock`,
`settings.conf` -> `settings.json.bak-N`); repository `is_pending` / `clear_pending`; `to_toml` / JSON key order; `ShowRecentLogsUseCase` (`paths()` then `pty_tail()`: 49 / 50 / 51 / 120 lines, empty file, missing file, no HOME).
Test modules over ~500 lines go to a sibling `tests.rs` (catalog, builder, use case).

**Net**: `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh` before every commit; `testlock.py . ef807c1` and `. b0fe04d` (MISSING 0, DUPLICATE 0 on both; read CHANGED by hand: only the locked tests above); `doctor-diff.sh` (cmp);
`integrations-diff.sh`, `inline-diff.sh` (h1-h4 and the Inline lines) empty against the baseline binary; `cargo check --locked --no-default-features --all-targets`; `cargo test --locked --release -- --ignored all_bundle_modules`;
the windows check when installed; `scripts/hexagonal-refactor/phase10-greps.sh`.

## 14. Rules for every step (the step prompts refer to this section)

1. Load the skills `xp-tdd`, `testing-standard`, `coding-standards`, `hexagonal-architecture` first (AGENTS.md). Read `agent-brief.md`, this document's sections named by the step, and the code to move (`git show phase10-start:src/doctor.rs`).
2. Refactor: move code verbatim first (script, then diff the moved block against `git show phase10-start:...`), then rename, then extract. New seams are test-first: RED commit-able test, GREEN, REFACTOR; one commit per GREEN / REFACTOR when the step says so.
3. Tests are locked (section 13). Never edit an existing test's assertions or inputs. Every new test name must be unique tree-wide.
4. One type per snake_case file, `pub(crate)` at most, `mod.rs` declares children only, no glob re-exports, no `use` in `main.rs`, no `anyhow` / `clap` / `serde_json` / `println!` / `std::fs` / `std::env` / `Command` / threads / cfg in domain or application (greps B).
5. Do not touch `Cargo.toml` / `Cargo.lock`. No new crates. `#![allow(dead_code)]` stays. Test-only items get `#[cfg(test)]` (D7): every item of a test double or fixture, including each `use` line only it needs, carries its own `#[cfg(test)]` (precedent `terminal_session/application/test_fixtures.rs`); never `#![cfg(test)]` (the fitness scanner does not understand it).
   Use named structs or type aliases for scripted maps (`clippy::type_complexity`). `BASELINE_CLIPPY=4` has no headroom: run `cargo clippy --all-targets --locked` before committing in the double-heavy steps (10.7a, 10.7b, 10.10a, 10.10b).
6. Before the commit: `BASELINE_CLIPPY=4 scripts/hexagonal-refactor/gate.sh` must print `GATE PASS` (use `--fast` while iterating); `scripts/hexagonal-refactor/phase10-greps.sh` must show no unexpected FAIL (A and E1 FAIL until the steps that clear them);
   `python3 -B scripts/hexagonal-refactor/testlock.py . ef807c1` and `. phase10-start`: MISSING 0, DUPLICATE 0.
7. Commit with `git commit -F <file>` (never `-m`; never amend / rebase / force; never push). Subject `refactor(diagnostics): <why>` / `test(diagnostics): ...` / `docs: ...` / `chore(scripts): ...`, at most 72 characters, trailers
   `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` and `Claude-Session: <the session link>`. `docs/git-convention.md` does not exist.
8. Report per step: what changed, commit hash(es), the gate lines, every CHANGED test with its justification, every deviation, anything suspicious left unfixed (never "fix" a behaviour).

## 15. Acceptance greps (`scripts/hexagonal-refactor/phase10-greps.sh [base]`)

Groups: A legacy gone (files, `mod` lines, `crate::cli` / `doctor::` paths, comments naming `doctor.rs` / `src/cli.rs`, `load_or_defaults`, `types::now_secs` callers); B layers (legacy modules, other modules' infrastructure / use cases,
`IntegrationInstaller` and friends only in bootstrap, re-exports, visibility, anyhow / clap only in `infrastructure/cli`, platform crates, serde, println / logger / paint, std I/O / env / process / thread / clock / `consts::OS` and cfg in domain and
application); C cfg lines before / after; D the locked test names once each, `#[ignore]` count, testlock lines; E registration, `LEGACY_MODULES`; F names only in diagnostics / bootstrap, one `diagnostics_module()`, one Ctrl-C file, `println!` only in the
reporter and the commands, `Failed to fix: `, the hint and the root message once each; G every string literal of the old files still present; Z file count. Expected REVIEW noise at the close: B8, B14 (none after `tabsmith_version` is a fact), B18, C1, D4, G1 (formatted pieces).

## 16. Phase 11 hand-offs

* `types.rs` holds only `now_ms` and `now_secs`, both without callers: delete the file; empty `LEGACY_MODULES` (`cli` and `doctor` are dead names).
* Dead-code watch list (D7): `CheckSeverity` accessors used only by tests, `OperatingSystem::Other` payload, `FileRepairs` clock field if unused, `InMemory*` doubles, `DiagnosticsError::DataDirectory`, `ToolVersion::components` if only tests read it.
* `docs/SPEC.md` rows naming `doctor.rs` / `cli.rs` (section 4.1, 4.3, 10) are Phase 11's.
* The manual smoke (agents cannot drive a terminal): `tabsmith doctor` in a real terminal shows the spinner on stderr and clears it, Ctrl-C during it restores the cursor and exits 1 (P10-S1); `tabsmith doctor --all` colours (P10-S2);
  Windows: `Windows Console Check`, `Windows version`, Ctrl-C handler (CI only, P10-S3). `doctor-diff.sh` pins the spinner clear bytes and Ctrl-C under a pty on Linux, not Windows, not macOS `sw_vers`.
* `debug_shell_post_script` is still a third wiring place of `RenderInitScriptUseCase`.

## 17. Open questions and the defaults applied

* Is `doctor-diff.sh` added to `gate.sh`? Default: no (it needs a built binary, a C compiler and python3, and takes 3.5 min); it is committed under `scripts/hexagonal-refactor/` and run at 10.17, 10.18a, 10.20.
* Windows target on this machine? Default: install in 10.0; else hand-check and say so.
* `cli` / `doctor` stay in `LEGACY_MODULES` (default keep).
* The PR: the user's request authorises one new PR for this branch off PR #1's branch; the orchestrator opens it after 10.20 (the steps never push).

## 18. Steps (28; each one reviewable commit group that leaves the gate green; locked tests are created only in 10.18a / 10.18b)

| Id | Title |
|---|---|
| 10.0 | Guards: tag, Windows check, greps and doctor-diff scripts, register `diagnostics` (skeleton), commit this design |
| 10.1 | shell_integration: `Integration: PartialEq, Eq`; `RestartReminderRepository::is_pending` / `clear_pending` (test-first) |
| 10.2a | Domain leaf VOs: `CheckSeverity`, `InfoLine`, `CheckFix`, `CheckOutcome`, `OperatingSystem`, `ToolVersion`, `DiagnosticsError` |
| 10.2b | Domain structure VOs: `TerminalEnvironment`, `SystemFacts`, `EnvironmentCheck`, `SystemCheck`, `CheckSubject`, `Check`, `CheckGroup` |
| 10.3 | `DoctorRun` entity |
| 10.4 | `DoctorCheckCatalog` part 1: dotfile and setup groups |
| 10.5 | `DoctorCheckCatalog` part 2: environment and system groups |
| 10.6 | `UsernameScrubber`, `DiagnosticValue`, `DiagnosticSection`, `DiagnosticReport` |
| 10.7a | Doctor ports, DTOs, doubles, `EventLog` |
| 10.7b | Diagnostic ports (`LogReader`, `DiagnosticHost`), `LogPaths` |
| 10.8 | `CheckGroupBuilder` |
| 10.9 | `CheckEvaluator` |
| 10.10a | `RunDoctorUseCase` group loop |
| 10.10b | `RunDoctorUseCase::execute` (whole run, summary, epilogue) |
| 10.11 | `CollectDiagnosticReportUseCase`, `ShowRecentLogsUseCase` |
| 10.12a | `OsSystemFactsProbe` (cfg(windows) bodies moved verbatim) |
| 10.12b | `LenientSettings`, `FileRepairs` |
| 10.13a | Move `Spinner` and `restore_cursor_on_ctrl_c` verbatim out of doctor.rs |
| 10.13b | `TerminalDoctorReporter` |
| 10.14 | `FileLogReader`, `OsDiagnosticHost` |
| 10.15a | `DiagnosticsModule`, `build_diagnostics_module`, `doctor_command` |
| 10.15b | `diagnostic_command`, `debug_logs_command` |
| 10.16 | Bootstrap bridges: `IntegrationProbeBridge`, `BundledSpecsBridge`, `CheckFixerBridge` |
| 10.17 | Rewire `bootstrap/cli.rs`; doctor-diff cmp |
| 10.18a | Delete `doctor.rs` / `cli.rs`; create the locked tests at their homes; repoint stale comments |
| 10.18b | Delete `load_or_defaults`; re-home its three tests |
| 10.19 | Docs: plan, glossary, README, hand-off |
| 10.20 | Close: full verification |

Runs (for the workflow): run 1 = 10.0-10.6 (guards and domain), run 2 = 10.7a-10.11 (application), run 3 = 10.12a-10.15b (infrastructure), run 4 = 10.16-10.20 (bridges, rewire, deletion, docs, close).
