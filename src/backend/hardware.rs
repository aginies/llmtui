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
pub enum GpuVendor {
    Amd,
    Nvidia,
    Intel,
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

/// Detect the GPU vendor by scanning /sys/class/drm/card*/device/vendor.
/// Returns the first detected vendor (legacy API, kept for backward compatibility).
pub fn detect_gpu_vendor() -> GpuVendor {
    for card_path in drm_card_paths() {
        let vendor_path = card_path.join("device/vendor");
        if let Ok(vendor_id) = fs::read_to_string(vendor_path) {
            let vendor_id = vendor_id.trim();
            if vendor_id == "0x1002" {
                return GpuVendor::Amd;
            } else if vendor_id == "0x10de" {
                return GpuVendor::Nvidia;
            } else if vendor_id == "0x8086" {
                return GpuVendor::Intel;
            }
        }
    }

    GpuVendor::Unknown
}

/// Detect all GPU vendors by scanning /sys/class/drm/card*/device/vendor.
/// Returns a Vec of unique vendors (preserves detection order, deduplicates).
pub fn detect_gpu_vendors() -> Vec<GpuVendor> {
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

/// Detect the GPU model name (e.g. "Radeon RX 7900 XTX")
pub fn detect_gpu_model() -> Option<String> {
    if !drm_card_paths().is_empty() {
        // Try reading device/device (PCI ID) or other sysfs attributes
        // On some systems, the model name isn't directly in sysfs without pci.ids mapping.
        // However, we can try common paths or just return vendor + GFX target if specific model is hard.
        // For now, let's try to find a "model" or "device" name if it exists.
        
        // fallback to vendor name + GFX if we can't get exact model
        let vendor = detect_gpu_vendor();
        let vendor_name = match vendor {
            GpuVendor::Amd => "AMD",
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Intel => "Intel",
            GpuVendor::Unknown => return None,
        };
        
        if vendor == GpuVendor::Amd {
            if let Some(gfx) = detect_amd_gfx_target() {
                return Some(format!("{} ({})", vendor_name, gfx));
            }
        }
        
        return Some(vendor_name.to_string());
    }

    None
}

/// Detect all GPU model names (one per GPU).
/// For AMD GPUs, includes the GFX target version.
pub fn detect_gpu_models() -> Vec<Option<String>> {
    let card_paths = drm_card_paths();
    if card_paths.is_empty() {
        return Vec::new();
    }

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
                GpuVendor::Unknown => continue,
            };

            if vendor == GpuVendor::Amd {
                if let Some(gfx) = detect_amd_gfx_target() {
                    models.push(Some(format!("{} ({})", vendor_name, gfx)));
                } else {
                    models.push(Some(vendor_name.to_string()));
                }
            } else {
                models.push(Some(vendor_name.to_string()));
            }
        }
    }

    models
}

/// Detect AMD GFX target version (e.g. "gfx1100")
pub fn detect_amd_gfx_target() -> Option<String> {
    let kfd_path = Path::new("/sys/class/kfd/kfd/topology/nodes");
    if !kfd_path.exists() {
        return None;
    }

    if let Ok(entries) = fs::read_dir(kfd_path) {
        for entry in entries.flatten() {
            let props_path = entry.path().join("properties");
            if let Ok(props) = fs::read_to_string(props_path) {
                for line in props.lines() {
                    if line.starts_with("gfx_target_version") {
                        if let Some(val_str) = line.split_whitespace().last() {
                            if let Ok(val) = val_str.parse::<u32>() {
                                // Format is usually 110000 for gfx1100, 100301 for gfx1031
                                let major = val / 10000;
                                let minor = (val % 10000) / 100;
                                let stepping = val % 100;
                                
                                if stepping > 0 {
                                    return Some(format!("gfx{}{}{}", major, minor, stepping));
                                } else {
                                    return Some(format!("gfx{}{}", major, minor));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
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
