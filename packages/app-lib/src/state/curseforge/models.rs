//! Plain data models mirroring the subset of CurseForge's REST API schema
//! (<https://docs.curseforge.com/rest-api/>) needed for search, project
//! details, and file download.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CurseForge's numeric game id for Minecraft.
pub const MINECRAFT_GAME_ID: u32 = 432;

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
    pub name: String,
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
