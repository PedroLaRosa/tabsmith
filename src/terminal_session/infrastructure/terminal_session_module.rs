use crate::dropdown::domain::repositories::theme_repository::ThemeRepository;
use crate::settings::domain::repositories::settings_repository::SettingsRepository;
use crate::shared::application::ports::clock::Clock;
use crate::shared::domain::shared_error::SharedError;
use crate::shared::domain::value_objects::data_directory::DataDirectory;
use crate::shared::infrastructure::shared_dependencies::SharedDependencies;
use crate::terminal_session::application::ports::generator_process_reaper::GeneratorProcessReaper;
use crate::terminal_session::application::ports::live_settings::LiveSettings;
use crate::terminal_session::application::ports::working_directory::WorkingDirectory;
use crate::terminal_session::application::prepare_session_launch_use_case::PrepareSessionLaunchUseCase;
use crate::terminal_session::application::report_session_context_use_case::ReportSessionContextUseCase;
use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
use crate::terminal_session::infrastructure::adapters::file_session_context_repository::FileSessionContextRepository;
use crate::terminal_session::infrastructure::adapters::lenient_live_settings::LenientLiveSettings;
use crate::terminal_session::infrastructure::adapters::process_working_directory::ProcessWorkingDirectory;
use crate::terminal_session::infrastructure::pty::completion_engine_starter::CompletionEngineStarter;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// The terminal session use cases over the run files in the data directory, and the collaborators the session
/// runner is handed instead of building them.
pub(crate) struct TerminalSessionModule {
    pub(crate) report_session_context: ReportSessionContextUseCase,
    pub(crate) prepare_session_launch: PrepareSessionLaunchUseCase,
    live_settings: Rc<dyn LiveSettings>,
    working_directory: Rc<dyn WorkingDirectory>,
    themes: Rc<dyn ThemeRepository>,
    generator_processes: Rc<dyn GeneratorProcessReaper>,
    session_contexts: Rc<dyn SessionContextRepository>,
    start_completion_engine: CompletionEngineStarter,
    clock: Arc<dyn Clock + Send + Sync>,
    data_directory: DataDirectory,
}

/// Only builds: the run file's path is resolved when a context is saved, so an unknown home fails only then.
pub(crate) fn build_terminal_session_module(
    dependencies: &SharedDependencies,
    settings: Rc<dyn SettingsRepository>,
    themes: Rc<dyn ThemeRepository>,
    generator_processes: Rc<dyn GeneratorProcessReaper>,
    start_completion_engine: CompletionEngineStarter,
) -> TerminalSessionModule {
    let session_contexts: Rc<dyn SessionContextRepository> = Rc::new(FileSessionContextRepository::new(
        dependencies.data_directory.clone(),
    ));
    TerminalSessionModule {
        report_session_context: ReportSessionContextUseCase::new(
            session_contexts.clone(),
            dependencies.clock.clone(),
        ),
        prepare_session_launch: PrepareSessionLaunchUseCase,
        live_settings: Rc::new(LenientLiveSettings::new(settings)),
        working_directory: Rc::new(ProcessWorkingDirectory),
        themes,
        generator_processes,
        session_contexts,
        start_completion_engine,
        clock: dependencies.clock.clone(),
        data_directory: dependencies.data_directory.clone(),
    }
}

impl TerminalSessionModule {
    pub(crate) fn live_settings(&self) -> Rc<dyn LiveSettings> {
        self.live_settings.clone()
    }

    pub(crate) fn working_directory(&self) -> Rc<dyn WorkingDirectory> {
        self.working_directory.clone()
    }

    pub(crate) fn themes(&self) -> Rc<dyn ThemeRepository> {
        self.themes.clone()
    }

    pub(crate) fn generator_processes(&self) -> Rc<dyn GeneratorProcessReaper> {
        self.generator_processes.clone()
    }

    pub(crate) fn session_contexts(&self) -> Rc<dyn SessionContextRepository> {
        self.session_contexts.clone()
    }

    pub(crate) fn start_completion_engine(&self) -> CompletionEngineStarter {
        self.start_completion_engine
    }

    pub(crate) fn clock(&self) -> Arc<dyn Clock + Send + Sync> {
        self.clock.clone()
    }

    /// The root of the data directory, resolved now: fails when the environment names no home.
    pub(crate) fn locate_data_root(&self) -> Result<PathBuf, SharedError> {
        self.data_directory.root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dropdown::domain::repositories::theme_repository::InMemoryThemeRepository;
    use crate::settings::domain::repositories::settings_repository::InMemorySettingsRepository;
    use crate::settings::domain::value_objects::settings::Settings;
    use crate::shared::application::ports::clock::FixedClock;
    use crate::shared::infrastructure::testing::temp_dir::TempDir;
    use crate::terminal_session::application::ports::completion_engine::{CompletionEngine, RecordingCompletionEngine};
    use crate::terminal_session::application::ports::generator_process_reaper::RecordingGeneratorProcessReaper;
    use crate::terminal_session::domain::repositories::session_context_repository::SessionContextRepository;
    use crate::terminal_session::domain::value_objects::session_id::SessionId;
    use crate::terminal_session::infrastructure::pty::session_message::SessionMessage;
    use std::collections::HashMap;
    use std::sync::mpsc::{self, Sender};
    use std::sync::Arc;

    fn start_recording_completion_engine(_inbox: Sender<SessionMessage>, _settings: Settings) -> Rc<dyn CompletionEngine> {
        Rc::new(RecordingCompletionEngine::new(mpsc::channel().0))
    }

    /// The run file goes under the data directory the module was built with, never one the env names.
    #[test]
    fn should_report_the_context_into_the_run_files_of_the_data_directory_it_was_built_with() {
        let temp_dir = TempDir::new("session-module");
        let data_directory = DataDirectory::new(Some(temp_dir.path().to_path_buf()), None);
        let module = build_terminal_session_module(
            &SharedDependencies {
                data_directory: data_directory.clone(),
                clock: Arc::new(FixedClock::new(7)),
            },
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(InMemoryThemeRepository::default()),
            Rc::new(RecordingGeneratorProcessReaper::new()),
            start_recording_completion_engine,
        );
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

    fn module_built_over(data_directory: DataDirectory) -> TerminalSessionModule {
        build_terminal_session_module(
            &SharedDependencies {
                data_directory,
                clock: Arc::new(FixedClock::new(7)),
            },
            Rc::new(InMemorySettingsRepository::default()),
            Rc::new(InMemoryThemeRepository::default()),
            Rc::new(RecordingGeneratorProcessReaper::new()),
            start_recording_completion_engine,
        )
    }

    /// The debug shell writes its rc files under this root; an unknown home fails with today's message.
    #[test]
    fn should_locate_the_data_root_of_the_data_directory_it_was_built_with() {
        let temp_dir = TempDir::new("session-module-root");
        let located = module_built_over(DataDirectory::new(Some(temp_dir.path().to_path_buf()), None));
        let homeless = module_built_over(DataDirectory::new(None, None));

        let found = located.locate_data_root().unwrap();
        let missing = homeless.locate_data_root().unwrap_err();

        assert_eq!(
            (found, missing.to_string()),
            (
                temp_dir.path().to_path_buf(),
                "could not determine the home directory (set HOME or TABSMITH_HOME)".to_string()
            )
        );
    }
}
