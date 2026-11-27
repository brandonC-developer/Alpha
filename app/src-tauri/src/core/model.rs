use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Herramienta de IA de origen de los eventos de uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tool {
    ClaudeCode,
    Antigravity,
    Opencode,
}

impl Tool {
    /// Devuelve el identificador textual usado en la base de datos y la API.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Antigravity => "antigravity",
            Self::Opencode => "opencode",
        }
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Tool {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "claude-code" => Ok(Self::ClaudeCode),
            "antigravity" => Ok(Self::Antigravity),
            "opencode" => Ok(Self::Opencode),
            _ => Err(CoreError::Parse {
                file: String::new(),
                line: 0,
                reason: format!("herramienta desconocida: {s}"),
            }),
        }
    }
}

/// Proveedor del modelo utilizado en un evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    Google,
    DeepSeek,
    Other,
}

impl Provider {
    /// Devuelve el identificador textual usado en la base de datos y la API.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::Google => "google",
            Self::DeepSeek => "deepseek",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "anthropic" => Ok(Self::Anthropic),
            "google" => Ok(Self::Google),
            "deepseek" => Ok(Self::DeepSeek),
            "other" => Ok(Self::Other),
            _ => Err(CoreError::Parse {
                file: String::new(),
                line: 0,
                reason: format!("proveedor desconocido: {s}"),
            }),
        }
    }
}

/// Determina el proveedor a partir del nombre del modelo y una pista opcional del colector.
pub fn provider_for(_tool: Tool, model: &str, provider_hint: Option<&str>) -> Provider {
    if model.starts_with("claude-") {
        Provider::Anthropic
    } else if model.starts_with("gemini-") {
        Provider::Google
    } else if provider_hint == Some("deepseek") {
        Provider::DeepSeek
    } else {
        Provider::Other
    }
}

/// Evento de uso normalizado procedente de cualquier colector.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageEvent {
    pub dedup_key: String,
    pub tool: Tool,
    pub provider: Provider,
    pub model: String,
    pub ts_utc: DateTime<Utc>,
    pub session_id: String,
    pub project: Option<String>,
    pub input: u64,
    pub cache_write: u64,
    pub cache_read: u64,
    pub output: u64,
    pub reasoning: u64,
}

/// Devuelve el identificador del plan asociado a la combinación herramienta/proveedor.
pub fn plan_for(tool: Tool, provider: Provider) -> &'static str {
    match (tool, provider) {
        (Tool::ClaudeCode, _) => "claude-pro",
        (Tool::Antigravity, Provider::Anthropic) => "google-ai-pro-claude",
        (Tool::Antigravity, _) => "google-ai-pro",
        (Tool::Opencode, _) => "deepseek",
    }
}
