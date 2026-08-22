use super::client;
use super::models::{
    CfFile, RELATION_TYPE_EMBEDDED_LIBRARY, RELATION_TYPE_INCOMPATIBLE,
    RELATION_TYPE_OPTIONAL_DEPENDENCY, RELATION_TYPE_REQUIRED_DEPENDENCY,
};
use crate::State;
use async_trait::async_trait;
use modrinth_content_management::{
    ContentMetadataProvider, Dependency, DependencyType, Error as ResolveError,
    Version,
};

/// Loader identifiers CurseForge mixes into a file's flat `gameVersions`
/// list alongside actual Minecraft version strings.
const CURSEFORGE_LOADER_TAGS: &[&str] =
    &["forge", "fabric", "quilt", "neoforge", "rift", "liteloader", "cauldron"];

/// [`ContentMetadataProvider`] implementation backed by the CurseForge API,
/// mirroring [`super::super::instances::commands::apply_content_install::CachedEntryContentProvider`]'s
/// role for Modrinth.
pub(crate) struct CurseForgeContentProvider<'a> {
    state: &'a State,
    api_key: String,
}

impl<'a> CurseForgeContentProvider<'a> {
    pub(crate) async fn new(state: &'a State) -> crate::Result<Self> {
        let api_key = super::api_key(state).await?;
        Ok(Self { state, api_key })
    }
}

#[async_trait]
impl ContentMetadataProvider for CurseForgeContentProvider<'_> {
    async fn get_version(
        &mut self,
        version_id: &str,
    ) -> Result<Option<Version>, ResolveError> {
        match client::get_file(&self.api_key, version_id, self.state).await {
            Ok(file) => Ok(Some(file_to_resolver_version(file))),
            Err(error) => Err(ResolveError::Provider(error.to_string())),
        }
    }

    async fn get_project_versions(
        &mut self,
        project_id: &str,
    ) -> Result<Vec<Version>, ResolveError> {
        let files =
            client::get_mod_files(&self.api_key, project_id, self.state)
                .await
                .map_err(|error| ResolveError::Provider(error.to_string()))?;

        Ok(files.into_iter().map(file_to_resolver_version).collect())
    }
}

fn file_to_resolver_version(file: CfFile) -> Version {
    let (game_versions, loaders) = split_game_versions(&file.game_versions);

    Version {
        id: file.id.to_string(),
        project_id: file.mod_id.to_string(),
        date_published: file.file_date,
        dependencies: file
            .dependencies
            .iter()
            .filter_map(file_dependency_to_resolver)
            .collect(),
        game_versions,
        loaders,
    }
}

fn file_dependency_to_resolver(
    dependency: &super::models::CfFileDependency,
) -> Option<Dependency> {
    let dependency_type = match dependency.relation_type {
        RELATION_TYPE_REQUIRED_DEPENDENCY => DependencyType::Required,
        RELATION_TYPE_OPTIONAL_DEPENDENCY => DependencyType::Optional,
        RELATION_TYPE_EMBEDDED_LIBRARY => DependencyType::Embedded,
        RELATION_TYPE_INCOMPATIBLE => DependencyType::Incompatible,
        // `Tool` (4) and `Include` (6) aren't modeled as installable
        // dependencies.
        _ => return None,
    };

    Some(Dependency {
        version_id: None,
        project_id: Some(dependency.mod_id.to_string()),
        file_name: None,
        dependency_type,
    })
}

/// Splits CurseForge's flat `gameVersions` list (which mixes actual
/// Minecraft version strings with loader names and environment tags) into
/// separate game-version and loader lists.
fn split_game_versions(raw: &[String]) -> (Vec<String>, Vec<String>) {
    let mut game_versions = Vec::new();
    let mut loaders = Vec::new();

    for entry in raw {
        let lower = entry.to_lowercase();
        if CURSEFORGE_LOADER_TAGS.contains(&lower.as_str()) {
            loaders.push(lower);
        } else if lower == "client" || lower == "server" {
            // Environment tags aren't modeled here.
        } else {
            game_versions.push(entry.clone());
        }
    }

    (game_versions, loaders)
}
