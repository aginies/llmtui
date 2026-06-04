use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

pub static TRANSLATIONS: LazyLock<HashMap<String, HashMap<&'static str, &'static str>>> = LazyLock::new(|| {
    let mut translations = HashMap::new();
    let locale_dir = locale_dir();
    
    if let Ok(entries) = fs::read_dir(&locale_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(lang) = filename.strip_suffix(".json") {
                    if let Ok(contents) = fs::read_to_string(&path) {
                        if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&contents) {
                            let mut static_map = HashMap::new();
                            for (k, v) in parsed {
                                let k_str: &'static str = String::leak(k);
                                let v_str: &'static str = String::leak(v);
                                static_map.insert(k_str, v_str);
                            }
                            translations.insert(lang.to_string(), static_map);
                        }
                    }
                }
            }
        }
    }
    
    translations
});

static CURRENT_LANG: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

fn locale_dir() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("locales");
            if candidate.is_dir() {
                return candidate;
            }
        }
    }
    
    if let Ok(p) = std::env::var("LLM_MANAGER_LOCALES") {
        let path = std::path::Path::new(&p);
        if path.is_dir() {
            return path.to_path_buf();
        }
    }
    
    std::path::Path::new("locales").to_path_buf()
}

pub fn set_language(lang: &str) {
    let mut current = CURRENT_LANG.lock().unwrap();
    *current = Some(lang.to_string());
}

pub fn get_language() -> String {
    let current = CURRENT_LANG.lock().unwrap();
    current.clone().unwrap_or_else(|| "en".to_string())
}

pub fn t(key: &str) -> &'static str {
    let lang = get_language();
    
    if let Some(lang_map) = TRANSLATIONS.get(&lang) {
        if let Some(&value) = lang_map.get(key) {
            return value;
        }
    }
    
    if let Some(en_map) = TRANSLATIONS.get("en") {
        if let Some(&value) = en_map.get(key) {
            return value;
        }
    }
    
    Box::leak(key.to_string().into_boxed_str())
}

pub fn field_help(field_id: &str) -> String {
    t(&format!("field.help.{}", field_id)).to_string()
}

#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::tui::i18n::t($key)
    };
}

pub fn t_fmt(key: &str, args: &[String]) -> String {
    let template = t(key);
    let mut result = template.to_string();
    for arg in args {
        if let Some(pos) = result.find("{}") {
            result.replace_range(pos..pos + 2, arg);
        }
    }
    result
}

#[macro_export]
macro_rules! t_fmt {
    ($key:expr $(,)?) => {
        $crate::tui::i18n::t($key).to_string()
    };
    ($key:expr, $arg1:expr $(,)?) => {
        $crate::tui::i18n::t_fmt($key, &[$arg1.to_string()])
    };
    ($key:expr, $arg1:expr, $arg2:expr $(,)?) => {
        $crate::tui::i18n::t_fmt($key, &[$arg1.to_string(), $arg2.to_string()])
    };
    ($key:expr, $arg1:expr, $arg2:expr, $arg3:expr $(,)?) => {
        $crate::tui::i18n::t_fmt($key, &[$arg1.to_string(), $arg2.to_string(), $arg3.to_string()])
    };
    ($key:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr $(,)?) => {
        $crate::tui::i18n::t_fmt($key, &[$arg1.to_string(), $arg2.to_string(), $arg3.to_string(), $arg4.to_string()])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t_falls_back_to_key() {
        let result = t("nonexistent.key.xyz");
        assert_eq!(result, "nonexistent.key.xyz");
    }

    #[test]
    fn test_set_language() {
        set_language("fr");
        assert_eq!(get_language(), "fr");
        set_language("en");
        assert_eq!(get_language(), "en");
    }
}
