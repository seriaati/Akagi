//! Cross-platform startup hooks. Called once from `run()` before any GUI
//! subsystem (Tauri / webkit2gtk / wry / WebView2) is initialised.

/// Apply environment workarounds required for the current OS / display server.
///
/// - **Linux + Wayland**: webkit2gtk's default dmabuf renderer regularly trips
///   `Gdk-Message: Error 71 (Protocol error)` against modern Wayland compositors
///   (Mutter / KWin / Hyprland). Force-disable it. Also disable the GL
///   compositing mode as a secondary fallback. Both are no-ops on X11.
/// - **Linux + Wayland + XWayland**: run under XWayland (`GDK_BACKEND=x11`).
///   Wayland has no protocol for a client to keep itself above other windows,
///   so GTK's `set_keep_above` — what Tauri's `always_on_top` becomes — is a
///   no-op there and the overlay sinks behind the game. Compositors do honour
///   `_NET_WM_STATE_ABOVE` for X11 clients. Skipped if the user already chose
///   a backend, or if there is no XWayland (`DISPLAY` unset) to fall back to.
/// - **Windows / macOS**: nothing required at the moment.
pub fn setup() {
    #[cfg(target_os = "linux")]
    setup_linux();
}

#[cfg(target_os = "linux")]
fn setup_linux() {
    for var in [
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "WEBKIT_DISABLE_COMPOSITING_MODE",
    ] {
        if std::env::var_os(var).is_none() {
            std::env::set_var(var, "1");
        }
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_some()
        && std::env::var_os("GDK_BACKEND").is_none()
    {
        std::env::set_var("GDK_BACKEND", "x11");
    }
}
