//! Thin HTTP client for the subset of the CurseForge REST API
//! (<https://docs.curseforge.com/rest-api/>) needed for search, project
//! details, and file download. Reuses the app's generic `fetch::fetch_advanced`
//! rather than a dedicated HTTP client, passing an explicit `x-api-key`
//! header per request.
//!
//! No response caching is implemented here (unlike the Modrinth path in
//! `state::cache`), and CurseForge's own documented rate limits are not
//! enforced -- both are acceptable for this MVP slice but should be
//! revisited before heavier use.

use super::models::{
    CfDataEnvelope, CfFile, CfMod, CfSearchEnvelope, MINECRAFT_GAME_ID,
};
use crate::State;
use crate::util::fetch;
use reqwest::Method;
use serde::de::DeserializeOwned;

const CURSEFORGE_API_BASE: &str = "https://api.curseforge.com/v1/";

async fn fetch_curseforge<T: DeserializeOwned>(
    method: Method,
    path: &str,
    api_key: &str,
    json_body: Option<serde_json::Value>,
    state: &State,
) -> crate::Result<T> {
    let url = format!("{CURSEFORGE_API_BASE}{path}");
    let bytes = fetch::fetch_advanced(
        method,
        &url,
        None,
        json_body,
        Some(("x-api-key", api_key)),
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    Ok(serde_json::from_slice(&bytes)?)
}

/// `GET /v1/mods/search`
pub(crate) async fn search_mods(
    api_key: &str,
    query: &str,
    game_version: Option<&str>,
    page: u32,
    page_size: u32,
    state: &State,
) -> crate::Result<(Vec<CfMod>, u32)> {
    let mut path = format!(
        "mods/search?gameId={MINECRAFT_GAME_ID}&searchFilter={}&index={}&pageSize={}",
        urlencoding::encode(query),
        page * page_size,
        page_size,
    );
    if let Some(game_version) = game_version {
        path.push_str(&format!(
            "&gameVersion={}",
            urlencoding::encode(game_version)
        ));
    }

    let response: CfSearchEnvelope =
        fetch_curseforge(Method::GET, &path, api_key, None, state).await?;

    Ok((response.data, response.pagination.total_count))
}

/// `GET /v1/mods/{modId}`
pub(crate) async fn get_mod(
    api_key: &str,
    mod_id: &str,
    state: &State,
) -> crate::Result<CfMod> {
    let path = format!("mods/{mod_id}");
    let response: CfDataEnvelope<CfMod> =
        fetch_curseforge(Method::GET, &path, api_key, None, state).await?;

    Ok(response.data)
}

/// `GET /v1/mods/{modId}/files`
pub(crate) async fn get_mod_files(
    api_key: &str,
    mod_id: &str,
    state: &State,
) -> crate::Result<Vec<CfFile>> {
    let path = format!("mods/{mod_id}/files");
    let response: CfDataEnvelope<Vec<CfFile>> =
        fetch_curseforge(Method::GET, &path, api_key, None, state).await?;

    Ok(response.data)
}

/// Looks up a single file by id via the batch `POST /v1/mods/files`
/// endpoint, which (unlike `GET /v1/mods/{modId}/files/{fileId}`) does not
/// require already knowing the parent mod id.
pub(crate) async fn get_file(
    api_key: &str,
    file_id: &str,
    state: &State,
) -> crate::Result<CfFile> {
    let file_id: i64 = file_id.parse().map_err(|_| {
        crate::ErrorKind::InputError(format!(
            "Invalid CurseForge file id: {file_id}"
        ))
    })?;
    let body = serde_json::json!({ "fileIds": [file_id] });
    let response: CfDataEnvelope<Vec<CfFile>> = fetch_curseforge(
        Method::POST,
        "mods/files",
        api_key,
        Some(body),
        state,
    )
    .await?;

    response.data.into_iter().next().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "CurseForge file {file_id} not found"
        ))
        .into()
    })
}
