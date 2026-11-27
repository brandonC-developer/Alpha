pub mod collectors;
pub mod cost;
pub mod db;
pub mod error;
pub mod ingest;
pub mod model;
pub mod pricing;
pub mod query;
pub mod time;

/// Versión actual del núcleo `gestor_core`, coincidente con `Cargo.toml`.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
