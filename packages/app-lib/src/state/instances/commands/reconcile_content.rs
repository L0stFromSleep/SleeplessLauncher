//! Reconciles content files that have no known provider/project (shown to
//! the user as "Uploaded") against both Modrinth (by sha1, reusing the same
//! cache lookup the content list already does for display) and CurseForge
//! (by `murmur2` fingerprint, which nothing else in this codebase does yet).
//! Unlike the read-only display lookup in `list_content.rs`, matches found
//! here are persisted via `record_project_file`, so a file only needs to be
//! resolved once.

use std::collections::HashMap;

use crate::State;
use crate::state::curseforge::{cf_fingerprint, client as curseforge_client};
use crate::state::instances::adapters::sqlite::content_rows;
use crate::state::instances::model::InstanceFile;
use crate::state::instances::{ContentProvider, ContentSourceKind};
use crate::state::{CacheBehaviour, CachedEntry, ProjectType};

use super::sync_content_files::{project_type_for_file, sync_content_files};
use super::{record_project_file, resolve_content_scope};

#[derive(Debug, Default)]
pub(crate) struct ReconcileSummary {
    pub modrinth_matched: usize,
    pub curseforge_matched: usize,
}

pub(crate) async fn reconcile_unresolved_content(
    instance_id: &str,
    state: &State,
) -> crate::Result<ReconcileSummary> {
    let scope = resolve_content_scope(instance_id, None, state).await?;
    let files = sync_content_files(instance_id, state).await?;
    let entries = content_rows::get_content_entries(
        &scope.content_set_id,
        &state.pool,
    )
    .await?;
    let resolved_file_ids = entries
        .iter()
        .filter(|entry| entry.project_id.is_some())
        .filter_map(|entry| entry.file_id.as_deref())
        .collect::<std::collections::HashSet<_>>();

    let unresolved_files = files
        .into_iter()
        .filter(|file| !file.missing && !resolved_file_ids.contains(file.id.as_str()))
        .filter_map(|file| {
            let project_type = project_type_for_file(&file)?;
            Some((file, project_type))
        })
        .collect::<Vec<_>>();

    let mut summary = ReconcileSummary::default();
    if unresolved_files.is_empty() {
        return Ok(summary);
    }

    // Modrinth pass -- sha1 hashes are already cached per-file, so this is a
    // pure lookup with no file I/O, mirroring `content_projects_for_scope`'s
    // display-only lookup in list_content.rs.
    let hashes = unresolved_files
        .iter()
        .map(|(file, _)| file.sha1.as_str())
        .collect::<Vec<_>>();
    let modrinth_matches = CachedEntry::get_file_many(
        &hashes,
        Some(CacheBehaviour::MustRevalidate),
        &state.pool,
        &state.api_semaphore,
    )
    .await
    .unwrap_or_default();
    let modrinth_by_hash = modrinth_matches
        .iter()
        .map(|file| (file.hash.as_str(), file))
        .collect::<HashMap<_, _>>();

    let mut still_unresolved = Vec::new();
    for (file, project_type) in unresolved_files {
        if let Some(matched) = modrinth_by_hash.get(file.sha1.as_str()) {
            record_project_file(
                instance_id,
                &file.relative_path,
                &file.sha1,
                file.size,
                project_type,
                ContentSourceKind::Local,
                ContentProvider::Modrinth,
                Some(&matched.project_id),
                Some(&matched.version_id),
                state,
            )
            .await?;
            summary.modrinth_matched += 1;
        } else {
            still_unresolved.push((file, project_type));
        }
    }

    // CurseForge pass, only for files Modrinth didn't resolve. Silently
    // skipped (not an error) if no CurseForge API key is configured -- this
    // must never fail a plain content-page refresh for users who don't use
    // CurseForge at all.
    let Ok(api_key) = crate::state::curseforge::api_key(state).await else {
        return Ok(summary);
    };
    if still_unresolved.is_empty() {
        return Ok(summary);
    }

    let instance_full_path =
        state.directories.instances_dir().join(&scope.instance.path);
    let mut file_by_fingerprint: HashMap<u32, (InstanceFile, ProjectType)> =
        HashMap::new();
    for (file, project_type) in still_unresolved {
        let full_path = instance_full_path.join(&file.relative_path);
        let Ok(bytes) = crate::util::io::read(&full_path).await else {
            continue;
        };
        file_by_fingerprint.insert(cf_fingerprint(&bytes), (file, project_type));
    }

    let fingerprints =
        file_by_fingerprint.keys().copied().collect::<Vec<_>>();
    for chunk in fingerprints.chunks(
        curseforge_client::GET_FINGERPRINT_MATCHES_MAX_BATCH_SIZE,
    ) {
        let matches = match curseforge_client::get_fingerprint_matches(
            &api_key, chunk, state,
        )
        .await
        {
            Ok(matches) => matches,
            Err(err) => {
                tracing::warn!(
                    "Failed to reconcile instance {instance_id} content \
                     against CurseForge fingerprints: {err}"
                );
                continue;
            }
        };

        for cf_match in matches {
            let Some(fingerprint) = cf_match.file.file_fingerprint else {
                continue;
            };
            let Some((file, project_type)) =
                file_by_fingerprint.get(&fingerprint)
            else {
                continue;
            };

            record_project_file(
                instance_id,
                &file.relative_path,
                &file.sha1,
                file.size,
                *project_type,
                ContentSourceKind::Local,
                ContentProvider::CurseForge,
                Some(&cf_match.file.mod_id.to_string()),
                Some(&cf_match.file.id.to_string()),
                state,
            )
            .await?;
            summary.curseforge_matched += 1;
        }
    }

    Ok(summary)
}
