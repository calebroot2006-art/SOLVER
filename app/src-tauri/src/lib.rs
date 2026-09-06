//! The desktop shell.
//!
//! Phase 0 scaffold. There are no application commands or added plugins. The
//! selected `default` capability grants no core or plugin API permissions.
//! Tauri still supplies its internal IPC machinery; this is not an IPC-free shell.
//! `app/README.md` records the boundary and its unverified runtime checks.
//!
//! Before adding a command, follow the manifest, permission, validation, and test
//! procedure in `app/README.md`. Registration here alone does not restrict callers.

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
