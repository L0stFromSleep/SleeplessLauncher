//! Downloads a server-only copy of a modpack (or sets up a plain
//! vanilla/loader server with no modpack at all) into a hosted server's own
//! directory, then hands off to `launcher::server_install` to acquire the
//! actual server loader jar/run script.
//!
//! Deliberately self-contained rather than reusing the client pack
//! installers (`api::pack::install_mrpack`, `api::pack::install_curseforge_pack`)
//! -- those are tightly coupled to the client `Instance`/content-set
//! tracking system, which hosted servers don't participate in, and
//! threading a "server vs client" mode through that already-complex,
//! must-not-regress code path was judged riskier than a smaller focused
//! implementation here that reuses only the low-level pieces (zip reading,
//! manifest types, CurseForge batch file resolution).

use super::{
    HostedServer, HostedServerInstallStage, HostedServerProvider,
    NewHostedServer,
};
use crate::api::pack::install_from::{EnvType, PackFormat};
use crate::api::pack::install_mrpack::PackZipReader;
use crate::install::{
    InstallJobDisplay, InstallJobEventKind, InstallJobState, InstallJobStatus,
    InstallPhaseDetails, InstallPhaseId, InstallProgress, InstallProgressReporter,
    InstallRequest,
};
use crate::launcher::server_install;
use crate::state::{CachedEntry, ModLoader, SideType, State};
use crate::util::fetch;
use crate::util::io;
use path_util::SafeRelativeUtf8UnixPathBuf;
use serde::Deserialize;
use std::path::Path;
use uuid::Uuid;

pub enum HostedServerSource {
    Vanilla {
        game_version: String,
    },
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    CurseForgeModpack {
        mod_id: String,
        file_id: String,
    },
    /// A `.mrpack` or CurseForge modpack `.zip` already on disk, picked via
    /// a native file dialog rather than searched for.
    LocalModpackFile {
        path: std::path::PathBuf,
    },
}

pub async fn create_and_install_hosted_server(
    name: String,
    source: HostedServerSource,
    loader_override: Option<ModLoader>,
    state: &State,
) -> crate::Result<HostedServer> {
    let (game_version, loader, loader_version, provider, project_id, version_id) =
        match &source {
            HostedServerSource::Vanilla { game_version } => (
                game_version.clone(),
                loader_override.unwrap_or(ModLoader::Vanilla),
                None,
                HostedServerProvider::Vanilla,
                None,
                None,
            ),
            HostedServerSource::ModrinthModpack { project_id, version_id } => {
                let version = CachedEntry::get_version(
                    version_id,
                    None,
                    &state.pool,
                    &state.api_semaphore,
                )
                .await?
                .ok_or_else(|| {
                    crate::ErrorKind::InputError(format!(
                        "Unknown Modrinth version {version_id}"
                    ))
                })?;
                let loader = version
                    .loaders
                    .iter()
                    .find_map(|loader| match loader.as_str() {
                        "forge" => Some(ModLoader::Forge),
                        "neoforge" => Some(ModLoader::NeoForge),
                        "fabric" => Some(ModLoader::Fabric),
                        "quilt" => Some(ModLoader::Quilt),
                        _ => None,
                    })
                    .unwrap_or(ModLoader::Vanilla);
                let game_version = version
                    .game_versions
                    .first()
                    .cloned()
                    .ok_or_else(|| {
                        crate::ErrorKind::InputError(
                            "Modrinth version has no game versions"
                                .to_string(),
                        )
                    })?;
                (
                    game_version,
                    loader,
                    None,
                    HostedServerProvider::Modrinth,
                    Some(project_id.clone()),
                    Some(version_id.clone()),
                )
            }
            HostedServerSource::CurseForgeModpack { mod_id, file_id } => {
                let file =
                    crate::state::curseforge::get_file(file_id, state)
                        .await?;
                let _ = &file;
                // Loader/game version for CurseForge modpacks live in the
                // pack's manifest.json, not the file metadata -- resolved
                // below once the pack zip is downloaded.
                (
                    String::new(),
                    ModLoader::Vanilla,
                    None,
                    HostedServerProvider::CurseForge,
                    Some(mod_id.clone()),
                    Some(file_id.clone()),
                )
            }
            HostedServerSource::LocalModpackFile { path } => {
                // Game version/loader are resolved once the file is
                // actually read during install; the format sniff here is
                // just to record whether this looked like a Modrinth or
                // CurseForge pack.
                let provider = match sniff_local_modpack_format(path).await? {
                    LocalModpackFormat::Mrpack => HostedServerProvider::Modrinth,
                    LocalModpackFormat::CurseForge => {
                        HostedServerProvider::CurseForge
                    }
                };
                (String::new(), ModLoader::Vanilla, None, provider, None, None)
            }
        };

    let new_server = NewHostedServer {
        name,
        game_version,
        loader,
        loader_version,
        provider,
        project_id,
        version_id,
    };
    let server = HostedServer::create(new_server, state).await?;

    let reporter = begin_install_job(&server, state).await?;

    let result =
        install_hosted_server_files(&server, source, state, &reporter).await;

    match result {
        Ok(updated) => {
            let _ = reporter.succeed().await;
            Ok(updated)
        }
        Err(err) => {
            HostedServer::set_install_stage(
                &server.id,
                HostedServerInstallStage::Error,
                state,
            )
            .await?;
            let _ = reporter.fail("hosting_error", err.to_string()).await;
            Err(err)
        }
    }
}

/// Creates the `install_jobs` record and `InstallProgressReporter` that
/// drives the same download-progress notification client instance installs
/// show (the popup in `AppActionBar.vue`, backed by `AppEvent::InstallJob`)
/// -- hosted servers aren't a real `Instance`, so this replicates the first
/// half of `install::runner::start()` directly rather than going through
/// it (see `InstallRequest::CreateHostedServer`'s doc comment for why).
async fn begin_install_job(
    server: &HostedServer,
    state: &State,
) -> crate::Result<InstallProgressReporter> {
    let job_id = Uuid::new_v4();
    let request = InstallRequest::CreateHostedServer {
        server_id: server.id.clone(),
        name: server.name.clone(),
    };
    let mut job_state = InstallJobState::new(request);
    job_state.display = Some(InstallJobDisplay {
        title: server.name.clone(),
        icon: server.icon_path.clone(),
    });

    let record = crate::install::store::insert(
        job_id,
        &job_state,
        InstallJobStatus::Queued,
        state,
    )
    .await?;
    crate::install::events::emit_install_job(&record.snapshot()).await?;

    job_state.record_event(InstallJobEventKind::JobStarted);
    let record = crate::install::store::update_status_if(
        job_id,
        InstallJobStatus::Queued,
        InstallJobStatus::Running,
        &job_state,
        state,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::OtherError(
            "Hosted server install job disappeared before it could start"
                .to_string(),
        )
    })?;
    crate::install::events::emit_install_job(&record.snapshot()).await?;

    Ok(InstallProgressReporter::new(job_id, job_state))
}

async fn install_hosted_server_files(
    server: &HostedServer,
    source: HostedServerSource,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<HostedServer> {
    HostedServer::set_install_stage(
        &server.id,
        HostedServerInstallStage::Installing,
        state,
    )
    .await?;
    reporter
        .update(
            InstallPhaseId::PreparingInstance,
            None,
            InstallPhaseDetails::HostedServer {
                name: server.name.clone(),
            },
        )
        .await?;

    let server_dir = state.directories.hosted_server_dir(&server.id);
    io::create_dir_all(&server_dir).await?;

    let is_local_source =
        matches!(source, HostedServerSource::LocalModpackFile { .. });

    let (resolved_game_version, resolved_loader, resolved_loader_version) =
        match source {
            HostedServerSource::Vanilla { game_version } => {
                (game_version, server.loader, None)
            }
            HostedServerSource::ModrinthModpack { project_id, version_id } => {
                // Resolved from `project_id` alone -- doesn't need the pack
                // downloaded first, so this runs before content download
                // starts rather than after, and the server's icon shows up
                // in the list immediately instead of only once installed.
                if let Err(err) = set_hosted_server_icon_from_modrinth_project(
                    &server.id, &project_id, state, reporter,
                )
                .await
                {
                    tracing::warn!(
                        "Failed to set hosted server icon from Modrinth \
                         project {project_id}: {err}"
                    );
                }

                install_modrinth_pack_files(
                    &version_id, &server_dir, state, reporter, &server.id,
                    &server.name,
                )
                .await?
            }
            HostedServerSource::CurseForgeModpack { mod_id, file_id } => {
                if let Err(err) = set_hosted_server_icon_from_curseforge_mod(
                    &server.id, &mod_id, state, reporter,
                )
                .await
                {
                    tracing::warn!(
                        "Failed to set hosted server icon from CurseForge \
                         mod {mod_id}: {err}"
                    );
                }

                install_curseforge_pack_files(
                    &mod_id, &file_id, &server_dir, state, reporter,
                    &server.id, &server.name,
                )
                .await?
            }
            HostedServerSource::LocalModpackFile { path } => {
                let bytes = tokio::fs::read(&path)
                    .await
                    .map_err(|e| crate::util::io::IOError::with_path(e, &path))?;
                let bytes = bytes::Bytes::from(bytes);
                match sniff_local_modpack_format(&path).await? {
                    LocalModpackFormat::Mrpack => {
                        install_mrpack_bytes(
                            bytes,
                            &server_dir,
                            state,
                            reporter,
                            &server.id,
                            &server.name,
                        )
                        .await?
                    }
                    LocalModpackFormat::CurseForge => {
                        install_curseforge_pack_bytes(
                            bytes,
                            &server_dir,
                            state,
                            reporter,
                            &server.id,
                            &server.name,
                        )
                        .await?
                    }
                }
            }
        };

    // A local modpack's overrides may have extracted its own icon.png --
    // Modrinth/CurseForge modpacks already had their icon set above, from
    // the project/mod's real icon rather than whatever happens to be in the
    // overrides folder (some packs don't include one there at all).
    if is_local_source {
        let potential_icon = server_dir.join("icon.png");
        if potential_icon.exists() {
            HostedServer::set_icon_path(
                &server.id,
                Some(&potential_icon.to_string_lossy()),
                state,
            )
            .await?;
        }
    }

    let acquired = server_install::acquire_server_loader(
        &server_dir,
        &resolved_game_version,
        resolved_loader,
        resolved_loader_version.as_deref(),
        state,
        reporter,
    )
    .await?;

    let loader_str = resolved_loader.as_str();
    let install_stage_str = HostedServerInstallStage::Installed.as_str();
    let java_major_version = acquired.java_major_version as i64;
    sqlx::query!(
        "
        UPDATE hosted_servers
        SET game_version = $2, loader = $3, loader_version = $4, install_stage = $5, java_major_version = $6
        WHERE id = $1
        ",
        server.id,
        resolved_game_version,
        loader_str,
        resolved_loader_version,
        install_stage_str,
        java_major_version,
    )
    .execute(&state.pool)
    .await?;

    let installed_server =
        HostedServer::get(&server.id, state).await?.ok_or_else(|| {
            crate::ErrorKind::OtherError(
                "Hosted server disappeared during install".to_string(),
            )
        })?;

    // Writes a starter server.properties with our own defaults (including
    // the configured port) if the server jar hasn't already generated one
    // -- makes the port setting actually take effect on the first launch,
    // and gives the settings panel something real to edit immediately
    // instead of only after the server has been started once.
    if let Err(err) =
        super::properties::write_default_properties(&installed_server, state)
            .await
    {
        tracing::warn!(
            "Failed to write default server.properties for hosted server \
             {}: {err}",
            server.id
        );
    }

    Ok(installed_server)
}

/// Downloads a Modrinth `.mrpack`'s server-relevant files
/// (`env[Server] != Unsupported`) and extracts its overrides, skipping
/// `client-overrides/`. Returns the pack's resolved game version/loader.
async fn install_modrinth_pack_files(
    version_id: &str,
    server_dir: &Path,
    state: &State,
    reporter: &InstallProgressReporter,
    server_id: &str,
    server_name: &str,
) -> crate::Result<(String, ModLoader, Option<String>)> {
    let version = CachedEntry::get_version(
        version_id,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unknown Modrinth version {version_id}"
        ))
    })?;
    let file = version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Modrinth version has no files".to_string(),
            )
        })?;

    let pack_bytes = fetch::fetch(
        &file.url,
        file.hashes.get("sha1").map(|hash| hash.as_str()),
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    install_mrpack_bytes(pack_bytes, server_dir, state, reporter, server_id, server_name).await
}

async fn install_mrpack_bytes(
    pack_bytes: bytes::Bytes,
    server_dir: &Path,
    state: &State,
    reporter: &InstallProgressReporter,
    server_id: &str,
    server_name: &str,
) -> crate::Result<(String, ModLoader, Option<String>)> {
    let mut zip_reader =
        PackZipReader::new(&crate::api::pack::install_from::CreatePackFile::Bytes(
            pack_bytes,
        ))
        .await?;

    let manifest_idx = zip_reader
        .file()
        .entries()
        .iter()
        .position(|entry| {
            matches!(entry.filename().as_str(), Ok("modrinth.index.json"))
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No pack manifest found in mrpack".to_string(),
            )
        })?;
    let manifest = zip_reader.read_entry_to_string(manifest_idx).await?;
    let pack: PackFormat = serde_json::from_str(&manifest)?;

    let mut loader = ModLoader::Vanilla;
    let mut loader_version = None;
    let mut game_version = String::new();
    for (dependency, value) in &pack.dependencies {
        use crate::api::pack::install_from::PackDependency;
        match dependency {
            PackDependency::Forge => {
                loader = ModLoader::Forge;
                loader_version = Some(value.clone());
            }
            PackDependency::NeoForge => {
                loader = ModLoader::NeoForge;
                loader_version = Some(value.clone());
            }
            PackDependency::FabricLoader => {
                loader = ModLoader::Fabric;
                loader_version = Some(value.clone());
            }
            PackDependency::QuiltLoader => {
                loader = ModLoader::Quilt;
                loader_version = Some(value.clone());
            }
            PackDependency::Minecraft => game_version = value.clone(),
        }
    }
    if game_version.is_empty() {
        return Err(crate::ErrorKind::InputError(
            "Pack did not specify a Minecraft version".to_string(),
        )
        .into());
    }

    // Recorded as soon as the manifest reveals the real loader/game
    // version -- for a locally-imported pack (the only path where the
    // caller doesn't already know these upfront), the server would
    // otherwise show the `HostedServer::create`-time placeholder
    // (`ModLoader::Vanilla`) in the list for its entire install, even for a
    // heavily modded pack.
    if let Err(err) = HostedServer::set_resolved_loader(
        server_id,
        &game_version,
        loader,
        loader_version.as_deref(),
        state,
    )
    .await
    {
        tracing::warn!(
            "Failed to record resolved loader for hosted server \
             {server_id}: {err}"
        );
    }

    let downloadable_files = pack
        .files
        .iter()
        .filter(|file| {
            !matches!(
                file.env.as_ref().and_then(|env| env.get(&EnvType::Server)),
                Some(&SideType::Unsupported)
            ) && file.downloads.first().is_some()
        })
        .count() as u64;
    let details = InstallPhaseDetails::HostedServer {
        name: server_name.to_string(),
    };
    reporter
        .update(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: downloadable_files,
                secondary: None,
            }),
            details.clone(),
        )
        .await?;

    let mut downloaded_files = 0u64;
    for file in &pack.files {
        if let Some(env) = &file.env
            && env.get(&EnvType::Server) == Some(&SideType::Unsupported)
        {
            continue;
        }
        let Some(download_url) = file.downloads.first() else {
            continue;
        };
        let sha1 = file
            .hashes
            .iter()
            .find_map(|(hash, value)| {
                matches!(
                    hash,
                    crate::api::pack::install_from::PackFileHash::Sha1
                )
                .then_some(value.as_str())
            });
        let bytes = fetch::fetch(
            download_url,
            sha1,
            None,
            None,
            &state.fetch_semaphore,
            &state.pool,
        )
        .await?;
        let target = server_dir.join(file.path.as_str());
        if let Some(parent) = target.parent() {
            io::create_dir_all(parent).await?;
        }
        io::write(&target, &bytes).await?;

        downloaded_files += 1;
        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: downloaded_files,
                    total: downloadable_files,
                    secondary: None,
                }),
                details.clone(),
            )
            .await?;
    }

    let override_entries = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let path = entry.filename().as_str().ok()?;
            if path.starts_with("client-overrides/") || path.ends_with('/') {
                return None;
            }
            let relative = path
                .strip_prefix("overrides/")
                .or_else(|| path.strip_prefix("server-overrides/"))?;
            Some((index, relative.to_string()))
        })
        .collect::<Vec<_>>();

    for (index, relative) in override_entries {
        let relative_path =
            SafeRelativeUtf8UnixPathBuf::try_from(relative.clone())?;
        let target = server_dir.join(relative_path.as_str());
        zip_reader
            .extract_entry(index, &target, &state.io_semaphore, None)
            .await?;
    }

    Ok((game_version, loader, loader_version))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeManifest {
    minecraft: CurseForgeMinecraft,
    #[serde(default = "default_overrides")]
    overrides: String,
    files: Vec<CurseForgeManifestFile>,
}

fn default_overrides() -> String {
    "overrides".to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CurseForgeModLoader>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CurseForgeManifestFile {
    // CurseForge's manifest.json uses an unusual all-caps "ID" suffix here
    // (`projectID`/`fileID`), unlike the mostly-normal camelCase used
    // elsewhere in both the manifest and their REST API -- plain
    // `rename_all = "camelCase"` would produce "fileId" and silently fail
    // to match every file in the pack.
    #[serde(rename = "fileID")]
    file_id: i64,
}

/// Downloads a CurseForge modpack's mod files (no client/server split
/// exists in CurseForge's manifest, so this includes every file the client
/// would get too) and extracts its overrides folder.
async fn install_curseforge_pack_files(
    mod_id: &str,
    file_id: &str,
    server_dir: &Path,
    state: &State,
    reporter: &InstallProgressReporter,
    server_id: &str,
    server_name: &str,
) -> crate::Result<(String, ModLoader, Option<String>)> {
    let file = crate::state::curseforge::get_file(file_id, state).await?;
    let download_url = file.download_url.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "CurseForge did not provide a download URL for file {file_id}. \
             The author may have disabled third-party downloads."
        ))
    })?;
    let _ = mod_id;

    // CurseForge only exposes the real loader/game version via
    // manifest.json, which lives inside the pack .zip -- for a large pack
    // that .zip can take a while to download, and until now the server list
    // kept showing "vanilla" for that entire stretch (`set_resolved_loader`
    // below only fires once the .zip is fully downloaded and parsed). This
    // file's own `gameVersions` array is available immediately, before any
    // of that download starts, and CurseForge already mixes loader names
    // into it right alongside the game version (see `Browse.vue`'s
    // `cfLoaders` for the frontend's mirror of this same trick) -- so use it
    // for an early best-effort guess. The manifest-based call below is still
    // the authoritative one (it also picks up the exact loader *version*,
    // which isn't available here) and will correct this if it guessed
    // wrong.
    if let Some(loader) = parse_curseforge_loader_hint(&file.game_versions) {
        let game_version_hint =
            parse_curseforge_game_version_hint(&file.game_versions)
                .unwrap_or_default();
        if let Err(err) = HostedServer::set_resolved_loader(
            server_id,
            game_version_hint,
            loader,
            None,
            state,
        )
        .await
        {
            tracing::warn!(
                "Failed to record early loader hint for hosted server \
                 {server_id}: {err}"
            );
        }
    }

    let pack_bytes = fetch::fetch(
        &download_url,
        file.sha1().as_deref(),
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    install_curseforge_pack_bytes(
        pack_bytes, server_dir, state, reporter, server_id, server_name,
    )
    .await
}

async fn install_curseforge_pack_bytes(
    pack_bytes: bytes::Bytes,
    server_dir: &Path,
    state: &State,
    reporter: &InstallProgressReporter,
    server_id: &str,
    server_name: &str,
) -> crate::Result<(String, ModLoader, Option<String>)> {
    let mut zip_reader =
        PackZipReader::new(&crate::api::pack::install_from::CreatePackFile::Bytes(
            pack_bytes,
        ))
        .await?;

    let manifest_idx = zip_reader
        .file()
        .entries()
        .iter()
        .position(|entry| {
            matches!(entry.filename().as_str(), Ok("manifest.json"))
        })
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "No manifest.json found in CurseForge modpack".to_string(),
            )
        })?;
    let manifest_json = zip_reader.read_entry_to_string(manifest_idx).await?;
    let manifest: CurseForgeManifest =
        serde_json::from_str(&manifest_json)?;

    let mod_loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first());
    let (loader, loader_version) = match mod_loader {
        Some(loader) => parse_curseforge_loader_id(&loader.id),
        None => (ModLoader::Vanilla, None),
    };

    // Recorded as soon as the manifest reveals the real loader/game
    // version, well before content download starts -- CurseForge doesn't
    // expose the loader anywhere else, so `HostedServer::create` has to
    // record the `ModLoader::Vanilla` placeholder up front. Without this,
    // the server list shows "vanilla" for the server's entire install, even
    // for a heavily modded pack.
    if let Err(err) = HostedServer::set_resolved_loader(
        server_id,
        &manifest.minecraft.version,
        loader,
        loader_version.as_deref(),
        state,
    )
    .await
    {
        tracing::warn!(
            "Failed to record resolved loader for hosted server \
             {server_id}: {err}"
        );
    }

    let file_ids = manifest
        .files
        .iter()
        .map(|file| file.file_id)
        .collect::<Vec<_>>();
    let mut resolved_files = Vec::new();
    for chunk in file_ids.chunks(
        crate::state::curseforge::client::GET_FILES_MAX_BATCH_SIZE,
    ) {
        let api_key = crate::state::curseforge::api_key(state).await?;
        resolved_files.extend(
            crate::state::curseforge::client::get_files(
                &api_key, chunk, state,
            )
            .await?,
        );
    }

    let mods_dir = server_dir.join("mods");
    io::create_dir_all(&mods_dir).await?;

    // Prefer a Modrinth-hosted copy of each mod when one is an exact match
    // (name, author, and version) -- see `modrinth_equivalent` for why.
    let modrinth_equivalents =
        crate::state::curseforge::modrinth_equivalent::find_exact_matches_for_files(
            &resolved_files,
            state,
        )
        .await
        .unwrap_or_default();

    let total_files = resolved_files.len() as u64;
    let details = InstallPhaseDetails::HostedServer {
        name: server_name.to_string(),
    };
    reporter
        .update(
            InstallPhaseId::DownloadingContent,
            Some(InstallProgress {
                current: 0,
                total: total_files,
                secondary: None,
            }),
            details.clone(),
        )
        .await?;

    let mut downloaded_files = 0u64;
    for file in resolved_files {
        if let Some(replacement) = modrinth_equivalents.get(&file.id) {
            let bytes = fetch::fetch(
                &replacement.url,
                replacement.sha1.as_deref(),
                None,
                None,
                &state.fetch_semaphore,
                &state.pool,
            )
            .await?;
            let target = mods_dir.join(&replacement.filename);
            io::write(&target, &bytes).await?;
            downloaded_files += 1;
            reporter
                .update(
                    InstallPhaseId::DownloadingContent,
                    Some(InstallProgress {
                        current: downloaded_files,
                        total: total_files,
                        secondary: None,
                    }),
                    details.clone(),
                )
                .await?;
            continue;
        }

        let Some(download_url) = file.download_url.clone() else {
            downloaded_files += 1;
            continue;
        };
        let bytes = fetch::fetch(
            &download_url,
            file.sha1().as_deref(),
            None,
            None,
            &state.fetch_semaphore,
            &state.pool,
        )
        .await?;
        let target = mods_dir.join(&file.file_name);
        io::write(&target, &bytes).await?;

        downloaded_files += 1;
        reporter
            .update(
                InstallPhaseId::DownloadingContent,
                Some(InstallProgress {
                    current: downloaded_files,
                    total: total_files,
                    secondary: None,
                }),
                details.clone(),
            )
            .await?;
    }

    let overrides_prefix = format!("{}/", manifest.overrides);
    let override_entries = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let path = entry.filename().as_str().ok()?;
            if path.ends_with('/') {
                return None;
            }
            let relative = path.strip_prefix(&overrides_prefix)?;
            Some((index, relative.to_string()))
        })
        .collect::<Vec<_>>();

    for (index, relative) in override_entries {
        let relative_path =
            SafeRelativeUtf8UnixPathBuf::try_from(relative.clone())?;
        let target = server_dir.join(relative_path.as_str());
        zip_reader
            .extract_entry(index, &target, &state.io_semaphore, None)
            .await?;
    }

    Ok((manifest.minecraft.version, loader, loader_version))
}

/// Downloads a Modrinth project's icon and sets it as a hosted server's icon
/// -- called right after a Modrinth modpack install so servers created from
/// a modpack visually match it, same as a client instance would.
async fn set_hosted_server_icon_from_modrinth_project(
    server_id: &str,
    project_id: &str,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<()> {
    let Some(project) =
        CachedEntry::get_project(project_id, None, &state.pool, &state.api_semaphore)
            .await?
    else {
        return Ok(());
    };
    let Some(icon_url) = project.icon_url else {
        return Ok(());
    };

    download_and_set_hosted_server_icon(server_id, &icon_url, state, reporter)
        .await
}

/// Downloads a CurseForge mod's logo and sets it as a hosted server's icon,
/// mirroring [`set_hosted_server_icon_from_modrinth_project`].
async fn set_hosted_server_icon_from_curseforge_mod(
    server_id: &str,
    mod_id: &str,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<()> {
    let cf_mod = crate::state::curseforge::get_mod(mod_id, state).await?;
    let Some(logo) = cf_mod.logo else {
        return Ok(());
    };

    download_and_set_hosted_server_icon(server_id, &logo.url, state, reporter)
        .await
}

async fn download_and_set_hosted_server_icon(
    server_id: &str,
    icon_url: &str,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<()> {
    let bytes = fetch::fetch(
        icon_url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    let server_dir = state.directories.hosted_server_dir(server_id);
    io::create_dir_all(&server_dir).await?;
    let icon_path =
        server_dir.join(format!("icon.{}", icon_extension_from_url(icon_url)));
    io::write(&icon_path, &bytes).await?;

    let icon_path_str = icon_path.to_string_lossy().to_string();
    HostedServer::set_icon_path(server_id, Some(&icon_path_str), state)
        .await?;

    // Best-effort: the install notification is already showing by this
    // point with a blank icon (it was created before the icon was known),
    // so push the now-resolved icon into it live rather than leaving it
    // blank for the rest of the install.
    if let Err(err) =
        reporter.set_display_icon(Some(icon_path_str)).await
    {
        tracing::warn!(
            "Failed to update install notification icon for hosted server \
             {server_id}: {err}"
        );
    }

    Ok(())
}

/// Copies a locally-picked image file in as a hosted server's icon -- used
/// by the manual "change icon" action in the server's settings, mirroring
/// [`download_and_set_hosted_server_icon`] but for a file already on disk
/// rather than one still needing to be downloaded.
pub async fn set_hosted_server_icon_from_local_file(
    server_id: &str,
    path: &Path,
    state: &State,
) -> crate::Result<()> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();

    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| crate::util::io::IOError::with_path(e, path))?;

    let server_dir = state.directories.hosted_server_dir(server_id);
    io::create_dir_all(&server_dir).await?;
    let icon_path = server_dir.join(format!("icon.{extension}"));
    io::write(&icon_path, &bytes).await?;

    HostedServer::set_icon_path(
        server_id,
        Some(&icon_path.to_string_lossy()),
        state,
    )
    .await
}

fn icon_extension_from_url(url: &str) -> &'static str {
    let lower = url.to_ascii_lowercase();
    let lower = lower.split(['?', '#']).next().unwrap_or(&lower);
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "jpg"
    } else if lower.ends_with(".webp") {
        "webp"
    } else if lower.ends_with(".gif") {
        "gif"
    } else {
        "png"
    }
}

/// Recognizes a loader name out of a CurseForge file's `gameVersions` array,
/// which mixes Minecraft version numbers, loader names, environment tags
/// ("Client"/"Server"), and other version-ish labels together with no way to
/// tell them apart by shape alone. Mirrors `Browse.vue`'s `cfLoaders`.
fn parse_curseforge_loader_hint(game_versions: &[String]) -> Option<ModLoader> {
    game_versions.iter().find_map(|value| {
        match value.to_ascii_lowercase().as_str() {
            "forge" => Some(ModLoader::Forge),
            "neoforge" => Some(ModLoader::NeoForge),
            "fabric" => Some(ModLoader::Fabric),
            "quilt" => Some(ModLoader::Quilt),
            _ => None,
        }
    })
}

/// Picks the Minecraft version number out of the same `gameVersions` array
/// -- the only entries that look like `1.20.1` (start with a digit, contain
/// a dot) among the loader names/environment tags/other labels mixed in.
fn parse_curseforge_game_version_hint(game_versions: &[String]) -> Option<&str> {
    game_versions.iter().find(|value| {
        value.contains('.')
            && value.chars().next().is_some_and(|c| c.is_ascii_digit())
    }).map(String::as_str)
}

fn parse_curseforge_loader_id(id: &str) -> (ModLoader, Option<String>) {
    match id.split_once('-') {
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

enum LocalModpackFormat {
    Mrpack,
    CurseForge,
}

/// Distinguishes a `.mrpack` from a CurseForge modpack `.zip` by checking
/// which manifest file it contains -- the two formats have no other
/// reliable signal (both are plain zips, CurseForge modpacks don't
/// necessarily use a `.zip` extension convention any more strictly than
/// mrpacks use `.mrpack`).
async fn sniff_local_modpack_format(
    path: &std::path::Path,
) -> crate::Result<LocalModpackFormat> {
    let zip_reader = PackZipReader::new(
        &crate::api::pack::install_from::CreatePackFile::Path(
            path.to_path_buf(),
        ),
    )
    .await?;

    let has_entry = |name: &str| {
        zip_reader
            .file()
            .entries()
            .iter()
            .any(|entry| matches!(entry.filename().as_str(), Ok(n) if n == name))
    };

    if has_entry("modrinth.index.json") {
        Ok(LocalModpackFormat::Mrpack)
    } else if has_entry("manifest.json") {
        Ok(LocalModpackFormat::CurseForge)
    } else {
        Err(crate::ErrorKind::InputError(
            "This file doesn't look like a .mrpack or CurseForge modpack \
             (no modrinth.index.json or manifest.json found)"
                .to_string(),
        )
        .into())
    }
}

/// The content subdirectories a hosted server's individual content installs
/// (as opposed to whole-modpack installs above) can target, mirroring the
/// folders `content_summary()` (`api::hosting`) already counts.
pub(super) const CONTENT_DIRS: [&str; 4] =
    ["mods", "resourcepacks", "datapacks", "shaderpacks"];

fn validate_content_dir(content_dir: &str) -> crate::Result<()> {
    if CONTENT_DIRS.contains(&content_dir) {
        Ok(())
    } else {
        Err(crate::ErrorKind::InputError(format!(
            "Invalid hosted server content directory: {content_dir}"
        ))
        .into())
    }
}

fn validate_content_file_name(file_name: &str) -> crate::Result<()> {
    if file_name.is_empty()
        || file_name == "."
        || file_name == ".."
        || file_name.contains('/')
        || file_name.contains('\\')
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Invalid file name: {file_name}"
        ))
        .into());
    }
    Ok(())
}

/// Downloads a single content file (a Modrinth version file, or a resolved
/// CurseForge file's download URL) directly into a hosted server's `mods`/
/// `resourcepacks`/`datapacks`/`shaderpacks` folder -- used by the "Browse
/// content" flow (see `api::hosting::install_modrinth_file`/
/// `install_curseforge_file`) to add individual content to an *existing*
/// server, as opposed to the whole-modpack installers above which create a
/// brand new one. No dependency resolution or content-set tracking (hosted
/// servers don't participate in that system) -- just the file itself.
pub async fn install_content_file(
    server_id: &str,
    content_dir: &str,
    file_name: &str,
    url: &str,
    sha1: Option<&str>,
    state: &State,
) -> crate::Result<()> {
    validate_content_dir(content_dir)?;
    validate_content_file_name(file_name)?;

    let target_dir =
        state.directories.hosted_server_dir(server_id).join(content_dir);
    io::create_dir_all(&target_dir).await?;

    let bytes = fetch::fetch(
        url,
        sha1,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    io::write(&target_dir.join(file_name), &bytes).await?;

    Ok(())
}

/// Copies a local file (picked via a native file dialog) into a hosted
/// server's content folder, auto-detecting which folder based on its
/// extension/contents since the "Upload files" flow doesn't know the
/// content type up front the way "Browse content" (which already knows the
/// project type being browsed) does:
/// - `.jar` -> `mods` (Forge/Fabric/NeoForge/Quilt mods are always jars)
/// - `.zip` -> sniffed by top-level folder: `data/` -> `datapacks`,
///   `assets/` -> `resourcepacks`, `shaders/` -> `shaderpacks`
pub async fn install_local_content_file(
    server_id: &str,
    path: &Path,
    state: &State,
) -> crate::Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "File has no valid file name".to_string(),
            )
        })?
        .to_string();

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let content_dir = match extension.as_str() {
        "jar" => "mods",
        "zip" => sniff_content_zip_dir(path).await?,
        other => {
            return Err(crate::ErrorKind::InputError(format!(
                "Don't know how to install a .{other} file as server \
                 content -- expected a .jar (mod) or .zip (resource pack, \
                 data pack, or shader pack)"
            ))
            .into());
        }
    };

    let target_dir =
        state.directories.hosted_server_dir(server_id).join(content_dir);
    io::create_dir_all(&target_dir).await?;

    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| crate::util::io::IOError::with_path(e, path))?;
    io::write(&target_dir.join(&file_name), &bytes).await?;

    Ok(())
}

async fn sniff_content_zip_dir(
    path: &std::path::Path,
) -> crate::Result<&'static str> {
    let zip_reader = PackZipReader::new(
        &crate::api::pack::install_from::CreatePackFile::Path(
            path.to_path_buf(),
        ),
    )
    .await?;

    let has_top_level = |prefix: &str| {
        zip_reader.file().entries().iter().any(|entry| {
            matches!(entry.filename().as_str(), Ok(name) if name.starts_with(prefix))
        })
    };

    if has_top_level("data/") {
        Ok("datapacks")
    } else if has_top_level("shaders/") {
        Ok("shaderpacks")
    } else if has_top_level("assets/") {
        Ok("resourcepacks")
    } else {
        Err(crate::ErrorKind::InputError(
            "Couldn't tell what kind of content this .zip is (found no \
             data/, assets/, or shaders/ folder inside it)"
                .to_string(),
        )
        .into())
    }
}
