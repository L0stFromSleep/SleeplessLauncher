//! Plain data models mirroring the subset of CurseForge's REST API schema
//! (<https://docs.curseforge.com/rest-api/>) needed for search, project
//! details, and file download.

use crate::state::ProjectType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CurseForge's numeric game id for Minecraft.
pub const MINECRAFT_GAME_ID: u32 = 432;

/// CurseForge's `classId` values scoping a mod/file to a Minecraft content
/// category. These aren't documented anywhere in CurseForge's REST API docs
/// (only discoverable live via `GET /v1/categories?gameId=432`), but are
/// stable, widely-relied-upon values used by other third-party CurseForge
/// clients (e.g. Prism Launcher, ferium).
pub mod class_id {
    pub const MODS: i64 = 6;
    pub const MODPACKS: i64 = 4471;
    pub const RESOURCE_PACKS: i64 = 12;
    /// Data packs are a category rather than their own top-level class, but
    /// CurseForge's search accepts this id directly as `classId` too.
    pub const DATA_PACKS: i64 = 6945;
    /// Shader packs are likewise a category id, filed under Resource Packs.
    pub const SHADER_PACKS: i64 = 6552;
}

/// Computes CurseForge's file "fingerprint": a `murmur2` hash (seed `1`) of
/// the file's bytes with whitespace bytes (tab, LF, CR, space) stripped
/// first. This isn't documented in CurseForge's REST API docs, but is the
/// well-established algorithm used by every third-party CurseForge client
/// for `POST /v1/fingerprints` matching -- CurseForge's own launcher hashes
/// files the same way so mods edited only by e.g. line-ending changes still
/// match.
pub fn cf_fingerprint(bytes: &[u8]) -> u32 {
    let filtered: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|&b| !matches!(b, 9 | 10 | 13 | 32))
        .collect();
    murmur2::murmur2(&filtered, 1)
}

/// Maps a CurseForge `classId` back to the launcher [`ProjectType`] it
/// represents, if it's one of the launcher's known content categories.
pub fn project_type_for_class_id(class_id: i64) -> Option<ProjectType> {
    match class_id {
        class_id::MODS => Some(ProjectType::Mod),
        class_id::RESOURCE_PACKS => Some(ProjectType::ResourcePack),
        class_id::DATA_PACKS => Some(ProjectType::DataPack),
        class_id::SHADER_PACKS => Some(ProjectType::ShaderPack),
        _ => None,
    }
}

/// CurseForge's `HashAlgo` enum: 1 = Sha1, 2 = Md5.
const HASH_ALGO_SHA1: i32 = 1;

/// CurseForge's `FileRelationType` enum.
pub const RELATION_TYPE_EMBEDDED_LIBRARY: i32 = 1;
pub const RELATION_TYPE_OPTIONAL_DEPENDENCY: i32 = 2;
pub const RELATION_TYPE_REQUIRED_DEPENDENCY: i32 = 3;
pub const RELATION_TYPE_INCOMPATIBLE: i32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CfDataEnvelope<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CfSearchEnvelope {
    pub data: Vec<CfMod>,
    pub pagination: CfPagination,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CfPagination {
    pub index: u32,
    pub page_size: u32,
    pub result_count: u32,
    pub total_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub logo: Option<CfAsset>,
    pub links: Option<CfModLinks>,
    /// CurseForge's classId for this mod (see [`class_id`]), scoping it to a
    /// content category (mod, modpack, resource pack, ...).
    pub class_id: Option<i64>,
    #[serde(default)]
    pub categories: Vec<CfCategory>,
    pub allow_mod_distribution: Option<bool>,
    #[serde(default)]
    pub latest_files: Vec<CfFile>,
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    pub download_count: Option<f64>,
    pub date_created: Option<DateTime<Utc>>,
    pub date_modified: Option<DateTime<Utc>>,
    #[serde(default)]
    pub screenshots: Vec<CfScreenshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfScreenshot {
    pub title: Option<String>,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfAsset {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfModLinks {
    pub website_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfCategory {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfAuthor {
    pub id: Option<i64>,
    pub name: String,
    /// Link to the author's CurseForge profile. CurseForge doesn't expose an
    /// avatar image for authors the way Modrinth does, so this is the only
    /// extra identity info available for a CurseForge-sourced content
    /// item's "owner" row.
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: i64,
    pub mod_id: i64,
    pub file_name: String,
    pub display_name: String,
    pub file_date: DateTime<Utc>,
    pub download_url: Option<String>,
    #[serde(default)]
    pub file_length: u64,
    /// CurseForge's `murmur2`-based content fingerprint for this exact file
    /// (see [`cf_fingerprint`]), used to match [`get_fingerprint_matches`]
    /// results back to the local file that produced the matching
    /// fingerprint.
    ///
    /// [`get_fingerprint_matches`]: super::client::get_fingerprint_matches
    #[serde(default)]
    pub file_fingerprint: Option<u32>,
    #[serde(default)]
    pub hashes: Vec<CfFileHash>,
    #[serde(default)]
    pub dependencies: Vec<CfFileDependency>,
    #[serde(default)]
    pub game_versions: Vec<String>,
}

impl CfFile {
    /// Returns the file's sha1 hash, if CurseForge provided one.
    pub fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|hash| hash.algo == HASH_ALGO_SHA1)
            .map(|hash| hash.value.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFileHash {
    pub value: String,
    pub algo: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFileDependency {
    pub mod_id: i64,
    pub relation_type: i32,
}

/// `POST /v1/fingerprints` response envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CfFingerprintMatchesEnvelope {
    pub data: CfFingerprintMatches,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CfFingerprintMatches {
    #[serde(default)]
    pub exact_matches: Vec<CfFingerprintMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFingerprintMatch {
    pub id: i64,
    pub file: CfFile,
}
