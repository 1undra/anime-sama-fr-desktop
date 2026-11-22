use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use crate::settings::{legacy_value, write_json};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub history: BTreeMap<String, Value>,
    pub watchlist: BTreeMap<String, Value>,
    pub favorite: BTreeMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileFile {
    #[serde(rename = "Anime Sama")]
    pub inner: Profile,
}

impl Profile {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("profile.json");
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(file) = serde_json::from_str::<ProfileFile>(&text) {
                return file.inner;
            }
        }
        if let Some(value) = legacy_value("profile.json") {
            if let Ok(profile) = serde_json::from_value::<Profile>(value) {
                return profile;
            }
        }
        Profile::default()
    }

    pub fn save(&self, path: &Path) {
        write_json(
            path,
            &ProfileFile {
                inner: self.clone(),
            },
        );
    }

    pub fn upsert_history(&mut self, key: String, patch: Value) {
        merge(&mut self.history, key, patch);
    }

    pub fn remove_history(&mut self, key: &str) {
        self.history.remove(key);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn toggle_watchlist(&mut self, key: String, patch: Value) {
        toggle(&mut self.watchlist, key, patch);
    }

    pub fn clear_watchlist(&mut self) {
        self.watchlist.clear();
    }

    pub fn remove_watchlist(&mut self, key: &str) {
        self.watchlist.remove(key);
    }

    pub fn toggle_favorite(&mut self, key: String, patch: Value) {
        toggle(&mut self.favorite, key, patch);
    }

    pub fn clear_favorite(&mut self) {
        self.favorite.clear();
    }

    pub fn remove_favorite(&mut self, key: &str) {
        self.favorite.remove(key);
    }

    pub fn merge_history(&mut self, entries: BTreeMap<String, Value>) {
        for (key, value) in entries {
            merge(&mut self.history, key, value);
        }
    }

    pub fn merge_watchlist(&mut self, entries: BTreeMap<String, Value>) {
        for (key, value) in entries {
            merge(&mut self.watchlist, key, value);
        }
    }

    pub fn merge_favorite(&mut self, entries: BTreeMap<String, Value>) {
        for (key, value) in entries {
            merge(&mut self.favorite, key, value);
        }
    }
}

fn merge(map: &mut BTreeMap<String, Value>, key: String, patch: Value) {
    match map.get_mut(&key) {
        Some(existing) => {
            if let (Some(target), Some(source)) = (existing.as_object_mut(), patch.as_object()) {
                for (name, value) in source {
                    target.insert(name.clone(), value.clone());
                }
            }
        }
        None => {
            map.insert(key, patch);
        }
    }
}

fn toggle(map: &mut BTreeMap<String, Value>, key: String, patch: Value) {
    if map.contains_key(&key) {
        map.remove(&key);
    } else {
        map.insert(key, patch);
    }
}
