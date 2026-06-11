use serde_json::{Value, json};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Value,
    pub exit_code: i32,
}

impl AppError {
    pub fn validation(code: impl Into<String>, message: impl Into<String>, details: Value) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
            exit_code: 1,
        }
    }

    pub fn input(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: json!({}),
            exit_code: 2,
        }
    }

    pub fn input_with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
            exit_code: 2,
        }
    }

    pub fn fs(
        code: impl Into<String>,
        message: impl Into<String>,
        path: impl Into<String>,
        source: &std::io::Error,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: json!({
                "path": path.into(),
                "source": source.to_string()
            }),
            exit_code: 2,
        }
    }
}
