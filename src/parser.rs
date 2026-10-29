//! Spec loading through the spec repository and the argv state machine (SPEC §6.3–6.4).
//! OWNER: parser agent. `ParseResult` is consumed by the suggest agent — extend, don't rename.
use crate::completion::application::command_line_parser::CommandLineParser;
use crate::completion::application::completion_notice::CompletionNotice;
use crate::completion::application::parse_context::ParseContext;
use crate::completion::domain::completion_error::CompletionError;
use crate::completion::domain::entities::parse_prefix_cache::ParsePrefixCache;
use crate::completion::domain::value_objects::command_line::CommandLine;
use crate::completion::domain::value_objects::parsed_command_line::ParsedCommandLine;
use crate::specs::application::ports::spec_runtime::SpecRuntime;
use crate::specs::domain::repositories::spec_repository::SpecRepository;
use crate::shared::infrastructure::logger;
use std::rc::Rc;

/// The engine's parser and its parse cache, until the engine holds both itself (intra-phase facade, 6.3d-6.3e).
pub struct Parser {
    parser: CommandLineParser,
    cache: ParsePrefixCache,
}

impl Parser {
    pub fn new(repository: Rc<dyn SpecRepository>, runtime: Rc<dyn SpecRuntime>) -> Parser {
        Parser {
            parser: CommandLineParser::new(repository, runtime),
            cache: ParsePrefixCache::default(),
        }
    }

    pub fn clear_caches(&mut self) {
        self.parser.clear_caches(&mut self.cache);
    }

    /// Logs a failed generateSpec at warn the moment the parser reports it.
    pub fn parse(&mut self, cmd: &CommandLine, ctx: &ParseContext) -> Result<ParsedCommandLine, CompletionError> {
        self.parser.parse(&mut self.cache, cmd, ctx, &mut |notice| match notice {
            CompletionNotice::GenerateSpecFailed(line) => logger::warn(&line),
        })
    }
}
