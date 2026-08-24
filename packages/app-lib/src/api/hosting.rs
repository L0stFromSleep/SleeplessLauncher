//! Public API surface for locally self-hosted servers, mirroring
//! `api::curseforge`'s role as a thin wrapper over `state::hosting`.

use crate::state::hosting::install::HostedServerSource;
use crate::state::hosting::{HostedServer, HostedServerProcessMetadata};
use crate::state::ModLoader;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CreateHostedServerSource {
    Vanilla {
        game_version: String,
        loader: Option<ModLoader>,
    },
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    #[serde(rename = "curseforge_modpack")]
    CurseForgeModpack {
        mod_id: String,
        file_id: String,
    },
    LocalModpackFile {
        path: String,
    },
}

#[tracing::instrument]
pub async fn list() -> crate::Result<Vec<HostedServer>> {
    let state = crate::State::get().await?;
    HostedServer::list(&state).await
}

#[tracing::instrument]
pub async fn get(id: String) -> crate::Result<Option<HostedServer>> {
    let state = crate::State::get().await?;
    HostedServer::get(&id, &state).await
}

#[tracing::instrument]
pub async fn create(
    name: String,
    source: CreateHostedServerSource,
) -> crate::Result<HostedServer> {
    let state = crate::State::get().await?;
    let (source, loader_override) = match source {
        CreateHostedServerSource::Vanilla { game_version, loader } => {
            (HostedServerSource::Vanilla { game_version }, loader)
        }
        CreateHostedServerSource::ModrinthModpack {
            project_id,
            version_id,
        } => (
            HostedServerSource::ModrinthModpack { project_id, version_id },
            None,
        ),
        CreateHostedServerSource::CurseForgeModpack { mod_id, file_id } => (
            HostedServerSource::CurseForgeModpack { mod_id, file_id },
            None,
        ),
        CreateHostedServerSource::LocalModpackFile { path } => (
            HostedServerSource::LocalModpackFile { path: path.into() },
            None,
        ),
    };

    crate::state::hosting::install::create_and_install_hosted_server(
        name,
        source,
        loader_override,
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn delete(id: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    let _ = crate::state::hosting::launch::stop(&id, &state).await;
    HostedServer::delete(&id, &state).await
}

#[tracing::instrument]
pub async fn start(id: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::launch::start(&id, &state).await
}

#[tracing::instrument]
pub async fn stop(id: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::launch::stop(&id, &state).await
}

#[tracing::instrument]
pub async fn send_command(id: String, line: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::launch::send_command(&id, &line, &state).await
}

#[tracing::instrument]
pub async fn is_running(id: String) -> crate::Result<bool> {
    let state = crate::State::get().await?;
    Ok(state.hosting_process_manager.is_running(&id))
}

#[tracing::instrument]
pub async fn get_process(
    id: String,
) -> crate::Result<Option<HostedServerProcessMetadata>> {
    let state = crate::State::get().await?;
    Ok(state.hosting_process_manager.get(&id))
}

#[tracing::instrument]
pub async fn get_log_buffer(id: String) -> crate::Result<Vec<String>> {
    Ok(crate::state::get_log_buffer(&id))
}

#[tracing::instrument]
pub async fn set_eula_accepted(
    id: String,
    accepted: bool,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    HostedServer::set_eula_accepted(&id, accepted, &state).await
}

/// The server's own directory on disk, so the frontend can point a generic
/// file-browser at it (mirroring `instance::get_full_path`).
#[tracing::instrument]
pub async fn get_directory(id: String) -> crate::Result<String> {
    let state = crate::State::get().await?;
    Ok(state
        .directories
        .hosted_server_dir(&id)
        .to_string_lossy()
        .into_owned())
}

#[derive(Debug, serde::Serialize)]
pub struct HostedServerContentSummary {
    pub mods: u32,
    pub resourcepacks: u32,
    pub datapacks: u32,
    pub shaderpacks: u32,
}

/// Cheap counts for the "Modpack content" summary card -- hosted servers
/// don't participate in the client instance/content-set tracking system
/// (see `state::hosting::install`'s module doc), so this just counts files
/// directly rather than querying a `ContentEntry` table.
#[tracing::instrument]
pub async fn content_summary(
    id: String,
) -> crate::Result<HostedServerContentSummary> {
    let state = crate::State::get().await?;
    let server_dir = state.directories.hosted_server_dir(&id);

    async fn count_entries(dir: std::path::PathBuf) -> u32 {
        let Ok(mut read_dir) = tokio::fs::read_dir(&dir).await else {
            return 0;
        };
        let mut count = 0u32;
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            count += 1;
        }
        count
    }

    let (mods, resourcepacks, datapacks, shaderpacks) = tokio::join!(
        count_entries(server_dir.join("mods")),
        count_entries(server_dir.join("resourcepacks")),
        count_entries(server_dir.join("datapacks")),
        count_entries(server_dir.join("shaderpacks")),
    );

    Ok(HostedServerContentSummary {
        mods,
        resourcepacks,
        datapacks,
        shaderpacks,
    })
}

/// Resolves the real Modrinth/CurseForge project behind each file in a
/// hosted server's content folders, by hash, keyed by relative path (e.g.
/// `"mods/somejar.jar"`) -- see `state::hosting::content_metadata` for why
/// this can't just be read back from install-time records the way a client
/// instance's content page does. Files with no match on either provider are
/// simply absent from the returned map.
#[tracing::instrument]
pub async fn content_metadata(
    id: String,
) -> crate::Result<
    std::collections::HashMap<
        String,
        crate::state::hosting::content_metadata::HostedContentMetadata,
    >,
> {
    let state = crate::State::get().await?;
    Ok(crate::state::hosting::content_metadata::resolve_content_metadata(
        &id, &state,
    )
    .await)
}

/// Sets (or clears, with `icon_path: None`) a hosted server's icon -- used
/// both by the automatic modpack-icon set on install and by the manual
/// "change icon" action in the server's settings.
#[tracing::instrument]
pub async fn set_icon_path(
    id: String,
    icon_path: Option<String>,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    HostedServer::set_icon_path(&id, icon_path.as_deref(), &state).await
}

/// Copies a locally-picked image file in as a hosted server's icon -- used
/// by the "change icon" action in the server's settings modal.
#[tracing::instrument]
pub async fn set_icon_from_path(id: String, path: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::install::set_hosted_server_icon_from_local_file(
        &id,
        std::path::Path::new(&path),
        &state,
    )
    .await
}

/// Installs a single Modrinth version file directly into a hosted server's
/// content folder (mods/resourcepacks/datapacks/shaderpacks) -- used by the
/// "Browse content" flow to add individual content to an already-created
/// server, mirroring how a client instance's content page installs one
/// project at a time.
#[tracing::instrument]
pub async fn install_modrinth_file(
    id: String,
    content_dir: String,
    file_name: String,
    url: String,
    sha1: Option<String>,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::install::install_content_file(
        &id,
        &content_dir,
        &file_name,
        &url,
        sha1.as_deref(),
        &state,
    )
    .await
}

/// Resolves and installs a single CurseForge file directly into a hosted
/// server's content folder, mirroring [`install_modrinth_file`].
#[tracing::instrument]
pub async fn install_curseforge_file(
    id: String,
    content_dir: String,
    file_id: String,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    let file = crate::state::curseforge::get_file(&file_id, &state).await?;
    let download_url = file.download_url.clone().ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "CurseForge did not provide a download URL for file {file_id}. \
             The author may have disabled third-party downloads."
        ))
    })?;

    crate::state::hosting::install::install_content_file(
        &id,
        &content_dir,
        &file.file_name,
        &download_url,
        file.sha1().as_deref(),
        &state,
    )
    .await
}

/// Copies a locally-picked file into a hosted server's content folder,
/// auto-detecting the right subfolder -- used by the "Upload files" flow.
#[tracing::instrument]
pub async fn install_local_content_file(
    id: String,
    path: String,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::install::install_local_content_file(
        &id,
        std::path::Path::new(&path),
        &state,
    )
    .await
}

#[tracing::instrument]
pub async fn update_settings(
    id: String,
    port: u16,
    max_memory_mb: u32,
    extra_java_args: Option<String>,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    HostedServer::update_settings(
        &id,
        port,
        max_memory_mb,
        extra_java_args.as_deref(),
        &state,
    )
    .await?;
    // Keeps `server.properties`' `server-port` in sync so the port setting
    // actually takes effect on the server's next launch -- previously this
    // column was stored but never applied anywhere.
    crate::state::hosting::properties::set_port(&id, port, &state).await
}

/// The common `server.properties` fields a server admin actually needs day
/// to day (MOTD, difficulty, whitelist, etc.) -- see
/// `state::hosting::properties` for what's managed and what's left alone.
#[tracing::instrument]
pub async fn get_properties(
    id: String,
) -> crate::Result<crate::state::hosting::properties::HostedServerProperties> {
    let state = crate::State::get().await?;
    crate::state::hosting::properties::get(&id, &state).await
}

#[tracing::instrument]
pub async fn set_properties(
    id: String,
    properties: crate::state::hosting::properties::HostedServerProperties,
) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::hosting::properties::set(&id, &properties, &state).await
}
