use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::application::complete_command_line_use_case::CompleteCommandLineUseCase;
use crate::completion::application::completion_request::CompletionRequest;
use crate::completion::application::generator_runner::GeneratorRunner;
use crate::completion::application::ports::directory_lister::InMemoryDirectoryLister;
use crate::completion::application::ports::executable_catalog::FixedExecutableCatalog;
use crate::completion::domain::entities::completion_session::CompletionSession;
use crate::completion::domain::repositories::recency_repository::{InMemoryRecencyRepository, RecencyRepository};
use crate::history::domain::repositories::history_repository::InMemoryHistoryRepository;
use crate::settings::domain::value_objects::settings::Settings;
use crate::shared::application::ports::clock::FixedClock;
use crate::shared::domain::value_objects::shell::Shell;
use crate::specs::application::ports::spec_runtime::InMemorySpecRuntime;
use crate::specs::domain::repositories::spec_repository::InMemorySpecRepository;
use crate::specs::domain::value_objects::spec_index::SpecIndex;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// The completion use cases over InMemory doubles, wired the way the completion module wires the adapters (Q18).
pub(crate) struct CompletionDoubles {
    pub(crate) specs: Rc<InMemorySpecRepository>,
    pub(crate) runtime: Rc<InMemorySpecRuntime>,
    pub(crate) directories: Rc<InMemoryDirectoryLister>,
    pub(crate) executables: Rc<FixedExecutableCatalog>,
    pub(crate) history: Rc<InMemoryHistoryRepository>,
    pub(crate) recency: Rc<InMemoryRecencyRepository>,
    pub(crate) clock: Arc<FixedClock>,
}

impl CompletionDoubles {
    /// Over the given specs and spec runtime; no directory listings, no executables, no history, no recency, and a
    /// clock far enough from the epoch that a test can store a cache entry in its past.
    pub(crate) fn new(specs: InMemorySpecRepository, runtime: InMemorySpecRuntime) -> Self {
        CompletionDoubles {
            specs: Rc::new(specs),
            runtime: Rc::new(runtime),
            directories: Rc::new(InMemoryDirectoryLister::default()),
            executables: Rc::new(FixedExecutableCatalog::new(Vec::new())),
            history: Rc::new(InMemoryHistoryRepository::new(Vec::new())),
            recency: Rc::new(InMemoryRecencyRepository::default()),
            clock: Arc::new(FixedClock::new(1_700_000_123_999)),
        }
    }

    pub(crate) fn complete_command_line(&self, index: SpecIndex) -> CompleteCommandLineUseCase {
        let parser = Rc::new(CommandLineParser::new(self.specs.clone(), self.runtime.clone()));
        let generators = Rc::new(GeneratorRunner::new(
            self.runtime.clone(),
            self.directories.clone(),
            self.clock.clone(),
        ));
        CompleteCommandLineUseCase::new(
            parser,
            self.runtime.clone(),
            generators,
            self.executables.clone(),
            self.history.clone(),
            self.clock.clone(),
            index,
        )
    }

    /// A session with `settings`, an empty wrapper env and the recency the repository holds.
    pub(crate) fn session(&self, settings: Settings) -> CompletionSession {
        CompletionSession::new(settings, HashMap::new(), self.recency.load())
    }
}

/// A bash request for `buffer` with the cursor at its end, typed in `/work` (script and custom generators need a cwd).
pub(crate) fn request(id: u64, buffer: &str) -> CompletionRequest {
    CompletionRequest {
        id,
        buffer: buffer.into(),
        cursor: buffer.len(),
        cwd: "/work".into(),
        shell: Shell::Bash,
        history_mode: false,
    }
}
