use crate::api::Result;
use theseus::prelude::*;

#[tauri::command]
pub async fn hosting_list() -> Result<Vec<HostedServer>> {
    Ok(theseus::hosting::list().await?)
}

#[tauri::command]
pub async fn hosting_get(id: String) -> Result<Option<HostedServer>> {
    Ok(theseus::hosting::get(id).await?)
}

#[tauri::command]
pub async fn hosting_create(
    name: String,
    source: theseus::hosting::CreateHostedServerSource,
) -> Result<HostedServer> {
    Ok(theseus::hosting::create(name, source).await?)
}

#[tauri::command]
pub async fn hosting_delete(id: String) -> Result<()> {
    Ok(theseus::hosting::delete(id).await?)
}

#[tauri::command]
pub async fn hosting_start(id: String) -> Result<()> {
    Ok(theseus::hosting::start(id).await?)
}

#[tauri::command]
pub async fn hosting_stop(id: String) -> Result<()> {
    Ok(theseus::hosting::stop(id).await?)
}

#[tauri::command]
pub async fn hosting_send_command(id: String, line: String) -> Result<()> {
    Ok(theseus::hosting::send_command(id, line).await?)
}

#[tauri::command]
pub async fn hosting_is_running(id: String) -> Result<bool> {
    Ok(theseus::hosting::is_running(id).await?)
}

#[tauri::command]
pub async fn hosting_get_process(
    id: String,
) -> Result<Option<HostedServerProcessMetadata>> {
    Ok(theseus::hosting::get_process(id).await?)
}

#[tauri::command]
pub async fn hosting_get_log_buffer(id: String) -> Result<Vec<String>> {
    Ok(theseus::hosting::get_log_buffer(id).await?)
}

#[tauri::command]
pub async fn hosting_set_eula_accepted(
    id: String,
    accepted: bool,
) -> Result<()> {
    Ok(theseus::hosting::set_eula_accepted(id, accepted).await?)
}

#[tauri::command]
pub async fn hosting_get_directory(id: String) -> Result<String> {
    Ok(theseus::hosting::get_directory(id).await?)
}

#[tauri::command]
pub async fn hosting_content_summary(
    id: String,
) -> Result<theseus::hosting::HostedServerContentSummary> {
    Ok(theseus::hosting::content_summary(id).await?)
}

#[tauri::command]
pub async fn hosting_set_icon_path(
    id: String,
    icon_path: Option<String>,
) -> Result<()> {
    Ok(theseus::hosting::set_icon_path(id, icon_path).await?)
}

#[tauri::command]
pub async fn hosting_set_icon_from_path(id: String, path: String) -> Result<()> {
    Ok(theseus::hosting::set_icon_from_path(id, path).await?)
}

#[tauri::command]
pub async fn hosting_install_modrinth_file(
    id: String,
    content_dir: String,
    file_name: String,
    url: String,
    sha1: Option<String>,
) -> Result<()> {
    Ok(theseus::hosting::install_modrinth_file(
        id, content_dir, file_name, url, sha1,
    )
    .await?)
}

#[tauri::command]
pub async fn hosting_install_curseforge_file(
    id: String,
    content_dir: String,
    file_id: String,
) -> Result<()> {
    Ok(theseus::hosting::install_curseforge_file(id, content_dir, file_id).await?)
}

#[tauri::command]
pub async fn hosting_install_local_content_file(
    id: String,
    path: String,
) -> Result<()> {
    Ok(theseus::hosting::install_local_content_file(id, path).await?)
}

#[tauri::command]
pub async fn hosting_update_settings(
    id: String,
    port: u16,
    max_memory_mb: u32,
    extra_java_args: Option<String>,
) -> Result<()> {
    Ok(theseus::hosting::update_settings(
        id,
        port,
        max_memory_mb,
        extra_java_args,
    )
    .await?)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("hosting")
        .invoke_handler(tauri::generate_handler![
            hosting_list,
            hosting_get,
            hosting_create,
            hosting_delete,
            hosting_start,
            hosting_stop,
            hosting_send_command,
            hosting_is_running,
            hosting_get_process,
            hosting_get_log_buffer,
            hosting_set_eula_accepted,
            hosting_get_directory,
            hosting_content_summary,
            hosting_set_icon_path,
            hosting_set_icon_from_path,
            hosting_install_modrinth_file,
            hosting_install_curseforge_file,
            hosting_install_local_content_file,
            hosting_update_settings,
        ])
        .build()
}
