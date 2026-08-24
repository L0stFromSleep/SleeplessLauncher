use crate::api::Result;
use theseus::prelude::*;

#[tauri::command]
pub async fn pinned_items_list() -> Result<Vec<PinnedItem>> {
    Ok(theseus::pinned_items::list().await?)
}

#[tauri::command]
pub async fn pinned_items_pin(
    kind: PinnedItemKind,
    ref_id: String,
) -> Result<()> {
    Ok(theseus::pinned_items::pin(kind, ref_id).await?)
}

#[tauri::command]
pub async fn pinned_items_unpin(
    kind: PinnedItemKind,
    ref_id: String,
) -> Result<()> {
    Ok(theseus::pinned_items::unpin(kind, ref_id).await?)
}

#[tauri::command]
pub async fn pinned_items_set_order(
    ordered: Vec<PinnedItemRef>,
) -> Result<()> {
    Ok(theseus::pinned_items::set_order(ordered).await?)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("pinned-items")
        .invoke_handler(tauri::generate_handler![
            pinned_items_list,
            pinned_items_pin,
            pinned_items_unpin,
            pinned_items_set_order,
        ])
        .build()
}
