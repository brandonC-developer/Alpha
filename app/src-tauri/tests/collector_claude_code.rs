use std::io::Write;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use gestor_core::collectors::Collector;
use gestor_core::collectors::claude_code::ClaudeCodeCollector;
use gestor_core::model::{Provider, Tool};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude_code")
        .join(name)
}

fn collector_for_fixtures() -> ClaudeCodeCollector {
    ClaudeCodeCollector {
        root: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("claude_code"),
    }
}

#[test]
fn sesion_normal() {
    let collector = collector_for_fixtures();
    let path = fixture_path("normal.jsonl");
    let file_len = std::fs::metadata(&path).unwrap().len();

    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.new_offset, file_len);
    assert_eq!(outcome.events.len(), 2);

    let first = &outcome.events[0];
    assert_eq!(first.dedup_key, "cc:msg_A:req_A");
    assert_eq!(first.tool, Tool::ClaudeCode);
    assert_eq!(first.provider, Provider::Anthropic);
    assert_eq!(first.model, "claude-opus-5-5");
    assert_eq!(
        first.ts_utc,
        "2026-11-26T15:00:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(first.session_id, "s-001");
    assert_eq!(first.project.as_deref(), Some("Alpha"));
    assert_eq!(first.input, 2);
    assert_eq!(first.cache_write, 22158);
    assert_eq!(first.cache_read, 29587);
    assert_eq!(first.output, 415);
    assert_eq!(first.reasoning, 271);

    let second = &outcome.events[1];
    assert_eq!(second.dedup_key, "cc:msg_B:req_B");
    assert_eq!(second.tool, Tool::ClaudeCode);
    assert_eq!(second.provider, Provider::Anthropic);
    assert_eq!(second.model, "claude-opus-5-5");
    assert_eq!(
        second.ts_utc,
        "2026-11-26T15:01:00Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(second.session_id, "s-001");
    assert_eq!(second.project.as_deref(), Some("Alpha"));
    assert_eq!(second.input, 10);
    assert_eq!(second.cache_write, 0);
    assert_eq!(second.cache_read, 51745);
    assert_eq!(second.output, 120);
    assert_eq!(second.reasoning, 0);
}

#[test]
fn mismo_mensaje_en_varias_lineas_cuenta_una_vez() {
    let collector = collector_for_fixtures();
    let path = fixture_path("bloques.jsonl");

    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_A:req_A");
    assert_eq!(outcome.events[0].input, 2);
    assert_eq!(outcome.events[0].output, 415);
}

#[test]
fn linea_incompleta_no_avanza_offset() {
    let collector = collector_for_fixtures();
    let path = fixture_path("incompleta.jsonl");
    let raw = std::fs::read(&path).unwrap();
    assert!(!raw.ends_with(b"\n"));
    let first_newline = raw.iter().position(|&b| b == b'\n').unwrap();
    let expected_offset = (first_newline + 1) as u64;

    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_A:req_A");
    assert_eq!(outcome.new_offset, expected_offset);
}

#[test]
fn linea_corrupta_se_salta_y_se_informa() {
    let collector = collector_for_fixtures();
    let path = fixture_path("corrupta.jsonl");

    let outcome = collector.parse_file(&path, 0).unwrap();
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_A:req_A");
    assert_eq!(outcome.events[1].dedup_key, "cc:msg_B:req_B");
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].line, 3);
    assert!(!outcome.errors[0].reason.is_empty());
}

#[test]
fn lineas_sin_usage_se_ignoran() {
    let collector = collector_for_fixtures();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"{{"type":"user","sessionId":"s-002","timestamp":"2026-11-26T15:00:00Z","message":{{"role":"user","content":"secreto-del-prompt"}}}}"#
    )
    .unwrap();
    writeln!(
        tmp,
        r#"{{"type":"summary","summary":"resumen","leafUuid":"u-1"}}"#
    )
    .unwrap();
    writeln!(
        tmp,
        r#"{{"type":"assistant","sessionId":"s-002","timestamp":"2026-11-26T15:00:05Z","message":{{"id":"msg_no_usage","model":"claude-opus-5-5"}}}}"#
    )
    .unwrap();
    writeln!(
        tmp,
        r#"{{"type":"assistant","sessionId":"s-002","timestamp":"2026-11-26T15:00:10Z","requestId":"req_S","message":{{"id":"msg_syn","model":"<synthetic>","usage":{{"input_tokens":100,"output_tokens":50}}}}}}"#
    )
    .unwrap();
    tmp.flush().unwrap();

    let outcome = collector.parse_file(tmp.path(), 0).unwrap();
    assert!(outcome.events.is_empty());
    assert!(outcome.errors.is_empty());
}

#[test]
fn no_guarda_contenido() {
    let collector = collector_for_fixtures();
    for fixture in ["normal.jsonl", "bloques.jsonl"] {
        let outcome = collector.parse_file(&fixture_path(fixture), 0).unwrap();
        assert!(!outcome.events.is_empty());
        for ev in &outcome.events {
            let dbg = format!("{ev:?}");
            assert!(!dbg.contains("secreto-del-prompt"));
        }
    }
}

#[test]
fn lee_desde_offset() {
    let collector = collector_for_fixtures();
    let path = fixture_path("normal.jsonl");
    let raw = std::fs::read(&path).unwrap();
    let newline_positions: Vec<usize> = raw
        .iter()
        .enumerate()
        .filter_map(|(i, &b)| (b == b'\n').then_some(i))
        .collect();
    // Línea 1: user, Línea 2: msg_A, Línea 3: msg_B
    let offset_after_second_line = (newline_positions[1] + 1) as u64;

    let outcome = collector
        .parse_file(&path, offset_after_second_line)
        .unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_B:req_B");
    assert_eq!(outcome.new_offset, raw.len() as u64);
}

#[test]
fn sin_request_id_usa_guion_y_metadatos() {
    let default_root = ClaudeCodeCollector::default_root();
    assert!(default_root.ends_with(PathBuf::from(".claude").join("projects")));

    let collector = ClaudeCodeCollector {
        root: default_root.clone(),
    };
    assert_eq!(collector.id(), "claude-code");
    assert_eq!(collector.roots(), vec![default_root]);

    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp,
        r#"{{"type":"assistant","sessionId":"s-003","timestamp":"2026-11-26T15:02:00Z","message":{{"id":"msg_C","model":"claude-sonnet-5-5","usage":{{"input_tokens":5,"output_tokens":15}}}}}}"#
    )
    .unwrap();
    tmp.flush().unwrap();

    let outcome = collector.parse_file(tmp.path(), 0).unwrap();
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_C:-");
    assert_eq!(outcome.events[0].project, None);
    assert_eq!(outcome.events[0].cache_write, 0);
    assert_eq!(outcome.events[0].cache_read, 0);
    assert_eq!(outcome.events[0].reasoning, 0);
}

#[test]
fn tipo_inesperado_se_salta_y_errores_sin_contenido() {
    let collector = collector_for_fixtures();
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    // Línea 1: JSON válido pero `message` tiene tipo inesperado (string con secreto) -> se salta sin error
    writeln!(
        tmp,
        r#"{{"type":"assistant","sessionId":"s-001","timestamp":"2026-11-26T15:00:00Z","message":"secreto-del-prompt"}}"#
    )
    .unwrap();
    // Línea 2: JSON sintácticamente inválido con secreto -> error con razón fija, línea y columna, sin secreto
    writeln!(
        tmp,
        r#"{{"type":"assistant","message":"secreto-del-prompt", roto"#
    )
    .unwrap();
    // Línea 3: timestamp inválido con secreto -> error con razón fija, sin secreto
    writeln!(
        tmp,
        r#"{{"type":"assistant","sessionId":"s-001","timestamp":"secreto-del-prompt","requestId":"req_X","message":{{"id":"msg_X","model":"claude-opus-5-5","usage":{{"input_tokens":1,"output_tokens":1}}}}}}"#
    )
    .unwrap();
    // Línea 4: UTF-8 inválido
    tmp.write_all(b"{\"type\":\"assistant\"\xff}\n").unwrap();
    tmp.flush().unwrap();

    let outcome = collector.parse_file(tmp.path(), 0).unwrap();
    assert!(outcome.events.is_empty());
    assert_eq!(outcome.errors.len(), 3);

    assert_eq!(outcome.errors[0].line, 2);
    assert!(outcome.errors[0].reason.contains("json_invalido"));
    assert!(outcome.errors[0].reason.contains("columna"));

    assert_eq!(outcome.errors[1].line, 3);
    assert!(outcome.errors[1].reason.contains("fecha_invalida"));

    assert_eq!(outcome.errors[2].line, 4);
    assert!(outcome.errors[2].reason.contains("utf8_invalido"));

    for err in &outcome.errors {
        assert!(!err.reason.contains("secreto-del-prompt"));
    }
}

#[test]
fn error_desde_offset_calcula_linea_exacta() {
    let collector = collector_for_fixtures();
    let path = fixture_path("corrupta.jsonl");
    let raw = std::fs::read(&path).unwrap();
    let second_newline = raw
        .iter()
        .enumerate()
        .filter_map(|(i, &b)| (b == b'\n').then_some(i))
        .nth(1)
        .unwrap();
    let offset = (second_newline + 1) as u64;

    let outcome = collector.parse_file(&path, offset).unwrap();
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].line, 3);
    assert!(outcome.errors[0].reason.contains("json_invalido"));
}

#[test]
fn from_offset_mayor_que_tamano_relee_desde_cero() {
    let collector = collector_for_fixtures();
    let path = fixture_path("normal.jsonl");
    let file_len = std::fs::metadata(&path).unwrap().len();

    let outcome = collector.parse_file(&path, file_len + 128).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.new_offset, file_len);
}

#[test]
fn archivo_vacio_y_crlf_con_bom() {
    let collector = collector_for_fixtures();

    let empty = tempfile::NamedTempFile::new().unwrap();
    let empty_outcome = collector.parse_file(empty.path(), 0).unwrap();
    assert!(empty_outcome.events.is_empty());
    assert!(empty_outcome.errors.is_empty());
    assert_eq!(empty_outcome.new_offset, 0);

    let mut bom_crlf = tempfile::NamedTempFile::new().unwrap();
    bom_crlf.write_all(&[0xEF, 0xBB, 0xBF]).unwrap();
    bom_crlf
        .write_all(
            b"{\"type\":\"assistant\",\"sessionId\":\"s-010\",\"timestamp\":\"2026-11-26T15:00:00Z\",\"requestId\":\"req_1\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-opus-5-5\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}}\r\n{\"type\":\"assistant\",\"sessionId\":\"s-010\",\"timestamp\":\"2026-11-26T15:01:00Z\",\"requestId\":\"req_2\",\"message\":{\"id\":\"msg_2\",\"model\":\"claude-opus-5-5\",\"usage\":{\"input_tokens\":5,\"output_tokens\":6}}}\r\n",
        )
        .unwrap();
    bom_crlf.flush().unwrap();

    let outcome = collector.parse_file(bom_crlf.path(), 0).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 2);
    assert_eq!(outcome.events[0].dedup_key, "cc:msg_1:req_1");
    assert_eq!(outcome.events[1].dedup_key, "cc:msg_2:req_2");
}
