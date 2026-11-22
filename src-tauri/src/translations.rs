use serde_json::Value;
use std::collections::HashMap;

const EN: &str = include_str!("../../translations/en.json");
const FR: &str = include_str!("../../translations/fr.json");

pub struct Translations {
    map: HashMap<String, Value>,
    current: String,
}

impl Translations {
    pub fn load(language: &str) -> Self {
        let mut map = HashMap::new();
        if let Ok(value) = serde_json::from_str::<Value>(EN) {
            map.insert("en".to_string(), value);
        }
        if let Ok(value) = serde_json::from_str::<Value>(FR) {
            map.insert("fr".to_string(), value);
        }
        let current = if map.contains_key(language) {
            language.to_string()
        } else {
            "en".to_string()
        };
        Self { map, current }
    }

    pub fn get(&self, key: &str) -> String {
        self.get_in(&self.current, key)
    }

    pub fn get_in(&self, language: &str, key: &str) -> String {
        let Some(root) = self.map.get(language) else {
            return key.to_string();
        };
        let mut value = root;
        for part in key.split('.') {
            match value.get(part) {
                Some(next) => value = next,
                None => return key.to_string(),
            }
        }
        value
            .as_str()
            .map(|text| text.to_string())
            .unwrap_or_else(|| key.to_string())
    }

    pub fn language_name(&self, language: &str) -> String {
        self.get_in(language, "LANGUAGE")
    }

    pub fn detect_locale() -> String {
        match sys_locale::get_locale() {
            Some(locale) if locale.to_ascii_lowercase().starts_with("fr") => "fr".to_string(),
            _ => "en".to_string(),
        }
    }

    pub fn languages(&self) -> Vec<&'static str> {
        vec!["en", "fr"]
    }

    pub fn has(&self, language: &str) -> bool {
        self.map.contains_key(language)
    }
}
