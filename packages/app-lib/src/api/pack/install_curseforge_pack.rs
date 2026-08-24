//! Installs a CurseForge modpack zip (`manifest.json` + an overrides
//! folder) as a new instance, mirroring `install_mrpack.rs`'s role for
//! Modrinth's `.mrpack` format. The two formats differ enough (CurseForge
//! references mod files by an opaque `fileID` resolved through the
//! CurseForge API rather than embedding direct download URLs/hashes) that
//! this is a separate installer rather than a shared code path, though it
//! reuses the same zip-reading (`PackZipReader`) and instance-launch
//! plumbing.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures::StreamExt;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::Deserialize;

use crate::State;
use crate::data::ProjectType;
use crate::event::emit::loading_try_for_each_concurrent;
use crate::install::{
    InstallErrorContext, InstallJobEventKind, InstallPhaseDetails,
    InstallPhaseId, InstallProgress, InstallProgressReporter,
    InstallProgressSecondary,
};
use crate::state::curseforge::CfFile;
use crate::state::instances::{ContentProvider, ContentSourceKind};
use crate::state::{
    AppliedContentSetPatch, EditInstance, InstanceInstallStage, InstanceLink,
    ModLoader,
};
use crate::util::fetch::{self, DownloadMeta, DownloadReason};
use crate::util::io;

use super::install_from::CreatePack;
use super::install_mrpack::PackZipReader;

const MODPACK_CONTENT_DOWNLOAD_CONCURRENCY: usize = 4;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifest {
    minecraft: CurseForgeMinecraft,
    name: String,
    #[serde(default)]
    overrides: Option<String>,
    files: Vec<CurseForgeManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CurseForgeModLoader>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifestFile {
    #[serde(rename = "projectID")]
    #[allow(dead_code)]
    project_id: i64,
    #[serde(rename = "fileID")]
    file_id: i64,
}

/// Parses CurseForge's `modLoaders[].id` (e.g. `"forge-47.2.0"`,
/// `"fabric-0.16.9"`) into a launcher [`ModLoader`] and loader version,
/// preferring the entry marked `primary`. Mirrors the equivalent parsing in
/// `api/pack/import/curseforge.rs` for locally-imported CurseForge App
/// instances, extended to cover NeoForge/Quilt as well.
fn parse_mod_loader(
    mod_loaders: &[CurseForgeModLoader],
) -> (ModLoader, Option<String>) {
    let Some(entry) = mod_loaders.iter().find(|l| l.primary).or(mod_loaders.first())
    else {
        return (ModLoader::Vanilla, None);
    };

    match entry.id.split_once('-') {
        Some(("forge", version)) => {
            (ModLoader::Forge, Some(version.to_string()))
        }
        Some(("neoforge", version)) => {
            (ModLoader::NeoForge, Some(version.to_string()))
        }
        Some(("fabric", version)) => {
            (ModLoader::Fabric, Some(version.to_string()))
        }
        Some(("quilt", version)) => {
            (ModLoader::Quilt, Some(version.to_string()))
        }
        _ => (ModLoader::Vanilla, None),
    }
}

#[derive(Clone)]
struct CfContentInstallContext {
    instance_id: String,
    instance_path: String,
    instance_full_path: PathBuf,
    download_meta: DownloadMeta,
    mod_id: String,
    file_id: String,
    reporter: InstallProgressReporter,
    modpack_details: InstallPhaseDetails,
    content_progress: Arc<AtomicU64>,
    content_bytes_progress: Arc<AtomicU64>,
    num_files: usize,
    content_total_bytes: u64,
}

impl CfContentInstallContext {
    async fn mark_downloaded(
        &self,
        file_size: u64,
        event: InstallJobEventKind,
    ) -> crate::Result<()> {
        let current = self.content_progress.fetch_add(1, Ordering::Relaxed) + 1;
        let current_bytes = self
            .content_bytes_progress
            .fetch_add(file_size, Ordering::Relaxed)
            + file_size;

        self.reporter
            .update_with_events(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current,
                    total: self.num_files as u64,
                    secondary: (self.content_total_bytes > 0).then_some(
                        InstallProgressSecondary {
                            current: current_bytes
                                .min(self.content_total_bytes),
                            total: self.content_total_bytes,
                        },
                    ),
                }),
                self.modpack_details.clone(),
                vec![event],
            )
            .await
    }
}

pub(crate) async fn install_zipped_curseforge_pack_with_reporter(
    create_pack: CreatePack,
    reason: DownloadReason,
    reporter: InstallProgressReporter,
) -> crate::Result<String> {
    let state = &State::get().await?;

    let file = create_pack.file;
    let description = create_pack.description.clone();
    let instance_id = create_pack.description.instance_id;
    let mod_id = description.project_id.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Missing CurseForge mod id for modpack install".to_string(),
        )
    })?;
    let file_id = description.version_id.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(
            "Missing CurseForge file id for modpack install".to_string(),
        )
    })?;

    let modpack_details = InstallPhaseDetails::Modpack {
        project_id: Some(mod_id.clone()),
        version_id: Some(file_id.clone()),
        title: description.override_title.clone(),
    };
    reporter
        .update(
            InstallPhaseId::ReadingPackManifest,
            None,
            modpack_details.clone(),
        )
        .await?;

    let mut zip_reader = PackZipReader::new(&file).await?;
    let Some(manifest_idx) = zip_reader
        .file()
        .entries()
        .iter()
        .position(|entry| matches!(entry.filename().as_str(), Ok("manifest.json")))
    else {
        return Err(crate::Error::from(crate::ErrorKind::InputError(
            "No manifest.json found in CurseForge modpack".to_string(),
        )));
    };
    let manifest_json = zip_reader.read_entry_to_string(manifest_idx).await?;
    let manifest: CurseForgeManifest = serde_json::from_str(&manifest_json)?;
    let overrides_folder =
        manifest.overrides.clone().unwrap_or_else(|| "overrides".to_string());

    reporter
        .update(InstallPhaseId::ResolvingPack, None, modpack_details.clone())
        .await?;

    let (mod_loader, loader_version_id) =
        parse_mod_loader(&manifest.minecraft.mod_loaders);
    let loader_version = if mod_loader != ModLoader::Vanilla {
        crate::launcher::get_loader_version_from_profile(
            &manifest.minecraft.version,
            mod_loader,
            loader_version_id.as_deref(),
        )
        .await?
    } else {
        None
    };

    let instance_full_path =
        crate::api::instance::get_full_path(&instance_id).await?;
    crate::api::instance::edit(
        &instance_id,
        EditInstance {
            install_stage: Some(InstanceInstallStage::PackInstalling),
            name: Some(
                description
                    .override_title
                    .clone()
                    .unwrap_or_else(|| manifest.name.clone()),
            ),
            icon_path: description
                .icon
                .as_ref()
                .map(|icon| Some(icon.to_string_lossy().to_string())),
            link: Some(InstanceLink::CurseForgeModpack {
                mod_id: mod_id.clone(),
                file_id: file_id.clone(),
            }),
            content_set_patch: Some(AppliedContentSetPatch {
                source_kind: Some(ContentSourceKind::CurseForgeModpack),
                game_version: Some(manifest.minecraft.version.clone()),
                protocol_version: Some(None),
                loader: Some(mod_loader),
                loader_version: Some(loader_version.map(|version| version.id)),
            }),
            ..EditInstance::default()
        },
    )
    .await?;

    let metadata =
        crate::api::instance::get(&instance_id)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Unknown instance {instance_id}"
                ))
            })?;
    let instance_path = metadata.instance.path.clone();
    let download_meta = DownloadMeta {
        reason,
        game_version: metadata.applied_content_set.game_version.clone(),
        loader: metadata.applied_content_set.loader.as_str().to_string(),
        dependent_on: Some(file_id.clone()),
    };

    // Resolve every referenced mod file's download URL/size via CurseForge's
    // batch files endpoint, chunked since a large pack can reference
    // thousands of files.
    let api_key = crate::state::curseforge::api_key(state).await?;
    let file_ids = manifest
        .files
        .iter()
        .map(|entry| entry.file_id)
        .collect::<Vec<_>>();
    let mut resolved_files: HashMap<i64, CfFile> = HashMap::new();
    for chunk in
        file_ids.chunks(crate::state::curseforge::client::GET_FILES_MAX_BATCH_SIZE)
    {
        let files =
            crate::state::curseforge::client::get_files(&api_key, chunk, state)
                .await?;
        resolved_files.extend(files.into_iter().map(|file| (file.id, file)));
    }

    // Prefer a Modrinth-hosted copy of each mod when one is an exact match
    // (name, author, and version) -- see
    // `state::curseforge::modrinth_equivalent` for why.
    let modrinth_equivalents = {
        let files = resolved_files.values().cloned().collect::<Vec<_>>();
        crate::state::curseforge::modrinth_equivalent::find_exact_matches_for_files(
            &files, state,
        )
        .await
        .unwrap_or_default()
    };

    let num_files = manifest.files.len();
    let content_total_bytes = manifest
        .files
        .iter()
        .filter_map(|entry| resolved_files.get(&entry.file_id))
        .map(|file| file.file_length)
        .sum::<u64>();
    reporter
        .update_with_events(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: num_files as u64,
                secondary: (content_total_bytes > 0).then_some(
                    InstallProgressSecondary {
                        current: 0,
                        total: content_total_bytes,
                    },
                ),
            }),
            modpack_details.clone(),
            vec![InstallJobEventKind::ContentDownloadStarted {
                files: num_files as u64,
                bytes: (content_total_bytes > 0).then_some(content_total_bytes),
            }],
        )
        .await?;

    let modrinth_equivalents = Arc::new(modrinth_equivalents);
    let content_context = CfContentInstallContext {
        instance_id: instance_id.clone(),
        instance_path: instance_path.clone(),
        instance_full_path: instance_full_path.clone(),
        download_meta,
        mod_id: mod_id.clone(),
        file_id: file_id.clone(),
        reporter: reporter.clone(),
        modpack_details: modpack_details.clone(),
        content_progress: Arc::new(AtomicU64::new(0)),
        content_bytes_progress: Arc::new(AtomicU64::new(0)),
        num_files,
        content_total_bytes,
    };

    loading_try_for_each_concurrent(
        futures::stream::iter(manifest.files)
            .map(Ok::<CurseForgeManifestFile, crate::Error>),
        Some(MODPACK_CONTENT_DOWNLOAD_CONCURRENCY),
        None,
        70.0,
        num_files,
        None,
        |entry| {
            let content_context = content_context.clone();
            let resolved = resolved_files.get(&entry.file_id).cloned();
            let modrinth_replacement =
                modrinth_equivalents.get(&entry.file_id).cloned();
            async move {
                let Some(cf_file) = resolved else {
                    content_context
                        .mark_downloaded(
                            0,
                            InstallJobEventKind::ContentFileSkipped {
                                path: format!("mods/(file {})", entry.file_id),
                                reason: "not found on CurseForge".to_string(),
                            },
                        )
                        .await?;
                    return Ok(());
                };

                // Prefer an exact Modrinth equivalent when one was found --
                // downloads directly from Modrinth instead of CurseForge,
                // and sidesteps CurseForge's "third-party downloads
                // disabled" restriction some authors set.
                let (download_url, file_name, file_length, sha1, provider, provider_project_id, provider_version_id) =
                    if let Some(replacement) = &modrinth_replacement {
                        (
                            replacement.url.clone(),
                            replacement.filename.clone(),
                            replacement.file_length,
                            replacement.sha1.clone(),
                            ContentProvider::Modrinth,
                            replacement.project_id.clone(),
                            replacement.version_id.clone(),
                        )
                    } else {
                        let Some(download_url) = cf_file.download_url.clone() else {
                            content_context
                                .mark_downloaded(
                                    cf_file.file_length,
                                    InstallJobEventKind::ContentFileSkipped {
                                        path: format!("mods/{}", cf_file.file_name),
                                        reason:
                                            "author disabled third-party downloads"
                                                .to_string(),
                                    },
                                )
                                .await?;
                            return Ok(());
                        };
                        (
                            download_url,
                            cf_file.file_name.clone(),
                            cf_file.file_length,
                            cf_file.sha1(),
                            ContentProvider::CurseForge,
                            cf_file.mod_id.to_string(),
                            cf_file.id.to_string(),
                        )
                    };

                let project_path = format!("mods/{}", file_name);
                let target_path =
                    content_context.instance_full_path.join(&project_path);

                let error_context =
                    InstallErrorContext::new("download modpack content file")
                        .project_id(content_context.mod_id.clone())
                        .version_id(content_context.file_id.clone())
                        .file_path(project_path.clone())
                        .target_path(target_path.display().to_string())
                        .urls(vec![download_url.clone()])
                        .maybe_expected_hash(sha1.clone())
                        .expected_size(file_length)
                        .build();
                content_context
                    .reporter
                    .set_transient_context(error_context.clone())
                    .await?;

                let bytes = match fetch::fetch(
                    &download_url,
                    sha1.as_deref(),
                    Some(&content_context.download_meta),
                    None,
                    &state.fetch_semaphore,
                    &state.pool,
                )
                .await
                {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        content_context
                            .reporter
                            .persist_failure_context(error_context)
                            .await;
                        return Err(error);
                    }
                };
                let downloaded_bytes = bytes.len() as u64;

                content_context
                    .reporter
                    .preserve_failure_context(
                        error_context.clone(),
                        fetch::write(&target_path, &bytes, &state.io_semaphore)
                            .await,
                    )
                    .await?;
                let modified_at_ns = crate::state::file_modified_at_ns(
                    &io::metadata(&target_path).await?,
                )?;

                {
                    let _permit = state.install_db_semaphore.acquire().await?;
                    content_context
                        .reporter
                        .preserve_failure_context(
                            error_context.clone(),
                            crate::state::cache_file_hash(
                                bytes.clone(),
                                &content_context.instance_path,
                                &project_path,
                                modified_at_ns,
                                sha1.as_deref(),
                                Some(ProjectType::Mod),
                                None,
                                &state.pool,
                            )
                            .await,
                        )
                        .await?;

                    if let Some(hash) = sha1 {
                        content_context
                            .reporter
                            .preserve_failure_context(
                                error_context,
                                crate::state::instances::commands::record_project_file(
                                    &content_context.instance_id,
                                    &project_path,
                                    &hash,
                                    downloaded_bytes,
                                    ProjectType::Mod,
                                    ContentSourceKind::CurseForgeModpack,
                                    provider,
                                    Some(&provider_project_id),
                                    Some(&provider_version_id),
                                    state,
                                )
                                .await,
                            )
                            .await?;
                    }
                }

                content_context
                    .mark_downloaded(
                        downloaded_bytes,
                        InstallJobEventKind::ContentFileCompleted {
                            path: project_path,
                            bytes: downloaded_bytes,
                        },
                    )
                    .await?;
                Ok(())
            }
        },
    )
    .await?;

    // Extract the overrides folder (config, resource packs, options, etc.)
    // over the fresh instance directory.
    let override_prefix = format!("{overrides_folder}/");
    let override_entries = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let filename = entry.filename().as_str().ok()?;
            (filename.starts_with(&override_prefix) && !filename.ends_with('/'))
                .then(|| (index, filename.to_string()))
        })
        .collect::<Vec<_>>();

    reporter
        .update(
            InstallPhaseId::ExtractingOverrides,
            None,
            modpack_details.clone(),
        )
        .await?;

    for (index, filename) in override_entries {
        let relative_override_path =
            SafeRelativeUtf8UnixPathBuf::try_from(filename.clone())?;
        let relative_override_path = relative_override_path
            .strip_prefix(overrides_folder.as_str())
            .map_err(|_| {
                crate::Error::from(crate::ErrorKind::OtherError(format!(
                    "Failed to strip overrides prefix from {filename}"
                )))
            })?;
        let path =
            instance_full_path.join(relative_override_path.as_str());
        let (size, hash) = zip_reader
            .extract_entry(index, &path, &state.io_semaphore, None)
            .await?;
        let modified_at_ns =
            crate::state::file_modified_at_ns(&io::metadata(&path).await?)?;

        crate::state::cache_file_hash_metadata(
            &instance_path,
            relative_override_path.as_str(),
            size,
            modified_at_ns,
            hash.clone(),
            ProjectType::get_from_parent_folder(relative_override_path.as_str()),
            None,
            &state.pool,
        )
        .await?;

        if let Some(project_type) =
            ProjectType::get_from_parent_folder(relative_override_path.as_str())
        {
            crate::state::instances::commands::record_project_file(
                &instance_id,
                relative_override_path.as_str(),
                &hash,
                size,
                project_type,
                ContentSourceKind::CurseForgeModpack,
                ContentProvider::CurseForge,
                None,
                None,
                state,
            )
            .await?;
        }
    }

    if description.icon.is_none() {
        let potential_icon = instance_full_path.join("icon.png");
        if potential_icon.exists() {
            crate::api::instance::edit_icon(&instance_id, Some(&potential_icon))
                .await?;
        }
    }

    // Overrides files (config, resource packs, etc. -- everything outside
    // `files[]`) got no project/version above -- try to identify them
    // against both providers now so they don't show as "Uploaded". Must
    // never fail the install itself.
    if let Err(err) =
        crate::state::reconcile_unresolved_content(&instance_id, state).await
    {
        tracing::warn!(
            "Failed to reconcile unresolved content for instance {instance_id} \
             after CurseForge modpack install: {err}"
        );
    }

    crate::launcher::install_minecraft_for_instance_id_with_reporter(
        &instance_id,
        false,
        Some(reporter.clone()),
    )
    .await?;
    reporter.clear_context().await?;

    Ok(instance_id)
}
