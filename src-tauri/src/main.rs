//! Cyonima-code-agent — binaire Tauri.
//!
//! Le cœur applicatif vit dans la lib `cyonima_core`. Le binaire ne fait que
//! monter le runtime Tauri et enregistrer les handlers IPC.

fn main() {
    cyonima_core::run();
}
