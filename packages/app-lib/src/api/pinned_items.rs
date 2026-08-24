//! Public API surface for the sidebar pin list, mirroring `api::hosting`'s
//! role as a thin wrapper over `state::pinned_items`.

pub use crate::state::pinned_items::{PinnedItem, PinnedItemKind, PinnedItemRef};

#[tracing::instrument]
pub async fn list() -> crate::Result<Vec<PinnedItem>> {
    let state = crate::State::get().await?;
    crate::state::pinned_items::list(&state).await
}

#[tracing::instrument]
pub async fn pin(kind: PinnedItemKind, ref_id: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::pinned_items::pin(kind, &ref_id, &state).await
}

#[tracing::instrument]
pub async fn unpin(kind: PinnedItemKind, ref_id: String) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::pinned_items::unpin(kind, &ref_id, &state).await
}

#[tracing::instrument]
pub async fn set_order(ordered: Vec<PinnedItemRef>) -> crate::Result<()> {
    let state = crate::State::get().await?;
    crate::state::pinned_items::set_order(ordered, &state).await
}
