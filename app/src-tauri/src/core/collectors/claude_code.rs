use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::collectors::{Collector, LineError, ParseOutcome};
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

    /// Devuelve la carpeta por defecto de Claude Code (`%USERPROFILE%\.claude\projects`).
    pub fn default_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".claude")
            .join("projects")
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
        vec![self.root.clone()]
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome> {
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        if from_offset >= file_len {
            return Ok(ParseOutcome {
                events: Vec::new(),
                new_offset: file_len,
                errors: Vec::new(),
            });
        }

        let mut reader = BufReader::new(file);
        let mut line_num: u64 = 1;
        if from_offset > 0 {
            line_num += count_newlines_up_to(&mut reader, from_offset)?;
        }

        let mut offset = from_offset;
        let mut events = Vec::new();
        let mut errors = Vec::new();
        let mut seen = HashSet::new();
        let mut buf = Vec::new();

        loop {
            buf.clear();
            let bytes_read = reader.read_until(b'\n', &mut buf)?;
            if bytes_read == 0 {
                break;
            }
            // Una línea sin '\n' final puede estar escribiéndose aún: no avanzamos el offset.
            if !buf.ends_with(b"\n") {
                break;
            }

            offset += bytes_read as u64;
            let current_line = line_num;
            line_num += 1;

            let text = match std::str::from_utf8(&buf) {
                Ok(s) => s.trim().trim_start_matches('\u{FEFF}'),
                Err(err) => {
                    errors.push(LineError {
                        line: current_line,
                        reason: format!("UTF-8 inválido: {err}"),
                    });
                    continue;
                }
            };

            if text.is_empty() {
                continue;
            }

            let raw: RawLine = match serde_json::from_str(text) {
                Ok(parsed) => parsed,
                Err(err) => {
                    errors.push(LineError {
                        line: current_line,
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            match normalize_line(raw) {
                Ok(Some(event)) => {
                    if seen.insert(event.dedup_key.clone()) {
                        events.push(event);
                    }
                }
                Ok(None) => {}
                Err(reason) => {
                    errors.push(LineError {
                        line: current_line,
                        reason,
                    });
                }
            }
        }

        Ok(ParseOutcome {
            events,
            new_offset: offset,
            errors,
        })
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

fn normalize_line(raw: RawLine) -> std::result::Result<Option<UsageEvent>, String> {
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
        .map_err(|err| format!("marca temporal inválida: {err}"))?;

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

fn count_newlines_up_to<R: Read>(reader: &mut R, limit: u64) -> Result<u64> {
    let mut limited = reader.take(limit);
    let mut buf = [0u8; 8192];
    let mut count = 0u64;
    loop {
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        count += buf[..n].iter().filter(|&&b| b == b'\n').count() as u64;
    }
    Ok(count)
}
