//! The desktop shell.
//!
//! Phase 0 scaffold, and deliberately empty. There is no `invoke_handler`, so no
//! command is reachable from the webview, and no plugin is registered, so the
//! webview gets nothing beyond the core permissions listed by name in
//! `capabilities/default.json`. `app/README.md` records that boundary.
//!
//! Adding a command means three edits, not one: register it here, grant it in
//! `capabilities/default.json`, and add it to the inventory in `app/README.md`.

/// Build the window and run until it closes.
///
/// Startup failure is reported and exits non-zero rather than unwinding through a
/// panic message the user cannot read.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tauri::Builder::default().run(tauri::generate_context!()) {
        eprintln!("GTO Solver APP could not start its window: {error}");
        std::process::exit(1);
    }
}
