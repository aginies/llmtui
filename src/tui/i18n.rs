use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

pub static TRANSLATIONS: LazyLock<HashMap<String, HashMap<&'static str, &'static str>>> =
    LazyLock::new(|| {
        let mut translations = HashMap::new();

        // 1. Load embedded translations at compile-time as default fallback
        let embedded_locales: &[(&str, &str)] = &[
            ("en", include_str!("../../locales/en.json")),
            ("fr", include_str!("../../locales/fr.json")),
            ("it", include_str!("../../locales/it.json")),
            ("de", include_str!("../../locales/de.json")),
        ];

        for (lang, json_content) in embedded_locales {
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(json_content) {
                let mut static_map = HashMap::new();
                for (k, v) in parsed {
                    let k_str: &'static str = String::leak(k);
                    let v_str: &'static str = String::leak(v);
                    static_map.insert(k_str, v_str);
                }
                translations.insert(lang.to_string(), static_map);
            }
        }

        // 2. Load filesystem translations to override or add custom languages
        let locale_dir = locale_dir();
        if let Ok(entries) = fs::read_dir(&locale_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                    && let Some(lang) = filename.strip_suffix(".json")
                    && let Ok(contents) = fs::read_to_string(&path)
                    && let Ok(parsed) = serde_json::from_str::<HashMap<String, String>>(&contents)
                {
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

        translations
    });

static CURRENT_LANG: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Interned fallback for missing translation keys: each unique key is leaked
/// exactly once, so repeated lookups of a missing key do not grow memory.
static MISSING_KEYS: LazyLock<std::sync::Mutex<HashMap<String, &'static str>>> =
    LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

fn locale_dir() -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let candidate = dir.join("locales");
        if candidate.is_dir() {
            return candidate;
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

#[allow(dead_code)]
pub fn reset_language() {
    let mut current = CURRENT_LANG.lock().unwrap();
    *current = None;
}

#[allow(dead_code)]
pub fn get_language() -> String {
    let current = CURRENT_LANG.lock().unwrap();
    current.clone().unwrap_or_else(|| "en".to_string())
}

pub fn t(key: &str) -> &'static str {
    let lang = get_language();

    if let Some(lang_map) = TRANSLATIONS.get(&lang)
        && let Some(&value) = lang_map.get(key)
    {
        return value;
    }

    if let Some(en_map) = TRANSLATIONS.get("en")
        && let Some(&value) = en_map.get(key)
    {
        return value;
    }

    // Missing key: intern it so the fallback string is leaked once per unique
    // key instead of once per call (render paths call t!() constantly).
    let mut missing = MISSING_KEYS.lock().unwrap();
    if let Some(&value) = missing.get(key) {
        return value;
    }
    let leaked: &'static str = Box::leak(key.to_string().into_boxed_str());
    missing.insert(key.to_string(), leaked);
    leaked
}

#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::tui::i18n::t($key)
    };
}

pub fn field_help(field_id: &str) -> String {
    let key = format!("field.help.{}", field_id);
    t(&key).to_string()
}

pub fn t_fmt(key: &str, args: &[String]) -> String {
    let template = t(key);
    // Single pass: replace each "{}" left-to-right with the next argument.
    // Already-substituted argument text is never re-scanned, so arguments
    // containing "{}" cannot be consumed by later placeholders.
    let mut result =
        String::with_capacity(template.len() + args.iter().map(|a| a.len()).sum::<usize>());
    let mut rest = template;
    for arg in args {
        match rest.find("{}") {
            Some(pos) => {
                result.push_str(&rest[..pos]);
                result.push_str(arg);
                rest = &rest[pos + 2..];
            }
            None => break,
        }
    }
    result.push_str(rest);
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
        $crate::tui::i18n::t_fmt(
            $key,
            &[$arg1.to_string(), $arg2.to_string(), $arg3.to_string()],
        )
    };
    ($key:expr, $arg1:expr, $arg2:expr, $arg3:expr, $arg4:expr $(,)?) => {
        $crate::tui::i18n::t_fmt(
            $key,
            &[
                $arg1.to_string(),
                $arg2.to_string(),
                $arg3.to_string(),
                $arg4.to_string(),
            ],
        )
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
    fn test_missing_key_is_interned() {
        // Repeated lookups of a missing key must return the same &'static str
        // (leaked once), not a fresh allocation each call.
        let a = t("nonexistent.intern.key");
        let b = t("nonexistent.intern.key");
        assert_eq!(a, b);
        assert!(
            std::ptr::eq(a.as_ptr(), b.as_ptr()),
            "missing key fallback should be interned"
        );
    }

    #[test]
    fn test_t_fmt_basic() {
        // Missing key falls back to the key itself as the template.
        let result = t_fmt("hello {} and {}", &["world".to_string(), "x".to_string()]);
        assert_eq!(result, "hello world and x");
    }

    #[test]
    fn test_t_fmt_arg_containing_placeholder() {
        // An argument containing "{}" must not be consumed by the next
        // placeholder substitution.
        let result = t_fmt("a {} b {}", &["x{}y".to_string(), "z".to_string()]);
        assert_eq!(result, "a x{}y b z");
    }

    #[test]
    fn test_t_fmt_extra_args_ignored() {
        let result = t_fmt(
            "only {}",
            &["one".to_string(), "two".to_string(), "three".to_string()],
        );
        assert_eq!(result, "only one");
    }

    #[test]
    fn test_set_language() {
        set_language("fr");
        assert_eq!(get_language(), "fr");
        set_language("en");
        assert_eq!(get_language(), "en");
    }

    #[test]
    fn test_embedded_translations_exist() {
        // Ensure that English translations are correctly embedded and retrieved
        let en_map = TRANSLATIONS
            .get("en")
            .expect("English translations not found");
        assert_eq!(
            *en_map.get("panel.title.models_active").unwrap(),
            " MODELS (F1) "
        );

        // Ensure French translations are loaded and distinct
        let fr_map = TRANSLATIONS
            .get("fr")
            .expect("French translations not found");
        let fr_val = *fr_map.get("panel.title.models_active").unwrap();
        assert_ne!(fr_val, " MODELS (F1) ");

        // Ensure Italian translations are loaded and distinct
        let it_map = TRANSLATIONS
            .get("it")
            .expect("Italian translations not found");
        let it_val = *it_map.get("panel.title.models_active").unwrap();
        assert_ne!(it_val, " MODELS (F1) ");
    }

    #[test]
    fn test_confirm_yes_uses_y_in_all_locales() {
        // The confirmation handler only accepts 'y'/'n' — every locale label
        // must advertise [y], not a localized letter like [o] or [j].
        for lang in ["en", "fr", "it", "de"] {
            let map = TRANSLATIONS
                .get(lang)
                .unwrap_or_else(|| panic!("{} translations not found", lang));
            let yes = map
                .get("dialog.confirm_yes")
                .unwrap_or_else(|| panic!("{} missing dialog.confirm_yes", lang));
            assert!(
                yes.contains("[y]"),
                "{} dialog.confirm_yes should use [y], got: {}",
                lang,
                yes
            );
        }
    }
}
