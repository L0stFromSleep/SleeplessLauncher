use super::client;
use super::models::{
    CfFile, RELATION_TYPE_EMBEDDED_LIBRARY, RELATION_TYPE_INCOMPATIBLE,
    RELATION_TYPE_OPTIONAL_DEPENDENCY, RELATION_TYPE_REQUIRED_DEPENDENCY,
};
use crate::State;
use async_trait::async_trait;
use modrinth_content_management::{
    ContentMetadataProvider, ContentType, Dependency, DependencyType,
    Error as ResolveError, Version,
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
    content_type: ContentType,
}

impl<'a> CurseForgeContentProvider<'a> {
    pub(crate) async fn new(
        state: &'a State,
        content_type: ContentType,
    ) -> crate::Result<Self> {
        let api_key = super::api_key(state).await?;
        Ok(Self { state, api_key, content_type })
    }
}

#[async_trait]
impl ContentMetadataProvider for CurseForgeContentProvider<'_> {
    async fn get_version(
        &mut self,
        version_id: &str,
    ) -> Result<Option<Version>, ResolveError> {
        match client::get_file(&self.api_key, version_id, self.state).await {
            Ok(file) => {
                Ok(Some(file_to_resolver_version(file, self.content_type)))
            }
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

        Ok(files
            .into_iter()
            .map(|file| file_to_resolver_version(file, self.content_type))
            .collect())
    }
}

fn file_to_resolver_version(file: CfFile, content_type: ContentType) -> Version {
    let (game_versions, mut loaders) = split_game_versions(&file.game_versions);

    // CurseForge only tags mod files with a loader (forge/fabric/...) in
    // `gameVersions`; resource pack, data pack, and shader files carry no
    // such tag at all. The generic resolver matches versions against a
    // synthetic loader string per content type (see `target_preferences` in
    // `apply_content_install.rs`), so without this, non-mod CurseForge files
    // would never satisfy that match and would look like they have no
    // installable versions.
    if loaders.is_empty()
        && let Some(synthetic_loader) = non_mod_loader_tag(content_type)
    {
        loaders.push(synthetic_loader.to_string());
    }

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

/// Mirrors `target_preferences`'s loader convention in
/// `apply_content_install.rs` for content types CurseForge doesn't tag with
/// a real loader identifier.
fn non_mod_loader_tag(content_type: ContentType) -> Option<&'static str> {
    match content_type {
        ContentType::DataPack => Some("datapack"),
        ContentType::ResourcePack => Some("minecraft"),
        ContentType::Shader => Some("iris"),
        _ => None,
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
