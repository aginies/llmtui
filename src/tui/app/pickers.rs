use super::types::App;
use crate::backend::hardware;

impl App {
    pub fn fetch_host_picker_entries() -> Vec<(String, String)> {
        let mut entries = Vec::new();

        // Always include these two at the top
        entries.push(("127.0.0.1".to_string(), "localhost".to_string()));
        entries.push(("0.0.0.0".to_string(), "All interfaces".to_string()));

        // Add real network interfaces
        if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
            for (name, ip) in ifaces {
                let ip_str = ip.to_string();
                if ip_str != "127.0.0.1" && ip_str != "0.0.0.0" {
                    entries.push((ip_str, name));
                }
            }
        }

        entries
    }

    pub fn fetch_backend_picker_entries(&self) -> Vec<(crate::models::Backend, Option<String>)> {
        let platform = hardware::detect_platform();
        let mut entries = Vec::new();

        // 1. Add "latest" entries for backends supported on this platform
        match platform {
            crate::backend::hardware::Platform::Linux => {
                entries.push((crate::models::Backend::Cpu, None));
                entries.push((crate::models::Backend::Vulkan, None));
                if hardware::is_arm64() {
                    entries.push((crate::models::Backend::CpuArm64, None));
                }
                for vendor in hardware::detect_gpu_vendors() {
                    match vendor {
                        hardware::GpuVendor::Amd => {
                            entries.push((crate::models::Backend::Rocm, None));
                            entries.push((crate::models::Backend::RocmLemonade, None));
                        }
                        hardware::GpuVendor::Nvidia => {
                            entries.push((crate::models::Backend::Cuda, None));
                        }
                        _ => {}
                    }
                }
            }
            crate::backend::hardware::Platform::Windows => {
                entries.push((crate::models::Backend::CpuWindows, None));
                entries.push((crate::models::Backend::VulkanWindows, None));
                for vendor in hardware::detect_gpu_vendors() {
                    match vendor {
                        hardware::GpuVendor::Nvidia => {
                            entries.push((crate::models::Backend::CudaWindows12_4, None));
                            entries.push((crate::models::Backend::CudaWindows13_1, None));
                        }
                        hardware::GpuVendor::Amd => {
                            entries.push((crate::models::Backend::HipWindows, None));
                        }
                        _ => {}
                    }
                }
            }
            crate::backend::hardware::Platform::Macos => {
                if hardware::is_arm64() {
                    entries.push((crate::models::Backend::CpuMacosArm64, None));
                } else {
                    entries.push((crate::models::Backend::CpuMacosX64, None));
                }
            }
        }

        // 2. Add all installed versions (filtered by platform)
        let installed = crate::backend::hub::list_installed_backends();
        for (b, tag) in installed {
            if hardware::backend_supported(b, platform) {
                entries.push((b, Some(tag)));
            }
        }

        entries
    }
}
