use crate::diagnostics::application::ports::doctor_reporter::DoctorReporter;
use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
use crate::diagnostics::domain::value_objects::info_line::InfoLine;
use crate::diagnostics::infrastructure::adapters::spinner::Spinner;
use crate::shared::infrastructure::terminal_style::{sgr, stdout_tty};
use std::cell::RefCell;

const SUPPORT_HINT: &str = "  tabsmith still not working? Check ~/.tabsmith/logs/pty.log";

/// Doctor's words on stdout, styled when stdout is a terminal; owns the progress spinner on stderr.
pub(crate) struct TerminalDoctorReporter {
    spinner: RefCell<Option<Spinner>>,
}

impl TerminalDoctorReporter {
    pub(crate) fn new() -> Self {
        Self {
            spinner: RefCell::new(None),
        }
    }

    fn print_all(lines: Vec<String>) {
        for line in lines {
            println!("{line}");
        }
    }
}

impl DoctorReporter for TerminalDoctorReporter {
    fn start_progress(&self) {
        *self.spinner.borrow_mut() = Spinner::start();
    }

    fn stop_progress(&self) {
        self.spinner.borrow_mut().take();
    }

    fn group_header(&self, header: &str) {
        println!("{}", Painter::on_stdout().paint("2", header));
    }

    fn outcome(&self, name: &str, outcome: &CheckOutcome) {
        Self::print_all(outcome_lines(Painter::on_stdout(), name, outcome));
    }

    fn fixing(&self) {
        println!("Attempting to fix automatically...");
    }

    fn fix_failed(&self, reason: &str) {
        println!("Failed to fix: {reason}");
    }

    fn rechecking(&self) {
        println!("Re-running check...");
    }

    fn blank_line(&self) {
        println!();
    }

    fn doctor_found_errors(&self) {
        println!("{}", found_errors_line(Painter::on_stdout()));
    }

    fn everything_looks_good(&self) {
        println!("{}", everything_looks_good_line(Painter::on_stdout()));
    }

    fn support_hint(&self) {
        println!("{SUPPORT_HINT}");
    }

    fn restart_notice(&self) {
        Self::print_all(restart_notice_lines(Painter::on_stdout()));
    }
}

/// Whether to wrap text in SGR escapes; decided when a line is built, not when the reporter is made.
#[derive(Clone, Copy)]
struct Painter {
    styled: bool,
}

impl Painter {
    fn on_stdout() -> Self {
        Painter {
            styled: stdout_tty(),
        }
    }

    fn paint(self, code: &str, text: &str) -> String {
        sgr(text, code, self.styled)
    }
}

fn outcome_lines(painter: Painter, name: &str, outcome: &CheckOutcome) -> Vec<String> {
    match outcome {
        CheckOutcome::Ok => vec![format!("{} {name}", painter.paint("32", "✔"))],
        CheckOutcome::Warn(message) => vec![format!("{} {message}", painter.paint("33", "●"))],
        CheckOutcome::Err { reason, info, .. } => {
            let mut lines = vec![format!("{} {name}: {reason}", painter.paint("31", "✘"))];
            if !info.is_empty() {
                lines.push(String::new());
                lines.extend(info.iter().map(|line| info_line_text(painter, line)));
            }
            lines
        }
    }
}

fn info_line_text(painter: Painter, line: &InfoLine) -> String {
    match line {
        InfoLine::Text(text) => format!("  {text}"),
        InfoLine::Command {
            before,
            command,
            after,
        } => format!("  {before}{}{after}", painter.paint("35", command)),
    }
}

fn found_errors_line(painter: Painter) -> String {
    format!(
        "{} Doctor found errors. Please fix them and try again.",
        painter.paint("31", "✘")
    )
}

fn everything_looks_good_line(painter: Painter) -> String {
    format!("{} Everything looks good!", painter.paint("32", "✔"))
}

fn restart_notice_lines(painter: Painter) -> Vec<String> {
    vec![
        format!(
            "  {}",
            painter.paint(
                "1",
                "PS. Autocomplete won't work in any existing terminal sessions, only new ones."
            )
        ),
        "  (You might want to restart your terminal emulator)".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::domain::value_objects::check_outcome::CheckOutcome;
    use crate::diagnostics::domain::value_objects::info_line::InfoLine;

    const STYLED: Painter = Painter { styled: true };
    const PLAIN: Painter = Painter { styled: false };

    fn failing(reason: &str, info: Vec<InfoLine>) -> CheckOutcome {
        CheckOutcome::Err {
            reason: reason.into(),
            info,
            fix: None,
        }
    }

    #[test]
    fn should_print_a_passed_check_with_a_green_tick_and_its_name() {
        let lines = outcome_lines(STYLED, "data directory", &CheckOutcome::Ok);

        assert_eq!(lines, ["\x1b[32m✔\x1b[0m data directory"]);
    }

    #[test]
    fn should_print_a_passed_check_without_escapes_when_not_styled() {
        let lines = outcome_lines(PLAIN, "data directory", &CheckOutcome::Ok);

        assert_eq!(lines, ["✔ data directory"]);
    }

    #[test]
    fn should_print_a_warning_with_a_yellow_dot_and_its_message_without_the_check_name() {
        let lines = outcome_lines(STYLED, "settings", &CheckOutcome::Warn("msg".into()));

        assert_eq!(lines, ["\x1b[33m●\x1b[0m msg"]);
    }

    #[test]
    fn should_print_a_warning_without_escapes_when_not_styled() {
        let lines = outcome_lines(PLAIN, "settings", &CheckOutcome::Warn("msg".into()));

        assert_eq!(lines, ["● msg"]);
    }

    #[test]
    fn should_print_an_error_with_a_red_cross_the_name_and_the_reason() {
        let lines = outcome_lines(STYLED, "name", &failing("reason", vec![]));

        assert_eq!(lines, ["\x1b[31m✘\x1b[0m name: reason"]);
    }

    #[test]
    fn should_print_an_error_without_escapes_when_not_styled() {
        let lines = outcome_lines(PLAIN, "name", &failing("reason", vec![]));

        assert_eq!(lines, ["✘ name: reason"]);
    }

    #[test]
    fn should_print_no_blank_line_for_an_error_without_info() {
        let lines = outcome_lines(PLAIN, "name", &failing("reason", vec![]));

        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn should_print_the_info_of_an_error_after_a_blank_line_indented_by_two_spaces() {
        let info = vec![InfoLine::Text("first".into()), InfoLine::Text("second".into())];

        let lines = outcome_lines(PLAIN, "name", &failing("reason", info));

        assert_eq!(lines, ["✘ name: reason", "", "  first", "  second"]);
    }

    #[test]
    fn should_print_an_empty_info_line_as_two_spaces() {
        let info = vec![InfoLine::Text("first".into()), InfoLine::Text(String::new())];

        let lines = outcome_lines(PLAIN, "name", &failing("reason", info));

        assert_eq!(lines[3], "  ");
    }

    #[test]
    fn should_keep_a_multi_line_reason_as_one_printed_entry() {
        let lines = outcome_lines(PLAIN, "name", &failing("one\ntwo", vec![]));

        assert_eq!(lines, ["✘ name: one\ntwo"]);
    }

    #[test]
    fn should_paint_the_command_of_an_info_line_magenta_inside_the_sentence() {
        let info = vec![InfoLine::Command {
            before: "Run ".into(),
            command: "tabsmith integrations install dotfiles bash".into(),
            after: " to reinstall".into(),
        }];

        let lines = outcome_lines(STYLED, "name", &failing("reason", info));

        assert_eq!(
            lines[2],
            "  Run \x1b[35mtabsmith integrations install dotfiles bash\x1b[0m to reinstall"
        );
    }

    #[test]
    fn should_print_the_command_of_an_info_line_plain_when_not_styled() {
        let info = vec![InfoLine::Command {
            before: "Run ".into(),
            command: "tabsmith integrations install dotfiles bash".into(),
            after: " to reinstall".into(),
        }];

        let lines = outcome_lines(PLAIN, "name", &failing("reason", info));

        assert_eq!(lines[2], "  Run tabsmith integrations install dotfiles bash to reinstall");
    }

    #[test]
    fn should_dim_the_header_of_a_group() {
        assert_eq!(STYLED.paint("2", "Checking dotfiles"), "\x1b[2mChecking dotfiles\x1b[0m");
    }

    #[test]
    fn should_leave_the_header_of_a_group_plain_when_not_styled() {
        assert_eq!(PLAIN.paint("2", "Checking dotfiles"), "Checking dotfiles");
    }

    #[test]
    fn should_report_the_errors_with_a_red_cross() {
        assert_eq!(
            found_errors_line(STYLED),
            "\x1b[31m✘\x1b[0m Doctor found errors. Please fix them and try again."
        );
    }

    #[test]
    fn should_report_that_everything_looks_good_with_a_green_tick() {
        assert_eq!(
            everything_looks_good_line(STYLED),
            "\x1b[32m✔\x1b[0m Everything looks good!"
        );
    }

    #[test]
    fn should_report_the_summaries_without_escapes_when_not_styled() {
        assert_eq!(
            [found_errors_line(PLAIN), everything_looks_good_line(PLAIN)],
            [
                "✘ Doctor found errors. Please fix them and try again.",
                "✔ Everything looks good!"
            ]
        );
    }

    #[test]
    fn should_point_to_the_pty_log_with_a_literal_tilde() {
        assert_eq!(
            SUPPORT_HINT,
            "  tabsmith still not working? Check ~/.tabsmith/logs/pty.log"
        );
    }

    #[test]
    fn should_print_the_restart_notice_in_bold_with_the_indent_outside_the_escape() {
        let lines = restart_notice_lines(STYLED);

        assert_eq!(
            lines,
            [
                "  \x1b[1mPS. Autocomplete won't work in any existing terminal sessions, only new ones.\x1b[0m",
                "  (You might want to restart your terminal emulator)"
            ]
        );
    }

    #[test]
    fn should_print_the_restart_notice_without_escapes_when_not_styled() {
        let lines = restart_notice_lines(PLAIN);

        assert_eq!(
            lines,
            [
                "  PS. Autocomplete won't work in any existing terminal sessions, only new ones.",
                "  (You might want to restart your terminal emulator)"
            ]
        );
    }
}
