//! CurseForge API client, kept isolated from Modrinth's `state::cache`
//! module so upstream Modrinth changes can be pulled with minimal conflict.
//!
//! Unlike Modrinth's API base URLs (baked in at compile time via `.env`),
//! CurseForge's base URL is a plain public constant, and the user's personal
//! API key is stored locally in [`crate::state::Settings`] rather than
//! compiled in -- see `Settings::curseforge_api_key`.

pub(crate) mod client;
pub mod modrinth_equivalent;
mod models;
mod provider;

pub use models::{
    CfAsset, CfAuthor, CfCategory, CfFile, CfMod, CfModLinks, CfScreenshot,
    cf_fingerprint, class_id, project_type_for_class_id,
};
pub(crate) use provider::CurseForgeContentProvider;

use crate::State;

/// Reads the user's CurseForge API key from local settings, returning a
/// descriptive error if it hasn't been configured yet.
pub(crate) async fn api_key(state: &State) -> crate::Result<String> {
    let settings = crate::state::Settings::get(&state.pool).await?;

    settings.curseforge_api_key.filter(|key| !key.is_empty()).ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Add your CurseForge API key in Settings before using CurseForge content.".to_string(),
        )
        .into()
    })
}

/// `GET /v1/mods/search`, returning `(mods, total_hits)`.
#[allow(clippy::too_many_arguments)]
pub async fn search_mods(
    query: &str,
    game_version: Option<&str>,
    class_id: Option<i64>,
    sort_field: Option<u32>,
    page: u32,
    page_size: u32,
    state: &State,
) -> crate::Result<(Vec<CfMod>, u32)> {
    let api_key = api_key(state).await?;
    client::search_mods(
        &api_key,
        query,
        game_version,
        class_id,
        sort_field,
        page,
        page_size,
        state,
    )
    .await
}

/// `GET /v1/mods/{modId}`.
pub async fn get_mod(mod_id: &str, state: &State) -> crate::Result<CfMod> {
    let api_key = api_key(state).await?;
    client::get_mod(&api_key, mod_id, state).await
}

/// `GET /v1/mods/{modId}/description`.
pub async fn get_mod_description(
    mod_id: &str,
    state: &State,
) -> crate::Result<String> {
    let api_key = api_key(state).await?;
    client::get_mod_description(&api_key, mod_id, state).await
}

/// `GET /v1/mods/{modId}/files`.
pub async fn get_mod_files(
    mod_id: &str,
    state: &State,
) -> crate::Result<Vec<CfFile>> {
    let api_key = api_key(state).await?;
    client::get_mod_files(&api_key, mod_id, state).await
}

/// Looks up a single file by id via CurseForge's batch files endpoint.
pub async fn get_file(file_id: &str, state: &State) -> crate::Result<CfFile> {
    let api_key = api_key(state).await?;
    client::get_file(&api_key, file_id, state).await
}
