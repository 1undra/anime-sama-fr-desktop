mod adblock;
#[cfg(windows)]
mod audio;
mod discord;
#[cfg(windows)]
mod identity;
mod menu;
mod navigation;
mod profile;
mod settings;
mod translations;
mod updater;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use url::Url;

use discord::{Activity, Discord};
use navigation::WatchData;
use profile::Profile;
use settings::Settings;
use translations::Translations;

pub const APP_KEY: &str = "Anime Sama";
pub const BASE_URL: &str = "https://anime-sama.to";
pub const REPO: &str = "traxcs/anime-sama-fr-desktop";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DISCORD_CLIENT_ID: &str = "1560680963674214461";

const PRELOAD: &str = include_str!("preload.js");

pub struct Paths {
    pub settings: PathBuf,
    pub profile: PathBuf,
    pub cache: PathBuf,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub profile: Mutex<Profile>,
    pub translations: Translations,
    pub discord: Mutex<Discord>,
    pub paths: Paths,
    pub watch: Mutex<Option<WatchData>>,
    pub error_code: Mutex<u16>,
    pub last_size_save: Mutex<Option<Instant>>,
    pub quitting: AtomicBool,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings
            .lock()
            .map(|settings| settings.clone())
            .unwrap_or_default()
    }

    pub fn discord_set(&self, activity: Activity) {
        if let Ok(discord) = self.discord.lock() {
            discord.set(activity);
        }
    }

    pub fn discord_disconnect(&self) {
        if let Ok(discord) = self.discord.lock() {
            discord.disconnect();
        }
    }
}

fn save_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    settings.save(&state.paths.settings);
}

fn save_profile(app: &AppHandle) {
    let state = app.state::<AppState>();
    let profile = state
        .profile
        .lock()
        .map(|profile| profile.clone())
        .unwrap_or_default();
    profile.save(&state.paths.profile);
}

fn string_field(data: &Value, name: &str) -> String {
    data.get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn number_field(data: &Value, name: &str) -> Option<i64> {
    data.get(name).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_f64().map(|number| number as i64))
    })
}

fn deep_link_to_url(deep: &Url) -> Url {
    let host = deep.host_str().unwrap_or("");
    let mut target = String::from(BASE_URL);
    if !host.is_empty() {
        target.push('/');
        target.push_str(host);
    }
    target.push_str(deep.path());
    if let Some(query) = deep.query() {
        target.push('?');
        target.push_str(query);
    }
    Url::parse(&target).unwrap_or_else(|_| Url::parse(BASE_URL).expect("base url"))
}

fn initial_url(args: &[String]) -> Url {
    for arg in args {
        if let Ok(url) = Url::parse(arg) {
            if url.scheme() == "anime-sama-to" {
                return deep_link_to_url(&url);
            }
        }
    }
    Url::parse(BASE_URL).expect("base url")
}

fn navigate_deep_link(app: &AppHandle, deep: &Url) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let url = deep_link_to_url(deep);
    let _ = window.navigate(url);
    let _ = window.show();
    let _ = window.set_focus();
}

fn apply_adblock(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings().adblock;
    adblock::set_enabled(enabled);
    if enabled {
        if let Some(window) = app.get_webview_window("main") {
            adblock::apply(&window);
            adblock::inject_frame_styles(&window);
        }
    }
    let _ = app.emit("adblock-changed", enabled);
}

#[cfg(windows)]
fn disable_webview_ui(window: &WebviewWindow) {
    let _ = window.with_webview(|platform| unsafe {
        let controller = platform.controller();
        if let Ok(webview) = controller.CoreWebView2() {
            if let Ok(settings) = webview.Settings() {
                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                let _ = settings.SetAreDevToolsEnabled(false);
            }
        }
    });
}

#[cfg(not(windows))]
fn disable_webview_ui(_window: &WebviewWindow) {}

fn sync_autostart(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings().start_up;
    if enabled {
        let _ = app.autolaunch().enable();
    } else {
        let _ = app.autolaunch().disable();
    }
}

fn set_bool(settings: &mut Settings, name: &str, value: bool) -> bool {
    match name {
        "START_UP" => settings.start_up = value,
        "START_HIDDEN" => settings.start_hidden = value,
        "START_FULL_SCREEN" => settings.start_full_screen = value,
        "MINIMIZE_TO_TRAY" => settings.minimize_to_tray = value,
        "SAVE_WINDOW_SIZE" => settings.save_window_size = value,
        "ALWAYS_ON_TOP" => settings.always_on_top = value,
        "ADBLOCK" => settings.adblock = value,
        "DISCORD_RICH_PRESENCE" => settings.discord_rich_presence = value,
        _ => return false,
    }
    true
}

fn apply_settings(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_always_on_top(settings.always_on_top);
    }
    sync_autostart(app);
    apply_adblock(app);
    if settings.discord_rich_presence {
        navigation::refresh_activity(app);
    } else {
        state.discord_disconnect();
    }
}

fn after_setting_change(app: &AppHandle, key: &str) {
    let state = app.state::<AppState>();
    let settings = state.settings();
    match key {
        "START_UP" => sync_autostart(app),
        "IMPORT" => apply_settings(app),
        "ALWAYS_ON_TOP" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(settings.always_on_top);
            }
        }
        "SAVE_WINDOW_SIZE" => {
            if let Some(window) = app.get_webview_window("main") {
                if settings.save_window_size {
                    save_window_size(app, &window, true);
                } else {
                    if let Ok(mut settings) = state.settings.lock() {
                        settings.width = None;
                        settings.height = None;
                    }
                }
            }
        }
        "ADBLOCK" => apply_adblock(app),
        "DISCORD_RICH_PRESENCE" => {
            if settings.discord_rich_presence {
                navigation::refresh_activity(app);
            } else {
                state.discord_disconnect();
            }
        }
        _ => {}
    }
    save_settings(app);
    menu::refresh(app);
}

fn save_window_size(app: &AppHandle, window: &WebviewWindow, force: bool) {
    let state = app.state::<AppState>();
    if !state.settings().save_window_size {
        return;
    }
    let Ok(size) = window.inner_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = size.to_logical::<u32>(scale);
    {
        let settings = state.settings();
        if settings.width == Some(size.width) && settings.height == Some(size.height) {
            return;
        }
    }
    if !force {
        if let Ok(mut last) = state.last_size_save.lock() {
            if last
                .as_ref()
                .map(|instant| instant.elapsed() < Duration::from_millis(800))
                .unwrap_or(false)
            {
                return;
            }
            *last = Some(Instant::now());
        }
    }
    if let Ok(mut settings) = state.settings.lock() {
        settings.width = Some(size.width);
        settings.height = Some(size.height);
    }
    save_settings(app);
}

fn toggle_setting(app: &AppHandle, key: &str) {
    let state = app.state::<AppState>();
    if let Ok(mut settings) = state.settings.lock() {
        match key {
            "START_UP" => settings.start_up = !settings.start_up,
            "START_HIDDEN" => {
                settings.start_hidden = !settings.start_hidden;
                if settings.start_hidden {
                    settings.start_full_screen = false;
                }
            }
            "START_FULL_SCREEN" => {
                settings.start_full_screen = !settings.start_full_screen;
                if settings.start_full_screen {
                    settings.start_hidden = false;
                }
            }
            "MINIMIZE_TO_TRAY" => settings.minimize_to_tray = !settings.minimize_to_tray,
            "SAVE_WINDOW_SIZE" => settings.save_window_size = !settings.save_window_size,
            "ALWAYS_ON_TOP" => settings.always_on_top = !settings.always_on_top,
            "DISCORD_RICH_PRESENCE" => {
                settings.discord_rich_presence = !settings.discord_rich_presence
            }
            _ => return,
        }
    }
    after_setting_change(app, key);
}

fn confirm_adblock(app: &AppHandle) {
    let state = app.state::<AppState>();
    let translations = &state.translations;
    let message = translations.get("CONFIRM_ADBLOCK_DIALOG.MESSAGE");
    let allow = translations.get("CONFIRM_ADBLOCK_DIALOG.ALLOW");
    let disallow = translations.get("CONFIRM_ADBLOCK_DIALOG.DISALLOW");
    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Anime Sama")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(allow, disallow))
        .show(move |allowed| {
            let state = app.state::<AppState>();
            if let Ok(mut settings) = state.settings.lock() {
                settings.adblock = allowed;
            }
            after_setting_change(&app, "ADBLOCK");
        });
}

fn confirm_restart(app: &AppHandle, message_key: &str) {
    let state = app.state::<AppState>();
    let translations = &state.translations;
    let message = translations.get(&format!("{message_key}.MESSAGE"));
    let allow = translations.get(&format!("{message_key}.ALLOW"));
    let disallow = translations.get(&format!("{message_key}.DISALLOW"));
    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Anime Sama")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(allow, disallow))
        .show(move |allowed| {
            if allowed {
                app.restart();
            }
        });
}

fn set_language(app: &AppHandle, language: &str) {
    let state = app.state::<AppState>();
    if !state.translations.has(language) {
        return;
    }
    let translations = &state.translations;
    let message = translations.get("CONFIRM_LANGUAGE_DIALOG.MESSAGE");
    let allow = translations.get("CONFIRM_LANGUAGE_DIALOG.ALLOW");
    let disallow = translations.get("CONFIRM_LANGUAGE_DIALOG.DISALLOW");
    let language = language.to_string();
    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Anime Sama")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(allow, disallow))
        .show(move |allowed| {
            if !allowed {
                menu::refresh(&app);
                return;
            }
            {
                let state = app.state::<AppState>();
                if let Ok(mut settings) = state.settings.lock() {
                    settings.language = Some(language.clone());
                };
            }
            save_settings(&app);
            app.restart();
        });
}

fn import_profile(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |path| {
            let Some(path) = path.and_then(|path| path.into_path().ok()) else {
                return;
            };
            let Ok(text) = std::fs::read_to_string(path) else {
                return;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            let Some(inner) = value.get(APP_KEY) else {
                return;
            };

            let history = inner
                .get("history")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let watchlist = inner
                .get("watchlist")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let favorite = inner
                .get("favorite")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            {
                let state = app.state::<AppState>();
                if let Ok(mut profile) = state.profile.lock() {
                    profile.merge_history(
                        history
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                    );
                    profile.merge_watchlist(
                        watchlist
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                    );
                    profile.merge_favorite(
                        favorite
                            .iter()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                    );
                };
            }
            save_profile(&app);

            if let Some(window) = app.get_webview_window("main") {
                for (key, value) in &history {
                    let _ = window.emit(
                        "import-history",
                        json!({
                            "url": format!("{BASE_URL}{key}"),
                            "type": value.get("type").cloned().unwrap_or(Value::Null),
                            "title": value.get("title").cloned().unwrap_or(Value::Null),
                            "image": value.get("image").cloned().unwrap_or(Value::Null),
                            "language": value.get("language").cloned().unwrap_or(Value::Null),
                            "episode": value.get("episode").cloned().unwrap_or(Value::Null),
                            "saved": value.get("saved").cloned().unwrap_or(Value::Null),
                        }),
                    );
                }
                for (key, value) in &watchlist {
                    let _ = window.emit(
                        "import-watchlist",
                        json!({
                            "url": format!("{BASE_URL}{key}"),
                            "title": value.get("title").cloned().unwrap_or(Value::Null),
                            "image": value.get("image").cloned().unwrap_or(Value::Null),
                        }),
                    );
                }
                for (key, value) in &favorite {
                    let _ = window.emit(
                        "import-favorite",
                        json!({
                            "url": format!("{BASE_URL}{key}"),
                            "title": value.get("title").cloned().unwrap_or(Value::Null),
                            "image": value.get("image").cloned().unwrap_or(Value::Null),
                        }),
                    );
                }
                let _ = window.eval("location.reload()");
            }
        });
}

fn import_settings(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |path| {
            let Some(path) = path.and_then(|path| path.into_path().ok()) else {
                return;
            };
            let Ok(text) = std::fs::read_to_string(path) else {
                return;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                return;
            };
            let Some(inner) = value.get(APP_KEY).and_then(Value::as_object) else {
                return;
            };

            let mut language = None;
            let mut width = None;
            let mut height = None;

            {
                let state = app.state::<AppState>();
                let Ok(mut settings) = state.settings.lock() else {
                    return;
                };
                for (name, value) in inner {
                    let name = name.to_ascii_uppercase();
                    match name.as_str() {
                        "LANGUAGE" => {
                            if let Some(value) = value.as_str() {
                                if settings.language.as_deref() != Some(value) {
                                    language = Some(value.to_string());
                                }
                            }
                        }
                        "WIDTH" => {
                            if let Some(value) = value.as_u64() {
                                width = Some(value as u32);
                                settings.width = Some(value as u32);
                            }
                        }
                        "HEIGHT" => {
                            if let Some(value) = value.as_u64() {
                                height = Some(value as u32);
                                settings.height = Some(value as u32);
                            }
                        }
                        _ => {
                            if let Some(value) = value.as_bool() {
                                set_bool(&mut settings, &name, value);
                            }
                        }
                    }
                }
            }
            save_settings(&app);

            if let Some(window) = app.get_webview_window("main") {
                let state = app.state::<AppState>();
                let settings = state.settings();
                if let (Some(width), Some(height)) = (width, height) {
                    let _ = window.set_size(tauri::LogicalSize::new(width as f64, height as f64));
                    let _ = window.center();
                } else if width.is_some() || height.is_some() {
                    let _ = window.set_size(tauri::LogicalSize::new(
                        settings.width.unwrap_or(width.unwrap_or(1280)) as f64,
                        settings.height.unwrap_or(height.unwrap_or(720)) as f64,
                    ));
                    let _ = window.center();
                }
            }

            after_setting_change(&app, "IMPORT");

            if let Some(language) = language {
                set_language(&app, &language);
            }
        });
}

fn export_profile(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .set_file_name("profile.json")
        .save_file(move |path| {
            let Some(path) = path.and_then(|path| path.into_path().ok()) else {
                return;
            };
            let state = app.state::<AppState>();
            let profile = state
                .profile
                .lock()
                .map(|profile| profile.clone())
                .unwrap_or_default();
            if let Ok(text) = serde_json::to_string(&json!({ APP_KEY: profile })) {
                let _ = std::fs::write(path, text);
            }
        });
}

fn export_settings(app: &AppHandle) {
    let app = app.clone();
    app.dialog()
        .file()
        .set_file_name("settings.json")
        .save_file(move |path| {
            let Some(path) = path.and_then(|path| path.into_path().ok()) else {
                return;
            };
            let state = app.state::<AppState>();
            let settings = state.settings();
            if let Ok(text) = serde_json::to_string(&json!({ APP_KEY: settings })) {
                let _ = std::fs::write(path, text);
            }
        });
}

fn reset_data(app: &AppHandle) {
    let state = app.state::<AppState>();
    let translations = &state.translations;
    let message = translations.get("CHANGE_RESTART_DIALOG.MESSAGE");
    let allow = translations.get("CHANGE_RESTART_DIALOG.ALLOW");
    let disallow = translations.get("CHANGE_RESTART_DIALOG.DISALLOW");
    let app = app.clone();
    app.dialog()
        .message(message)
        .title("Anime Sama")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(allow, disallow))
        .show(move |allowed| {
            if !allowed {
                return;
            }
            {
                let state = app.state::<AppState>();
                if let Ok(mut settings) = state.settings.lock() {
                    *settings = Settings::default();
                }
                if let Ok(mut profile) = state.profile.lock() {
                    *profile = Profile::default();
                }
                save_settings(&app);
                save_profile(&app);
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.clear_all_browsing_data();
                }
            }
            app.restart();
        });
}

fn home(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    match Url::parse(BASE_URL) {
        Ok(url) => {
            if window.navigate(url).is_err() {
                navigation::show_error(app, 500);
            }
        }
        Err(_) => navigation::show_error(app, 500),
    }
}

fn close_window(app: &AppHandle) {
    app.state::<AppState>()
        .quitting
        .store(true, Ordering::SeqCst);
    app.exit(0);
}

pub fn on_menu_event(app: &AppHandle, id: &str) {
    match id {
        "import-profile" => import_profile(app),
        "import-settings" => import_settings(app),
        "export-profile" => export_profile(app),
        "export-settings" => export_settings(app),
        "back" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("history.back()");
            }
        }
        "forward" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("history.forward()");
            }
        }
        "restore" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        "reduce" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        "refresh" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("location.reload()");
            }
        }
        "force-refresh" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.eval("location.reload(true)");
            }
        }
        "center" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.center();
            }
        }
        "reload" => home(app),
        "restart" => confirm_restart(app, "RESTART_DIALOG"),
        "reset-data" => reset_data(app),
        "force-close" => close_window(app),
        "close" => close_window(app),
        "start-up" => toggle_setting(app, "START_UP"),
        "start-hidden" => toggle_setting(app, "START_HIDDEN"),
        "start-full-screen" => toggle_setting(app, "START_FULL_SCREEN"),
        "minimize-to-tray" => toggle_setting(app, "MINIMIZE_TO_TRAY"),
        "save-window-size" => toggle_setting(app, "SAVE_WINDOW_SIZE"),
        "always-on-top" => toggle_setting(app, "ALWAYS_ON_TOP"),
        "adblock" => confirm_adblock(app),
        "discord-rich-presence" => toggle_setting(app, "DISCORD_RICH_PRESENCE"),
        value if value.starts_with("lang:") => set_language(app, &value["lang:".len()..]),
        _ => {}
    }
}

#[tauri::command]
fn window_minimize(window: WebviewWindow) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_maximize(window: WebviewWindow) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn window_close(window: WebviewWindow) {
    let _ = window.close();
}

#[tauri::command]
fn window_home(app: AppHandle) {
    home(&app);
}

#[tauri::command]
fn window_state(window: WebviewWindow) -> bool {
    window.is_maximized().unwrap_or(false)
}

#[tauri::command]
fn delete_all_history(app: AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut profile) = state.profile.lock() {
        profile.clear_history();
    }
    save_profile(&app);
}

#[tauri::command]
fn delete_all_watchlist(app: AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut profile) = state.profile.lock() {
        profile.clear_watchlist();
    }
    save_profile(&app);
}

#[tauri::command]
fn delete_all_favorite(app: AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(mut profile) = state.profile.lock() {
        profile.clear_favorite();
    }
    save_profile(&app);
}

fn url_key(url: &str) -> Option<String> {
    Url::parse(url).ok().map(|url| url.path().to_string())
}

#[tauri::command]
fn delete_history(app: AppHandle, url: String) {
    if let Some(key) = url_key(&url) {
        let state = app.state::<AppState>();
        if let Ok(mut profile) = state.profile.lock() {
            profile.remove_history(&key);
        }
        save_profile(&app);
    }
}

#[tauri::command]
fn delete_watchlist(app: AppHandle, url: String) {
    if let Some(key) = url_key(&url) {
        let state = app.state::<AppState>();
        if let Ok(mut profile) = state.profile.lock() {
            profile.remove_watchlist(&key);
        }
        save_profile(&app);
    }
}

#[tauri::command]
fn delete_favorite(app: AppHandle, url: String) {
    if let Some(key) = url_key(&url) {
        let state = app.state::<AppState>();
        if let Ok(mut profile) = state.profile.lock() {
            profile.remove_favorite(&key);
        }
        save_profile(&app);
    }
}

#[tauri::command]
fn episode_change(app: AppHandle, data: Value) {
    let url = string_field(&data, "url");
    if url.is_empty() {
        return;
    }
    let kind = string_field(&data, "type");
    let is_movie = kind == "MOVIE";
    let title = string_field(&data, "title");
    let image = string_field(&data, "image");
    let language = string_field(&data, "language");
    let episode = string_field(&data, "episode");
    let saved = number_field(&data, "saved");
    let url_info = string_field(&data, "url_info");
    let raw_season = string_field(&data, "season");
    let season = match raw_season.find("Saison") {
        Some(index) => raw_season[index + "Saison".len()..].trim().to_string(),
        None => String::new(),
    };

    {
        let state = app.state::<AppState>();
        let Ok(mut profile) = state.profile.lock() else {
            return;
        };
        let mut patch = Map::new();
        if profile.history.contains_key(&url) {
            if !episode.is_empty() {
                patch.insert("episode".into(), json!(format!("Episode {episode}")));
            }
            if let Some(saved) = saved {
                patch.insert("saved".into(), json!(saved));
            }
        } else {
            patch.insert(
                "type".into(),
                json!(if is_movie { "Film" } else { kind.as_str() }),
            );
            if !title.is_empty() {
                patch.insert("title".into(), json!(title));
            }
            if !image.is_empty() {
                patch.insert("image".into(), json!(image));
            }
            if !language.is_empty() {
                patch.insert("language".into(), json!(language));
            }
            if !episode.is_empty() {
                patch.insert("episode".into(), json!(format!("Episode {episode}")));
            }
            if let Some(saved) = saved {
                patch.insert("saved".into(), json!(saved));
            }
        }
        profile.upsert_history(url.clone(), Value::Object(patch));
    }
    save_profile(&app);

    let watch = WatchData {
        kind: if is_movie { "MOVIE" } else { "SERIE" }.to_string(),
        title,
        image,
        url,
        url_info,
        season,
        episode,
        chapter: String::new(),
        language,
        playing: false,
        progress: None,
    };
    {
        let state = app.state::<AppState>();
        if let Ok(mut current) = state.watch.lock() {
            *current = Some(watch.clone());
        };
    }
    if let Some(window) = app.get_webview_window("main") {
        navigation::apply_watch(&app, &window, &watch);
    }
}

#[tauri::command]
fn scan_change(app: AppHandle, data: Value) {
    let url = string_field(&data, "url");
    if url.is_empty() {
        return;
    }
    let title = string_field(&data, "title");
    let image = string_field(&data, "image");
    let language = string_field(&data, "language");
    let chapter = string_field(&data, "chapter");
    let saved = number_field(&data, "saved");
    let url_info = string_field(&data, "url_info");

    {
        let state = app.state::<AppState>();
        let Ok(mut profile) = state.profile.lock() else {
            return;
        };
        let mut patch = Map::new();
        if profile.history.contains_key(&url) {
            if !chapter.is_empty() {
                patch.insert("episode".into(), json!(format!("Chapitre {chapter}")));
            }
            if let Some(saved) = saved {
                patch.insert("saved".into(), json!(saved));
            }
        } else {
            patch.insert("type".into(), json!("Scans"));
            if !title.is_empty() {
                patch.insert("title".into(), json!(title));
            }
            if !image.is_empty() {
                patch.insert("image".into(), json!(image));
            }
            if !language.is_empty() {
                patch.insert("language".into(), json!(language));
            }
            if !chapter.is_empty() {
                patch.insert("episode".into(), json!(format!("Chapitre {chapter}")));
            }
            if let Some(saved) = saved {
                patch.insert("saved".into(), json!(saved));
            }
        }
        profile.upsert_history(url.clone(), Value::Object(patch));
    }
    save_profile(&app);

    let watch = WatchData {
        kind: "SCAN".to_string(),
        title,
        image,
        url,
        url_info,
        season: String::new(),
        episode: String::new(),
        chapter,
        language,
        playing: false,
        progress: None,
    };
    {
        let state = app.state::<AppState>();
        if let Ok(mut current) = state.watch.lock() {
            *current = Some(watch.clone());
        };
    }
    if let Some(window) = app.get_webview_window("main") {
        navigation::apply_watch(&app, &window, &watch);
    }
}

#[tauri::command]
fn watchlist_change(app: AppHandle, data: Value) {
    let url = string_field(&data, "url");
    if url.is_empty() {
        return;
    }
    let title = string_field(&data, "title");
    let image = string_field(&data, "image");
    let mut patch = Map::new();
    if !title.is_empty() {
        patch.insert("title".into(), json!(title));
    }
    if !image.is_empty() {
        patch.insert("image".into(), json!(image));
    }
    let state = app.state::<AppState>();
    if let Ok(mut profile) = state.profile.lock() {
        profile.toggle_watchlist(url, Value::Object(patch));
    }
    save_profile(&app);
}

#[tauri::command]
fn favorite_change(app: AppHandle, data: Value) {
    let url = string_field(&data, "url");
    if url.is_empty() {
        return;
    }
    let title = string_field(&data, "title");
    let image = string_field(&data, "image");
    let mut patch = Map::new();
    if !title.is_empty() {
        patch.insert("title".into(), json!(title));
    }
    if !image.is_empty() {
        patch.insert("image".into(), json!(image));
    }
    let state = app.state::<AppState>();
    if let Ok(mut profile) = state.profile.lock() {
        profile.toggle_favorite(url, Value::Object(patch));
    }
    save_profile(&app);
}

#[tauri::command]
fn media_state(app: AppHandle, state: String) {
    let playing = state == "playing";
    let _ = app.emit("media-visibility", playing);
    let watch = {
        let state = app.state::<AppState>();
        let Ok(mut watch) = state.watch.lock() else {
            return;
        };
        let Some(watch) = watch.as_mut() else {
            return;
        };
        watch.playing = playing;
        watch.clone()
    };
    if let Some(window) = app.get_webview_window("main") {
        navigation::apply_watch(&app, &window, &watch);
    }
}

#[tauri::command]
fn video_progress(app: AppHandle, current: f64, duration: f64) {
    if !current.is_finite() || !duration.is_finite() || duration <= 0.0 {
        return;
    }
    let watch = {
        let state = app.state::<AppState>();
        let Ok(mut watch) = state.watch.lock() else {
            return;
        };
        let Some(watch) = watch.as_mut() else {
            return;
        };
        watch.progress = Some((current, duration));
        watch.clone()
    };
    if watch.playing {
        if let Some(window) = app.get_webview_window("main") {
            navigation::apply_watch(&app, &window, &watch);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(windows)]
    identity::set_process_app_user_model_id();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            for arg in &argv {
                if let Ok(url) = Url::parse(arg) {
                    if url.scheme() == "anime-sama-to" {
                        navigate_deep_link(app, &url);
                        return;
                    }
                }
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            window_minimize,
            window_maximize,
            window_close,
            window_home,
            window_state,
            delete_all_history,
            delete_all_watchlist,
            delete_all_favorite,
            delete_history,
            delete_watchlist,
            delete_favorite,
            episode_change,
            scan_change,
            watchlist_change,
            favorite_change,
            media_state,
            video_progress
        ])
        .setup(|app| {
            #[cfg(windows)]
            if let Ok(exe_path) = std::env::current_exe() {
                audio::spawn_audio_session_fixer(exe_path.to_string_lossy().into_owned());
            }

            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            std::fs::create_dir_all(&config_dir).ok();
            std::fs::create_dir_all(&cache_dir).ok();

            let paths = Paths {
                settings: config_dir.join("settings.json"),
                profile: config_dir.join("profile.json"),
                cache: cache_dir.clone(),
            };

            let mut settings = Settings::load(&config_dir);
            if settings.language.is_none() {
                settings.language = Some(Translations::detect_locale());
            }
            settings.save(&paths.settings);

            let language = settings
                .language
                .clone()
                .unwrap_or_else(|| "en".to_string());
            let translations = Translations::load(&language);
            let profile = Profile::load(&config_dir);
            if settings.adblock {
                adblock::load_cached(&cache_dir);
            }

            let available = app
                .primary_monitor()
                .ok()
                .flatten()
                .map(|monitor| monitor.size().to_logical::<u32>(monitor.scale_factor()))
                .unwrap_or(tauri::LogicalSize::new(1920, 1080));
            let width = settings
                .width
                .filter(|width| *width <= available.width)
                .unwrap_or((available.width as f64 * 0.8) as u32);
            let height = settings
                .height
                .filter(|height| *height <= available.height)
                .unwrap_or((available.height as f64 * 0.8) as u32);

            let discord = Discord::start(DISCORD_CLIENT_ID.to_string(), std::process::id());
            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                profile: Mutex::new(profile),
                translations,
                discord: Mutex::new(discord),
                paths,
                watch: Mutex::new(None),
                error_code: Mutex::new(500),
                last_size_save: Mutex::new(None),
                quitting: AtomicBool::new(false),
            });

            if !settings.start_hidden {
                let splash =
                    WebviewWindowBuilder::new(app, "splash", WebviewUrl::App("splash.html".into()))
                        .title("Anime Sama")
                        .devtools(false)
                        .inner_size(542.0, 132.0)
                        .resizable(false)
                        .decorations(false)
                        .visible(false)
                        .center()
                        .skip_taskbar(true)
                        .on_page_load(|window, payload| {
                            if payload.event() == PageLoadEvent::Finished {
                                disable_webview_ui(&window);
                                let _ = window.show();
                            }
                        })
                        .build();
                #[cfg(windows)]
                if let Ok(splash) = splash {
                    if let Ok(hwnd) = splash.hwnd() {
                        identity::set_window_app_user_model_id(hwnd);
                    }
                }
                #[cfg(not(windows))]
                let _ = splash;
            }

            let args: Vec<String> = std::env::args().collect();
            let start_url = initial_url(&args);
            let new_window_app = app.handle().clone();
            let load_app = app.handle().clone();
            let title_app = app.handle().clone();

            let ad_domains = serde_json::to_string(&adblock::blocked_domains(&cache_dir))
                .unwrap_or_else(|_| "[]".to_string());
            let adblock_script = format!(
                "window.__ANIME_SAMA_ADBLOCK__ = {};window.__ANIME_SAMA_AD_DOMAINS__ = {};",
                settings.adblock, ad_domains
            );
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(start_url))
                .title("Anime Sama")
                .devtools(false)
                .additional_browser_args("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required")
                .inner_size(width as f64, height as f64)
                .min_inner_size(640.0, 480.0)
                .decorations(false)
                .visible(false)
                .fullscreen(settings.start_full_screen)
                .initialization_script(&adblock_script)
                .initialization_script(PRELOAD)
                .on_navigation(move |url| navigation::is_internal(url))
                .on_new_window(move |url, _features| {
                    if navigation::is_internal(&url) {
                        if let Some(window) = new_window_app.get_webview_window("main") {
                            let _ = window.navigate(url);
                        }
                    }
                    NewWindowResponse::Deny
                })
                .on_page_load(move |window, payload| {
                    apply_adblock(&load_app);
                    if payload.event() == PageLoadEvent::Finished {
                        if navigation::is_error_url(payload.url()) {
                            navigation::fill_error(&load_app, &window);
                        } else {
                            navigation::apply_style(&window);
                            navigation::page_changed(&load_app, &window, payload.url(), None);
                        }
                        if let Some(splash) = load_app.get_webview_window("splash") {
                            let _ = splash.destroy();
                        }
                        let state = load_app.state::<AppState>();
                        if !state.settings().start_hidden {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    } else {
                        disable_webview_ui(&window);
                    }
                })
                .on_document_title_changed(move |window, title| {
                    if let Ok(url) = window.url() {
                        navigation::page_changed(&title_app, &window, &url, Some(title.as_str()));
                    }
                })
                .build()?;

            #[cfg(windows)]
            if let Ok(hwnd) = window.hwnd() {
                identity::set_window_app_user_model_id(hwnd);
            }

            if settings.adblock {
                adblock::apply(&window);
                adblock::inject_frame_styles(&window);
            }

            if let Ok(scale) = window.scale_factor() {
                let _ = window.set_zoom(1.0 / scale);
            }
            if settings.always_on_top {
                let _ = window.set_always_on_top(true);
            }

            let watchdog_app = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(15));
                let Some(splash) = watchdog_app.get_webview_window("splash") else {
                    return;
                };
                let _ = splash.destroy();
                if let Some(window) = watchdog_app.get_webview_window("main") {
                    if !watchdog_app.state::<AppState>().settings().start_hidden {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            });

            let _ = menu::create(app.handle());

            sync_autostart(app.handle());

            let back_app = app.handle().clone();
            let _ = app.global_shortcut().on_shortcut(
                "CommandOrControl+Shift+Z",
                move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(window) = back_app.get_webview_window("main") {
                            if window.is_focused().unwrap_or(false) {
                                let _ = window.eval("history.back()");
                            }
                        }
                    }
                },
            );
            let forward_app = app.handle().clone();
            let _ = app.global_shortcut().on_shortcut(
                "CommandOrControl+Shift+Y",
                move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        if let Some(window) = forward_app.get_webview_window("main") {
                            if window.is_focused().unwrap_or(false) {
                                let _ = window.eval("history.forward()");
                            }
                        }
                    }
                },
            );

            #[cfg(any(windows, target_os = "linux"))]
            {
                let _ = app.deep_link().register_all();
            }

            let deep_app = app.handle().clone();
            let _ = app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    navigate_deep_link(&deep_app, &url);
                }
            });

            let adblock_app = app.handle().clone();
            std::thread::spawn(move || {
                let state = adblock_app.state::<AppState>();
                adblock::load_lists(&state.paths.cache);
                if state.settings().adblock {
                    if let Some(window) = adblock_app.get_webview_window("main") {
                        adblock::apply(&window);
                    }
                }
            });

            updater::spawn(app.handle().clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    if window.label() == "splash" {
                        api.prevent_close();
                        return;
                    }
                    if window.label() != "main" {
                        return;
                    }
                    let state = app.state::<AppState>();
                    if state.quitting.load(Ordering::SeqCst) {
                        return;
                    }
                    if let Some(webview) = app.get_webview_window("main") {
                        save_window_size(app, &webview, true);
                    }
                    if state.settings().minimize_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        api.prevent_close();
                        state.quitting.store(true, Ordering::SeqCst);
                        app.exit(0);
                    }
                }
                WindowEvent::Resized(_) => {
                    if window.label() == "main" {
                        if let Some(webview) = app.get_webview_window("main") {
                            save_window_size(app, &webview, false);
                        }
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Anime Sama");
}
