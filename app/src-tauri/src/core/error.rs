use thiserror::Error;

/// Tipo resultado estándar del núcleo `gestor_core`.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Errores posibles en las operaciones del núcleo.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("error de base de datos: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),

    #[error("error de análisis en {file} (línea {line}): {reason}")]
    Parse {
        file: String,
        line: u64,
        reason: String,
    },

    #[error("error de credenciales: {0}")]
    Secret(String),

    #[error("error de red: {0}")]
    Http(String),
}
