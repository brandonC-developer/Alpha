use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use gestor_core::collectors::Collector;
use gestor_core::collectors::antigravity::AntigravityCollector;
use gestor_core::model::{Provider, Tool, plan_for};

const FIXTURE_UUID: &str = "7f0c2a10-0000-4000-8000-000000000001";

fn fixture_brain() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("antigravity")
}

fn fixture_logs_dir() -> PathBuf {
    fixture_brain()
        .join(FIXTURE_UUID)
        .join(".system_generated")
        .join("logs")
}

fn fixture_transcript() -> PathBuf {
    fixture_logs_dir().join("transcript.jsonl")
}

fn collector_for_fixtures() -> AntigravityCollector {
    AntigravityCollector {
        roots: vec![fixture_brain()],
    }
}

fn collect_matching_files(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        visit_dir(root, &mut out);
    }
    out.sort();
    out
}

fn visit_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_dir(&path, out);
        } else if AntigravityCollector::accepts_path(&path) {
            out.push(path);
        }
    }
}

#[test]
fn input_sin_restar_cache() {
    let collector = collector_for_fixtures();
    let path = fixture_transcript();
    let file_len = fs::metadata(&path).unwrap().len();

    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.new_offset, file_len);
    assert_eq!(outcome.events.len(), 3);

    let step8 = &outcome.events[1];
    assert_eq!(step8.dedup_key, format!("ag:{FIXTURE_UUID}:8"));
    assert_eq!(step8.input, 26040);
    assert_eq!(step8.cache_read, 14167);
    assert_eq!(step8.cache_write, 0);
    assert_eq!(step8.output, 509);
    assert_eq!(step8.reasoning, 0);
    assert_eq!(step8.project, None);
    assert_eq!(step8.session_id, FIXTURE_UUID);
}

#[test]
fn input_menor_que_cache() {
    let collector = collector_for_fixtures();
    let outcome = collector.parse_file(&fixture_transcript(), 0).unwrap();
    assert_eq!(outcome.events.len(), 3);

    let step9 = &outcome.events[2];
    assert_eq!(step9.dedup_key, format!("ag:{FIXTURE_UUID}:9"));
    assert_eq!(step9.model, "gemini-4-hydrogen-high");
    assert_eq!(step9.input, 100);
    assert_eq!(step9.cache_read, 150);
    assert_eq!(step9.output, 20);
}

#[test]
fn ignora_transcript_full() {
    let collector = collector_for_fixtures();
    let full_path = fixture_logs_dir().join("transcript_full.jsonl");
    assert!(full_path.exists());
    assert!(!AntigravityCollector::accepts_path(&full_path));

    let full_outcome = collector.parse_file(&full_path, 0).unwrap();
    assert!(full_outcome.events.is_empty());
    assert!(full_outcome.errors.is_empty());

    let files = collect_matching_files(&collector.roots());
    assert_eq!(files, vec![fixture_transcript()]);

    let mut total_events = Vec::new();
    for file in files {
        total_events.extend(collector.parse_file(&file, 0).unwrap().events);
    }
    assert_eq!(total_events.len(), 3);
}

#[test]
fn ignora_chunks() {
    let tmp = tempfile::tempdir().unwrap();
    let chunks_dir = tmp
        .path()
        .join(FIXTURE_UUID)
        .join(".system_generated")
        .join("logs")
        .join("chunks");
    fs::create_dir_all(&chunks_dir).unwrap();
    let chunk_file = chunks_dir.join("transcript.jsonl");
    fs::copy(fixture_transcript(), &chunk_file).unwrap();

    assert!(!AntigravityCollector::accepts_path(&chunk_file));
    let collector = AntigravityCollector {
        roots: vec![tmp.path().to_path_buf()],
    };
    assert!(collect_matching_files(&collector.roots()).is_empty());
    assert!(
        collector
            .parse_file(&chunk_file, 0)
            .unwrap()
            .events
            .is_empty()
    );
}

#[test]
fn lee_tambien_la_cli() {
    let tmp = tempfile::tempdir().unwrap();
    let app_brain = tmp.path().join("antigravity").join("brain");
    let cli_brain = tmp.path().join("antigravity-cli").join("brain");

    let app_logs = app_brain
        .join(FIXTURE_UUID)
        .join(".system_generated")
        .join("logs");
    fs::create_dir_all(&app_logs).unwrap();
    fs::copy(fixture_transcript(), app_logs.join("transcript.jsonl")).unwrap();

    let cli_uuid = "8a1d3b20-0000-4000-8000-000000000002";
    let cli_logs = cli_brain
        .join(cli_uuid)
        .join(".system_generated")
        .join("logs");
    fs::create_dir_all(&cli_logs).unwrap();
    let mut f = fs::File::create(cli_logs.join("transcript.jsonl")).unwrap();
    writeln!(
        f,
        r#"{{"step_index":2,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-22T10:00:00Z","model":"gemini-4-hydrogen-tiered-high","input_tokens":500,"cache_read_tokens":200,"output_tokens":80}}"#
    )
    .unwrap();

    let collector = AntigravityCollector {
        roots: vec![app_brain, cli_brain],
    };
    let files = collect_matching_files(&collector.roots());
    assert_eq!(files.len(), 2);

    let mut events = Vec::new();
    for file in files {
        events.extend(collector.parse_file(&file, 0).unwrap().events);
    }
    assert_eq!(events.len(), 4);
    assert!(
        events
            .iter()
            .any(|e| e.dedup_key == format!("ag:{cli_uuid}:2") && e.session_id == cli_uuid)
    );
}

#[test]
fn solo_pasos_de_modelo() {
    let tmp = tempfile::tempdir().unwrap();
    let uuid = "11111111-2222-3333-4444-555555555555";
    let logs = tmp.path().join(uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("transcript.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    writeln!(
        f,
        r#"{{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-11-21T17:47:22Z","content":"secreto-del-prompt"}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-11-21T17:47:25Z","model":"gemini-4-neon-xhigh","output_tokens":10}}"#
    )
    .unwrap();
    writeln!(
        f,
        r#"{{"step_index":2,"source":"SYSTEM","type":"CHECKPOINT","status":"DONE","created_at":"2026-11-21T17:47:30Z","input_tokens":999}}"#
    )
    .unwrap();
    drop(f);

    let collector = AntigravityCollector {
        roots: vec![tmp.path().to_path_buf()],
    };
    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.events.is_empty());
    assert!(outcome.errors.is_empty());
}

#[test]
fn dedup_por_step_index() {
    let collector = collector_for_fixtures();
    let outcome = collector.parse_file(&fixture_transcript(), 0).unwrap();
    let step8_events: Vec<_> = outcome
        .events
        .iter()
        .filter(|e| e.dedup_key == format!("ag:{FIXTURE_UUID}:8"))
        .collect();
    assert_eq!(step8_events.len(), 1);
    assert_eq!(outcome.events.len(), 3);
}

#[test]
fn modelo_y_proveedor() {
    let collector = collector_for_fixtures();
    let outcome = collector.parse_file(&fixture_transcript(), 0).unwrap();
    let first = &outcome.events[0];
    assert_eq!(first.dedup_key, format!("ag:{FIXTURE_UUID}:1"));
    assert_eq!(first.tool, Tool::Antigravity);
    assert_eq!(first.provider, Provider::Google);
    assert_eq!(first.model, "gemini-4-neon-xhigh");
    assert_eq!(
        first.ts_utc,
        "2026-11-21T17:47:22Z".parse::<DateTime<Utc>>().unwrap()
    );
    assert_eq!(plan_for(first.tool, first.provider), "google-ai-pro");

    // Verificar también modelos claude-* dentro de Antigravity y default_roots
    let default_roots = AntigravityCollector::default_roots();
    assert_eq!(default_roots.len(), 2);
    assert!(default_roots[0].ends_with(PathBuf::from(".gemini").join("antigravity").join("brain")));
    assert!(
        default_roots[1].ends_with(
            PathBuf::from(".gemini")
                .join("antigravity-cli")
                .join("brain")
        )
    );
    assert_eq!(collector.id(), "antigravity");

    let tmp = tempfile::tempdir().unwrap();
    let uuid = "22222222-3333-4444-5555-666666666666";
    let logs = tmp.path().join(uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("transcript.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    writeln!(
        f,
        r#"{{"step_index":0,"source":"MODEL","created_at":"2026-11-21T18:00:00Z","model":"claude-opus-5-5-high","input_tokens":50,"output_tokens":10}}"#
    )
    .unwrap();
    drop(f);

    let out_claude = collector.parse_file(&path, 0).unwrap();
    assert_eq!(out_claude.events.len(), 1);
    assert_eq!(out_claude.events[0].tool, Tool::Antigravity);
    assert_eq!(out_claude.events[0].provider, Provider::Anthropic);
    assert_eq!(
        plan_for(out_claude.events[0].tool, out_claude.events[0].provider),
        "google-ai-pro-claude"
    );
}

#[test]
fn no_guarda_contenido() {
    let collector = collector_for_fixtures();
    let outcome = collector.parse_file(&fixture_transcript(), 0).unwrap();
    assert!(!outcome.events.is_empty());
    for ev in &outcome.events {
        let dbg = format!("{ev:?}");
        assert!(!dbg.contains("secreto-del-prompt"));
    }
}

#[test]
fn linea_incompleta_y_corrupta_y_offset() {
    let tmp = tempfile::tempdir().unwrap();
    let uuid = "33333333-4444-5555-6666-777777777777";
    let logs = tmp.path().join(uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("transcript.jsonl");

    let line1 = "{\"step_index\":1,\"source\":\"MODEL\",\"created_at\":\"2026-11-21T18:00:00Z\",\"model\":\"gemini-4-neon-xhigh\",\"input_tokens\":10,\"output_tokens\":5}\n";
    let line2 = "{\"step_index\":2, esto no es json\n";
    let line3_partial = "{\"step_index\":3,\"source\":\"MODEL\",\"input_tokens\":20";
    fs::write(&path, format!("{line1}{line2}{line3_partial}")).unwrap();

    let collector = AntigravityCollector {
        roots: vec![tmp.path().to_path_buf()],
    };
    let outcome = collector.parse_file(&path, 0).unwrap();
    assert_eq!(outcome.events.len(), 1);
    assert_eq!(outcome.events[0].dedup_key, format!("ag:{uuid}:1"));
    assert_eq!(outcome.errors.len(), 1);
    assert_eq!(outcome.errors[0].line, 2);
    assert_eq!(outcome.new_offset, (line1.len() + line2.len()) as u64);

    // Completar la tercera línea y leer desde new_offset
    let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
    writeln!(
        f,
        r#","created_at":"2026-11-21T18:01:00Z","model":"gemini-4-hydrogen-high","output_tokens":7}}"#
    )
    .unwrap();
    drop(f);

    let outcome2 = collector.parse_file(&path, outcome.new_offset).unwrap();
    assert!(outcome2.errors.is_empty());
    assert_eq!(outcome2.events.len(), 1);
    assert_eq!(outcome2.events[0].dedup_key, format!("ag:{uuid}:3"));
    assert_eq!(outcome2.events[0].input, 20);
}

#[test]
fn tipo_inesperado_se_salta_y_errores_sin_contenido() {
    let tmp = tempfile::tempdir().unwrap();
    let uuid = "44444444-5555-6666-7777-888888888888";
    let logs = tmp.path().join(uuid).join(".system_generated").join("logs");
    fs::create_dir_all(&logs).unwrap();
    let path = logs.join("transcript.jsonl");

    let mut f = fs::File::create(&path).unwrap();
    // Línea 1: JSON válido con campo de tipo inesperado y secreto -> se salta sin error
    writeln!(
        f,
        r#"{{"step_index":"secreto-del-prompt","source":"MODEL","model":"gemini-4-neon-xhigh","input_tokens":10}}"#
    )
    .unwrap();
    // Línea 2: JSON roto con secreto -> razón fija sin secreto
    writeln!(
        f,
        r#"{{"step_index":2,"thinking":"secreto-del-prompt", roto"#
    )
    .unwrap();
    // Línea 3: fecha inválida con secreto -> razón fija sin secreto
    writeln!(
        f,
        r#"{{"step_index":3,"source":"MODEL","created_at":"secreto-del-prompt","model":"gemini-4-neon-xhigh","input_tokens":10,"output_tokens":5}}"#
    )
    .unwrap();
    // Línea 4: UTF-8 inválido
    f.write_all(b"{\"step_index\":4\xff}\n").unwrap();
    drop(f);

    let collector = AntigravityCollector {
        roots: vec![tmp.path().to_path_buf()],
    };
    let outcome = collector.parse_file(&path, 0).unwrap();
    assert!(outcome.events.is_empty());
    assert_eq!(outcome.errors.len(), 3);
    assert_eq!(outcome.errors[0].line, 2);
    assert!(outcome.errors[0].reason.contains("json_invalido"));
    assert_eq!(outcome.errors[1].line, 3);
    assert!(outcome.errors[1].reason.contains("fecha_invalida"));
    assert_eq!(outcome.errors[2].line, 4);
    assert!(outcome.errors[2].reason.contains("utf8_invalido"));

    for err in &outcome.errors {
        assert!(!err.reason.contains("secreto-del-prompt"));
    }
}

#[test]
fn from_offset_mayor_que_tamano_relee_desde_cero() {
    let collector = collector_for_fixtures();
    let path = fixture_transcript();
    let file_len = fs::metadata(&path).unwrap().len();

    let outcome = collector.parse_file(&path, file_len + 64).unwrap();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.events.len(), 3);
    assert_eq!(outcome.new_offset, file_len);
}
