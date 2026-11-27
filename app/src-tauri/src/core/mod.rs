pub mod collectors;
pub mod db;
pub mod error;
pub mod model;
pub mod time;

/// Versión actual del núcleo `gestor_core`, coincidente con `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
