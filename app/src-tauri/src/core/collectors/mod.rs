pub mod claude_code;

use std::path::{Path, PathBuf};

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
