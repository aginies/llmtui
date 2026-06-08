//! build.rs — Compile-time assertions for parameter struct field counts.
//!
//! Ensures DefaultParams, ModelSettings, and ModelOverride stay in sync.
//! If a field is added/removed from any struct, the build fails with a
//! helpful message listing all locations that must be updated.

use std::env;
use std::fs;
use std::path::Path;

/// Expected field counts for each struct.
/// These represent the total number of `pub field_name: Type` lines
/// within each struct body (excluding comments and blank lines).
const EXPECTED_DEFAULT_PARAMS_FIELDS: usize = 85;
const EXPECTED_MODEL_SETTINGS_FIELDS: usize = 75;
const EXPECTED_MODEL_OVERRIDE_FIELDS: usize = 70;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = Path::new(&crate_dir).join("src");

    // Count fields in each struct
    let default_params_count = count_struct_fields(&src_dir, "config.rs", "DefaultParams");
    let model_settings_count = count_struct_fields(&src_dir, "models.rs", "ModelSettings");
    let model_override_count = count_struct_fields(&src_dir, "config.rs", "ModelOverride");

    let mut errors = Vec::new();

    if default_params_count != EXPECTED_DEFAULT_PARAMS_FIELDS {
        errors.push((
            "DefaultParams",
            default_params_count,
            EXPECTED_DEFAULT_PARAMS_FIELDS,
            vec![
                "  1. src/config.rs — DefaultParams struct field definition",
                "  2. src/config.rs — DefaultParams Default impl (default values)",
                "  3. src/models.rs — ModelSettings struct field (if shared)",
                "  4. src/models.rs — From<DefaultParams> for ModelSettings",
                "  5. src/config.rs — ModelOverride struct field (if shared)",
                "  6. src/config.rs — ModelOverride::from_settings()",
                "  7. src/config.rs — ModelOverride::apply() (macro call)",
                "  8. src/tui/settings.rs — all_fields() SettingField entry",
                "  9. src/tui/settings.rs — profile_settings_parts() diff macro",
                "10. src/tui/app/profiles.rs — settings_fingerprint()",
            ],
        ));
    }

    if model_settings_count != EXPECTED_MODEL_SETTINGS_FIELDS {
        errors.push((
            "ModelSettings",
            model_settings_count,
            EXPECTED_MODEL_SETTINGS_FIELDS,
            vec![
                "  1. src/models.rs — ModelSettings struct field",
                "  2. src/models.rs — From<DefaultParams> for ModelSettings",
                "  3. src/config.rs — DefaultParams struct (if shared)",
                "  4. src/config.rs — DefaultParams Default impl (if shared)",
                "  5. src/config.rs — ModelOverride struct (if shared)",
                "  6. src/config.rs — ModelOverride::from_settings()",
                "  7. src/config.rs — ModelOverride::apply() (macro call)",
                "  8. src/tui/settings.rs — all_fields() SettingField entry",
                "  9. src/tui/settings.rs — profile_settings_parts() diff macro",
                "10. src/tui/app/profiles.rs — settings_fingerprint()",
                "11. src/tui/event/helpers.rs — sync_global_settings() (if global)",
            ],
        ));
    }

    if model_override_count != EXPECTED_MODEL_OVERRIDE_FIELDS {
        errors.push((
            "ModelOverride",
            model_override_count,
            EXPECTED_MODEL_OVERRIDE_FIELDS,
            vec![
                "  1. src/config.rs — ModelOverride struct field (Option<T>)",
                "  2. src/config.rs — ModelOverride::from_settings()",
                "  3. src/config.rs — ModelOverride::apply() (macro call)",
                "  4. src/tui/settings.rs — profile_settings_parts() diff macro",
            ],
        ));
    }

    if !errors.is_empty() {
        eprintln!("\nERROR: Parameter struct field count mismatch!\n");
        for (name, actual, expected, locations) in &errors {
            eprintln!("  {} has {} fields (expected {})", name, actual, expected);
            eprintln!("  When adding a field, update:");
            for loc in locations {
                eprintln!("{}", loc);
            }
            eprintln!();
        }
        eprintln!(
            "The derived PartialEq on ModelSettings and DefaultParams provides\n\
            compile-time guarantees for is_dirty() correctness.\n\
            This build script provides an additional runtime check on field counts."
        );
        std::process::exit(1);
    }

    // Copy locales directory to the output directory so translations are
    // available next to the built binary at runtime.
    // OUT_DIR is like target/release/build/llm-manager-xxx/, so parent().parent()
    // gives us target/release/ where the binary lands.
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let locales_src = Path::new(&crate_dir).join("locales");
    let out_dir = env::var("OUT_DIR").unwrap();
    let locales_dst = Path::new(&out_dir)
        .parent()
        .and_then(|p| p.parent())
        .expect("OUT_DIR should have two parents")
        .join("locales");

    // Debug: write to a file to trace execution
    let _ = fs::write("/tmp/build-debug.log", format!(
        "locales_src={:?} out_dir={} locales_dst={:?} is_dir={}\n",
        locales_src, out_dir, locales_dst, locales_src.is_dir()
    ));

    if locales_src.is_dir() {
        copy_dir_recursive(&locales_src, &locales_dst).unwrap_or_else(|e| {
            eprintln!("build.rs: failed to copy locales: {}", e);
            std::process::exit(1);
        });
        println!("cargo:rerun-if-changed=locales");
    }
}

/// Recursively copy a directory from `src` to `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)?;
        }
    }
    Ok(())
}

/// Count the number of `pub field_name: Type` lines within a struct body.
/// Looks for the struct by name in the given file within the src directory.
fn count_struct_fields(src_dir: &Path, file: &str, struct_name: &str) -> usize {
    let file_path = src_dir.join(file);
    let content = fs::read_to_string(&file_path).unwrap_or_default();

    // Find the line number of the struct definition
    let _struct_line_num = content
        .lines()
        .position(|l| {
            l.trim().starts_with(&format!("pub struct {}", struct_name))
                || l.trim()
                    .starts_with(&format!("pub struct  {}", struct_name))
        })
        .unwrap_or_else(|| {
            eprintln!("WARNING: Could not find struct {} in {}", struct_name, file);
            panic!("Could not find struct {}", struct_name);
        });

    // Find the opening brace on or after the struct line
    let after_struct = &content[content
        .find(&format!("pub struct {}", struct_name))
        .unwrap_or(0)..];
    let brace_offset = after_struct.find('{').unwrap_or_else(|| {
        eprintln!(
            "WARNING: Could not find opening brace for struct {} in {}",
            struct_name, file
        );
        panic!("Could not find opening brace for struct {}", struct_name);
    });

    // Calculate absolute position of the opening brace
    let abs_start = content
        .find(&format!("pub struct {}", struct_name))
        .unwrap_or(0)
        + brace_offset;

    // Find the matching closing brace
    let end_idx = find_matching_brace(&content, abs_start);

    // Extract the struct body
    let body = &content[abs_start + 1..end_idx];

    // Count pub field lines (lines starting with "pub " that contain ":")
    // Exclude: #[derive], #[serde], comments, blank lines
    body.lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("pub ")
                && trimmed.contains(':')
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("#")
        })
        .count()
}

/// Find the matching closing brace for an opening brace at the given index.
fn find_matching_brace(content: &str, start: usize) -> usize {
    let mut depth = 0;
    let mut i = start;

    // Skip past the opening brace
    if content.as_bytes().get(start) == Some(&b'{') {
        depth = 1;
        i += 1;
    }

    while i < content.len() && depth > 0 {
        let byte = content.as_bytes()[i];
        match byte {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            b'"' => {
                // Skip string literals
                i += 1;
                while i < content.len() {
                    if content.as_bytes()[i] == b'"' {
                        break;
                    }
                    if content.as_bytes()[i] == b'\\' {
                        i += 1; // Skip escaped character
                    }
                    i += 1;
                }
            }
            b'/' if content.as_bytes().get(i + 1) == Some(&b'/') => {
                // Skip single-line comments
                while i < content.len() && content.as_bytes()[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if content.as_bytes().get(i + 1) == Some(&b'*') => {
                // Skip multi-line comments
                i += 2;
                while i + 1 < content.len() {
                    if content.as_bytes()[i] == b'*' && content.as_bytes()[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    i
}
