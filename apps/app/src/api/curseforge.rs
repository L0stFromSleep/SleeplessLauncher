use crate::api::Result;
use theseus::prelude::*;

#[tauri::command]
pub async fn curseforge_search(
    query: String,
    game_version: Option<String>,
    page: u32,
    page_size: u32,
) -> Result<theseus::curseforge::CurseForgeSearchResults> {
    Ok(theseus::curseforge::search(query, game_version, page, page_size).await?)
}

#[tauri::command]
pub async fn curseforge_get_mod(mod_id: String) -> Result<CfMod> {
    Ok(theseus::curseforge::get_mod(mod_id).await?)
}

#[tauri::command]
pub async fn curseforge_get_mod_files(mod_id: String) -> Result<Vec<CfFile>> {
    Ok(theseus::curseforge::get_mod_files(mod_id).await?)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("curseforge")
        .invoke_handler(tauri::generate_handler![
            curseforge_search,
            curseforge_get_mod,
            curseforge_get_mod_files,
        ])
        .build()
}
