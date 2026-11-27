use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Deserialize;

use crate::error::{CoreError, Result};
use crate::plans::{CalibrationSource, WindowName, try_calibrate_with_source};

/// Cubeta de cuota oficial devuelta por `agy -p "/usage" --output-format json`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgyBucket {
    pub plan_id: String,
    pub window: WindowName,
    pub remaining_fraction: f64,
    pub resets_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
struct AgyOutput {
    command: AgyCommand,
}

#[derive(Debug, Deserialize)]
struct AgyCommand {
    data: AgyData,
}

#[derive(Debug, Deserialize)]
struct AgyData {
    groups: Vec<AgyGroup>,
}

#[derive(Debug, Deserialize)]
struct AgyGroup {
    buckets: Vec<AgyBucketRaw>,
}

#[derive(Debug, Deserialize)]
struct AgyBucketRaw {
    id: String,
    #[allow(dead_code)]
    #[serde(default)]
    window: Option<String>,
    remaining_fraction: f64,
    reset_time: String,
}

/// Interpreta la salida JSON de `agy /usage` extrayendo las cubetas conocidas y contando las desconocidas ignoradas.
pub fn parse_agy_usage(json: &str) -> Result<(Vec<AgyBucket>, usize)> {
    let parsed: AgyOutput = serde_json::from_str(json).map_err(|err| CoreError::Parse {
        file: "agy_usage".to_string(),
        line: 0,
        reason: format!("JSON de agy /usage inválido: {err}"),
    })?;

    let mut out = Vec::new();
    let mut unknown_buckets = 0usize;
    for group in parsed.command.data.groups {
        for raw in group.buckets {
            let Some((plan_id, window)) = map_bucket_id(&raw.id) else {
                unknown_buckets += 1;
                continue;
            };
            if !raw.remaining_fraction.is_finite() || !(0.0..=1.0).contains(&raw.remaining_fraction)
            {
                return Err(CoreError::Parse {
                    file: "agy_usage".to_string(),
                    line: 0,
                    reason: format!(
                        "remaining_fraction fuera de rango para '{}': {}",
                        raw.id, raw.remaining_fraction
                    ),
                });
            }

            let parsed_reset =
                raw.reset_time
                    .parse::<DateTime<Utc>>()
                    .map_err(|err| CoreError::Parse {
                        file: "agy_usage".to_string(),
                        line: 0,
                        reason: format!("reset_time inválido '{}': {err}", raw.reset_time),
                    })?;

            // 1970-01-01T00:00:00Z significa «sin ventana abierta».
            let resets_at = if parsed_reset.timestamp() == 0 {
                None
            } else {
                Some(parsed_reset)
            };

            out.push(AgyBucket {
                plan_id: plan_id.to_string(),
                window,
                remaining_fraction: raw.remaining_fraction,
                resets_at,
            });
        }
    }

    Ok((out, unknown_buckets))
}

/// Aplica calibraciones `source = 'agy'` a partir de las cubetas interpretadas, devolviendo cuántas se registraron.
pub fn apply_agy_usage(
    conn: &Connection,
    buckets: &[AgyBucket],
    now: DateTime<Utc>,
) -> Result<usize> {
    let mut applied = 0usize;

    for bucket in buckets {
        if !bucket.remaining_fraction.is_finite()
            || !(0.0..=1.0).contains(&bucket.remaining_fraction)
        {
            return Err(CoreError::Parse {
                file: "agy_usage".to_string(),
                line: 0,
                reason: format!(
                    "remaining_fraction fuera de rango para '{}': {}",
                    bucket.plan_id, bucket.remaining_fraction
                ),
            });
        }

        let percent = (1.0 - bucket.remaining_fraction) * 100.0;
        if percent <= 0.0 {
            continue;
        }

        if try_calibrate_with_source(
            conn,
            &bucket.plan_id,
            bucket.window,
            percent,
            CalibrationSource::Agy,
            bucket.resets_at,
            now,
        )?
        .is_some()
        {
            applied += 1;
        }
    }

    Ok(applied)
}

fn map_bucket_id(id: &str) -> Option<(&'static str, WindowName)> {
    match id {
        "gemini-5h" => Some(("google-ai-pro", WindowName::FiveHours)),
        "gemini-weekly" => Some(("google-ai-pro", WindowName::Weekly)),
        "3p-5h" => Some(("google-ai-pro-claude", WindowName::FiveHours)),
        "3p-weekly" => Some(("google-ai-pro-claude", WindowName::Weekly)),
        _ => None,
    }
}
