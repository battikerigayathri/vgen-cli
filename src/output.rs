use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Display;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Human,
    Json,
}

impl OutputMode {
    pub fn from_json_flag(json: bool) -> Self {
        if json {
            OutputMode::Json
        } else {
            OutputMode::Human
        }
    }
}

pub struct CliContext {
    pub mode: OutputMode,
    pub command: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<WarningBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarningBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliExitCode {
    Success = 0,
    RuntimeError = 1,
    UsageError = 2,
    ValidationFailed = 3,
}

impl From<CliExitCode> for ExitCode {
    fn from(code: CliExitCode) -> Self {
        ExitCode::from(code as u8)
    }
}

pub fn emit_success<T: Serialize>(ctx: &CliContext, data: T) {
    let json = build_success_json(ctx, &data);
    if ctx.mode == OutputMode::Json {
        println!("{}", json);
    }
}

pub fn build_success_json<T: Serialize>(ctx: &CliContext, data: &T) -> String {
    let envelope = Envelope {
        ok: true,
        command: ctx.command.to_string(),
        data: Some(data),
        error: None,
        warnings: vec![],
    };
    serde_json::to_string_pretty(&envelope).expect("envelope serialization should not fail")
}

pub fn build_error_json(
    ctx: &CliContext,
    code: &str,
    message: impl Display,
    details: Option<Value>,
) -> String {
    let envelope: Envelope<()> = Envelope {
        ok: false,
        command: ctx.command.to_string(),
        data: None,
        error: Some(ErrorBody {
            code: code.to_string(),
            message: message.to_string(),
            details,
        }),
        warnings: vec![],
    };
    serde_json::to_string_pretty(&envelope).expect("envelope serialization should not fail")
}

pub fn emit_error(
    ctx: &CliContext,
    code: &str,
    message: impl Display,
    details: Option<Value>,
    exit: CliExitCode,
) -> ExitCode {
    match ctx.mode {
        OutputMode::Json => {
            println!("{}", build_error_json(ctx, code, message, details));
        }
        OutputMode::Human => {
            eprintln!("Error: {}", message);
        }
    }
    exit.into()
}
