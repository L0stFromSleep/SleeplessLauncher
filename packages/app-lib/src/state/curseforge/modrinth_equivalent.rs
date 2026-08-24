//! Finds a Modrinth project+version that is an *exact* match (name, author,
//! and version) for a CurseForge mod file, so CurseForge modpack installers
//! can prefer downloading the Modrinth-hosted copy when one exists (a direct
//! download straight from Modrinth's CDN, rather than routing through
//! CurseForge, and avoiding CurseForge's "third-party downloads disabled"
//! restriction some authors set).
//!
//! Deliberately conservative: only ever swaps a file when the Modrinth
//! project's title matches the CurseForge mod's name, at least one of the
//! CurseForge mod's authors matches the Modrinth project's listed author,
//! and a Modrinth version exists whose `version_number` or `name` matches
//! the CurseForge file's `displayName` exactly (case-insensitive). No fuzzy
//! matching -- a near-miss just means the CurseForge file is used as normal.

use super::models::CfFile;
use crate::state::{CachedEntry, State};

#[derive(Clone)]
pub struct ModrinthEquivalentFile {
    pub url: String,
    pub filename: String,
    pub sha1: Option<String>,
    pub file_length: u64,
    pub project_id: String,
    pub version_id: String,
}

/// Looks for an exact Modrinth equivalent of a single CurseForge file.
/// `mod_name`/`authors` describe the parent CurseForge mod (not the file);
/// `authors` should be every author name CurseForge lists for that mod.
pub async fn find_exact_match(
    mod_name: &str,
    authors: &[String],
    cf_file: &CfFile,
    state: &State,
) -> crate::Result<Option<ModrinthEquivalentFile>> {
    let search_key =
        format!("?query={}&limit=10", urlencoding::encode(mod_name));

    let search = match CachedEntry::get_search_results(
        &search_key,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(search) => search,
        Err(err) => {
            tracing::debug!(
                "Modrinth search failed while looking for an equivalent of \
                 CurseForge mod {mod_name:?}: {err}"
            );
            return Ok(None);
        }
    };
    let Some(search) = search else {
        return Ok(None);
    };

    let Some(hit) = search.result.hits.iter().find(|hit| {
        hit.title.eq_ignore_ascii_case(mod_name)
            && authors
                .iter()
                .any(|author| hit.author.eq_ignore_ascii_case(author))
    }) else {
        return Ok(None);
    };

    let versions = match CachedEntry::get_project_versions(
        &hit.project_id,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await
    {
        Ok(Some(versions)) => versions,
        Ok(None) => return Ok(None),
        Err(err) => {
            tracing::debug!(
                "Failed to fetch Modrinth versions for project {} while \
                 looking for an equivalent of CurseForge mod {mod_name:?}: {err}",
                hit.project_id,
            );
            return Ok(None);
        }
    };

    let file_version_label = cf_file.display_name.trim();
    let Some(version) = versions.iter().find(|version| {
        version.version_number.trim().eq_ignore_ascii_case(file_version_label)
            || version.name.trim().eq_ignore_ascii_case(file_version_label)
    }) else {
        return Ok(None);
    };

    let Some(file) = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
    else {
        return Ok(None);
    };

    Ok(Some(ModrinthEquivalentFile {
        url: file.url.clone(),
        filename: file.filename.clone(),
        sha1: file.hashes.get("sha1").cloned(),
        file_length: file.size as u64,
        project_id: hit.project_id.clone(),
        version_id: version.id.clone(),
    }))
}

/// Convenience used by callers that only have the raw `mods/files` batch
/// response and still need each file's parent mod name/authors -- resolves
/// every distinct mod id referenced by `files` in as few CurseForge API
/// calls as possible, then looks up a Modrinth-exact match per file.
/// Returns a map from CurseForge file id to its Modrinth replacement, only
/// containing entries where an exact match was actually found.
pub async fn find_exact_matches_for_files(
    files: &[CfFile],
    state: &State,
) -> crate::Result<std::collections::HashMap<i64, ModrinthEquivalentFile>> {
    let mut mod_ids: Vec<i64> =
        files.iter().map(|file| file.mod_id).collect();
    mod_ids.sort_unstable();
    mod_ids.dedup();

    if mod_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let api_key = super::api_key(state).await?;
    let mut mods_by_id = std::collections::HashMap::new();
    for chunk in
        mod_ids.chunks(super::client::GET_MODS_MAX_BATCH_SIZE)
    {
        match super::client::get_mods(&api_key, chunk, state).await {
            Ok(mods) => {
                for cf_mod in mods {
                    mods_by_id.insert(cf_mod.id, cf_mod);
                }
            }
            Err(err) => {
                tracing::debug!(
                    "Failed to batch-fetch CurseForge mod metadata for \
                     Modrinth-equivalent matching: {err}"
                );
                return Ok(std::collections::HashMap::new());
            }
        }
    }

    let mut matches = std::collections::HashMap::new();
    for file in files {
        let Some(cf_mod) = mods_by_id.get(&file.mod_id) else {
            continue;
        };
        let authors = cf_mod
            .authors
            .iter()
            .map(|author| author.name.clone())
            .collect::<Vec<_>>();
        if let Some(replacement) =
            find_exact_match(&cf_mod.name, &authors, file, state).await?
        {
            matches.insert(file.id, replacement);
        }
    }

    Ok(matches)
}
