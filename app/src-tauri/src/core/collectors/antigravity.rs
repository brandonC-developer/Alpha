use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::collectors::{Collector, LineError, ParseOutcome};
use crate::error::Result;
use crate::model::{Tool, UsageEvent, provider_for};

/// Colector de registros locales JSONL escritos por Antigravity (app y CLI).
#[derive(Debug, Clone)]
pub struct AntigravityCollector {
    pub roots: Vec<PathBuf>,
}

impl AntigravityCollector {
    /// Crea un colector apuntando a las carpetas raíz indicadas.
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    /// Devuelve las dos carpetas raíz por defecto de Antigravity (app y CLI).
    pub fn default_roots() -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_default();
        vec![
            home.join(".gemini").join("antigravity").join("brain"),
            home.join(".gemini").join("antigravity-cli").join("brain"),
        ]
    }

    /// Indica si la ruta corresponde a `*/.system_generated/logs/transcript.jsonl`
    /// (excluyendo `transcript_full.jsonl` y cualquier archivo bajo `chunks/`).
    pub fn accepts_path(path: &Path) -> bool {
        conversation_id_from_path(path).is_some()
    }
}

impl Default for AntigravityCollector {
    fn default() -> Self {
        Self::new(Self::default_roots())
    }
}

impl Collector for AntigravityCollector {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.roots.clone()
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome> {
        let Some(conversation_id) = conversation_id_from_path(path) else {
            return Ok(ParseOutcome {
                events: Vec::new(),
                new_offset: from_offset,
                errors: Vec::new(),
            });
        };

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

            match normalize_line(raw, conversation_id) {
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
    step_index: Option<u64>,
    source: Option<String>,
    model: Option<String>,
    created_at: Option<String>,
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    output_tokens: Option<u64>,
}

fn conversation_id_from_path(path: &Path) -> Option<&str> {
    let file_name = path.file_name()?.to_str()?;
    if !file_name.eq_ignore_ascii_case("transcript.jsonl") {
        return None;
    }

    let logs_dir = path.parent()?;
    let logs_name = logs_dir.file_name()?.to_str()?;
    if !logs_name.eq_ignore_ascii_case("logs") {
        return None;
    }

    let sys_dir = logs_dir.parent()?;
    let sys_name = sys_dir.file_name()?.to_str()?;
    if !sys_name.eq_ignore_ascii_case(".system_generated") {
        return None;
    }

    let conv_dir = sys_dir.parent()?;
    let conv_name = conv_dir.file_name()?.to_str()?;
    if conv_name.is_empty() {
        return None;
    }

    Some(conv_name)
}

fn normalize_line(
    raw: RawLine,
    conversation_id: &str,
) -> std::result::Result<Option<UsageEvent>, String> {
    if raw.source.as_deref() != Some("MODEL") {
        return Ok(None);
    }

    let Some(input) = raw.input_tokens else {
        return Ok(None);
    };
    let Some(step_index) = raw.step_index else {
        return Ok(None);
    };
    let Some(model) = raw.model.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(ts_str) = raw.created_at.as_deref() else {
        return Ok(None);
    };
    let ts_utc = DateTime::parse_from_rfc3339(ts_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| format!("marca temporal inválida: {err}"))?;

    let dedup_key = format!("ag:{conversation_id}:{step_index}");
    let provider = provider_for(Tool::Antigravity, &model, None);

    Ok(Some(UsageEvent {
        dedup_key,
        tool: Tool::Antigravity,
        provider,
        model,
        ts_utc,
        session_id: conversation_id.to_string(),
        project: None,
        input,
        cache_write: 0,
        cache_read: raw.cache_read_tokens.unwrap_or(0),
        output: raw.output_tokens.unwrap_or(0),
        reasoning: 0,
    }))
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
