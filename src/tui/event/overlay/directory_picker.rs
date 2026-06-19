use crate::config::config_base_dir;

pub fn default_chat_templates_dir() -> String {
    config_base_dir()
        .join("llm-manager")
        .join("chat_templates")
        .to_string_lossy()
        .to_string()
}

pub fn load_jinja_files_recursive(dir_path: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    collect_jinja_files(dir_path, dir_path, &mut files);
    files.sort_by_key(|a| a.0.to_lowercase());
    files
}

fn collect_jinja_files(base_path: &str, dir_path: &str, files: &mut Vec<(String, String)>) {
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().map(|e| e == "jinja").unwrap_or(false) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let display = if dir_path == base_path {
                        name.to_string()
                    } else {
                        // Show relative path from base
                        if let Ok(stripped) = path.strip_prefix(base_path) {
                            stripped
                                .to_string_lossy()
                                .to_string()
                                .replace(std::path::MAIN_SEPARATOR, "/")
                        } else {
                            name.to_string()
                        }
                    };
                    files.push((display, path.to_string_lossy().to_string()));
                }
            } else if path.is_dir() {
                collect_jinja_files(base_path, &path.to_string_lossy(), files);
            }
        }
    }
}
