use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone, Timelike, Utc};

/// Calcula las horas transcurridas entre `a` y `b` (positivo si `b` es posterior a `a`).
pub fn hours_between(a: DateTime<Utc>, b: DateTime<Utc>) -> f64 {
    b.signed_duration_since(a).num_milliseconds() as f64 / 3_600_000.0
}

/// Convierte un instante UTC a su fecha local en la zona horaria `tz`.
pub fn local_date<Tz: TimeZone>(ts: DateTime<Utc>, tz: &Tz) -> NaiveDate {
    ts.with_timezone(tz).date_naive()
}

/// Formatea un instante UTC en ISO 8601 con precisión de segundos y sufijo `Z`.
pub fn fmt_utc(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Trunca un instante UTC al inicio de su hora y lo formatea como `'YYYY-MM-DDTHH:00:00Z'`.
pub fn hour_utc_str(ts: DateTime<Utc>) -> String {
    let truncated = ts
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or(ts);
    fmt_utc(truncated)
}
