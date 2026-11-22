use std::io::Write;
use std::time::Duration;

use serde_json::Value;
use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::{AppState, REPO, VERSION};

pub fn spawn(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut notified = false;
        loop {
            if !notified {
                notified = check(&app);
            }
            std::thread::sleep(Duration::from_secs(6 * 60 * 60));
        }
    });
}

fn check(app: &tauri::AppHandle) -> bool {
    if cfg!(debug_assertions) {
        return false;
    }
    let Ok(release) = latest_release() else {
        return false;
    };
    let Some(tag) = release.get("tag_name").and_then(Value::as_str) else {
        return false;
    };
    if !is_newer(tag, VERSION) {
        return false;
    }
    let Some(url) = installer_url(&release) else {
        return false;
    };
    let Some(path) = download(&url) else {
        return false;
    };

    let state = app.state::<AppState>();
    let message = state.translations.get("UPDATE_DIALOG.MESSAGE");
    let allow = state.translations.get("UPDATE_DIALOG.ALLOW");
    let disallow = state.translations.get("UPDATE_DIALOG.DISALLOW");
    drop(state);

    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Anime Sama")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(allow, disallow))
        .show(move |install| {
            if !install {
                return;
            }
            install_and_exit(&app, &path);
        });
    true
}

fn latest_release() -> Result<Value, ()> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let response = ureq::get(&url)
        .set("User-Agent", "Anime-Sama-Desktop")
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|_| ())?;
    response.into_json::<Value>().map_err(|_| ())
}

fn installer_url(release: &Value) -> Option<String> {
    release
        .get("assets")?
        .as_array()?
        .iter()
        .filter_map(|asset| {
            asset
                .get("name")?
                .as_str()
                .zip(asset.get("browser_download_url")?.as_str())
        })
        .find(|(name, _)| {
            name.ends_with(".exe") && !name.ends_with(".blockmap") && !name.ends_with(".sig")
        })
        .map(|(_, url)| url.to_string())
}

fn download(url: &str) -> Option<std::path::PathBuf> {
    let path = std::env::temp_dir().join("anime-sama-update.exe");
    let response = ureq::get(url)
        .set("User-Agent", "Anime-Sama-Desktop")
        .timeout(Duration::from_secs(60))
        .call()
        .ok()?;
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(&path).ok()?;
    std::io::copy(&mut reader, &mut file).ok()?;
    file.flush().ok()?;
    Some(path)
}

fn is_newer(candidate: &str, current: &str) -> bool {
    let candidate = numbers(candidate);
    let current = numbers(current);
    for index in 0..candidate.len().max(current.len()) {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split(['.', '-', '+'])
        .filter_map(|part| part.parse::<u64>().ok())
        .collect()
}

#[cfg(windows)]
fn install_and_exit(app: &tauri::AppHandle, path: &std::path::Path) {
    let _ = std::process::Command::new(path).arg("/S").spawn();
    app.exit(0);
}

#[cfg(not(windows))]
fn install_and_exit(app: &tauri::AppHandle, _path: &std::path::Path) {
    app.exit(0);
}
