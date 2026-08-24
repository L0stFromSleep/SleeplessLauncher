//! Sidebar pin list -- lets the user pin both client instances and locally
//! hosted servers (see `state::hosting`) into the quick-switcher sidebar.
//! Kept as its own small table (rather than reusing instance groups, see
//! `api::instance::groups::FAVORITES_GROUP_ID`) since that system is
//! instance-only and isn't a natural fit for hosted servers too.

use serde::{Deserialize, Serialize};

use crate::state::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PinnedItemKind {
    Instance,
    HostedServer,
}

impl PinnedItemKind {
    fn as_str(&self) -> &'static str {
        match self {
            PinnedItemKind::Instance => "instance",
            PinnedItemKind::HostedServer => "hosted_server",
        }
    }

    fn from_str(s: &str) -> crate::Result<Self> {
        match s {
            "instance" => Ok(PinnedItemKind::Instance),
            "hosted_server" => Ok(PinnedItemKind::HostedServer),
            other => Err(crate::ErrorKind::InputError(format!(
                "Unknown pinned item kind {other}"
            ))
            .into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinnedItem {
    pub kind: PinnedItemKind,
    pub ref_id: String,
    pub position: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinnedItemRef {
    pub kind: PinnedItemKind,
    pub ref_id: String,
}

pub async fn list(state: &State) -> crate::Result<Vec<PinnedItem>> {
    let rows = sqlx::query!(
        "SELECT kind, ref_id, position FROM pinned_items ORDER BY position ASC"
    )
    .fetch_all(&state.pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok(PinnedItem {
                kind: PinnedItemKind::from_str(&row.kind)?,
                ref_id: row.ref_id,
                position: row.position,
            })
        })
        .collect()
}

pub async fn pin(
    kind: PinnedItemKind,
    ref_id: &str,
    state: &State,
) -> crate::Result<()> {
    let kind_str = kind.as_str();
    let next_position: i64 = sqlx::query_scalar!(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM pinned_items"
    )
    .fetch_one(&state.pool)
    .await?;

    sqlx::query!(
        "INSERT INTO pinned_items (kind, ref_id, position, created)
         VALUES (?, ?, ?, unixepoch())
         ON CONFLICT (kind, ref_id) DO NOTHING",
        kind_str,
        ref_id,
        next_position,
    )
    .execute(&state.pool)
    .await?;

    Ok(())
}

pub async fn unpin(
    kind: PinnedItemKind,
    ref_id: &str,
    state: &State,
) -> crate::Result<()> {
    let kind_str = kind.as_str();
    sqlx::query!(
        "DELETE FROM pinned_items WHERE kind = ? AND ref_id = ?",
        kind_str,
        ref_id,
    )
    .execute(&state.pool)
    .await?;

    Ok(())
}

pub async fn set_order(
    ordered: Vec<PinnedItemRef>,
    state: &State,
) -> crate::Result<()> {
    let mut tx = state.pool.begin().await?;

    for (position, item) in ordered.into_iter().enumerate() {
        let kind_str = item.kind.as_str();
        let position = position as i64;
        sqlx::query!(
            "UPDATE pinned_items SET position = ? WHERE kind = ? AND ref_id = ?",
            position,
            kind_str,
            item.ref_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(())
}
