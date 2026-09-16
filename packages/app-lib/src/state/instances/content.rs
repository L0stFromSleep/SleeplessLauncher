use super::ContentSourceKind;
use crate::state::{
    ContentProvider, License, Project, ProjectType, Version,
    VersionEnvironment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItem {
    pub file_name: String,
    pub file_path: String,
    pub id: String,
    pub size: u64,
    pub enabled: bool,
    pub locked: bool,
    pub project_type: ProjectType,
    pub project: Option<ContentItemProject>,
    pub version: Option<ContentItemVersion>,
    pub environment: Option<VersionEnvironment>,
    pub owner: Option<ContentItemOwner>,
    pub has_update: bool,
    pub update_version_id: Option<String>,
    pub date_added: Option<String>,
    pub source_kind: Option<ContentSourceKind>,
    pub embedded_metadata: Option<EmbeddedContentMetadata>,
    /// The upstream registry this item's `project`/`version` ids belong to.
    /// `None` for items with no resolved project/version metadata at all
    /// (e.g. unrecognized uploaded files).
    #[serde(default)]
    pub provider: Option<ContentProvider>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct EmbeddedContentMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub icon_path: Option<String>,
}

impl EmbeddedContentMetadata {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.version.is_none()
            && self.icon_path.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemProject {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    pub license: License,
    pub categories: Vec<String>,
    pub additional_categories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemVersion {
    pub id: String,
    pub version_number: String,
    pub file_name: String,
    pub date_published: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContentItemOwner {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
    /// Set only for CurseForge-sourced owners, whose profile lives outside
    /// this app (unlike a Modrinth user/organization, which has an in-app
    /// route). When present, the frontend should link out to this instead
    /// of building an in-app `/user/:id` route.
    #[serde(default)]
    pub profile_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OwnerType {
    User,
    Organization,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LinkedModpackInfo {
    pub project: Project,
    pub version: Option<Version>,
    pub owner: Option<ContentItemOwner>,
    pub has_update: bool,
    pub update_version_id: Option<String>,
    pub update_version: Option<Version>,
}
