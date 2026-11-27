pub mod antigravity;
pub mod claude_code;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use serde::de::{DeserializeOwned, IgnoredAny};

use crate::error::Result;
use crate::model::UsageEvent;

/// Error de análisis encontrado en una línea concreta de un archivo de registro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineError {
    pub line: u64,
    pub reason: String,
}

/// Resultado de analizar un archivo de registro desde un desplazamiento inicial.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseOutcome {
    pub events: Vec<UsageEvent>,
    pub new_offset: u64,
    pub errors: Vec<LineError>,
}

/// Interfaz común para todos los colectores de registros locales.
pub trait Collector: Send + Sync {
    /// Identificador del colector (por ejemplo `"claude-code"`).
    fn id(&self) -> &'static str;

    /// Carpetas raíz que vigila el colector.
    fn roots(&self) -> Vec<PathBuf>;

    /// Lee y normaliza eventos nuevos a partir de `from_offset`.
    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome>;
}

pub(crate) enum LineFailure {
    InvalidDate,
}

enum PendingErrorKind {
    Utf8 { column: usize },
    Json { column: usize },
    InvalidDate,
}

struct PendingError {
    rel_line: u64,
    kind: PendingErrorKind,
}

impl PendingError {
    fn into_line_error(self, base_line: u64) -> LineError {
        let line = base_line + self.rel_line;
        let reason = match self.kind {
            PendingErrorKind::Utf8 { column } => {
                format!("utf8_invalido (línea {line}, columna {column})")
            }
            PendingErrorKind::Json { column } => {
                format!("json_invalido (línea {line}, columna {column})")
            }
            PendingErrorKind::InvalidDate => {
                format!("fecha_invalida (línea {line})")
            }
        };
        LineError { line, reason }
    }
}

/// Lee un archivo JSONL desde `from_offset`, sin releer el prefijo salvo si hay errores de línea.
pub(crate) fn parse_jsonl_file<T, F>(
    path: &Path,
    from_offset: u64,
    mut normalize: F,
) -> Result<ParseOutcome>
where
    T: DeserializeOwned,
    F: FnMut(T) -> std::result::Result<Option<UsageEvent>, LineFailure>,
{
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    if from_offset == file_len {
        return Ok(ParseOutcome {
            events: Vec::new(),
            new_offset: file_len,
            errors: Vec::new(),
        });
    }

    let effective_offset = if from_offset > file_len {
        0
    } else {
        from_offset
    };

    if effective_offset > 0 {
        file.seek(SeekFrom::Start(effective_offset))?;
    }

    let mut reader = BufReader::new(file);
    let mut offset = effective_offset;
    let mut rel_line: u64 = 0;
    let mut events = Vec::new();
    let mut pending_errors = Vec::new();
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
        rel_line += 1;

        let text = match std::str::from_utf8(&buf) {
            Ok(s) => s.trim_start_matches('\u{FEFF}').trim(),
            Err(err) => {
                pending_errors.push(PendingError {
                    rel_line,
                    kind: PendingErrorKind::Utf8 {
                        column: err.valid_up_to() + 1,
                    },
                });
                continue;
            }
        };

        if text.is_empty() {
            continue;
        }

        let raw = match parse_json_or_skip::<T>(text) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => continue,
            Err(column) => {
                pending_errors.push(PendingError {
                    rel_line,
                    kind: PendingErrorKind::Json { column },
                });
                continue;
            }
        };

        match normalize(raw) {
            Ok(Some(event)) => {
                if seen.insert(event.dedup_key.clone()) {
                    events.push(event);
                }
            }
            Ok(None) => {}
            Err(LineFailure::InvalidDate) => {
                pending_errors.push(PendingError {
                    rel_line,
                    kind: PendingErrorKind::InvalidDate,
                });
            }
        }
    }

    let errors = if pending_errors.is_empty() {
        Vec::new()
    } else {
        let base_line = if effective_offset == 0 {
            0
        } else {
            let mut inner = reader.into_inner();
            inner.seek(SeekFrom::Start(0))?;
            count_newlines_up_to(&mut inner, effective_offset)?
        };
        pending_errors
            .into_iter()
            .map(|err| err.into_line_error(base_line))
            .collect()
    };

    Ok(ParseOutcome {
        events,
        new_offset: offset,
        errors,
    })
}

fn parse_json_or_skip<T: DeserializeOwned>(text: &str) -> std::result::Result<Option<T>, usize> {
    match serde_json::from_str::<T>(text) {
        Ok(val) => Ok(Some(val)),
        Err(err) => {
            if err.is_data() {
                match serde_json::from_str::<IgnoredAny>(text) {
                    Ok(_) => Ok(None),
                    Err(syntax_err) => Err(syntax_err.column()),
                }
            } else {
                Err(err.column())
            }
        }
    }
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
