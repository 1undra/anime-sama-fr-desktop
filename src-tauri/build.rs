fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "window_minimize",
                "window_maximize",
                "window_close",
                "window_home",
                "window_state",
                "delete_all_history",
                "delete_all_watchlist",
                "delete_all_favorite",
                "delete_history",
                "delete_watchlist",
                "delete_favorite",
                "episode_change",
                "scan_change",
                "watchlist_change",
                "favorite_change",
                "media_state",
                "video_progress",
            ]),
        ),
    )
    .expect("failed to run tauri-build");
}
