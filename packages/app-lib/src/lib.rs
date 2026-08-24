/*!
# Theseus

Theseus is a library which provides utilities for launching minecraft, creating Modrinth mod packs,
and launching Modrinth mod packs
*/
#![warn(unused_import_braces)]
#![deny(unused_must_use)]

#[macro_use]
mod util;

mod api;
mod error;
mod event;
pub mod install;
mod launcher;
mod logger;
mod state;

pub use api::*;
pub use error::*;
#[cfg(feature = "export-ts")]
pub use event::export_app_event_bindings;
pub use event::{
    AppEvent, EventState, LoadingBar, LoadingBarType, emit::emit_loading,
    emit::init_loading,
};
pub use logger::start_logger;
pub use state::State;
pub use util::fetch::DownloadReason;

pub fn launcher_user_agent() -> String {
    // Modrinth's own API docs require a uniquely-identifying User-Agent
    // per client (see docs.modrinth.com/api's "best practice" format:
    // `github_username/project_name/version (contact)`) -- this used to
    // literally send "modrinth/theseus" with Modrinth's own support email,
    // which both fails that requirement and misrepresents every request
    // this fork makes as coming from Modrinth's own official app.
    const LAUNCHER_BASE_USER_AGENT: &str =
        concat!("L0stFromSleep/SleeplessLauncher/", env!("CARGO_PKG_VERSION"),);

    format!(
        "{} ({}; https://github.com/L0stFromSleep/SleeplessLauncher)",
        LAUNCHER_BASE_USER_AGENT,
        std::env::consts::OS
    )
}
