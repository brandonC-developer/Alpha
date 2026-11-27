use std::collections::BTreeSet;

use chrono::NaiveDate;
use globset::Glob;
use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::model::Provider;

const PRICING_JSON: &str = include_str!("../../resources/pricing.json");

/// Tarifa versionada por fecha para un patrón de modelo (`$ / 1M tokens`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Price {
    pub id: i64,
    pub provider: Provider,
    pub model_pattern: String,
    pub valid_from: NaiveDate,
    pub valid_to: Option<NaiveDate>,
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub source_url: String,
    pub approximate: bool,
    pub user_edited: bool,
}

/// Datos de entrada para crear o actualizar una tarifa en la base de datos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceInput {
    pub provider: Provider,
    pub model_pattern: String,
    pub valid_from: NaiveDate,
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
    pub source_url: String,
    pub approximate: bool,
}

/// Siembra los precios iniciales de `resources/pricing.json` sin pisar filas editadas por el usuario.
pub fn seed_prices(conn: &Connection) -> Result<usize> {
    let entries: Vec<PriceInput> =
        serde_json::from_str(PRICING_JSON).map_err(|err| CoreError::Parse {
            file: "resources/pricing.json".to_string(),
            line: 0,
            reason: err.to_string(),
        })?;

    let mut stmt = conn.prepare(
        "INSERT INTO price (
            provider, model_pattern, valid_from, valid_to,
            input, output, cache_write, cache_read,
            source_url, approximate, user_edited
         ) VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9, 0)
         ON CONFLICT(model_pattern, valid_from) DO UPDATE SET
            provider = excluded.provider,
            input = excluded.input,
            output = excluded.output,
            cache_write = excluded.cache_write,
            cache_read = excluded.cache_read,
            source_url = excluded.source_url,
            approximate = excluded.approximate
         WHERE price.user_edited = 0",
    )?;

    let mut count = 0usize;
    let mut patterns = BTreeSet::new();
    for entry in &entries {
        let valid_from = entry.valid_from.format("%Y-%m-%d").to_string();
        let changed = stmt.execute(params![
            entry.provider.as_str(),
            &entry.model_pattern,
            &valid_from,
            entry.input,
            entry.output,
            entry.cache_write,
            entry.cache_read,
            &entry.source_url,
            i64::from(entry.approximate),
        ])?;
        count += changed;
        patterns.insert(entry.model_pattern.as_str());
    }

    for pattern in patterns {
        refresh_valid_to(conn, pattern)?;
    }

    Ok(count)
}

/// Busca el precio vigente para `model` en la fecha `at`; gana el patrón `globset` más largo.
pub fn find_price(conn: &Connection, model: &str, at: NaiveDate) -> Result<Option<Price>> {
    let date_str = at.format("%Y-%m-%d").to_string();
    let mut stmt = conn.prepare(
        "SELECT id, provider, model_pattern, valid_from, valid_to,
                input, output, cache_write, cache_read,
                source_url, approximate, user_edited
         FROM price
         WHERE valid_from <= ?1 AND (valid_to IS NULL OR ?1 < valid_to)",
    )?;

    let rows = stmt.query_map([&date_str], row_to_price)?;
    let mut best: Option<Price> = None;

    for row in rows {
        let candidate = row??;
        let matcher = compile_pattern(&candidate.model_pattern)?;
        if !matcher.is_match(model) {
            continue;
        }
        let is_better = match &best {
            None => true,
            Some(current) => {
                (
                    candidate.model_pattern.len(),
                    candidate.valid_from,
                    candidate.id,
                ) > (current.model_pattern.len(), current.valid_from, current.id)
            }
        };
        if is_better {
            best = Some(candidate);
        }
    }

    Ok(best)
}

pub(crate) fn compile_pattern(pattern: &str) -> Result<globset::GlobMatcher> {
    let glob = Glob::new(pattern).map_err(|err| CoreError::Parse {
        file: "price".to_string(),
        line: 0,
        reason: format!("patrón de modelo inválido '{pattern}': {err}"),
    })?;
    Ok(glob.compile_matcher())
}

pub(crate) fn refresh_valid_to(conn: &Connection, model_pattern: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, valid_from
         FROM price
         WHERE model_pattern = ?1
         ORDER BY valid_from ASC",
    )?;
    let rows: Vec<(i64, String)> = stmt
        .query_map([model_pattern], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut update_stmt = conn.prepare("UPDATE price SET valid_to = ?1 WHERE id = ?2")?;
    for (idx, (id, _)) in rows.iter().enumerate() {
        let next_valid_to = rows.get(idx + 1).map(|(_, d)| d.as_str());
        update_stmt.execute(params![next_valid_to, id])?;
    }
    Ok(())
}

fn row_to_price(row: &Row<'_>) -> rusqlite::Result<Result<Price>> {
    let id: i64 = row.get(0)?;
    let provider_str: String = row.get(1)?;
    let model_pattern: String = row.get(2)?;
    let valid_from_str: String = row.get(3)?;
    let valid_to_str: Option<String> = row.get(4)?;
    let input: f64 = row.get(5)?;
    let output: f64 = row.get(6)?;
    let cache_write: f64 = row.get(7)?;
    let cache_read: f64 = row.get(8)?;
    let source_url: String = row.get(9)?;
    let approximate: i64 = row.get(10)?;
    let user_edited: i64 = row.get(11)?;

    Ok((|| {
        let provider: Provider = provider_str.parse()?;
        let valid_from = parse_date(&valid_from_str)?;
        let valid_to = valid_to_str.as_deref().map(parse_date).transpose()?;
        Ok(Price {
            id,
            provider,
            model_pattern,
            valid_from,
            valid_to,
            input,
            output,
            cache_write,
            cache_read,
            source_url,
            approximate: approximate != 0,
            user_edited: user_edited != 0,
        })
    })())
}

fn parse_date(s: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|err| CoreError::Parse {
        file: "price".to_string(),
        line: 0,
        reason: format!("fecha de precio inválida '{s}': {err}"),
    })
}
