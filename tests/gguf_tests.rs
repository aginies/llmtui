//! Tests for the built-in GGUF header parser (`src/gguf.rs`).
//!
//! Regression coverage for the NVFP4 panic: models quantized with NVFP4
//! (GGML type 40) used to make the previous parser hit
//! `unreachable!("GGMLType::Count is not a real data format")` and break
//! the TUI. The built-in parser keeps tensor types as raw u32 and must
//! handle any type value.

use llm_manager::GgufMetadata;
use llm_manager::gguf::{GgufValue, human_number, parse_header};

/// Minimal in-memory GGUF v3 (little-endian) builder for tests.
#[derive(Default)]
struct GgufBuilder {
    data: Vec<u8>,
}

impl GgufBuilder {
    fn header(&mut self, tensor_count: u64, kv_count: u64) {
        self.data.extend_from_slice(b"GGUF");
        self.data.extend_from_slice(&3u32.to_le_bytes());
        self.data.extend_from_slice(&tensor_count.to_le_bytes());
        self.data.extend_from_slice(&kv_count.to_le_bytes());
    }

    fn kv_string(&mut self, key: &str, value: &str) {
        let kb = key.as_bytes();
        self.data
            .extend_from_slice(&(kb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(kb);
        self.data.extend_from_slice(&8u32.to_le_bytes()); // value type: string
        let vb = value.as_bytes();
        self.data
            .extend_from_slice(&(vb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(vb);
    }

    fn kv_u32(&mut self, key: &str, value: u32) {
        let kb = key.as_bytes();
        self.data
            .extend_from_slice(&(kb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(kb);
        self.data.extend_from_slice(&4u32.to_le_bytes()); // value type: uint32
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    fn kv_u64(&mut self, key: &str, value: u64) {
        let kb = key.as_bytes();
        self.data
            .extend_from_slice(&(kb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(kb);
        self.data.extend_from_slice(&10u32.to_le_bytes()); // value type: uint64
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    fn kv_string_array(&mut self, key: &str, values: &[&str]) {
        let kb = key.as_bytes();
        self.data
            .extend_from_slice(&(kb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(kb);
        self.data.extend_from_slice(&9u32.to_le_bytes()); // value type: array
        self.data.extend_from_slice(&8u32.to_le_bytes()); // element type: string
        self.data
            .extend_from_slice(&(values.len() as u64).to_le_bytes());
        for v in values {
            let vb = v.as_bytes();
            self.data
                .extend_from_slice(&(vb.len() as u64).to_le_bytes());
            self.data.extend_from_slice(vb);
        }
    }

    fn tensor(&mut self, name: &str, dims: &[u64], ggml_type: u32, offset: u64) {
        let nb = name.as_bytes();
        self.data
            .extend_from_slice(&(nb.len() as u64).to_le_bytes());
        self.data.extend_from_slice(nb);
        self.data
            .extend_from_slice(&(dims.len() as u32).to_le_bytes());
        for d in dims {
            self.data.extend_from_slice(&d.to_le_bytes());
        }
        self.data.extend_from_slice(&ggml_type.to_le_bytes());
        self.data.extend_from_slice(&offset.to_le_bytes());
    }

    fn write_to(&self, path: &std::path::Path) {
        std::fs::write(path, &self.data).unwrap();
    }
}

fn temp_gguf(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gguf_test_{}_{}.gguf", name, std::process::id()));
    p
}

// ── NVFP4 regression ─────────────────────────────────────────────

#[test]
fn nvfp4_and_unknown_tensor_types_parse_without_panic() {
    // Tensors with GGML types 40 (NVFP4), 41 (Q1_0) and 42 (Q2_0) — all
    // unknown to the previous gguf-rs parser, which panicked on 40.
    let mut b = GgufBuilder::default();
    b.header(3, 6);
    b.kv_string("general.architecture", "qwen3");
    b.kv_u32("qwen3.block_count", 48);
    b.kv_u32("qwen3.embedding_length", 3584);
    b.kv_u64("qwen3.context_length", 4096);
    b.kv_u32("general.file_type", 39); // NVFP4
    b.kv_string_array(
        "general.capabilities",
        &["chat", "hybrid", "text-generation"],
    );
    b.tensor("blk.0.attn.q", &[3584, 3584], 40, 64); // NVFP4
    b.tensor("blk.0.attn.k", &[512, 3584], 41, 128); // Q1_0
    b.tensor("token_embd", &[151936, 3584], 42, 192); // Q2_0
    let path = temp_gguf("nvfp4");
    b.write_to(&path);

    let header = parse_header(&path).expect("NVFP4/unknown types must parse");
    assert_eq!(
        header
            .kv
            .get("general.architecture")
            .and_then(|v| v.as_str()),
        Some("qwen3")
    );
    // 3584*3584 + 512*3584 + 151936*3584 = 12_845_056 + 1_835_008 + 544_538_624
    assert_eq!(header.parameters, 559_218_688);

    let meta = GgufMetadata::from_path(&path).expect("GgufMetadata must parse");
    assert_eq!(meta.arch, "qwen3");
    assert_eq!(meta.layers, 48);
    assert_eq!(meta.hidden_size, 3584);
    assert_eq!(meta.n_ctx_train, 4096);
    assert_eq!(meta.file_type, "NVFP4");
    assert_eq!(meta.model_parameters, "559M");
    assert_eq!(meta.capabilities, vec!["chat", "hybrid", "text-generation"]);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn all_known_tensor_types_parse() {
    // Every GGML type value 0..=42 must parse (unknown ones are raw u32).
    let kv_count: u64 = 1;
    let tensor_count: u64 = 43;
    let mut b = GgufBuilder::default();
    b.header(tensor_count, kv_count);
    b.kv_string("general.architecture", "llama");
    for t in 0..=42u32 {
        b.tensor(&format!("t{}", t), &[1024, 1024], t, 64);
    }
    let path = temp_gguf("alltypes");
    b.write_to(&path);

    let header = parse_header(&path).expect("all type values must parse");
    assert_eq!(header.parameters, 43 * 1024 * 1024);
    let _ = std::fs::remove_file(&path);
}

// ── Error handling ───────────────────────────────────────────────

#[test]
fn bad_magic_is_an_error() {
    let path = temp_gguf("badmagic");
    std::fs::write(&path, b"NOTAGGUF_FILE_CONTENT_HERE").unwrap();
    let err = parse_header(&path).unwrap_err();
    assert!(err.to_string().contains("not a GGUF file"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn truncated_header_is_an_error_not_a_panic() {
    // Header claims 2 tensors but file ends after the first.
    let mut b = GgufBuilder::default();
    b.header(2, 0);
    b.tensor("t0", &[4, 4], 2, 64);
    let path = temp_gguf("truncated");
    b.write_to(&path);
    assert!(parse_header(&path).is_err());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn unsupported_version_is_an_error() {
    let mut data = Vec::new();
    data.extend_from_slice(b"GGUF");
    data.extend_from_slice(&99u32.to_le_bytes());
    let path = temp_gguf("version");
    std::fs::write(&path, &data).unwrap();
    let err = parse_header(&path).unwrap_err();
    assert!(err.to_string().contains("unsupported GGUF version"));
    let _ = std::fs::remove_file(&path);
}

// ── Value accessors ──────────────────────────────────────────────

#[test]
fn value_accessors_follow_serde_json_semantics() {
    assert_eq!(GgufValue::U32(42).as_u64(), Some(42));
    assert_eq!(GgufValue::I64(-5).as_i64(), Some(-5));
    assert_eq!(GgufValue::I64(-5).as_u64(), None);
    assert_eq!(GgufValue::F32(42.5).as_f64(), Some(42.5));
    assert_eq!(GgufValue::U64(7).as_f64(), Some(7.0));
    assert_eq!(GgufValue::String("x".into()).as_str(), Some("x"));
    assert_eq!(
        GgufValue::Array(vec![GgufValue::String("a".into())])
            .as_array()
            .map(|a| a.len()),
        Some(1)
    );
    assert_eq!(GgufValue::Bool(true).as_u64(), None);
}

// ── human_number ─────────────────────────────────────────────────

#[test]
fn human_number_formats_like_previous_parser() {
    assert_eq!(human_number(900), "900");
    assert_eq!(human_number(1_500), "2K");
    assert_eq!(human_number(1_200_000), "1M");
    assert_eq!(human_number(8_200_000_000), "8B");
}
