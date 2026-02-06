use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorItem {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ErrorItem {
    pub fn new(code: impl Into<String>, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Message(String),
}

pub const SR_POL_001: &str = "SR-POL-001";
pub const SR_POL_002: &str = "SR-POL-002";
pub const SR_POL_003: &str = "SR-POL-003";
pub const SR_CMP_001: &str = "SR-CMP-001";
pub const SR_CMP_002: &str = "SR-CMP-002";
pub const SR_RUN_001: &str = "SR-RUN-001";
pub const SR_RUN_002: &str = "SR-RUN-002";
pub const SR_RUN_003: &str = "SR-RUN-003";
pub const SR_EVD_001: &str = "SR-EVD-001";
pub const SR_EVD_002: &str = "SR-EVD-002";
