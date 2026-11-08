use crate::modules::diagnostics::application::ports::check_fixer::RecordingCheckFixer;
use crate::modules::diagnostics::application::ports::integration_probe::InMemoryIntegrationProbe;
use crate::modules::diagnostics::application::test_fixtures::{sample_paths, DoctorHarness};
use crate::modules::diagnostics::domain::entities::doctor_run::DoctorRun;
use crate::modules::diagnostics::domain::services::doctor_check_catalog as catalog;
use crate::modules::diagnostics::domain::value_objects::check::Check;
use crate::modules::diagnostics::domain::value_objects::check_fix::CheckFix;
use crate::modules::diagnostics::domain::value_objects::check_group::CheckGroup;
use crate::modules::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::modules::diagnostics::domain::value_objects::check_severity::CheckSeverity;
use crate::modules::diagnostics::domain::value_objects::check_subject::CheckSubject;
use crate::modules::shared::domain::value_objects::shell::Shell;
use crate::modules::shell_integration::domain::entities::integration::Integration;
use crate::modules::shell_integration::domain::shell_integration_error::ShellIntegrationError;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

const DATA_ROOT: &str = "/home/u/.tabsmith";
const DIRECTORY_QUESTION: &str = "system.directory_exists(/home/u/.tabsmith)";
const SPECS_QUESTION: &str = "bundled_specs.is_extracted(/home/u/.tabsmith)";
const DOTFILE_QUESTION: &str = "integrations.check(bash pre post into .bashrc)";

fn check(name: &str, severity: CheckSeverity, subject: CheckSubject) -> Check {
    Check::new(name.to_string(), severity, subject)
}

fn data_directory() -> Check {
    check("Data directory", CheckSeverity::Normal, CheckSubject::DataDirectory)
}

fn specs() -> Check {
    check("Specs", CheckSeverity::Normal, CheckSubject::SpecsExtracted)
}

fn listing_failure(severity: CheckSeverity) -> Check {
    check("Listing", severity, CheckSubject::ListingFailure("cannot list".to_string()))
}

fn bashrc() -> Integration {
    Integration::dotfile(Shell::Bash, PathBuf::from("/home/u/.bashrc"))
}

fn soft_dotfile() -> Check {
    check("bashrc", CheckSeverity::Soft, CheckSubject::Dotfile(bashrc()))
}

fn group(checks: Vec<Check>) -> CheckGroup {
    CheckGroup::new("Setup".to_string(), checks)
}

fn missing_directory() -> CheckOutcome {
    catalog::data_directory_outcome(Path::new(DATA_ROOT), false)
}

fn broken_dotfile() -> CheckOutcome {
    catalog::dotfile_outcome(&bashrc(), Err(ShellIntegrationError::NotInstalled("no block".to_string())))
}

fn outcome_event(name: &str, outcome: &CheckOutcome) -> String {
    format!("reporter.outcome({name}, {outcome:?})")
}

fn event(name: &str) -> String {
    format!("reporter.{name}")
}

fn run_group(harness: &DoctorHarness, run: &mut DoctorRun, group: &CheckGroup) -> bool {
    harness.use_case().run_group(run, group, &sample_paths())
}

fn harness_without_the_data_directory() -> DoctorHarness {
    let harness = DoctorHarness::new();
    harness.system.script_directory(DATA_ROOT, false);
    harness
}

fn harness_that_can_create_the_data_directory() -> DoctorHarness {
    let harness = harness_without_the_data_directory();
    let system = harness.system.clone();
    let creating = RecordingCheckFixer::succeeding().with_effect(move || system.script_directory(DATA_ROOT, true));
    harness.with_fixer(creating)
}

fn harness_with_a_dotfile_broken_until(repaired: &Rc<Cell<bool>>) -> DoctorHarness {
    let flag = repaired.clone();
    let probe = InMemoryIntegrationProbe::default().checking(bashrc(), move || {
        if flag.get() {
            Ok(())
        } else {
            Err(ShellIntegrationError::NotInstalled("no block".to_string()))
        }
    });
    DoctorHarness::new().with_integrations(probe)
}

fn harness_that_repairs_the_dotfile() -> DoctorHarness {
    let repaired = Rc::new(Cell::new(false));
    let flag = repaired.clone();
    let repairing = RecordingCheckFixer::succeeding().with_effect(move || flag.set(true));
    harness_with_a_dotfile_broken_until(&repaired).with_fixer(repairing)
}

fn asked(harness: &DoctorHarness, question: &str) -> usize {
    harness.probe_questions().iter().filter(|entry| entry.as_str() == question).count()
}

#[test]
fn should_print_nothing_and_go_on_when_every_check_passes_without_all() {
    let harness = DoctorHarness::new();
    harness.system.script_directory(DATA_ROOT, true);
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![data_directory(), specs()]));

    assert_eq!((go_on, harness.reporter_events(), run.counts()), (true, Vec::<String>::new(), (0, 0)));
}

#[test]
fn should_not_stop_the_progress_for_a_passing_check_without_all() {
    let harness = DoctorHarness::new();
    harness.system.script_directory(DATA_ROOT, true);
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert!(!harness.reporter_events().contains(&event("stop_progress")));
}

#[test]
fn should_stop_the_progress_before_the_first_check_that_is_not_ok() {
    let harness = DoctorHarness::new();
    harness.system.script_directory(DATA_ROOT, true);
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory(), listing_failure(CheckSeverity::Normal)]));

    assert_eq!(
        harness.reporter_events()[0..2],
        [event("stop_progress"), outcome_event("Listing", &CheckOutcome::err("cannot list"))]
    );
}

#[test]
fn should_print_a_header_an_outcome_per_check_and_a_blank_line_for_a_group_with_all() {
    let harness = DoctorHarness::new();
    harness.system.script_directory(DATA_ROOT, true);
    let mut run = DoctorRun::new(true, false);

    run_group(&harness, &mut run, &group(vec![data_directory(), specs()]));

    assert_eq!(
        harness.reporter_events(),
        [
            event("group_header(Setup)"),
            event("stop_progress"),
            outcome_event("Data directory", &CheckOutcome::Ok),
            event("stop_progress"),
            outcome_event("Specs", &CheckOutcome::Ok),
            event("blank_line"),
        ]
    );
}

#[test]
fn should_print_the_header_of_each_group_it_runs_with_all() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(true, false);

    run_group(&harness, &mut run, &CheckGroup::new("First".to_string(), Vec::new()));
    run_group(&harness, &mut run, &CheckGroup::new("Second".to_string(), Vec::new()));

    assert_eq!(
        harness.reporter_events(),
        [
            event("group_header(First)"),
            event("blank_line"),
            event("group_header(Second)"),
            event("blank_line"),
        ]
    );
}

#[test]
fn should_not_print_the_header_of_a_group_without_all() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(Vec::new()));

    assert_eq!(harness.reporter_events(), Vec::<String>::new());
}

#[test]
fn should_report_the_fix_and_the_second_outcome_when_the_fix_repairs_the_check() {
    let harness = harness_that_can_create_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(
        harness.reporter_events(),
        [
            event("stop_progress"),
            outcome_event("Data directory", &missing_directory()),
            event("fixing"),
            event("rechecking"),
            event("blank_line"),
            outcome_event("Data directory", &CheckOutcome::Ok),
        ]
    );
}

#[test]
fn should_hand_the_fix_of_the_failed_check_to_the_fixer_with_the_paths() {
    let harness = harness_that_can_create_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(
        (harness.fixer.fixes(), harness.fixer.paths().len()),
        (vec![CheckFix::CreateDataDirectory(PathBuf::from(DATA_ROOT))], 1)
    );
}

#[test]
fn should_ask_the_probe_again_for_a_check_it_fixed() {
    let harness = harness_that_can_create_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(asked(&harness, DIRECTORY_QUESTION), 2);
}

#[test]
fn should_go_on_without_counting_the_error_of_a_check_the_fix_repaired() {
    let harness = harness_that_can_create_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!((go_on, run.counts()), (true, (0, 0)));
}

#[test]
fn should_report_the_failure_and_abort_when_the_fixer_fails() {
    let harness = harness_without_the_data_directory().with_fixer(RecordingCheckFixer::failing("disk full"));
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(
        (go_on, harness.reporter_events()),
        (
            false,
            vec![
                event("stop_progress"),
                outcome_event("Data directory", &missing_directory()),
                event("fixing"),
                event("fix_failed(disk full)"),
                event("blank_line"),
            ]
        )
    );
}

#[test]
fn should_count_one_error_when_the_fixer_fails() {
    let harness = harness_without_the_data_directory().with_fixer(RecordingCheckFixer::failing("disk full"));
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(run.counts(), (1, 0));
}

#[test]
fn should_print_the_second_error_and_abort_when_the_fix_does_not_repair_the_check() {
    let harness = harness_without_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(
        (go_on, harness.reporter_events()),
        (
            false,
            vec![
                event("stop_progress"),
                outcome_event("Data directory", &missing_directory()),
                event("fixing"),
                event("rechecking"),
                event("blank_line"),
                outcome_event("Data directory", &missing_directory()),
                event("blank_line"),
            ]
        )
    );
}

#[test]
fn should_fix_a_check_only_once_when_the_rerun_still_fails() {
    let harness = harness_without_the_data_directory();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!((harness.fixer.fixes().len(), run.counts()), (1, (1, 0)));
}

#[test]
fn should_abort_without_a_fix_attempt_when_the_error_carries_no_fix() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![listing_failure(CheckSeverity::Normal)]));

    assert_eq!(
        (go_on, harness.reporter_events(), harness.fixer.fixes(), run.counts()),
        (
            false,
            vec![
                event("stop_progress"),
                outcome_event("Listing", &CheckOutcome::err("cannot list")),
                event("blank_line"),
            ],
            Vec::new(),
            (1, 0)
        )
    );
}

#[test]
fn should_ask_no_probe_for_a_listing_failure() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![listing_failure(CheckSeverity::Normal)]));

    assert_eq!(harness.probe_questions(), Vec::<String>::new());
}

#[test]
fn should_never_probe_a_check_that_is_skipped() {
    let harness = DoctorHarness::new();
    let skipped = check("Data directory", CheckSeverity::Skip, CheckSubject::DataDirectory);
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![skipped, specs()]));

    assert_eq!((go_on, harness.probe_questions()), (true, vec![SPECS_QUESTION.to_string()]));
}

#[test]
fn should_not_print_a_skipped_check_with_all() {
    let harness = DoctorHarness::new();
    let skipped = check("Data directory", CheckSeverity::Skip, CheckSubject::DataDirectory);
    let mut run = DoctorRun::new(true, false);

    run_group(&harness, &mut run, &group(vec![skipped]));

    assert_eq!(harness.reporter_events(), [event("group_header(Setup)"), event("blank_line")]);
}

#[test]
fn should_never_probe_the_checks_after_an_abort() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![listing_failure(CheckSeverity::Normal), data_directory()]));

    assert_eq!((go_on, harness.probe_questions()), (false, Vec::<String>::new()));
}

#[test]
fn should_turn_a_soft_error_into_a_counted_warning_and_go_on() {
    let harness = DoctorHarness::new();
    harness.specs.set_extracted(false);
    let soft_specs = check("Specs", CheckSeverity::Soft, CheckSubject::SpecsExtracted);
    let mut run = DoctorRun::new(false, false);

    let go_on = run_group(&harness, &mut run, &group(vec![soft_specs]));

    assert_eq!(
        (go_on, harness.reporter_events(), run.counts()),
        (
            true,
            vec![
                event("stop_progress"),
                outcome_event("Specs", &CheckOutcome::Warn("Completion specs are not extracted".to_string())),
            ],
            (0, 1)
        )
    );
}

#[test]
fn should_not_fix_a_soft_error_that_carries_a_fix_when_not_strict() {
    let harness = DoctorHarness::new();
    harness.specs.set_extracted(false);
    let soft_specs = check("Specs", CheckSeverity::Soft, CheckSubject::SpecsExtracted);
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![soft_specs]));

    assert_eq!(harness.fixer.fixes(), Vec::new());
}

#[test]
fn should_probe_the_checks_after_a_soft_warning() {
    let harness = DoctorHarness::new();
    harness.specs.set_extracted(false);
    harness.system.script_directory(DATA_ROOT, true);
    let soft_specs = check("Specs", CheckSeverity::Soft, CheckSubject::SpecsExtracted);
    let mut run = DoctorRun::new(false, false);

    run_group(&harness, &mut run, &group(vec![soft_specs, data_directory()]));

    assert_eq!(harness.probe_questions(), [SPECS_QUESTION, DIRECTORY_QUESTION]);
}

#[test]
fn should_keep_a_soft_error_without_a_fix_when_strict_and_abort() {
    let harness = DoctorHarness::new();
    let mut run = DoctorRun::new(false, true);

    let go_on = run_group(&harness, &mut run, &group(vec![listing_failure(CheckSeverity::Soft)]));

    assert_eq!(
        (go_on, harness.reporter_events(), run.counts()),
        (
            false,
            vec![
                event("stop_progress"),
                outcome_event("Listing", &CheckOutcome::err("cannot list")),
                event("blank_line"),
            ],
            (1, 0)
        )
    );
}

#[test]
fn should_fix_a_soft_dotfile_error_under_strict_and_count_the_rerun() {
    let harness = harness_that_repairs_the_dotfile();
    let mut run = DoctorRun::new(false, true);

    let go_on = run_group(&harness, &mut run, &group(vec![soft_dotfile()]));

    assert_eq!(
        (go_on, harness.reporter_events(), harness.fixer.fixes(), run.counts(), asked(&harness, DOTFILE_QUESTION)),
        (
            true,
            vec![
                event("stop_progress"),
                outcome_event("bashrc", &broken_dotfile()),
                event("fixing"),
                event("rechecking"),
                event("blank_line"),
                outcome_event("bashrc", &CheckOutcome::Ok),
            ],
            vec![CheckFix::InstallIntegration(bashrc())],
            (0, 0),
            2
        )
    );
}

#[test]
fn should_abort_a_soft_dotfile_error_under_strict_when_the_fix_does_not_repair_it() {
    let repaired = Rc::new(Cell::new(false));
    let harness = harness_with_a_dotfile_broken_until(&repaired);
    let mut run = DoctorRun::new(false, true);

    let go_on = run_group(&harness, &mut run, &group(vec![soft_dotfile()]));

    assert_eq!(
        (go_on, harness.fixer.fixes().len(), run.counts()),
        (false, 1, (1, 0))
    );
}

#[test]
fn should_not_fix_a_second_time_when_a_soft_dotfile_stays_broken_under_strict() {
    let repaired = Rc::new(Cell::new(false));
    let harness = harness_with_a_dotfile_broken_until(&repaired);
    let mut run = DoctorRun::new(false, true);

    run_group(&harness, &mut run, &group(vec![soft_dotfile()]));

    assert_eq!(
        harness.reporter_events().iter().filter(|e| **e == event("fixing")).count(),
        1
    );
}

#[test]
fn should_print_and_count_every_check_with_all_without_fixing_or_aborting() {
    let harness = harness_without_the_data_directory();
    harness.specs.set_extracted(false);
    let soft_specs = check("Specs", CheckSeverity::Soft, CheckSubject::SpecsExtracted);
    let checks = vec![data_directory(), soft_specs, listing_failure(CheckSeverity::Normal)];
    let mut run = DoctorRun::new(true, false);

    let go_on = run_group(&harness, &mut run, &group(checks));

    assert_eq!(
        (go_on, run.counts(), harness.fixer.fixes(), asked(&harness, DIRECTORY_QUESTION)),
        (true, (2, 1), Vec::new(), 1)
    );
}

#[test]
fn should_not_announce_a_fix_with_all() {
    let harness = harness_without_the_data_directory();
    let mut run = DoctorRun::new(true, false);

    run_group(&harness, &mut run, &group(vec![data_directory()]));

    assert_eq!(
        harness.reporter_events(),
        [
            event("group_header(Setup)"),
            event("stop_progress"),
            outcome_event("Data directory", &missing_directory()),
            event("blank_line"),
        ]
    );
}

fn soft_console() -> Check {
    check("Console", CheckSeverity::Soft, CheckSubject::WindowsConsole)
}

fn skipped_check() -> Check {
    let settings = CheckSubject::SettingsCorruption(PathBuf::from("/home/u/settings.json"));
    check("Skipped", CheckSeverity::Skip, settings)
}

fn check_after_the_abort() -> Check {
    check("After", CheckSeverity::Normal, CheckSubject::Dotfile(bashrc()))
}

fn fatal_check() -> Check {
    listing_failure(CheckSeverity::Normal)
}

/// The data directory is missing until the fixer creates it, the console is not attached; the skipped and the
/// after-abort subjects are unscripted, so their probe panics if it is ever asked.
fn harness_of_the_legacy_group() -> DoctorHarness {
    let harness = harness_that_can_create_the_data_directory();
    harness.system.script_console_attached(false);
    harness
}

#[test]
fn doctor_group_soft_fix_abort_flow() {
    // Soft error -> warning (not with --strict); fixed error -> continue; unfixed error -> abort.
    let checks = || {
        vec![
            soft_console(),
            skipped_check(),
            data_directory(),
            fatal_check(),
            check_after_the_abort(),
        ]
    };
    let harness = harness_of_the_legacy_group();
    let mut run = DoctorRun::new(false, false);
    assert!(!run_group(&harness, &mut run, &group(checks())));
    assert_eq!(
        harness.fixer.fixes(),
        vec![CheckFix::CreateDataDirectory(PathBuf::from(DATA_ROOT))]
    );
    assert_eq!(run.counts(), (1, 1));
    assert_eq!(
        harness.probe_questions(),
        ["system.console_attached", DIRECTORY_QUESTION, DIRECTORY_QUESTION]
    );

    // --strict: a Soft error stays an error.
    let harness = harness_of_the_legacy_group();
    let mut run = DoctorRun::new(false, true);
    assert!(!run_group(&harness, &mut run, &group(vec![soft_console()])));
    assert_eq!(run.counts(), (1, 0));
    assert_eq!(harness.probe_questions(), ["system.console_attached"]);

    // --all: never fixes, never aborts.
    let harness = harness_of_the_legacy_group();
    let mut run = DoctorRun::new(true, false);
    let mut list = checks();
    list.pop();
    assert!(run_group(&harness, &mut run, &group(list)));
    assert_eq!(harness.fixer.fixes(), Vec::new());
    assert_eq!(run.counts(), (2, 1));
    assert_eq!(
        harness.probe_questions(),
        ["system.console_attached", DIRECTORY_QUESTION]
    );
}
