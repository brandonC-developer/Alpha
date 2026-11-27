use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::collectors::{Collector, LineFailure, ParseOutcome, parse_jsonl_file};
use crate::error::Result;
use crate::model::{Tool, UsageEvent, provider_for};

/// Colector de registros locales JSONL escritos por Claude Code.
#[derive(Debug, Clone)]
pub struct ClaudeCodeCollector {
    pub root: PathBuf,
}

impl ClaudeCodeCollector {
    /// Crea un colector apuntando a una carpeta raíz concreta.
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Devuelve la carpeta por defecto de Claude Code (`%USERPROFILE%\.claude\projects`),
    /// o una ruta vacía si no hay directorio de usuario.
    pub fn default_root() -> PathBuf {
        Self::default_root_in(dirs::home_dir())
    }

    fn default_root_in(home: Option<PathBuf>) -> PathBuf {
        match home {
            Some(home) => home.join(".claude").join("projects"),
            None => PathBuf::new(),
        }
    }

    /// Indica si la ruta corresponde a un archivo soportado (`*.jsonl`).
    pub fn accepts_path(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
    }
}

impl Default for ClaudeCodeCollector {
    fn default() -> Self {
        Self::new(Self::default_root())
    }
}

impl Collector for ClaudeCodeCollector {
    fn id(&self) -> &'static str {
        "claude-code"
    }

    fn roots(&self) -> Vec<PathBuf> {
        if self.root.as_os_str().is_empty() {
            Vec::new()
        } else {
            vec![self.root.clone()]
        }
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome> {
        parse_jsonl_file(path, from_offset, normalize_line)
    }
}

#[derive(Deserialize)]
struct RawLine {
    #[serde(rename = "type")]
    line_type: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<RawMessage>,
}

#[derive(Deserialize)]
struct RawMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize)]
struct RawUsage {
    input_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    output_tokens_details: Option<RawOutputDetails>,
}

#[derive(Deserialize)]
struct RawOutputDetails {
    thinking_tokens: Option<u64>,
}

fn normalize_line(raw: RawLine) -> std::result::Result<Option<UsageEvent>, LineFailure> {
    if raw.line_type.as_deref() != Some("assistant") {
        return Ok(None);
    }

    let Some(message) = raw.message else {
        return Ok(None);
    };
    let Some(usage) = message.usage else {
        return Ok(None);
    };
    let Some(msg_id) = message.id.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(model) = message.model.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if model == "<synthetic>" {
        return Ok(None);
    }
    let Some(session_id) = raw.session_id.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(ts_str) = raw.timestamp.as_deref() else {
        return Ok(None);
    };
    let ts_utc = DateTime::parse_from_rfc3339(ts_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| LineFailure::InvalidDate)?;

    let req = raw
        .request_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    let dedup_key = format!("cc:{msg_id}:{req}");
    let provider = provider_for(Tool::ClaudeCode, &model, None);
    let project = raw.cwd.as_deref().and_then(project_from_cwd);
    let reasoning = usage
        .output_tokens_details
        .and_then(|d| d.thinking_tokens)
        .unwrap_or(0);

    Ok(Some(UsageEvent {
        dedup_key,
        tool: Tool::ClaudeCode,
        provider,
        model,
        ts_utc,
        session_id,
        project,
        input: usage.input_tokens.unwrap_or(0),
        cache_write: usage.cache_creation_input_tokens.unwrap_or(0),
        cache_read: usage.cache_read_input_tokens.unwrap_or(0),
        output: usage.output_tokens.unwrap_or(0),
        reasoning,
    }))
}

fn project_from_cwd(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return None;
    }
    let last = trimmed.rsplit(['\\', '/']).next()?;
    if last.is_empty() || (last.len() == 2 && last.ends_with(':')) {
        return None;
    }
    Some(last.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_directorio_de_usuario_devuelve_raices_vacias() {
        let root = ClaudeCodeCollector::default_root_in(None);
        assert!(root.as_os_str().is_empty());
        assert!(ClaudeCodeCollector::new(root).roots().is_empty());
    }
}
