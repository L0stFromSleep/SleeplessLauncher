//! Resolves the real Modrinth/CurseForge project each file in a hosted
//! server's content folders came from, by hash -- hosted servers don't
//! participate in the client `Instance` content-tracking system that would
//! otherwise record this at install time (see `hosting::install`'s module
//! doc), so every file the content page shows was previously rendered as a
//! bare, unidentified "Uploaded" file even when it was actually installed
//! straight from a modpack.
//!
//! Best-effort and keyed by relative path: a file with no match on either
//! provider (genuinely hand-uploaded, or simply unpublished/removed) is
//! just absent from the result map rather than failing the whole batch.

use super::install::CONTENT_DIRS;
use crate::state::cache::CachedEntry;
use crate::state::curseforge;
use crate::state::State;
use crate::util::fetch::sha1_async;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedContentProvider {
    Modrinth,
    CurseForge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedContentMetadata {
    pub provider: HostedContentProvider,
    pub project_id: String,
    pub version_id: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    /// A full external URL to the project's page -- Modrinth's own or
    /// CurseForge's, since hosted servers have no in-app project route for
    /// either the way client instance content does.
    pub project_url: String,
}

/// Lists every file under a hosted server's content directories, relative to
/// the server's own directory (e.g. `"mods/somejar.jar"`).
async fn list_content_files(
    server_id: &str,
    state: &State,
) -> Vec<(String, PathBuf)> {
    let server_dir = state.directories.hosted_server_dir(server_id);
    let mut files = Vec::new();
    for dir in CONTENT_DIRS {
        let Ok(mut read_dir) = tokio::fs::read_dir(server_dir.join(dir)).await
        else {
            continue;
        };
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            files.push((format!("{dir}/{name}"), entry.path()));
        }
    }
    files
}

pub async fn resolve_content_metadata(
    server_id: &str,
    state: &State,
) -> HashMap<String, HostedContentMetadata> {
    let files = list_content_files(server_id, state).await;

    let mut bytes_by_path = HashMap::new();
    for (relative, absolute) in &files {
        if let Ok(bytes) = tokio::fs::read(absolute).await {
            bytes_by_path.insert(relative.clone(), bytes::Bytes::from(bytes));
        }
    }

    let mut result = HashMap::new();
    resolve_modrinth(&bytes_by_path, &mut result, state).await;
    resolve_curseforge(&bytes_by_path, &mut result, state).await;
    result
}

async fn resolve_modrinth(
    bytes_by_path: &HashMap<String, bytes::Bytes>,
    result: &mut HashMap<String, HostedContentMetadata>,
    state: &State,
) {
    let mut sha1_by_path = HashMap::new();
    for (relative, bytes) in bytes_by_path {
        if let Ok(hash) = sha1_async(bytes.clone()).await {
            sha1_by_path.insert(relative.clone(), hash);
        }
    }
    if sha1_by_path.is_empty() {
        return;
    }

    let hashes =
        sha1_by_path.values().map(String::as_str).collect::<Vec<_>>();
    let Ok(files_info) = CachedEntry::get_file_many(
        &hashes,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    else {
        return;
    };
    let by_hash = files_info
        .into_iter()
        .map(|file| (file.hash.clone(), file))
        .collect::<HashMap<_, _>>();
    if by_hash.is_empty() {
        return;
    }

    let project_ids = by_hash
        .values()
        .map(|file| file.project_id.as_str())
        .collect::<Vec<_>>();
    let projects = CachedEntry::get_project_many(
        &project_ids,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    .unwrap_or_default();
    let projects_by_id = projects
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect::<HashMap<_, _>>();

    for (relative, hash) in &sha1_by_path {
        let Some(file_info) = by_hash.get(hash) else { continue };
        let Some(project) = projects_by_id.get(&file_info.project_id) else {
            continue;
        };
        result.insert(
            relative.clone(),
            HostedContentMetadata {
                provider: HostedContentProvider::Modrinth,
                project_id: file_info.project_id.clone(),
                version_id: Some(file_info.version_id.clone()),
                title: project.title.clone(),
                icon_url: project.icon_url.clone(),
                project_url: format!(
                    "https://modrinth.com/project/{}",
                    project.slug.clone().unwrap_or_else(|| project.id.clone())
                ),
            },
        );
    }
}

async fn resolve_curseforge(
    bytes_by_path: &HashMap<String, bytes::Bytes>,
    result: &mut HashMap<String, HostedContentMetadata>,
    state: &State,
) {
    let unmatched = bytes_by_path
        .iter()
        .filter(|(path, _)| !result.contains_key(*path))
        .collect::<Vec<_>>();
    if unmatched.is_empty() {
        return;
    }
    let Ok(api_key) = curseforge::api_key(state).await else {
        return;
    };

    let fingerprints_by_value = unmatched
        .iter()
        .map(|(path, bytes)| (curseforge::cf_fingerprint(bytes), (*path).clone()))
        .collect::<HashMap<_, _>>();
    let fingerprint_values =
        fingerprints_by_value.keys().copied().collect::<Vec<_>>();

    let mut matches = Vec::new();
    for chunk in fingerprint_values.chunks(
        crate::state::curseforge::client::GET_FINGERPRINT_MATCHES_MAX_BATCH_SIZE,
    ) {
        let Ok(chunk_matches) = crate::state::curseforge::client::get_fingerprint_matches(
            &api_key, chunk, state,
        )
        .await
        else {
            continue;
        };
        matches.extend(chunk_matches);
    }
    if matches.is_empty() {
        return;
    }

    let mod_ids =
        matches.iter().map(|m| m.file.mod_id).collect::<Vec<_>>();
    let mods = crate::state::curseforge::client::get_mods(
        &api_key, &mod_ids, state,
    )
    .await
    .unwrap_or_default();
    let mods_by_id =
        mods.into_iter().map(|m| (m.id, m)).collect::<HashMap<_, _>>();

    for m in matches {
        let Some(fingerprint) = m.file.file_fingerprint else { continue };
        let Some(path) = fingerprints_by_value.get(&fingerprint) else {
            continue;
        };
        let Some(cf_mod) = mods_by_id.get(&m.file.mod_id) else { continue };

        let project_url = cf_mod
            .links
            .as_ref()
            .and_then(|links| links.website_url.clone())
            .unwrap_or_else(|| {
                format!(
                    "https://www.curseforge.com/minecraft/mc-mods/{}",
                    cf_mod.id
                )
            });

        result.insert(
            path.clone(),
            HostedContentMetadata {
                provider: HostedContentProvider::CurseForge,
                project_id: cf_mod.id.to_string(),
                version_id: Some(m.file.id.to_string()),
                title: cf_mod.name.clone(),
                icon_url: cf_mod.logo.as_ref().map(|logo| logo.url.clone()),
                project_url,
            },
        );
    }
}
