use std::fs;
use std::path::Path;

/// Detected operating system platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Macos,
}

/// GPU vendors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum GpuVendor {
    Amd,
    Nvidia,
    Intel,
    Apple,
    Unknown,
}

/// Detect the current operating system platform.
pub fn detect_platform() -> Platform {
    match std::env::consts::OS {
        "windows" => Platform::Windows,
        "macos" => Platform::Macos,
        _ => Platform::Linux,
    }
}

/// Check if the current architecture is ARM64.
pub fn is_arm64() -> bool {
    cfg!(target_arch = "aarch64")
}

/// Get the platform as a string slice.
pub fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Linux => "linux",
        Platform::Windows => "windows",
        Platform::Macos => "macos",
    }
}

/// Check if a backend variant is available on the given platform.
pub fn backend_supported(backend: crate::models::Backend, platform: Platform) -> bool {
    match platform {
        Platform::Linux => backend.is_linux(),
        Platform::Windows => backend.is_windows(),
        Platform::Macos => backend.is_macos(),
    }
}

/// Returns paths to all primary DRM card directories (card0, card1, ...).
fn drm_card_paths() -> Vec<std::path::PathBuf> {
    let drm_path = Path::new("/sys/class/drm");
    if !drm_path.exists() {
        return Vec::new();
    }
    fs::read_dir(drm_path)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    let n = e.file_name();
                    let s = n.to_string_lossy();
                    s.starts_with("card") && !s.contains('-')
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

/// Detect all GPU vendors by scanning /sys/class/drm/card*/device/vendor (Linux).
/// Returns a Vec of unique vendors (preserves detection order, deduplicates).
fn detect_gpu_vendors_linux_impl() -> Vec<GpuVendor> {
    let mut vendors = Vec::new();
    for card_path in drm_card_paths() {
        let vendor_path = card_path.join("device/vendor");
        if let Ok(vendor_id) = fs::read_to_string(vendor_path) {
            let vendor_id = vendor_id.trim();
            let vendor = match vendor_id {
                "0x1002" => GpuVendor::Amd,
                "0x10de" => GpuVendor::Nvidia,
                "0x8086" => GpuVendor::Intel,
                _ => continue,
            };
            if !vendors.contains(&vendor) {
                vendors.push(vendor);
            }
        }
    }

    if vendors.is_empty() {
        vendors.push(GpuVendor::Unknown);
    }

    vendors
}

/// Detect all GPU model names (one per GPU, Linux).
/// For AMD GPUs, includes the GFX target version.
fn detect_gpu_models_linux_impl() -> Vec<Option<String>> {
    let card_paths = drm_card_paths();
    if card_paths.is_empty() {
        return Vec::new();
    }

    let amd_gfx_targets = detect_amd_gfx_targets();
    let mut amd_card_idx: usize = 0;
    let mut models = Vec::new();
    for card_path in &card_paths {
        let vendor_path = card_path.join("device/vendor");
        if let Ok(vendor_id) = fs::read_to_string(vendor_path) {
            let vendor_id = vendor_id.trim();
            let vendor = match vendor_id {
                "0x1002" => GpuVendor::Amd,
                "0x10de" => GpuVendor::Nvidia,
                "0x8086" => GpuVendor::Intel,
                _ => continue,
            };

            let vendor_name = match vendor {
                GpuVendor::Amd => "AMD",
                GpuVendor::Nvidia => "NVIDIA",
                GpuVendor::Intel => "Intel",
                GpuVendor::Apple => continue,
                GpuVendor::Unknown => continue,
            };

            if vendor == GpuVendor::Amd {
                if let Some(gfx) = amd_gfx_targets.get(amd_card_idx % amd_gfx_targets.len()) {
                    models.push(Some(format!("{} ({})", vendor_name, gfx)));
                } else {
                    models.push(Some(vendor_name.to_string()));
                }
                amd_card_idx += 1;
            } else {
                models.push(Some(vendor_name.to_string()));
            }
        }
    }

    models
}

/// Format a raw GFX target version value to a string (e.g. 110003 -> "gfx1103").
/// Returns None for value 0 (CPU node).
fn gfx_target_to_string(val: u32) -> Option<String> {
    if val == 0 {
        return None;
    }
    let major = val / 10000;
    let minor = (val % 10000) / 100;
    let stepping = val % 100;

    if stepping > 0 {
        Some(format!("gfx{}{}{}", major, minor, stepping))
    } else {
        Some(format!("gfx{}{}", major, minor))
    }
}

/// Collect all unique, non-zero AMD GFX target versions from KFD nodes.
/// Skips CPU nodes (gfx_target_version == 0).
/// Returns deduplicated targets in detection order.
pub fn detect_amd_gfx_targets() -> Vec<String> {
    let kfd_path = Path::new("/sys/class/kfd/kfd/topology/nodes");
    if !kfd_path.exists() {
        return Vec::new();
    }

    let mut targets = Vec::new();
    if let Ok(entries) = fs::read_dir(kfd_path) {
        for entry in entries.flatten() {
            let props_path = entry.path().join("properties");
            if let Ok(props) = fs::read_to_string(props_path) {
                for line in props.lines() {
                    if line.starts_with("gfx_target_version")
                        && let Some(val_str) = line.split_whitespace().last()
                        && let Ok(val) = val_str.parse::<u32>()
                        && let Some(gfx) = gfx_target_to_string(val)
                    {
                        if !targets.contains(&gfx) {
                            targets.push(gfx);
                        }
                        break;
                    }
                }
            }
        }
    }
    targets
}

/// Detect AMD GFX target version (e.g. "gfx1100").
/// Returns the first non-zero GFX target found, or None.
pub fn detect_amd_gfx_target() -> Option<String> {
    detect_amd_gfx_targets().into_iter().next()
}

/// Get the best Lemonade asset suffix for the detected AMD architecture
pub fn get_lemonade_gfx_suffix(gfx: &str) -> &'static str {
    if gfx.starts_with("gfx103") {
        "gfx103X"
    } else if gfx.starts_with("gfx110") {
        "gfx110X"
    } else if gfx == "gfx1150" {
        "gfx1150"
    } else if gfx == "gfx1151" {
        "gfx1151"
    } else if gfx.starts_with("gfx120") {
        "gfx120X"
    } else {
        // Fallback to most common recent if unknown
        "gfx110X"
    }
}

// ── Platform-specific GPU detection ──────────────────────────────────

/// Detect GPU vendors on Windows using wmic.
#[cfg(target_os = "windows")]
pub fn detect_gpu_vendors_windows() -> Vec<GpuVendor> {
    let mut vendors = Vec::new();
    let output = std::process::Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "Name"])
        .output();

    let names = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return Vec::new(),
    };

    for line in names.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("Name") {
            continue;
        }

        let lower = line.to_lowercase();
        if lower.contains("nvidia") {
            if !vendors.contains(&GpuVendor::Nvidia) {
                vendors.push(GpuVendor::Nvidia);
            }
        } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("rx ") {
            if !vendors.contains(&GpuVendor::Amd) {
                vendors.push(GpuVendor::Amd);
            }
        } else if lower.contains("intel") {
            if !vendors.contains(&GpuVendor::Intel) {
                vendors.push(GpuVendor::Intel);
            }
        }
    }

    if vendors.is_empty() {
        vendors.push(GpuVendor::Unknown);
    }

    vendors
}

/// Detect GPU models on Windows using wmic.
#[cfg(target_os = "windows")]
pub fn detect_gpu_models_windows() -> Vec<Option<String>> {
    let output = std::process::Command::new("wmic")
        .args(["path", "win32_VideoController", "get", "Name"])
        .output();

    let names = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return Vec::new(),
    };

    let mut models = Vec::new();
    for line in names.lines() {
        let line = line.trim();
        if line.is_empty() || line.eq_ignore_ascii_case("Name") {
            continue;
        }
        models.push(Some(line.to_string()));
    }

    models
}

/// Detect GPU vendors on macOS using system_profiler.
#[cfg(target_os = "macos")]
pub fn detect_gpu_vendors_macos() -> Vec<GpuVendor> {
    let mut vendors = Vec::new();
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output();

    let data = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return Vec::new(),
    };

    for line in data.lines() {
        let trimmed = line.trim();
        if !trimmed.contains(":") {
            continue;
        }

        let gpu_name = trimmed.split(':').nth(1).unwrap_or("").trim();
        let lower = gpu_name.to_lowercase();

        if lower.contains("apple")
            && (lower.contains("m1")
                || lower.contains("m2")
                || lower.contains("m3")
                || lower.contains("m4")
                || lower.contains("apple gpu")
                || lower.contains("apple silicon"))
        {
            if !vendors.contains(&GpuVendor::Apple) {
                vendors.push(GpuVendor::Apple);
            }
        } else if lower.contains("nvidia") {
            if !vendors.contains(&GpuVendor::Nvidia) {
                vendors.push(GpuVendor::Nvidia);
            }
        } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("firepro") {
            if !vendors.contains(&GpuVendor::Amd) {
                vendors.push(GpuVendor::Amd);
            }
        } else if lower.contains("intel") {
            if !vendors.contains(&GpuVendor::Intel) {
                vendors.push(GpuVendor::Intel);
            }
        }
    }

    if vendors.is_empty() {
        vendors.push(GpuVendor::Unknown);
    }

    vendors
}

/// Detect GPU models on macOS using system_profiler.
#[cfg(target_os = "macos")]
pub fn detect_gpu_models_macos() -> Vec<Option<String>> {
    let output = std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType"])
        .output();

    let data = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => return Vec::new(),
    };

    let mut models = Vec::new();
    let mut in_gpu_section = false;

    for line in data.lines() {
        let trimmed = line.trim();

        if trimmed.contains("Chipset Model") || trimmed.contains("GPU Name") {
            in_gpu_section = true;
            if let Some(name) = trimmed.split(':').nth(1) {
                let name = name.trim();
                if !name.is_empty() {
                    models.push(Some(name.to_string()));
                }
            }
        } else if in_gpu_section && trimmed.contains("Vendor") {
            in_gpu_section = false;
        } else if in_gpu_section && trimmed.is_empty() {
            in_gpu_section = false;
        }
    }

    models
}

/// Detect GPU vendors using platform-specific methods.
#[cfg(target_os = "linux")]
pub fn detect_gpu_vendors() -> Vec<GpuVendor> {
    detect_gpu_vendors_linux_impl()
}

/// Detect GPU models using platform-specific methods.
#[cfg(target_os = "linux")]
pub fn detect_gpu_models() -> Vec<Option<String>> {
    detect_gpu_models_linux_impl()
}

/// Detect GPU vendors using platform-specific methods.
#[cfg(target_os = "windows")]
pub fn detect_gpu_vendors() -> Vec<GpuVendor> {
    detect_gpu_vendors_windows()
}

/// Detect GPU models using platform-specific methods.
#[cfg(target_os = "windows")]
pub fn detect_gpu_models() -> Vec<Option<String>> {
    detect_gpu_models_windows()
}

/// Detect GPU vendors using platform-specific methods.
#[cfg(target_os = "macos")]
pub fn detect_gpu_vendors() -> Vec<GpuVendor> {
    detect_gpu_vendors_macos()
}

/// Detect GPU models using platform-specific methods.
#[cfg(target_os = "macos")]
pub fn detect_gpu_models() -> Vec<Option<String>> {
    detect_gpu_models_macos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_windows_nvidia() {
        let input = "Name\nNVIDIA GeForce RTX 4090\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.contains(&GpuVendor::Nvidia));
    }

    #[test]
    fn test_parse_windows_amd() {
        let input = "Name\nAMD Radeon RX 7900 XTX\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.contains(&GpuVendor::Amd));
    }

    #[test]
    fn test_parse_windows_intel() {
        let input = "Name\nIntel(R) UHD Graphics 770\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.contains(&GpuVendor::Intel));
    }

    #[test]
    fn test_parse_windows_radeon() {
        let input = "Name\nAMD Radeon RX 6600\nName\nRadeon RX 580\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.contains(&GpuVendor::Amd));
        assert_eq!(vendors.len(), 1);
    }

    #[test]
    fn test_parse_windows_multiple_gpus() {
        let input = "Name\nNVIDIA GeForce RTX 3080\nName\nIntel(R) UHD Graphics 750\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.contains(&GpuVendor::Nvidia));
        assert!(vendors.contains(&GpuVendor::Intel));
        assert_eq!(vendors.len(), 2);
    }

    #[test]
    fn test_parse_windows_empty() {
        let input = "Name\n\n";
        let vendors = parse_gpu_name_for_vendor(input);
        assert!(vendors.is_empty());
    }

    #[test]
    fn test_parse_macos_apple_silicon() {
        let input = "Chipset Model: Apple M2\nType: GPU\nBus: Built-In\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Apple));
    }

    #[test]
    fn test_parse_macos_amd() {
        let input = "Chipset Model: AMD Radeon Pro 5500M\nType: GPU\nBus: PCIe\nVendor: AMD\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Amd));
    }

    #[test]
    fn test_parse_macos_nvidia() {
        let input = "Chipset Model: NVIDIA GeForce GTX 775M\nType: GPU\nBus: PCIe\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Nvidia));
    }

    #[test]
    fn test_parse_macos_intel() {
        let input = "Chipset Model: Intel Iris Pro\nType: GPU\nBus: Built-In\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Intel));
    }

    #[test]
    fn test_parse_macos_m3() {
        let input = "Chipset Model: Apple M3 Max\nType: GPU\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Apple));
    }

    #[test]
    fn test_parse_macos_m4() {
        let input = "Chipset Model: Apple M4 Pro\nType: GPU\n";
        let vendors = parse_macos_gpu_output(input);
        assert!(vendors.contains(&GpuVendor::Apple));
    }

    // Helper function to parse GPU names from wmic-like output
    fn parse_gpu_name_for_vendor(input: &str) -> Vec<GpuVendor> {
        let mut vendors = Vec::new();
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.eq_ignore_ascii_case("Name") {
                continue;
            }
            let lower = line.to_lowercase();
            if lower.contains("nvidia") {
                if !vendors.contains(&GpuVendor::Nvidia) {
                    vendors.push(GpuVendor::Nvidia);
                }
            } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("rx ") {
                if !vendors.contains(&GpuVendor::Amd) {
                    vendors.push(GpuVendor::Amd);
                }
            } else if lower.contains("intel")
                && !vendors.contains(&GpuVendor::Intel)
            {
                vendors.push(GpuVendor::Intel);
            }
        }
        vendors
    }

    // Helper function to parse GPU names from system_profiler output
    fn parse_macos_gpu_output(input: &str) -> Vec<GpuVendor> {
        let mut vendors = Vec::new();
        for line in input.lines() {
            let trimmed = line.trim();
            if !trimmed.contains(":") {
                continue;
            }
            let gpu_name = trimmed.split(':').nth(1).unwrap_or("").trim();
            let lower = gpu_name.to_lowercase();
            if lower.contains("apple")
                && (lower.contains("m1")
                    || lower.contains("m2")
                    || lower.contains("m3")
                    || lower.contains("m4")
                    || lower.contains("apple gpu")
                    || lower.contains("apple silicon"))
            {
                if !vendors.contains(&GpuVendor::Apple) {
                    vendors.push(GpuVendor::Apple);
                }
            } else if lower.contains("nvidia") {
                if !vendors.contains(&GpuVendor::Nvidia) {
                    vendors.push(GpuVendor::Nvidia);
                }
            } else if lower.contains("amd") || lower.contains("radeon") || lower.contains("firepro")
            {
                if !vendors.contains(&GpuVendor::Amd) {
                    vendors.push(GpuVendor::Amd);
                }
            } else if lower.contains("intel")
                && !vendors.contains(&GpuVendor::Intel)
            {
                vendors.push(GpuVendor::Intel);
            }
        }
        vendors
    }
}
