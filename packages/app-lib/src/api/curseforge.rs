//! Public API surface for CurseForge search/browsing, mirroring `api::cache`'s
//! role for Modrinth. Installing CurseForge content goes through
//! `api::instance::install_curseforge_project_with_dependencies` instead,
//! since it reuses the shared instance/install pipeline.

use crate::state::curseforge::{CfFile, CfMod};

#[derive(Debug, serde::Serialize)]
pub struct CurseForgeSearchResults {
    pub hits: Vec<CfMod>,
    pub total_hits: u32,
}

#[tracing::instrument]
#[allow(clippy::too_many_arguments)]
pub async fn search(
    query: String,
    game_version: Option<String>,
    class_id: Option<i64>,
    sort_field: Option<u32>,
    page: u32,
    page_size: u32,
) -> crate::Result<CurseForgeSearchResults> {
    let state = crate::State::get().await?;
    let (hits, total_hits) = crate::state::curseforge::search_mods(
        &query,
        game_version.as_deref(),
        class_id,
        sort_field,
        page,
        page_size,
        &state,
    )
    .await?;

    Ok(CurseForgeSearchResults { hits, total_hits })
}

#[tracing::instrument]
pub async fn get_mod(mod_id: String) -> crate::Result<CfMod> {
    let state = crate::State::get().await?;
    crate::state::curseforge::get_mod(&mod_id, &state).await
}

#[tracing::instrument]
pub async fn get_mod_description(mod_id: String) -> crate::Result<String> {
    let state = crate::State::get().await?;
    crate::state::curseforge::get_mod_description(&mod_id, &state).await
}

#[tracing::instrument]
pub async fn get_mod_files(mod_id: String) -> crate::Result<Vec<CfFile>> {
    let state = crate::State::get().await?;
    crate::state::curseforge::get_mod_files(&mod_id, &state).await
}
