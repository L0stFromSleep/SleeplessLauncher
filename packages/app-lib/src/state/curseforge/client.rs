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
    CfDataEnvelope, CfFile, CfFingerprintMatch, CfFingerprintMatchesEnvelope,
    CfMod, CfSearchEnvelope, MINECRAFT_GAME_ID,
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
///
/// `sort_field` is CurseForge's `SearchSortField` enum (2 = Popularity,
/// 3 = LastUpdated, 6 = TotalDownloads, 11 = ReleasedDate, ...) -- without
/// it, CurseForge returns results in its own default order (roughly
/// "Featured"), which is *not* sorted by downloads/date/etc. at all. Since
/// only a `page_size`-sized page is ever fetched, omitting this means the
/// handful of mods fetched (and then sorted client-side) are the wrong ones
/// entirely, not just wrongly ordered.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn search_mods(
    api_key: &str,
    query: &str,
    game_version: Option<&str>,
    class_id: Option<i64>,
    sort_field: Option<u32>,
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
    if let Some(class_id) = class_id {
        path.push_str(&format!("&classId={class_id}"));
    }
    if let Some(sort_field) = sort_field {
        path.push_str(&format!("&sortField={sort_field}&sortOrder=desc"));
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

/// `GET /v1/mods/{modId}/description`
pub(crate) async fn get_mod_description(
    api_key: &str,
    mod_id: &str,
    state: &State,
) -> crate::Result<String> {
    let path = format!("mods/{mod_id}/description");
    let response: CfDataEnvelope<String> =
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
    let files = get_files(api_key, &[file_id], state).await?;

    files.into_iter().next().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "CurseForge file {file_id} not found"
        ))
        .into()
    })
}

/// `POST /v1/mods`, resolving many mod ids (e.g. every distinct mod
/// referenced by a modpack manifest) in one request -- avoids one
/// `GET /v1/mods/{modId}` round-trip per mod when we just need
/// name/author metadata (e.g. for the Modrinth-equivalent lookup in
/// `super::modrinth_equivalent`).
pub(crate) const GET_MODS_MAX_BATCH_SIZE: usize = 500;

pub(crate) async fn get_mods(
    api_key: &str,
    mod_ids: &[i64],
    state: &State,
) -> crate::Result<Vec<CfMod>> {
    if mod_ids.is_empty() {
        return Ok(Vec::new());
    }

    let body = serde_json::json!({ "modIds": mod_ids });
    let response: CfDataEnvelope<Vec<CfMod>> =
        fetch_curseforge(Method::POST, "mods", api_key, Some(body), state)
            .await?;

    Ok(response.data)
}

/// `POST /v1/mods/files`, resolving many file ids (e.g. every mod file
/// referenced by a modpack manifest) in one request. CurseForge doesn't
/// document a hard cap on `fileIds` length for this endpoint; this is a
/// conservative chunk size for callers with very large modpacks, not a
/// documented API limit.
pub(crate) const GET_FILES_MAX_BATCH_SIZE: usize = 500;

pub(crate) async fn get_files(
    api_key: &str,
    file_ids: &[i64],
    state: &State,
) -> crate::Result<Vec<CfFile>> {
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }

    let body = serde_json::json!({ "fileIds": file_ids });
    let response: CfDataEnvelope<Vec<CfFile>> = fetch_curseforge(
        Method::POST,
        "mods/files",
        api_key,
        Some(body),
        state,
    )
    .await?;

    Ok(response.data)
}

/// `POST /v1/fingerprints`, resolving many file fingerprints (see
/// `super::models::cf_fingerprint`) to their exact CurseForge file matches
/// in one request. Same conservative per-request chunk size as
/// [`get_files`] -- CurseForge doesn't document a hard cap here either.
pub(crate) const GET_FINGERPRINT_MATCHES_MAX_BATCH_SIZE: usize = 500;

pub(crate) async fn get_fingerprint_matches(
    api_key: &str,
    fingerprints: &[u32],
    state: &State,
) -> crate::Result<Vec<CfFingerprintMatch>> {
    if fingerprints.is_empty() {
        return Ok(Vec::new());
    }

    let body = serde_json::json!({ "fingerprints": fingerprints });
    let response: CfFingerprintMatchesEnvelope = fetch_curseforge(
        Method::POST,
        "fingerprints",
        api_key,
        Some(body),
        state,
    )
    .await?;

    Ok(response.data.exact_matches)
}
