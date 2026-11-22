use serde_json::json;
use tauri::{AppHandle, Manager, WebviewWindow};
use url::Url;

use crate::discord::Activity;
use crate::translations::Translations;
use crate::{AppState, BASE_URL, REPO};

pub const NAVIGATION_HEIGHT: u32 = 64;

#[derive(Clone)]
pub struct WatchData {
    pub kind: String,
    pub title: String,
    pub image: String,
    pub url: String,
    pub url_info: String,
    pub season: String,
    pub episode: String,
    pub chapter: String,
    pub language: String,
    pub playing: bool,
    pub progress: Option<(f64, f64)>,
}

impl WatchData {
    pub fn new(kind: String) -> Self {
        Self {
            kind,
            title: String::new(),
            image: String::new(),
            url: String::new(),
            url_info: String::new(),
            season: String::new(),
            episode: String::new(),
            chapter: String::new(),
            language: String::new(),
            playing: false,
            progress: None,
        }
    }
}

pub fn base_host() -> String {
    Url::parse(BASE_URL)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_default()
}

pub fn is_internal(url: &Url) -> bool {
    if matches!(url.scheme(), "tauri" | "data") {
        return true;
    }
    if let Some(host) = url.host_str() {
        if host == "tauri.localhost" {
            return true;
        }
        let base = base_host();
        return host == base || host.ends_with(&format!(".{base}"));
    }
    false
}

pub fn apply_style(window: &WebviewWindow) {
    let css = format!(
        "body {{padding-top: {height}px;}} nav.asn-navbar {{position: fixed;top: 0;left: 0;right: 0;-webkit-app-region: drag;padding-right: 0;}} nav.asn-navbar a, nav.asn-navbar button, nav.asn-navbar input, nav.asn-navbar select, nav.asn-navbar textarea, nav.asn-navbar [role=\"button\"], #desktop-window-controls {{-webkit-app-region: no-drag;}} html.anime-sama-media-playing #desktop-window-controls {{display:none!important}} ::-webkit-scrollbar {{display: none;}} div ::-webkit-scrollbar {{display: block;}}",
        height = NAVIGATION_HEIGHT
    );
    let script = format!(
        "(() => {{const style = document.getElementById('anime-sama-desktop-style') || document.createElement('style');style.id = 'anime-sama-desktop-style';style.textContent = {css};if (!style.isConnected) document.head.appendChild(style);const nav = document.querySelector('nav.asn-navbar');if (nav) nav.setAttribute('data-tauri-drag-region', '');document.querySelectorAll('a').forEach(item => {{item.removeAttribute('target');item.setAttribute('draggable', 'false');}});}})();",
        css = json!(css)
    );
    let _ = window.eval(&script);
    inject_window_controls(window);
}

pub fn inject_window_controls(window: &WebviewWindow) {
    let script = r##"(() => {
        if (!document.head) return;
        const hide = document.getElementById('anime-sama-controls-hide') || document.createElement('style');
        hide.id = 'anime-sama-controls-hide';
        hide.textContent = "html.anime-sama-media-playing #desktop-window-controls{display:none!important}";
        if (!hide.isConnected) document.head.appendChild(hide);
        if (document.getElementById('desktop-window-controls') || !document.body) return;
        const style = document.createElement('style');
        style.textContent = '#desktop-window-controls{display:flex;align-self:stretch;margin-left:auto;-webkit-app-region:no-drag}#desktop-window-controls button{display:flex;align-items:center;justify-content:center;width:46px;padding:0;border:0;background:transparent;color:#CBD5E1;cursor:default;transition:background-color 150ms,color 150ms}#desktop-window-controls button:hover{background:rgba(255,255,255,0.1);color:#FFF}#desktop-window-controls button:active{background:rgba(255,255,255,0.05)}#desktop-window-controls button[data-control=close]:hover{background:#E81123;color:#FFF}#desktop-window-controls svg{width:10px;height:10px;fill:none;stroke:currentColor;stroke-width:1}body>#desktop-window-controls{position:fixed;top:0;right:0;height:48px;z-index:2147483647}';
        document.head.appendChild(style);
        const controls = document.createElement('div');
        controls.id = 'desktop-window-controls';
        const icons = {
            minimize: '<svg viewBox="0 0 10 10"><path d="M0 5h10"/></svg>',
            maximize: '<svg viewBox="0 0 10 10"><rect x="0.5" y="0.5" width="9" height="9"/></svg>',
            restore: '<svg viewBox="0 0 10 10"><path d="M2.5 2.5v-2h7v7h-2"/><rect x="0.5" y="2.5" width="7" height="7"/></svg>',
            close: '<svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10"/></svg>'
        };
        const invoke = window.__TAURI__ && window.__TAURI__.core ? window.__TAURI__.core.invoke : null;
        for (const control of ['minimize', 'maximize', 'close']) {
            const button = document.createElement('button');
            button.type = 'button';
            button.dataset.control = control;
            button.innerHTML = icons[control];
            if (invoke) button.addEventListener('click', () => invoke(`window_${control}`).catch(() => {}));
            controls.appendChild(button);
        }
        (document.querySelector('nav.asn-navbar') || document.body).appendChild(controls);
        const maximize = controls.querySelector('[data-control=maximize]');
        const drawMaximize = maximized => maximize.innerHTML = maximized ? icons.restore : icons.maximize;
        if (invoke) invoke('window_state').then(drawMaximize).catch(() => {});
    })();"##;
    let _ = window.eval(script);
}

pub fn page_changed(app: &AppHandle, window: &WebviewWindow, url: &Url, title: Option<&str>) {
    if !is_internal(url) || url.scheme() == "data" {
        return;
    }

    let path = url.path().trim_end_matches('/');
    if !path.starts_with("/catalogue/") {
        if let Ok(mut watch) = app.state::<AppState>().watch.lock() {
            watch.take();
        }
    }

    let state = app.state::<AppState>();
    let translations = &state.translations;
    let mut activity = Activity::default();
    let page_title;

    match path {
        "" => {
            page_title = Some(translations.get("TITLE.HOME"));
            activity.details = Some(translations.get("ACTIVITY.HOME.DETAILS"));
            activity.large_image = Some("logo".into());
            activity
                .buttons
                .push((translations.get("ACTIVITY.HOME.BUTTON"), url.to_string()));
        }
        "/catalogue" => {
            page_title = Some(translations.get("TITLE.CATALOG"));
            activity.details = Some(translations.get("ACTIVITY.CATALOG.DETAILS"));
            activity.large_image = Some("catalog".into());
            activity
                .buttons
                .push((translations.get("ACTIVITY.CATALOG.BUTTON"), url.to_string()));
        }
        "/planning" => {
            page_title = Some(translations.get("TITLE.PLANNING"));
            activity.details = Some(translations.get("ACTIVITY.PLANNING.DETAILS"));
            activity.large_image = Some("planning".into());
            activity.buttons.push((
                translations.get("ACTIVITY.PLANNING.BUTTON"),
                url.to_string(),
            ));
        }
        "/aide" => {
            page_title = Some(translations.get("TITLE.HELP"));
            activity.details = Some(translations.get("ACTIVITY.HELP.DETAILS"));
            activity.large_image = Some("help".into());
            activity
                .buttons
                .push((translations.get("ACTIVITY.HELP.BUTTON"), url.to_string()));
        }
        "/profil" => {
            page_title = Some(translations.get("TITLE.PROFILE"));
            activity.details = Some(translations.get("ACTIVITY.PROFILE.DETAILS"));
            activity.large_image = Some("profile".into());
            activity
                .buttons
                .push((translations.get("ACTIVITY.PROFILE.BUTTON"), url.to_string()));
        }
        "/contact" => {
            page_title = Some(translations.get("TITLE.CONTACT"));
            activity.details = Some(translations.get("ACTIVITY.CONTACT.DETAILS"));
            activity.large_image = Some("contact".into());
            activity
                .buttons
                .push((translations.get("ACTIVITY.CONTACT.BUTTON"), url.to_string()));
        }
        other if other.starts_with("/catalogue/") => {
            let rest = &other["/catalogue/".len()..];
            if rest.contains('/') {
                return;
            }
            let name = title
                .and_then(|value| value.split('|').next())
                .map(str::trim)
                .unwrap_or("");
            page_title = Some(translations.get("TITLE.INFO").replace("#NAME#", name));
            activity.details = Some(translations.get("ACTIVITY.INFO.DETAILS"));
            activity.large_image = Some(format!(
                "https://cdn.statically.io/gh/Anime-Sama/IMG/img/contenu/{rest}.jpg"
            ));
            activity.large_text = Some(name.to_string());
            activity
                .buttons
                .push((translations.get("ACTIVITY.INFO.BUTTON"), url.to_string()));
        }
        _ => return,
    }

    if let Some(page_title) = page_title {
        let _ = window.set_title(&page_title);
    }

    if state.settings().discord_rich_presence {
        state.discord_set(activity);
    }
}

pub fn watch_activity(translations: &Translations, watch: &WatchData) -> Activity {
    let mut activity = Activity::default();
    let status = if watch.playing {
        translations.get("ACTIVITY.WATCH.PLAYING")
    } else {
        translations.get("ACTIVITY.WATCH.PAUSED")
    };

    match watch.kind.as_str() {
        "SCAN" => {
            activity.details = Some(translations.get("ACTIVITY.WATCH.SCAN.DETAILS"));
            activity.state = Some(
                translations
                    .get("ACTIVITY.WATCH.SCAN.STATE")
                    .replace("#CHAPTER#", &watch.chapter),
            );
            activity.buttons.push((
                translations.get("ACTIVITY.WATCH.SCAN.BUTTON"),
                format!("{BASE_URL}{}", watch.url),
            ));
        }
        "MOVIE" => {
            activity.details = Some(
                translations
                    .get("ACTIVITY.WATCH.MOVIE.DETAILS")
                    .replace("#STATUS#", &status),
            );
            activity.buttons.push((
                translations.get("ACTIVITY.WATCH.MOVIE.BUTTON"),
                format!("{BASE_URL}{}", watch.url),
            ));
        }
        _ => {
            activity.details = Some(
                translations
                    .get("ACTIVITY.WATCH.SERIE.DETAILS")
                    .replace("#STATUS#", &status),
            );
            let key = if watch.season.is_empty() {
                "ACTIVITY.WATCH.SERIE.STATE.OTHER"
            } else {
                "ACTIVITY.WATCH.SERIE.STATE.SEASON"
            };
            activity.state = Some(
                translations
                    .get(key)
                    .replace("#SEASON#", &watch.season)
                    .replace("#EPISODE#", &watch.episode),
            );
            activity.buttons.push((
                translations.get("ACTIVITY.WATCH.SERIE.BUTTON"),
                format!("{BASE_URL}{}", watch.url),
            ));
        }
    }

    if !watch.title.is_empty() {
        activity.large_text = Some(watch.title.clone());
    }
    if !watch.image.is_empty() {
        activity.large_image = Some(watch.image.clone());
    }

    let language = match watch.language.to_ascii_lowercase() {
        value if value.starts_with("vf") => Some("vf"),
        value if value.starts_with("vcn") => Some("vcn"),
        value if value.starts_with("va") => Some("va"),
        value if value.starts_with("vo") => Some("vo"),
        _ => None,
    };
    if let Some(language) = language {
        activity.small_image = Some(format!(
            "https://cdn.jsdelivr.net/gh/{REPO}@main/assets/img/discord-rich-presence/{language}.png"
        ));
        activity.small_text =
            Some(translations.get(&format!("ACTIVITY.{}", language.to_uppercase())));
    }

    if !watch.url_info.is_empty() {
        activity.buttons.push((
            translations.get("ACTIVITY.INFO.BUTTON"),
            format!("{BASE_URL}{}", watch.url_info),
        ));
    }

    if watch.playing {
        if let Some((current, duration)) = watch.progress {
            let now = crate::discord::now_ms();
            activity.start = Some(now - (current * 1000.0) as i64);
            activity.end = Some(now + ((duration - current) * 1000.0) as i64);
        }
    }

    activity
}

pub fn watch_title(translations: &Translations, watch: &WatchData) -> String {
    match watch.kind.as_str() {
        "SCAN" => translations
            .get("TITLE.WATCH.SCAN")
            .replace("#CHAPTER#", &watch.chapter)
            .replace("#NAME#", &watch.title),
        "MOVIE" => translations
            .get("TITLE.WATCH.MOVIE")
            .replace("#NAME#", &watch.title),
        _ => {
            let key = if watch.season.is_empty() {
                "TITLE.WATCH.SERIE.OTHER"
            } else {
                "TITLE.WATCH.SERIE.SEASON"
            };
            translations
                .get(key)
                .replace("#SEASON#", &watch.season)
                .replace("#EPISODE#", &watch.episode)
                .replace("#NAME#", &watch.title)
        }
    }
}

pub fn apply_watch(app: &AppHandle, window: &WebviewWindow, watch: &WatchData) {
    let state = app.state::<AppState>();
    let title = watch_title(&state.translations, watch);
    let _ = window.set_title(&title);
    if state.settings().discord_rich_presence {
        state.discord_set(watch_activity(&state.translations, watch));
    }
}

pub fn refresh_activity(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let watch = state.watch.lock().ok().and_then(|value| value.clone());
    match watch {
        Some(watch) => apply_watch(app, &window, &watch),
        None => {
            if let Ok(url) = window.url() {
                page_changed(app, &window, &url, None);
            }
        }
    }
}

#[cfg(windows)]
fn error_url() -> Option<Url> {
    Url::parse("http://tauri.localhost/error.html").ok()
}

#[cfg(not(windows))]
fn error_url() -> Option<Url> {
    Url::parse("tauri://localhost/error.html").ok()
}

pub fn is_error_url(url: &Url) -> bool {
    url.path().ends_with("/error.html")
}

pub fn show_error(app: &AppHandle, code: u16) {
    let state = app.state::<AppState>();
    if let Ok(mut current) = state.error_code.lock() {
        *current = code;
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Some(url) = error_url() {
            let _ = window.navigate(url);
        }
    }
}

pub fn fill_error(app: &AppHandle, window: &WebviewWindow) {
    let state = app.state::<AppState>();
    let code = state.error_code.lock().map(|code| *code).unwrap_or(500);
    let translations = &state.translations;
    let label = translations.get("ERROR.LABEL");
    let status = translations
        .get("ERROR.STATUS")
        .replace("#CODE#", &code.to_string());
    let redirect = translations.get("ERROR.REDIRECT");

    let script = format!(
        "(() => {{const apply = () => {{const title = document.getElementById('title');const code = document.getElementById('code');const link = document.getElementById('redirect');if (title) title.innerText = {label};if (code) code.innerText = {status};if (link) {{link.href = {home};link.innerText = {redirect};}}}};if (document.readyState == 'loading') document.addEventListener('DOMContentLoaded', apply);else apply();}})();",
        label = json!(&label),
        status = json!(&status),
        home = json!(BASE_URL),
        redirect = json!(&redirect)
    );
    let _ = window.eval(&script);

    let title = translations
        .get("TITLE.ERROR")
        .replace("#CODE#", &code.to_string());
    let _ = window.set_title(&title);

    if state.settings().discord_rich_presence {
        let mut activity = Activity::default();
        activity.details = Some(label);
        activity.large_image = Some("error".into());
        activity.large_text = Some(status);
        state.discord_set(activity);
    }
}
