use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::APP_KEY;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(rename = "LANGUAGE")]
    pub language: Option<String>,
    #[serde(rename = "START_UP")]
    pub start_up: bool,
    #[serde(rename = "START_HIDDEN")]
    pub start_hidden: bool,
    #[serde(rename = "START_FULL_SCREEN")]
    pub start_full_screen: bool,
    #[serde(rename = "MINIMIZE_TO_TRAY")]
    pub minimize_to_tray: bool,
    #[serde(rename = "SAVE_WINDOW_SIZE")]
    pub save_window_size: bool,
    #[serde(rename = "ALWAYS_ON_TOP")]
    pub always_on_top: bool,
    #[serde(rename = "ADBLOCK")]
    pub adblock: bool,
    #[serde(rename = "DISCORD_RICH_PRESENCE")]
    pub discord_rich_presence: bool,
    #[serde(rename = "WIDTH")]
    pub width: Option<u32>,
    #[serde(rename = "HEIGHT")]
    pub height: Option<u32>,
    #[serde(rename = "MIGRATED")]
    pub migrated: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            start_up: false,
            start_hidden: false,
            start_full_screen: false,
            minimize_to_tray: false,
            save_window_size: false,
            always_on_top: false,
            adblock: true,
            discord_rich_presence: true,
            width: None,
            height: None,
            migrated: false,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    #[serde(rename = "Anime Sama")]
    inner: Settings,
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        if let Some(settings) = read_file(&path) {
            return settings;
        }
        if let Some(settings) = read_legacy() {
            return settings;
        }
        Settings::default()
    }

    pub fn save(&self, path: &Path) {
        let file = SettingsFile {
            inner: self.clone(),
        };
        write_json(path, &file);
    }
}

fn read_file(path: &Path) -> Option<Settings> {
    let text = fs::read_to_string(path).ok()?;
    let file = serde_json::from_str::<SettingsFile>(&text).ok()?;
    Some(file.inner)
}

fn read_legacy() -> Option<Settings> {
    let dir = legacy_dir()?;
    let text = fs::read_to_string(dir.join("settings.json")).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    for key in ["Anime Sama", "ANIME_SAMA_FR", "ANIME_SAMA"] {
        if let Some(inner) = value.get(key) {
            if let Ok(settings) = serde_json::from_value::<Settings>(inner.clone()) {
                return Some(settings);
            }
        }
    }
    None
}

#[cfg(windows)]
pub fn legacy_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|path| PathBuf::from(path).join("anime-sama-fr-desktop"))
}

#[cfg(not(windows))]
pub fn legacy_dir() -> Option<PathBuf> {
    None
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) {
    let Ok(text) = serde_json::to_string_pretty(value) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, text);
}

pub fn legacy_value(file: &str) -> Option<serde_json::Value> {
    let dir = legacy_dir()?;
    let text = fs::read_to_string(dir.join(file)).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    for key in [APP_KEY, "ANIME_SAMA_FR", "ANIME_SAMA"] {
        if let Some(inner) = value.get(key) {
            return Some(inner.clone());
        }
    }
    None
}
