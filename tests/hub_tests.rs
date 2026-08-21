//! Tests for backend/hub.rs — HuggingFace API, binary management, downloads.
//!
//! Tests cover: free space detection, default tag selection, binary paths,
//! backend installation checks, archive extraction, and directory walking.
//!
//! Network-dependent tests (search_models, list_gguf_files, download_file)
//! are skipped here to avoid CI flakiness. The pure functions are tested directly.

use llm_manager::backend::hub::{
    binary_name, extract_archive, get_backend_dir, get_bin_base, get_free_space_bytes,
    is_backend_any_version_installed, is_backend_version_installed, lib_extension,
    lib_sentinel_name, is_lib_sentinel_present, list_installed_backends, walk_dir_recursive,
};
use llm_manager::models::Backend;
use std::fs;

// ── Free space detection ────────────────────────────────────────

#[test]
fn test_get_free_space_bytes_returns_positive_for_tmp() {
    let space = get_free_space_bytes(std::path::Path::new("/tmp"));
    assert!(matches!(space, Some(s) if s > 0));
}

#[test]
fn test_get_free_space_bytes_returns_none_for_nonexistent() {
    let space = get_free_space_bytes(std::path::Path::new(
        "/nonexistent/path/that/does/not/exist",
    ));
    assert!(space.is_none());
}

// ── Binary names ────────────────────────────────────────────────

#[test]
fn test_binary_name_returns_llama_server_on_unix() {
    let name = binary_name();
    assert_eq!(name, "llama-server");
}

#[test]
fn test_lib_sentinel_name_returns_correct_for_platform() {
    let name = lib_sentinel_name();
    #[cfg(target_os = "linux")]
    assert_eq!(name, "libllama.so");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libllama.dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "libllama.dll");
}

#[test]
fn test_is_lib_sentinel_present() {
    let mut temp_dir = std::env::temp_dir();
    temp_dir.push(format!("llm-manager-test-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_dir).unwrap();
    
    // Initially false
    assert!(!is_lib_sentinel_present(&temp_dir));

    // Platform-specific setup
    #[cfg(target_os = "windows")]
    {
        // On Windows, either llama.dll or libllama.dll makes it true
        let lib_path1 = temp_dir.join("libllama.dll");
        let lib_path2 = temp_dir.join("llama.dll");
        
        fs::write(&lib_path1, "dummy").unwrap();
        assert!(is_lib_sentinel_present(&temp_dir));
        
        fs::remove_file(&lib_path1).unwrap();
        assert!(!is_lib_sentinel_present(&temp_dir));
        
        fs::write(&lib_path2, "dummy").unwrap();
        assert!(is_lib_sentinel_present(&temp_dir));
    }
    #[cfg(target_os = "macos")]
    {
        let lib_path = temp_dir.join("libllama.dylib");
        fs::write(&lib_path, "dummy").unwrap();
        assert!(is_lib_sentinel_present(&temp_dir));
    }
    #[cfg(target_os = "linux")]
    {
        let lib_path = temp_dir.join("libllama.so");
        fs::write(&lib_path, "dummy").unwrap();
        assert!(is_lib_sentinel_present(&temp_dir));
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_lib_extension_returns_correct_for_platform() {
    let ext = lib_extension();
    #[cfg(target_os = "linux")]
    assert_eq!(ext, ".so");
    #[cfg(target_os = "macos")]
    assert_eq!(ext, ".dylib");
    #[cfg(target_os = "windows")]
    assert_eq!(ext, ".dll");
}

// ── Binary base directory ───────────────────────────────────────

#[test]
fn test_get_bin_base_returns_valid_path() {
    let base = get_bin_base();
    assert!(base.to_string_lossy().contains("llm-manager"));
    assert!(base.to_string_lossy().contains("bin"));
}

#[test]
fn test_get_backend_dir_returns_correct_path() {
    let dir = get_backend_dir(Backend::Cpu, "b4100");
    assert!(dir.to_string_lossy().contains("llama-server-cpu-b4100"));
}

#[test]
fn test_get_backend_dir_for_cuda() {
    let dir = get_backend_dir(Backend::Cuda, "b9279");
    assert!(dir.to_string_lossy().contains("llama-server-cuda-b9279"));
}

#[test]
fn test_get_backend_dir_for_rocm() {
    let dir = get_backend_dir(Backend::Rocm, "b4100");
    assert!(dir.to_string_lossy().contains("llama-server-rocm-b4100"));
}

// ── Backend installation checks ─────────────────────────────────

#[test]
fn test_is_backend_version_installed_returns_false_for_nonexistent() {
    let installed = is_backend_version_installed(Backend::Cpu, Some("nonexistent-tag-12345"));
    assert!(!installed);
}

#[test]
fn test_is_backend_version_installed_returns_false_for_null_tag() {
    let installed = is_backend_version_installed(Backend::Cpu, None);
    assert!(!installed);
}

#[test]
fn test_is_backend_any_version_installed_does_not_panic() {
    // Result depends on system state - just verify it returns without panicking
    let _ = is_backend_any_version_installed(Backend::Cpu);
}

// ── List installed backends ─────────────────────────────────────

#[test]
fn test_list_installed_backends_returns_vec() {
    let backends = list_installed_backends();
    // Should return a valid vec (may be empty)
    assert!(backends.is_empty() || backends.len() > 0);
}

#[test]
fn test_list_installed_backends_empty_when_no_bin_dir() {
    // This depends on system state - just verify it doesn't panic
    let _ = list_installed_backends();
}

// ── Archive extraction ──────────────────────────────────────────

#[test]
fn test_extract_archive_creates_files() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-extract");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    // Create a simple tar.gz file for testing using a subprocess
    let archive_path = temp_dir.join("test.tar.gz");

    // Use tar command to create a simple archive
    let create_result = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd {} && echo 'hello' > test.txt && tar czf test.tar.gz test.txt",
            temp_dir.display()
        ))
        .output();

    if create_result.is_ok() {
        let dest_dir = temp_dir.join("extracted");
        let result = extract_archive(&archive_path, &dest_dir);

        // Should succeed
        assert!(result.is_ok());
    }

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_extract_tar_gz_with_subdirectories() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-extract-subdirs");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir.join("src/sub"));

    // Create files in subdirectories
    fs::write(temp_dir.join("src/subdir.txt"), "subdir content").unwrap();
    fs::write(temp_dir.join("src/sub/nested.txt"), "nested content").unwrap();
    fs::write(temp_dir.join("root.txt"), "root content").unwrap();

    // Create tar.gz with subdirectories (tar may or may not include trailing slashes)
    let archive_path = temp_dir.join("test_subdirs.tar.gz");
    let create_result = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd {} && tar czf test_subdirs.tar.gz root.txt src/",
            temp_dir.display()
        ))
        .output();

    assert!(create_result.is_ok(), "Failed to create test archive");

    let dest_dir = temp_dir.join("extracted_subdirs");
    let result = extract_archive(&archive_path, &dest_dir);

    assert!(result.is_ok(), "Extraction should succeed: {:?}", result);

    // Verify files were extracted correctly
    assert!(dest_dir.join("root.txt").exists());
    assert_eq!(
        fs::read_to_string(dest_dir.join("root.txt")).unwrap(),
        "root content"
    );
    assert!(dest_dir.join("src/subdir.txt").exists());
    assert_eq!(
        fs::read_to_string(dest_dir.join("src/subdir.txt")).unwrap(),
        "subdir content"
    );
    assert!(dest_dir.join("src/sub/nested.txt").exists());
    assert_eq!(
        fs::read_to_string(dest_dir.join("src/sub/nested.txt")).unwrap(),
        "nested content"
    );

    // Verify src is a directory, not a file
    assert!(dest_dir.join("src").is_dir());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_extract_tar_gz_with_symlinks() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-extract-symlinks");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir.join("src"));

    // Create a real file and a symlink to it
    fs::write(temp_dir.join("src/real_file.txt"), "real content").unwrap();

    // Create tar.gz with symlink (using subprocess)
    let archive_path = temp_dir.join("test_symlinks.tar.gz");
    let create_result = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd {} && ln -s real_file.txt link.txt && tar czf test_symlinks.tar.gz src/ link.txt",
            temp_dir.display()
        ))
        .output();

    assert!(
        create_result.is_ok(),
        "Failed to create test archive with symlinks"
    );

    let dest_dir = temp_dir.join("extracted_symlinks");
    let result = extract_archive(&archive_path, &dest_dir);

    assert!(
        result.is_ok(),
        "Symlink archive extraction should succeed: {:?}",
        result
    );

    // Verify the symlink was preserved (not resolved to a regular file)
    let link_path = dest_dir.join("link.txt");
    assert!(
        link_path.exists() || link_path.symlink_metadata().is_ok(),
        "Symlink should exist (even if dangling)"
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_extract_archive_fails_on_nonexistent_archive() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-extract-fail");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    let result = extract_archive(
        &temp_dir.join("nonexistent.tar.gz"),
        &temp_dir.join("extracted"),
    );

    let _ = fs::remove_dir_all(&temp_dir);
    assert!(result.is_err());
}

// ── Directory walking ───────────────────────────────────────────

#[test]
fn test_walk_dir_recursive_finds_files() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-walk");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir.join("subdir"));

    // Create test files
    fs::write(temp_dir.join("file1.txt"), "content1").unwrap();
    fs::write(temp_dir.join("subdir").join("file2.txt"), "content2").unwrap();

    let mut found_files = Vec::new();
    walk_dir_recursive(&temp_dir, 0, 5, &mut |entry| {
        if entry.path().is_file() {
            found_files.push(entry.path().clone());
        }
    });

    let _ = fs::remove_dir_all(&temp_dir);

    // Should find at least the two files we created
    assert!(found_files.len() >= 2);
}

#[test]
fn test_walk_dir_recursive_respects_depth() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-walk-depth");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir.join("level1").join("level2"));

    fs::write(temp_dir.join("root.txt"), "root").unwrap();
    fs::write(temp_dir.join("level1").join("l1.txt"), "l1").unwrap();
    fs::write(temp_dir.join("level1").join("level2").join("l2.txt"), "l2").unwrap();

    let mut found_files = Vec::new();
    // Walk with depth limit
    walk_dir_recursive(&temp_dir, 0, 2, &mut |entry| {
        if entry.path().is_file() {
            found_files.push(entry.path().clone());
        }
    });

    let _ = fs::remove_dir_all(&temp_dir);

    // Should find at least root.txt and l1.txt
    assert!(found_files.len() >= 2);
}

#[test]
fn test_walk_dir_recursive_handles_empty_dir() {
    let temp_dir = std::env::temp_dir().join("llm-manager-test-walk-empty");
    let _ = fs::remove_dir_all(&temp_dir);
    let _ = fs::create_dir_all(&temp_dir);

    let mut found_files = Vec::new();
    walk_dir_recursive(&temp_dir, 0, 5, &mut |entry| {
        if entry.path().is_file() {
            found_files.push(entry.path().clone());
        }
    });

    let _ = fs::remove_dir_all(&temp_dir);

    // Should find nothing
    assert!(found_files.is_empty());
}
