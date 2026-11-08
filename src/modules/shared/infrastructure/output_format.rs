use clap::ValueEnum;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    #[default]
    Plain,
    Json,
    JsonPretty,
}

impl OutputFormat {
    /// Plain → `plain()`; json → compact; json-pretty → 2-space indent.
    pub(crate) fn render(self, plain: impl FnOnce() -> String, json: impl FnOnce() -> Value) -> String {
        match self {
            OutputFormat::Plain => plain(),
            OutputFormat::Json => json().to_string(),
            OutputFormat::JsonPretty => format!("{:#}", json()),
        }
    }
}
