use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::collectors::{Collector, LineFailure, ParseOutcome, parse_jsonl_file};
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

    /// Devuelve las dos carpetas raíz por defecto de Antigravity (app y CLI),
    /// o una lista vacía si no hay directorio de usuario.
    pub fn default_roots() -> Vec<PathBuf> {
        Self::default_roots_in(dirs::home_dir())
    }

    fn default_roots_in(home: Option<PathBuf>) -> Vec<PathBuf> {
        let Some(home) = home else {
            return Vec::new();
        };
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

        parse_jsonl_file(path, from_offset, |raw| {
            normalize_line(raw, conversation_id)
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
) -> std::result::Result<Option<UsageEvent>, LineFailure> {
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
        .map_err(|_| LineFailure::InvalidDate)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_directorio_de_usuario_devuelve_raices_vacias() {
        let roots = AntigravityCollector::default_roots_in(None);
        assert!(roots.is_empty());
        assert!(AntigravityCollector::new(roots).roots().is_empty());
    }
}
