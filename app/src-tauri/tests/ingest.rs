use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use gestor_core::collectors::antigravity::AntigravityCollector;
use gestor_core::collectors::claude_code::ClaudeCodeCollector;
use gestor_core::collectors::{Collector, ParseOutcome};
use gestor_core::db::open_memory;
use gestor_core::error::{CoreError, Result};
use gestor_core::ingest::{IngestStats, ingest_file, scan_all};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn copy_claude_fixture(name: &str, dest_dir: &Path, dest_name: &str) -> PathBuf {
    fs::create_dir_all(dest_dir).unwrap();
    let dest = dest_dir.join(dest_name);
    fs::copy(fixtures_dir().join("claude_code").join(name), &dest).unwrap();
    dest
}

#[test]
fn reingestar_no_duplica() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    copy_claude_fixture("normal.jsonl", &cc_root, "normal.jsonl");
    copy_claude_fixture("bloques.jsonl", &cc_root, "bloques.jsonl");
    copy_claude_fixture("incompleta.jsonl", &cc_root, "incompleta.jsonl");

    let ag_uuid = "7f0c2a10-0000-4000-8000-000000000001";
    let ag_root = tmp.path().join("antigravity");
    let ag_logs = ag_root.join(ag_uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&ag_logs).unwrap();
    let ag_src_dir = fixtures_dir()
        .join("antigravity")
        .join(ag_uuid)
        .join(".system_generated")
        .join("logs");
    fs::copy(
        ag_src_dir.join("transcript.jsonl"),
        ag_logs.join("transcript.jsonl"),
    )
    .unwrap();
    fs::copy(
        ag_src_dir.join("transcript_full.jsonl"),
        ag_logs.join("transcript_full.jsonl"),
    )
    .unwrap();

    let chunks_dir = ag_logs.join("chunks");
    fs::create_dir_all(&chunks_dir).unwrap();
    fs::copy(
        ag_src_dir.join("transcript.jsonl"),
        chunks_dir.join("transcript.jsonl"),
    )
    .unwrap();

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(ClaudeCodeCollector::new(cc_root)),
        Box::new(AntigravityCollector::new(vec![ag_root])),
    ];

    let progress_calls = RefCell::new(Vec::new());
    let stats1: IngestStats = scan_all(&mut conn, &collectors, &|done, total| {
        progress_calls.borrow_mut().push((done, total));
    })
    .unwrap();

    // 3 archivos jsonl de Claude Code + 1 transcript.jsonl de Antigravity (ignora _full y chunks)
    assert_eq!(stats1.files_seen, 4);
    assert_eq!(stats1.files_read, 4);
    // normal.jsonl (2: msg_A, msg_B) + bloques.jsonl (duplica msg_A) + incompleta.jsonl (duplica msg_A) + transcript.jsonl (3) = 5 únicos
    assert_eq!(stats1.new_events, 5);
    assert_eq!(stats1.duplicates, 2);
    assert_eq!(stats1.line_errors, 0);
    assert_eq!(progress_calls.borrow().last().copied(), Some((4, 4)));

    let rows_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_event", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows_before, 5);

    let stats2 = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats2.files_seen, 4);
    assert_eq!(stats2.files_read, 0);
    assert_eq!(stats2.new_events, 0);
    assert_eq!(stats2.duplicates, 0);
    assert_eq!(stats2.line_errors, 0);

    let rows_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_event", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows_after, rows_before);
}

#[test]
fn solo_lee_lo_nuevo() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    let file_path = copy_claude_fixture("normal.jsonl", &cc_root, "sesion.jsonl");

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(ClaudeCodeCollector::new(cc_root))];

    let stats1 = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats1.new_events, 2);

    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&file_path)
        .unwrap();
    writeln!(
        f,
        r#"{{"type":"assistant","sessionId":"s-001","cwd":"E:\\Proyectos\\Gestior de Tokens\\Alpha","timestamp":"2026-11-26T15:02:00.000Z","requestId":"req_C","message":{{"id":"msg_C","model":"claude-opus-5-5","role":"assistant","content":[],"usage":{{"input_tokens":20,"cache_creation_input_tokens":0,"cache_read_input_tokens":100,"output_tokens":50}}}}}}"#
    )
    .unwrap();
    drop(f);

    let stats2 = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats2.files_seen, 1);
    assert_eq!(stats2.files_read, 1);
    assert_eq!(stats2.new_events, 1);
    assert_eq!(stats2.duplicates, 0);

    let total: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_event", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 3);

    // También comprobar que una línea incompleta al final no se pierde ni se duplica al completarse
    let inc_path = copy_claude_fixture("incompleta.jsonl", tmp.path(), "inc.jsonl");
    let collector = ClaudeCodeCollector::new(tmp.path().to_path_buf());
    let inc1 = ingest_file(&mut conn, &collector, &inc_path).unwrap();
    assert_eq!(inc1.new_events, 0);
    assert_eq!(inc1.duplicates, 1); // msg_A ya estaba de sesion.jsonl

    let mut f2 = fs::OpenOptions::new().append(true).open(&inc_path).unwrap();
    writeln!(
        f2,
        r#","role":"assistant","content":[],"usage":{{"input_tokens":15,"cache_creation_input_tokens":0,"cache_read_input_tokens":50,"output_tokens":30}}}},"requestId":"req_D_fix"}}"#
    )
    .unwrap();
    drop(f2);

    // Completar con un ID nuevo para verificar que la línea completada sí entra
    let inc_path2 = tmp.path().join("inc2.jsonl");
    let part1 = "{\"type\":\"assistant\",\"sessionId\":\"s-009\",\"timestamp\":\"2026-11-26T16:00:00Z\",\"requestId\":\"req_P1\",\"message\":{\"id\":\"msg_P1\",\"model\":\"claude-opus-5-5\",\"usage\":{\"input_tokens\":7,\"output_tokens\":11}}}\n";
    let part2_half = "{\"type\":\"assistant\",\"sessionId\":\"s-009\",\"timestamp\":\"2026-11-26T16:01:00Z\",\"requestId\":\"req_P2\",\"message\":{\"id\":\"msg_P2\",\"model\":\"claude-opus-5-5\"";
    fs::write(&inc_path2, format!("{part1}{part2_half}")).unwrap();

    let res_p1 = ingest_file(&mut conn, &collector, &inc_path2).unwrap();
    assert_eq!(res_p1.new_events, 1);

    let mut f3 = fs::OpenOptions::new()
        .append(true)
        .open(&inc_path2)
        .unwrap();
    writeln!(
        f3,
        r#","usage":{{"input_tokens":9,"output_tokens":13}}}}}}"#
    )
    .unwrap();
    drop(f3);

    let res_p2 = ingest_file(&mut conn, &collector, &inc_path2).unwrap();
    assert_eq!(res_p2.new_events, 1);
    assert_eq!(res_p2.duplicates, 0);
}

#[test]
fn archivo_rotado_se_relee() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    let file_path = copy_claude_fixture("normal.jsonl", &cc_root, "rotado.jsonl");

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(ClaudeCodeCollector::new(cc_root))];

    let stats1 = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats1.new_events, 2);

    let old_size = fs::metadata(&file_path).unwrap().len();

    // Reescribir más corto con un evento ya visto (msg_A) y uno nuevo (msg_R)
    let shorter_content = concat!(
        r#"{"type":"assistant","sessionId":"s-001","timestamp":"2026-11-26T15:00:00.000Z","requestId":"req_A","message":{"id":"msg_A","model":"claude-opus-5-5","usage":{"input_tokens":2,"cache_creation_input_tokens":22158,"cache_read_input_tokens":29587,"output_tokens":415,"output_tokens_details":{"thinking_tokens":271}}}}"#,
        "\n",
        r#"{"type":"assistant","sessionId":"s-001","timestamp":"2026-11-26T15:05:00.000Z","requestId":"req_R","message":{"id":"msg_R","model":"claude-opus-5-5","usage":{"input_tokens":33,"output_tokens":44}}}"#,
        "\n",
    );
    assert!((shorter_content.len() as u64) < old_size);
    fs::write(&file_path, shorter_content).unwrap();

    let stats2 = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats2.files_read, 1);
    assert_eq!(stats2.new_events, 1);
    assert_eq!(stats2.duplicates, 1);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM usage_event", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
}

struct FailingSecondCollector {
    inner: ClaudeCodeCollector,
    fail_on_file_name: String,
}

impl Collector for FailingSecondCollector {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.inner.roots()
    }

    fn parse_file(&self, path: &Path, from_offset: u64) -> Result<ParseOutcome> {
        if path.file_name().and_then(|n| n.to_str()) == Some(self.fail_on_file_name.as_str()) {
            return Err(CoreError::Io(std::io::Error::other(
                "fallo simulado en el segundo archivo",
            )));
        }
        self.inner.parse_file(path, from_offset)
    }
}

#[test]
fn importacion_interrumpida_continua() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    let file_a = copy_claude_fixture("normal.jsonl", &cc_root, "a_primero.jsonl");
    let file_b = cc_root.join("b_segundo.jsonl");
    fs::write(
        &file_b,
        r#"{"type":"assistant","sessionId":"s-002","timestamp":"2026-11-26T16:10:00Z","requestId":"req_Z","message":{"id":"msg_Z","model":"claude-sonnet-5-5","usage":{"input_tokens":50,"output_tokens":75}}}"#.to_string()
            + "\n",
    )
    .unwrap();

    let mut conn_interrupted = open_memory().unwrap();
    let failing: Vec<Box<dyn Collector>> = vec![Box::new(FailingSecondCollector {
        inner: ClaudeCodeCollector::new(cc_root.clone()),
        fail_on_file_name: "b_segundo.jsonl".to_string(),
    })];

    let err = scan_all(&mut conn_interrupted, &failing, &|_, _| {}).unwrap_err();
    assert!(matches!(err, CoreError::Io(_)));

    // El primer archivo debe haber quedado guardado con su offset completo
    let (saved_count, saved_offset): (i64, u64) = conn_interrupted
        .query_row(
            "SELECT COUNT(*), COALESCE(MAX(read_offset), 0) FROM source_file",
            [],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)),
        )
        .unwrap();
    assert_eq!(saved_count, 1);
    assert_eq!(saved_offset, fs::metadata(&file_a).unwrap().len());

    // Reanudar con el colector sano
    let healthy: Vec<Box<dyn Collector>> = vec![Box::new(ClaudeCodeCollector::new(cc_root))];
    let resumed = scan_all(&mut conn_interrupted, &healthy, &|_, _| {}).unwrap();
    assert_eq!(resumed.files_seen, 2);
    assert_eq!(resumed.files_read, 1);
    assert_eq!(resumed.new_events, 1);

    // Comparar con un scan limpio desde cero
    let mut conn_clean = open_memory().unwrap();
    let clean = scan_all(&mut conn_clean, &healthy, &|_, _| {}).unwrap();
    assert_eq!(clean.new_events, 3);

    let totals_interrupted: (i64, i64, i64, i64, i64) = conn_interrupted
        .query_row(
            "SELECT COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output) FROM usage_event",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    let totals_clean: (i64, i64, i64, i64, i64) = conn_clean
        .query_row(
            "SELECT COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output) FROM usage_event",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(totals_interrupted, totals_clean);
}

#[test]
fn agregado_horario_cuadra() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    copy_claude_fixture("normal.jsonl", &cc_root, "normal.jsonl");

    // Segundo archivo en la misma hora/modelo/proyecto para comprobar acumulación
    let extra = cc_root.join("extra.jsonl");
    fs::write(
        &extra,
        r#"{"type":"assistant","sessionId":"s-001","cwd":"E:\\Proyectos\\Gestior de Tokens\\Alpha","timestamp":"2026-11-26T15:30:00Z","requestId":"req_E","message":{"id":"msg_E","model":"claude-opus-5-5","usage":{"input_tokens":8,"cache_creation_input_tokens":100,"cache_read_input_tokens":200,"output_tokens":30}}}"#.to_string()
            + "\n",
    )
    .unwrap();

    let ag_uuid = "7f0c2a10-0000-4000-8000-000000000001";
    let ag_root = tmp.path().join("antigravity");
    let ag_logs = ag_root.join(ag_uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&ag_logs).unwrap();
    fs::copy(
        fixtures_dir()
            .join("antigravity")
            .join(ag_uuid)
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl"),
        ag_logs.join("transcript.jsonl"),
    )
    .unwrap();

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![
        Box::new(ClaudeCodeCollector::new(cc_root)),
        Box::new(AntigravityCollector::new(vec![ag_root])),
    ];
    let stats = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats.new_events, 6);

    let event_sums: (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output) FROM usage_event",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    let agg_sums: (i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT SUM(turns), SUM(input), SUM(cache_write), SUM(cache_read), SUM(output), SUM(unpriced) FROM hourly_agg",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .unwrap();

    assert_eq!(event_sums.0, agg_sums.0);
    assert_eq!(event_sums.1, agg_sums.1);
    assert_eq!(event_sums.2, agg_sums.2);
    assert_eq!(event_sums.3, agg_sums.3);
    assert_eq!(event_sums.4, agg_sums.4);
    assert_eq!(agg_sums.5, event_sums.0);

    // Comprobar formato de hour_utc
    let hour_sample: String = conn
        .query_row(
            "SELECT hour_utc FROM hourly_agg WHERE tool = 'claude-code' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hour_sample, "2026-11-26T15:00:00Z");
}

#[test]
fn errores_de_linea_se_guardan() {
    let tmp = tempfile::tempdir().unwrap();
    let cc_root = tmp.path().join("claude");
    let corrupt_path = copy_claude_fixture("corrupta.jsonl", &cc_root, "corrupta.jsonl");

    let mut conn = open_memory().unwrap();
    let collectors: Vec<Box<dyn Collector>> = vec![Box::new(ClaudeCodeCollector::new(cc_root))];

    let stats = scan_all(&mut conn, &collectors, &|_, _| {}).unwrap();
    assert_eq!(stats.new_events, 2);
    assert_eq!(stats.line_errors, 1);

    let (stored_path, line, reason, at): (String, i64, String, String) = conn
        .query_row(
            "SELECT sf.path, ie.line, ie.reason, ie.at
             FROM ingest_error ie
             JOIN source_file sf ON sf.id = ie.source_file_id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();

    assert_eq!(stored_path, corrupt_path.to_string_lossy());
    assert_eq!(line, 3);
    assert!(!reason.is_empty());
    assert!(at.ends_with('Z'));
}
