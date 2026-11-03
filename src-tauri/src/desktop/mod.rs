//! Desktop-only behaviour around the engine. The engine stays free of Tauri so
//! the CLI can share it; anything needing an `AppHandle` lives here.

pub mod center;
