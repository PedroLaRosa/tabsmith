use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::terminal_session::application::report_session_context_use_case::ReportSessionContextUseCase;
use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
use std::rc::Rc;

/// The terminal session use cases over the run files in the data directory.
pub(crate) struct TerminalSessionModule {
    pub(crate) report_session_context: ReportSessionContextUseCase,
}

/// Only builds: the run file's path is resolved when a context is saved, so an unknown home fails only then.
pub(crate) fn build_terminal_session_module(
    dependencies: &SharedDependencies,
) -> TerminalSessionModule {
    TerminalSessionModule {
        report_session_context: ReportSessionContextUseCase::new(
            Rc::new(FileSessionContextRepository::new(
                dependencies.data_directory.clone(),
            )),
            dependencies.clock.clone(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shared::domain::value_objects::data_directory::DataDirectory;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
    use crate::terminal_session::domain::value_objects::session_id::SessionId;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// The run file goes under the data directory the module was built with, never one the env names.
    #[test]
    fn should_report_the_context_into_the_run_files_of_the_data_directory_it_was_built_with() {
        let temp_dir = TempDir::new("session-module");
        let data_directory = DataDirectory::new(Some(temp_dir.path().to_path_buf()), None);
        let module = build_terminal_session_module(&SharedDependencies {
            data_directory: data_directory.clone(),
            clock: Arc::new(FixedClock::new(7)),
        });
        let id = SessionId::new("abc123".to_string()).unwrap();

        module
            .report_session_context
            .execute(id.clone(), HashMap::new(), Some("ll='ls -l'".to_string()))
            .unwrap();

        let saved = FileSessionContextRepository::new(data_directory)
            .find(&id)
            .unwrap();
        assert_eq!(
            (saved.alias(), saved.reported_at_ms()),
            (Some("ll='ls -l'"), 7)
        );
    }
}
