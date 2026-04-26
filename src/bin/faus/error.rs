use serde_json::{json, Value};
use std::io;

#[derive(Debug)]
pub(crate) enum CliFailure {
    Human(String),
    Json(CliError),
}

impl CliFailure {
    pub(crate) fn human(error: impl std::fmt::Display) -> Self {
        Self::Human(error.to_string())
    }

    pub(crate) fn client(error: CliError, json_output: bool) -> Self {
        if json_output {
            Self::Json(error)
        } else {
            Self::Human(format!("{}: {}", error.code, error.message))
        }
    }

    pub(crate) fn write(self) {
        match self {
            Self::Human(message) => eprintln!("[error] {message}"),
            Self::Json(error) => println!(
                "{}",
                serde_json::to_string(&json!({
                    "status": "error",
                    "error": error.to_json(),
                }))
                .expect("CLI error JSON should serialize")
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) details: Option<Value>,
    pub(crate) retryable: Option<bool>,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CliError {}

impl CliError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            details: None,
            retryable: None,
        }
    }

    pub(crate) fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub(crate) fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    fn to_json(self) -> Value {
        let mut error = serde_json::Map::new();
        error.insert("code".to_string(), Value::String(self.code));
        error.insert("message".to_string(), Value::String(self.message));
        if let Some(details) = self.details {
            error.insert("details".to_string(), details);
        }
        if let Some(retryable) = self.retryable {
            error.insert("retryable".to_string(), Value::Bool(retryable));
        }
        Value::Object(error)
    }
}

pub(crate) fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
