use adblock::engine::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime};
use url::Url;

const LISTS: &[&str] = &[
    "https://ublockorigin.github.io/uAssets/filters/filters.txt",
    "https://ublockorigin.github.io/uAssets/filters/badware.txt",
    "https://ublockorigin.github.io/uAssets/filters/privacy.txt",
    "https://ublockorigin.github.io/uAssets/filters/unbreak.txt",
    "https://ublockorigin.github.io/uAssets/filters/quick-fixes.txt",
    "https://ublockorigin.github.io/uAssets/thirdparties/easylist.txt",
    "https://ublockorigin.github.io/uAssets/thirdparties/easyprivacy.txt",
    "https://pgl.yoyo.org/adservers/serverlist.php?hostformat=hosts&showintro=1&mimetype=plaintext",
    "https://malware-filter.gitlab.io/urlhaus-filter/urlhaus-filter-ag-online.txt",
];

const CUSTOM: &[&str] = &["anime-sama.to##.ad-container", "anime-sama.to##.ad-label"];

const CRITICAL: &[&str] = &[
    "acscdn.com",
    "adcash.com",
    "myadcash.com",
    "onclckmn.com",
    "onclickads.net",
    "onclickalgo.com",
    "a-zzz.com",
    "aqle3.com",
    "btagmedia.com",
    "luugy.com",
    "luugy.net",
    "pefimatupajany.com",
    "aedilealumnae.cyou",
    "pdzmkymbyhqsu.website",
    "ablelaunchshorts.com",
    "ads-twitter.com",
    "imasdk.googleapis.com",
    "connatix.com",
    "amkdipsafdpo.in",
    "demandingoverdriveunthread.com",
    "pgo99.com",
    "autorepairworcester.com",
    "ouo.io",
    "ouo.press",
    "adf.ly",
    "adfoc.us",
    "iclickcdn.com",
    "imiclick.org",
    "qualiclicks.com",
    "vericlick.com",
    "popcash.net",
    "juicyads.com",
    "exoclick.com",
    "clickadu.com",
    "adsterra.com",
    "popads.net",
    "trafficjunky.com",
    "googlesyndication.com",
    "googleadservices.com",
    "doubleclick.net",
    "adservice.google.com",
];

const WHITELIST: &[&str] = &[
    "anime-sama.to",
    "tauri.localhost",
    "cdn.jsdelivr.net",
    "jsdelivr.net",
    "raw.githubusercontent.com",
    "githubusercontent.com",
    "flagcdn.com",
    "img.youtube.com",
    "ytimg.com",
    "googleusercontent.com",
    "gstatic.com",
    "cloudflare.com",
    "cloudflare.net",
    "jquery.com",
    "fontawesome.com",
    "ansembed.net",
    "vmpx.online",
    "upvm.online",
    "jwpcdn.com",
    "jwplayer.com",
    "staticmoly.me",
];

const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const CACHE_FILE: &str = "adblock-engine-v1.txt";

static ENGINE: OnceLock<Arc<RwLock<Option<Engine>>>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);
static INTERCEPT_READY: AtomicBool = AtomicBool::new(false);

fn engine_slot() -> &'static Arc<RwLock<Option<Engine>>> {
    ENGINE.get_or_init(|| Arc::new(RwLock::new(None)))
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn blocked_domains(cache_dir: &Path) -> Vec<String> {
    let mut domains: BTreeSet<String> = CRITICAL.iter().map(|domain| domain.to_string()).collect();
    if let Ok(text) = fs::read_to_string(cache_dir.join(CACHE_FILE)) {
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("@@") || !(line.contains("$popup") || line.contains("$document")) {
                continue;
            }
            let Some(rest) = line.strip_prefix("||") else {
                continue;
            };
            let end = rest
                .find(|c: char| c == '^' || c == '/' || c == '$' || c == ',' || c == '*')
                .unwrap_or(rest.len());
            let domain = &rest[..end];
            if is_valid_domain(domain) {
                domains.insert(domain.to_ascii_lowercase());
            }
        }
    }
    domains.into_iter().collect()
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 || !domain.contains('.') {
        return false;
    }
    if !domain
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
    {
        return false;
    }
    let tld = domain.rsplit('.').next().unwrap_or("");
    tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic())
}

pub fn load_cached(cache_dir: &Path) -> bool {
    let path = cache_dir.join(CACHE_FILE);
    if !is_fresh(&path) {
        return false;
    }
    match fs::read_to_string(&path) {
        Ok(text) if !text.is_empty() => {
            build_engine(&text);
            true
        }
        _ => false,
    }
}

pub fn load_lists(cache_dir: &Path) -> bool {
    let path = cache_dir.join(CACHE_FILE);
    if is_fresh(&path) {
        return load_cached(cache_dir);
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build();
    let mut text = String::new();
    let mut loaded = 0;
    for list in LISTS {
        if let Ok(response) = agent.get(list).call() {
            if let Ok(body) = response.into_string() {
                append_list(&mut text, &body);
                loaded += 1;
            }
        }
    }
    if loaded == 0 {
        return load_cached(cache_dir);
    }
    for filter in CUSTOM {
        text.push_str(filter);
        text.push('\n');
    }

    let _ = fs::create_dir_all(cache_dir);
    let _ = fs::write(&path, &text);
    build_engine(&text);
    true
}

fn append_list(target: &mut String, body: &str) {
    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line
            .strip_prefix("0.0.0.0")
            .or_else(|| line.strip_prefix("127.0.0.1"))
        {
            if let Some(host) = rest.split_whitespace().next() {
                if host.contains('.') && !host.starts_with('#') {
                    target.push_str("||");
                    target.push_str(host);
                    target.push_str("^\n");
                }
            }
            continue;
        }
        target.push_str(line);
        target.push('\n');
    }
}

fn build_engine(text: &str) {
    let mut set = FilterSet::new(false);
    set.add_filter_list(text.to_string(), ParseOptions::default());
    let engine = Engine::new_with_filter_set(set);
    if let Ok(mut slot) = engine_slot().write() {
        *slot = Some(engine);
    }
}

fn is_fresh(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(modified)
        .map(|age| age < MAX_AGE)
        .unwrap_or(false)
}

fn is_allowed_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    WHITELIST.iter().any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
}

fn is_critical_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    CRITICAL
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}

fn should_block(params: &Value, engine: &Arc<RwLock<Option<Engine>>>) -> bool {
    let Some(request) = params.get("request") else {
        return false;
    };
    let Some(url) = request.get("url").and_then(Value::as_str) else {
        return false;
    };
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if parsed.host_str().is_some_and(is_allowed_host) {
        return false;
    }
    if parsed.host_str().is_some_and(is_critical_host) {
        return true;
    }

    let source = params
        .get("initiator")
        .and_then(|value| value.get("url"))
        .and_then(Value::as_str)
        .or_else(|| params.get("documentURL").and_then(Value::as_str))
        .unwrap_or("");
    let frame = params.get("frameId").and_then(Value::as_str).unwrap_or("");
    let parent = params.get("parentFrameId").and_then(Value::as_str).unwrap_or("");
    let resource_type = match params.get("resourceType").and_then(Value::as_str).unwrap_or("") {
        "Stylesheet" => "stylesheet",
        "Image" => "image",
        "Script" => "script",
        "Font" => "font",
        "Media" => "media",
        "XHR" => "xmlhttprequest",
        "Fetch" => "fetch",
        "WebSocket" => "websocket",
        "Ping" => "ping",
        "Document" if !frame.is_empty() && !parent.is_empty() && frame != parent => "sub_frame",
        "Document" => "main_frame",
        _ => "other",
    };
    let method = request.get("method").and_then(Value::as_str).unwrap_or("GET");
    let Ok(ad_request) = Request::new(url, source, resource_type, method) else {
        return false;
    };
    let Ok(engine) = engine.read() else {
        return false;
    };
    engine
        .as_ref()
        .is_some_and(|engine| engine.check_network_request(&ad_request).should_block())
}

#[cfg(windows)]
pub fn inject_frame_styles(window: &tauri::WebviewWindow) {
    static READY: AtomicBool = AtomicBool::new(false);
    if READY.swap(true, Ordering::SeqCst) {
        return;
    }
    use webview2_com::{
        ExecuteScriptCompletedHandler, FrameCreatedEventHandler,
        FrameNavigationCompletedEventHandler,
        Microsoft::Web::WebView2::Win32::{
            ICoreWebView2, ICoreWebView2Frame, ICoreWebView2Frame2,
            ICoreWebView2FrameCreatedEventArgs, ICoreWebView2_4,
        },
    };
    use windows::core::{HSTRING, Interface};

    const SCRIPT: &str = r#"(() => {
        const apply = () => {
            if (!document.head || !document.documentElement) { window.setTimeout(apply, 20); return; }
            if (document.getElementById('anime-sama-frame-ad-style')) return;
            const style = document.createElement('style');
            style.id = 'anime-sama-frame-ad-style';
            style.textContent = '#vidmolyadblocktest,.jw-ads,.jw-ad,.jwp-ad,.jwp-ad-overlay,.advert,.ads,ins.adsbygoogle,.ad-container,.ad-label,iframe[src*="/pub/"]{display:none!important;visibility:hidden!important;width:0!important;height:0!important;overflow:hidden!important}';
            document.head.appendChild(style);
        };
        apply();
    })();"#;

    let _ = window.with_webview(move |platform| {
        let Ok(webview) = (unsafe { platform.controller().CoreWebView2() }) else {
            return;
        };
        let Ok(webview4) = webview.cast::<ICoreWebView2_4>() else {
            READY.store(false, Ordering::SeqCst);
            return;
        };
        let handler = FrameCreatedEventHandler::create(Box::new(
            move |_sender: Option<ICoreWebView2>,
                  args: Option<ICoreWebView2FrameCreatedEventArgs>| {
                let Some(args) = args else {
                    return Ok(());
                };
                let Ok(frame) = (unsafe { args.Frame() }) else {
                    return Ok(());
                };
                let Ok(frame) = frame.cast::<ICoreWebView2Frame2>() else {
                    return Ok(());
                };
                let handler = FrameNavigationCompletedEventHandler::create(Box::new(
                    move |frame: Option<ICoreWebView2Frame>, _args| {
                        let Some(frame) = frame else {
                            return Ok(());
                        };
                        let Ok(frame) = frame.cast::<ICoreWebView2Frame2>() else {
                            return Ok(());
                        };
                        let script = ExecuteScriptCompletedHandler::create(Box::new(
                            |_result: windows::core::Result<()>, _json: String| Ok(()),
                        ));
                        let _ = unsafe { frame.ExecuteScript(&HSTRING::from(SCRIPT), &script) };
                        Ok(())
                    },
                ));
                let mut token = 0i64;
                if unsafe { frame.add_NavigationCompleted(&handler, &mut token) }.is_ok() {
                    std::mem::forget(frame);
                }
                Ok(())
            },
        ));
        let mut token = 0i64;
        if unsafe { webview4.add_FrameCreated(&handler, &mut token) }.is_ok() {
            std::mem::forget(webview4);
        } else {
            READY.store(false, Ordering::SeqCst);
        }
    });
}

#[cfg(not(windows))]
pub fn inject_frame_styles(window: &tauri::WebviewWindow) {
    let _ = window;
}

pub fn apply(window: &tauri::WebviewWindow) {
    #[cfg(windows)]
    apply_windows(window);
    #[cfg(not(windows))]
    let _ = window;
}

#[cfg(windows)]
fn resource_type(context: i32) -> &'static str {
    match context {
        1 => "main_frame",
        2 => "stylesheet",
        3 => "image",
        4 => "media",
        5 => "font",
        6 => "script",
        7 => "xmlhttprequest",
        8 => "fetch",
        11 => "websocket",
        14 => "ping",
        _ => "other",
    }
}

#[cfg(windows)]
fn apply_windows(window: &tauri::WebviewWindow) {
    use webview2_com::{
        take_pwstr, WebResourceRequestedEventHandler,
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_OTHER,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL, ICoreWebView2,
            ICoreWebView2WebResourceRequestedEventArgs, ICoreWebView2_2, ICoreWebView2_22,
        },
    };
    use windows::core::{HSTRING, PWSTR, Interface};
    use windows::Win32::System::Com::IStream;

    if INTERCEPT_READY.swap(true, Ordering::SeqCst) {
        return;
    }

    let engine = Arc::clone(engine_slot());
    let _ = window.with_webview(move |platform| {
        let controller = platform.controller();
        let Ok(webview) = (unsafe { controller.CoreWebView2() }) else {
            INTERCEPT_READY.store(false, Ordering::SeqCst);
            return;
        };
        let Ok(webview2) = webview.cast::<ICoreWebView2_2>() else {
            INTERCEPT_READY.store(false, Ordering::SeqCst);
            return;
        };
        let Ok(environment) = (unsafe { webview2.Environment() }) else {
            INTERCEPT_READY.store(false, Ordering::SeqCst);
            return;
        };

        let contexts = [
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FONT,
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_OTHER,
        ];
        let mut filter_result = unsafe {
            webview.AddWebResourceRequestedFilter(
                &HSTRING::from("*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )
        };
        for context in contexts {
            if let Err(error) = unsafe {
                webview.AddWebResourceRequestedFilter(&HSTRING::from("*"), context)
            } {
                filter_result = Err(error);
            }
        }
        if let Ok(webview22) = webview.cast::<ICoreWebView2_22>() {
            for context in contexts {
                if let Err(error) = unsafe {
                    webview22.AddWebResourceRequestedFilterWithRequestSourceKinds(
                        &HSTRING::from("*"),
                        context,
                        COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                    )
                } {
                    filter_result = Err(error);
                }
            }
        }
        if filter_result.is_err() {
            INTERCEPT_READY.store(false, Ordering::SeqCst);
            return;
        }

        let handler = WebResourceRequestedEventHandler::create(Box::new(
            move |_sender: Option<ICoreWebView2>,
                  args: Option<ICoreWebView2WebResourceRequestedEventArgs>| {
                let Some(args) = args else {
                    return Ok(());
                };
                if !ENABLED.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let Ok(request) = (unsafe { args.Request() }) else {
                    return Ok(());
                };
                let mut uri = PWSTR::null();
                if unsafe { request.Uri(&mut uri) }.is_err() {
                    return Ok(());
                }
                let url = take_pwstr(uri);
                let mut method = PWSTR::null();
                let _ = unsafe { request.Method(&mut method) };
                let http_method = take_pwstr(method);
                let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL;
                let _ = unsafe { args.ResourceContext(&mut context) };
                let params = serde_json::json!({
                    "request": { "url": url, "method": http_method },
                    "resourceType": resource_type(context.0),
                });
                if !should_block(&params, &engine) {
                    return Ok(());
                }
                let response = unsafe {
                    environment.CreateWebResourceResponse(
                        None::<&IStream>,
                        200,
                        &HSTRING::from("OK"),
                        &HSTRING::from("Content-Type: text/html"),
                    )
                };
                if let Ok(response) = response {
                    let _ = unsafe { args.SetResponse(&response) };
                }
                Ok(())
            },
        ));

        let mut token = 0i64;
        let reg = unsafe { webview.add_WebResourceRequested(&handler, &mut token) };
        if reg.is_err() {
            INTERCEPT_READY.store(false, Ordering::SeqCst);
            return;
        }
        std::mem::forget(handler);
    });
}
