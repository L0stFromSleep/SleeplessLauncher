//! Locally self-hosted Minecraft servers -- a separate concept from the
//! regular client instance library. A hosted server downloads a
//! server-only copy of a modpack (or a plain vanilla/loader server) into
//! its own directory (`DirectoryInfo::hosted_server_dir`) and runs the
//! dedicated server jar as a tracked child process instead of launching the
//! client.

pub mod content_metadata;
pub mod install;
pub mod launch;
pub mod process;
pub mod properties;

pub use process::HostedServerProcessMetadata;

use crate::State;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedServerProvider {
    Vanilla,
    Modrinth,
    CurseForge,
}

impl HostedServerProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Modrinth => "modrinth",
            Self::CurseForge => "curseforge",
        }
    }

    pub fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "vanilla" => Ok(Self::Vanilla),
            "modrinth" => Ok(Self::Modrinth),
            "curseforge" => Ok(Self::CurseForge),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unknown hosted server provider {other}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostedServerInstallStage {
    NotInstalled,
    Installing,
    Installed,
    Error,
}

impl HostedServerInstallStage {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::Installing => "installing",
            Self::Installed => "installed",
            Self::Error => "error",
        }
    }

    pub fn from_str(value: &str) -> crate::Result<Self> {
        match value {
            "not_installed" => Ok(Self::NotInstalled),
            "installing" => Ok(Self::Installing),
            "installed" => Ok(Self::Installed),
            "error" => Ok(Self::Error),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unknown hosted server install stage {other}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedServer {
    pub id: String,
    pub name: String,
    pub icon_path: Option<String>,
    pub game_version: String,
    pub loader: crate::state::ModLoader,
    pub loader_version: Option<String>,
    pub provider: HostedServerProvider,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub port: u16,
    pub max_memory_mb: u32,
    pub extra_java_args: Option<String>,
    pub eula_accepted: bool,
    pub install_stage: HostedServerInstallStage,
    /// The Java major version resolved for this server's loader/game
    /// version during install (see `launcher::server_install`). `None`
    /// until install finishes; `launch` looks this up in the shared
    /// `java_versions` table to find the actual executable to run, rather
    /// than trusting whatever `java` happens to be first on PATH.
    pub java_major_version: Option<u32>,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

struct HostedServerRow {
    id: String,
    name: String,
    icon_path: Option<String>,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
    provider: String,
    project_id: Option<String>,
    version_id: Option<String>,
    port: i64,
    max_memory_mb: i64,
    extra_java_args: Option<String>,
    eula_accepted: bool,
    install_stage: String,
    java_major_version: Option<i64>,
    created: i64,
    modified: i64,
}

impl TryFrom<HostedServerRow> for HostedServer {
    type Error = crate::Error;

    fn try_from(row: HostedServerRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            name: row.name,
            icon_path: row.icon_path,
            game_version: row.game_version,
            loader: crate::state::ModLoader::from_string(&row.loader),
            loader_version: row.loader_version,
            provider: HostedServerProvider::from_str(&row.provider)?,
            project_id: row.project_id,
            version_id: row.version_id,
            port: row.port as u16,
            max_memory_mb: row.max_memory_mb as u32,
            extra_java_args: row.extra_java_args,
            eula_accepted: row.eula_accepted,
            install_stage: HostedServerInstallStage::from_str(
                &row.install_stage,
            )?,
            java_major_version: row.java_major_version.map(|v| v as u32),
            created: DateTime::from_timestamp(row.created, 0)
                .unwrap_or_default(),
            modified: DateTime::from_timestamp(row.modified, 0)
                .unwrap_or_default(),
        })
    }
}

pub struct NewHostedServer {
    pub name: String,
    pub game_version: String,
    pub loader: crate::state::ModLoader,
    pub loader_version: Option<String>,
    pub provider: HostedServerProvider,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

impl HostedServer {
    pub async fn create(
        new: NewHostedServer,
        state: &State,
    ) -> crate::Result<HostedServer> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        let loader = new.loader.as_str();
        let provider = new.provider.as_str();
        let install_stage = HostedServerInstallStage::NotInstalled.as_str();

        sqlx::query!(
            "
            INSERT INTO hosted_servers (
                id, name, game_version, loader, loader_version, provider,
                project_id, version_id, install_stage, created, modified
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $10)
            ",
            id,
            new.name,
            new.game_version,
            loader,
            new.loader_version,
            provider,
            new.project_id,
            new.version_id,
            install_stage,
            now,
        )
        .execute(&state.pool)
        .await?;

        Self::get(&id, state)
            .await?
            .ok_or_else(|| {
                crate::ErrorKind::OtherError(
                    "Failed to read back newly created hosted server"
                        .to_string(),
                )
                .into()
            })
    }

    pub async fn get(
        id: &str,
        state: &State,
    ) -> crate::Result<Option<HostedServer>> {
        let row = sqlx::query_as!(
            HostedServerRow,
            r#"
            SELECT
                id, name, icon_path, game_version, loader, loader_version,
                provider, project_id, version_id, port, max_memory_mb,
                extra_java_args, eula_accepted as "eula_accepted!: bool",
                install_stage, java_major_version, created, modified
            FROM hosted_servers
            WHERE id = $1
            "#,
            id,
        )
        .fetch_optional(&state.pool)
        .await?;

        row.map(TryFrom::try_from).transpose()
    }

    pub async fn list(state: &State) -> crate::Result<Vec<HostedServer>> {
        let rows = sqlx::query_as!(
            HostedServerRow,
            r#"
            SELECT
                id, name, icon_path, game_version, loader, loader_version,
                provider, project_id, version_id, port, max_memory_mb,
                extra_java_args, eula_accepted as "eula_accepted!: bool",
                install_stage, java_major_version, created, modified
            FROM hosted_servers
            ORDER BY created DESC
            "#,
        )
        .fetch_all(&state.pool)
        .await?;

        rows.into_iter().map(TryFrom::try_from).collect()
    }

    pub async fn delete(id: &str, state: &State) -> crate::Result<()> {
        sqlx::query!("DELETE FROM hosted_servers WHERE id = $1", id)
            .execute(&state.pool)
            .await?;

        let dir = state.directories.hosted_server_dir(id);
        if dir.exists() {
            crate::util::io::remove_dir_all(&dir).await?;
        }

        Ok(())
    }

    pub async fn set_install_stage(
        id: &str,
        stage: HostedServerInstallStage,
        state: &State,
    ) -> crate::Result<()> {
        let now = Utc::now().timestamp();
        let stage = stage.as_str();
        sqlx::query!(
            "UPDATE hosted_servers SET install_stage = $2, modified = $3 WHERE id = $1",
            id,
            stage,
            now,
        )
        .execute(&state.pool)
        .await?;
        Ok(())
    }

    /// Updates the loader/game version as soon as they're actually known
    /// from a modpack's manifest -- for CurseForge-sourced (and any
    /// locally-imported) servers, the real loader isn't known until partway
    /// through content install (the manifest has to be downloaded and
    /// parsed first), so `create` has to record a placeholder
    /// (`ModLoader::Vanilla`) up front. Without this, the server list shows
    /// "vanilla" for the server's entire install, even for a heavily
    /// modded pack. Called as soon as the manifest is parsed, well before
    /// content download finishes.
    pub async fn set_resolved_loader(
        id: &str,
        game_version: &str,
        loader: crate::state::ModLoader,
        loader_version: Option<&str>,
        state: &State,
    ) -> crate::Result<()> {
        let now = Utc::now().timestamp();
        let loader_str = loader.as_str();
        sqlx::query!(
            "
            UPDATE hosted_servers
            SET game_version = $2, loader = $3, loader_version = $4, modified = $5
            WHERE id = $1
            ",
            id,
            game_version,
            loader_str,
            loader_version,
            now,
        )
        .execute(&state.pool)
        .await?;
        Ok(())
    }

    pub async fn set_eula_accepted(
        id: &str,
        accepted: bool,
        state: &State,
    ) -> crate::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query!(
            "UPDATE hosted_servers SET eula_accepted = $2, modified = $3 WHERE id = $1",
            id,
            accepted,
            now,
        )
        .execute(&state.pool)
        .await?;
        Ok(())
    }

    pub async fn set_icon_path(
        id: &str,
        icon_path: Option<&str>,
        state: &State,
    ) -> crate::Result<()> {
        let now = Utc::now().timestamp();
        sqlx::query!(
            "UPDATE hosted_servers SET icon_path = $2, modified = $3 WHERE id = $1",
            id,
            icon_path,
            now,
        )
        .execute(&state.pool)
        .await?;
        Ok(())
    }

    pub async fn update_settings(
        id: &str,
        port: u16,
        max_memory_mb: u32,
        extra_java_args: Option<&str>,
        state: &State,
    ) -> crate::Result<()> {
        let now = Utc::now().timestamp();
        let port = port as i64;
        let max_memory_mb = max_memory_mb as i64;
        sqlx::query!(
            "
            UPDATE hosted_servers
            SET port = $2, max_memory_mb = $3, extra_java_args = $4, modified = $5
            WHERE id = $1
            ",
            id,
            port,
            max_memory_mb,
            extra_java_args,
            now,
        )
        .execute(&state.pool)
        .await?;
        Ok(())
    }
}
