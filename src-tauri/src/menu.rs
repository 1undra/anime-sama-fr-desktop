use tauri::image::Image;
use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder,
};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::settings::Settings;
use crate::translations::Translations;
use crate::AppState;

pub fn build(
    app: &AppHandle,
    translations: &Translations,
    settings: &Settings,
) -> tauri::Result<Menu<Wry>> {
    let title = MenuItemBuilder::with_id("title", "Anime Sama")
        .enabled(false)
        .build(app)?;

    let import = SubmenuBuilder::new(app, translations.get("TRAY.IMPORT"))
        .item(
            &MenuItemBuilder::with_id("import-profile", translations.get("TRAY.PROFILE"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("import-settings", translations.get("TRAY.SETTINGS.LABEL"))
                .build(app)?,
        )
        .build()?;

    let export = SubmenuBuilder::new(app, translations.get("TRAY.EXPORT"))
        .item(
            &MenuItemBuilder::with_id("export-profile", translations.get("TRAY.PROFILE"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("export-settings", translations.get("TRAY.SETTINGS.LABEL"))
                .build(app)?,
        )
        .build()?;

    let actions = SubmenuBuilder::new(app, translations.get("TRAY.ACTIONS.LABEL"))
        .item(&MenuItemBuilder::with_id("back", translations.get("TRAY.ACTIONS.BACK")).build(app)?)
        .item(
            &MenuItemBuilder::with_id("forward", translations.get("TRAY.ACTIONS.FORWARD"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("restore", translations.get("TRAY.ACTIONS.RESTORE"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("reduce", translations.get("TRAY.ACTIONS.REDUCE"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("refresh", translations.get("TRAY.ACTIONS.REFRESH"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(
                "force-refresh",
                translations.get("TRAY.ACTIONS.FORCE_REFRESH"),
            )
            .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("center", translations.get("TRAY.ACTIONS.CENTER"))
                .build(app)?,
        )
        .build()?;

    let mut languages = SubmenuBuilder::new(app, translations.get("TRAY.SETTINGS.LANGUAGES"));
    for language in translations.languages() {
        let selected = settings.language.as_deref() == Some(language);
        let label = translations.language_name(language);
        let item =
            CheckMenuItemBuilder::with_id(format!("lang:{language}"), label).checked(selected);
        languages = languages.item(&item.build(app)?);
    }
    let languages = languages.build()?;

    let settings_menu = SubmenuBuilder::new(app, translations.get("TRAY.SETTINGS.LABEL"))
        .item(&languages)
        .item(&check(
            app,
            "start-up",
            translations.get("TRAY.SETTINGS.START_UP"),
            settings.start_up,
        )?)
        .item(&check(
            app,
            "start-hidden",
            translations.get("TRAY.SETTINGS.START_HIDDEN"),
            settings.start_hidden && !settings.start_full_screen,
        )?)
        .item(&check(
            app,
            "start-full-screen",
            translations.get("TRAY.SETTINGS.START_FULL_SCREEN"),
            settings.start_full_screen && !settings.start_hidden,
        )?)
        .item(&check(
            app,
            "minimize-to-tray",
            translations.get("TRAY.SETTINGS.MINIMIZE_TO_TRAY"),
            settings.minimize_to_tray,
        )?)
        .item(&check(
            app,
            "save-window-size",
            translations.get("TRAY.SETTINGS.SAVE_WINDOW_SIZE"),
            settings.save_window_size,
        )?)
        .item(&check(
            app,
            "always-on-top",
            translations.get("TRAY.SETTINGS.ALWAYS_ON_TOP"),
            settings.always_on_top,
        )?)
        .item(&check(
            app,
            "adblock",
            translations.get("TRAY.SETTINGS.ADBLOCK"),
            settings.adblock,
        )?)
        .item(&check(
            app,
            "discord-rich-presence",
            translations.get("TRAY.SETTINGS.DISCORD_RICH_PRESENCE"),
            settings.discord_rich_presence,
        )?)
        .build()?;

    let advanced = SubmenuBuilder::new(app, translations.get("TRAY.ADVANCED.LABEL"))
        .item(
            &MenuItemBuilder::with_id("reload", translations.get("TRAY.ADVANCED.RELOAD"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("restart", translations.get("TRAY.ADVANCED.RESTART"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("reset-data", translations.get("TRAY.ADVANCED.RESET_DATA"))
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("force-close", translations.get("TRAY.ADVANCED.FORCE_CLOSE"))
                .build(app)?,
        )
        .build()?;

    let close = MenuItemBuilder::with_id("close", translations.get("TRAY.CLOSE")).build(app)?;

    MenuBuilder::new(app)
        .item(&title)
        .separator()
        .item(&import)
        .item(&export)
        .separator()
        .item(&actions)
        .item(&settings_menu)
        .item(&advanced)
        .separator()
        .item(&close)
        .build()
}

fn check(
    app: &AppHandle,
    id: &str,
    label: String,
    checked: bool,
) -> tauri::Result<CheckMenuItem<Wry>> {
    CheckMenuItemBuilder::with_id(id, label)
        .checked(checked)
        .build(app)
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let menu = build(app, &state.translations, &settings)?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Anime Sama")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| crate::on_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                let Some(window) = app.get_webview_window("main") else {
                    return;
                };
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });

    let icon = app
        .default_window_icon()
        .cloned()
        .or_else(|| Image::from_bytes(include_bytes!("../icons/icon.png")).ok());
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let settings = state.settings();
    if let Ok(menu) = build(app, &state.translations, &settings) {
        let _ = tray.set_menu(Some(menu));
    }
}
