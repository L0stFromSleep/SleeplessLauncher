//! Applies [`ModJarPatch`] overrides to an instance's mod jars right before
//! launch.
//!
//! Each configured patch keeps a pristine copy of its target jar
//! (`<jar>.orig`) next to it the first time it's seen, and always rebuilds
//! the live jar from that pristine copy rather than editing it
//! incrementally. That makes patches fully reversible (clear the setting
//! and the original comes back) and safe to edit back and forth (a mixin
//! taken off the removal list reappears, a replaced file reverts) instead
//! of compounding edits on top of each other.
//!
//! Three things can require a rebuild on a given launch: the patch
//! definition changed, the jar in `mods/` no longer matches what we last
//! produced (for example because the launcher's own integrity check, or
//! the user, reverted/updated it), or there's no prior state at all. All
//! three are detected with cheap `(len, mtime)` checks so an unmodified
//! instance costs only a couple of `stat` calls per configured jar on every
//! launch, never touching the file's contents.
use crate::state::instances::ModJarPatch;
use crate::util::io::{self, IOError};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing::warn;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// Mod jars live under this folder within an instance.
const MODS_FOLDER: &str = "mods";

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default,
)]
struct FileFingerprint {
    len: u64,
    modified_secs: i64,
}

impl FileFingerprint {
    async fn of(path: &Path) -> Option<Self> {
        let metadata = io::metadata(path).await.ok()?;
        let modified_secs = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Some(Self {
            len: metadata.len(),
            modified_secs,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct JarPatchState {
    patch_fingerprint: String,
    applied_output: FileFingerprint,
}

/// Applies every configured [`ModJarPatch`] to the jars in
/// `<instance_dir>/mods`. Failures patching one jar are logged and skipped
/// rather than aborting the launch, since a stale or misconfigured patch
/// for one mod shouldn't stop every other instance (or the rest of this
/// one) from starting.
pub async fn apply_jar_patches(
    instance_dir: &Path,
    patches: &[ModJarPatch],
) -> crate::Result<()> {
    let mods_dir = instance_dir.join(MODS_FOLDER);

    for patch in patches {
        if patch.jar_file_name.trim().is_empty() || patch.is_noop() {
            continue;
        }

        let jar_path = mods_dir.join(&patch.jar_file_name);
        if let Err(err) = apply_single_jar_patch(&jar_path, patch).await {
            warn!(
                "Skipping jar patch for {} ({}): {err:?}",
                patch.jar_file_name,
                jar_path.display()
            );
        }
    }

    Ok(())
}

async fn apply_single_jar_patch(
    jar_path: &Path,
    patch: &ModJarPatch,
) -> crate::Result<()> {
    if io::metadata(jar_path).await.is_err() {
        // This instance doesn't have the target mod installed; nothing to
        // patch.
        return Ok(());
    }

    let backup_path = sibling_path(jar_path, "orig");
    let state_path = sibling_path(jar_path, "patchstate.json");

    let current_fingerprint =
        FileFingerprint::of(jar_path).await.unwrap_or_default();
    let backup_fingerprint = FileFingerprint::of(&backup_path).await;

    match backup_fingerprint {
        None => {
            // First time we've seen this jar: it's our pristine baseline.
            io::copy(jar_path, &backup_path).await?;
        }
        Some(backup_fingerprint)
            if backup_fingerprint != current_fingerprint =>
        {
            let state = read_state(&state_path).await;
            let is_our_own_output = state
                .as_ref()
                .is_some_and(|s| s.applied_output == current_fingerprint);
            if !is_our_own_output {
                // The jar in `mods/` is neither the pristine copy nor the
                // last thing we produced from it (most likely the mod was
                // updated or reinstalled): treat it as the new baseline.
                io::copy(jar_path, &backup_path).await?;
            }
        }
        _ => {}
    }

    let patch_fingerprint = compute_patch_fingerprint(patch).await?;
    let state = read_state(&state_path).await;
    if let Some(state) = &state
        && state.patch_fingerprint == patch_fingerprint
        && Some(state.applied_output) == FileFingerprint::of(jar_path).await
    {
        // Already up to date.
        return Ok(());
    }

    rebuild_patched_jar(&backup_path, jar_path, patch).await?;
    clear_connector_cache(jar_path).await;

    let applied_output =
        FileFingerprint::of(jar_path).await.ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "patched jar vanished immediately after writing it: {}",
                jar_path.display()
            ))
        })?;
    write_state(
        &state_path,
        &JarPatchState {
            patch_fingerprint,
            applied_output,
        },
    )
    .await?;

    Ok(())
}

fn sibling_path(path: &Path, extra_extension: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(extra_extension);
    path.with_file_name(name)
}

async fn compute_patch_fingerprint(
    patch: &ModJarPatch,
) -> crate::Result<String> {
    // Small, deterministic summary of everything that can change the
    // patched output: the patch definition itself, plus the current
    // contents of whatever files it reads from disk (so editing a
    // replacement file is picked up without needing to touch the setting).
    let mut replacement_hashes: BTreeMap<String, String> = BTreeMap::new();
    for (entry_path, source) in &patch.replace_entries {
        let bytes = io::read(source).await?;
        replacement_hashes.insert(
            entry_path.clone(),
            format!("{:x}", sha2::Sha256::digest(&bytes)),
        );
    }

    let summary = serde_json::json!({
        "jar_file_name": patch.jar_file_name,
        "replacement_hashes": replacement_hashes,
        "remove_mixins": patch.remove_mixins,
    });
    let bytes = serde_json::to_vec(&summary)?;
    Ok(format!("{:x}", sha2::Sha256::digest(&bytes)))
}

async fn read_state(state_path: &Path) -> Option<JarPatchState> {
    let bytes = io::read(state_path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

async fn write_state(
    state_path: &Path,
    state: &JarPatchState,
) -> crate::Result<()> {
    let bytes = serde_json::to_vec(state)?;
    io::write(state_path, bytes).await?;
    Ok(())
}

/// Rebuilds `out_path` from `orig_path`, applying `patch`'s entry
/// replacements and mixin removals, and copying every other entry through
/// unchanged (via `raw_copy_file`, so large untouched entries are neither
/// decompressed nor recompressed).
async fn rebuild_patched_jar(
    orig_path: &Path,
    out_path: &Path,
    patch: &ModJarPatch,
) -> crate::Result<()> {
    let orig_path = orig_path.to_owned();
    let out_path = out_path.to_owned();
    let patch = patch.clone();

    tokio::task::spawn_blocking(move || {
        rebuild_patched_jar_blocking(&orig_path, &out_path, &patch)
    })
    .await
    .map_err(|_| {
        crate::ErrorKind::OtherError("jar patching task panicked".to_string())
    })??;

    Ok(())
}

fn rebuild_patched_jar_blocking(
    orig_path: &Path,
    out_path: &Path,
    patch: &ModJarPatch,
) -> crate::Result<()> {
    let orig_file = std::fs::File::open(orig_path)
        .map_err(|e| IOError::with_path(e, orig_path))?;
    let mut archive =
        ZipArchive::new(orig_file).map_err(std::io::Error::from)?;

    let parent = out_path.parent().ok_or_else(|| {
        crate::ErrorKind::OtherError(format!(
            "no parent directory for {}",
            out_path.display()
        ))
    })?;
    let tempfile =
        tempfile::NamedTempFile::new_in(parent).map_err(IOError::from)?;

    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(tempfile);

    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(std::io::Error::from)?;
        let name = entry.name().to_string();

        if let Some(replacement_path) = patch.replace_entries.get(&name) {
            let bytes = std::fs::read(replacement_path)
                .map_err(|e| IOError::with_path(e, replacement_path))?;
            drop(entry);
            writer
                .start_file(&name, options)
                .map_err(std::io::Error::from)?;
            writer.write_all(&bytes)?;
        } else if let Some(mixin_names) = patch.remove_mixins.get(&name) {
            let mut entry = entry;
            let mut raw = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut raw)?;
            let patched = strip_mixin_names(&raw, mixin_names)
                .unwrap_or_else(|| raw.clone());
            writer
                .start_file(&name, options)
                .map_err(std::io::Error::from)?;
            writer.write_all(&patched)?;
        } else {
            writer.raw_copy_file(entry).map_err(std::io::Error::from)?;
        }
    }

    let tempfile = writer.finish().map_err(std::io::Error::from)?;
    let tmp_path = tempfile.into_temp_path();
    tmp_path
        .persist(out_path)
        .map_err(|e| IOError::with_path(e.error, out_path))?;

    Ok(())
}

/// Removes `names` from every string array in a SpongePowered Mixin config
/// JSON file (covers `"mixins"`, `"client"`, `"server"`, or any other array
/// of class names the mod happens to use), leaving everything else
/// untouched. Returns `None` if the file isn't valid JSON, in which case
/// the caller falls back to leaving it unmodified rather than discarding
/// the entry.
fn strip_mixin_names(bytes: &[u8], names: &[String]) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if let serde_json::Value::Object(map) = &mut value {
        for entry_value in map.values_mut() {
            if let serde_json::Value::Array(items) = entry_value {
                items.retain(|item| {
                    !matches!(
                        item,
                        serde_json::Value::String(s)
                            if names.iter().any(|name| name == s)
                    )
                });
            }
        }
    }
    serde_json::to_vec_pretty(&value).ok()
}

/// Best-effort removal of the Sinytra Connector's cached, already-processed
/// copy of a jar we just rewrote, so it reprocesses the patched version
/// instead of serving the stale cache. Harmless (and silently skipped) for
/// instances that aren't running Connector.
async fn clear_connector_cache(jar_path: &Path) {
    let Some(instance_dir) = jar_path.parent().and_then(Path::parent) else {
        return;
    };
    let Some(jar_name) = jar_path.file_name() else {
        return;
    };

    let cache_dir = instance_dir.join(".cache").join("connector");
    let cached_jar = cache_dir.join(jar_name);
    let mut cached_input = cached_jar.clone().into_os_string();
    cached_input.push(".input");

    let _ = io::remove_file(&cached_jar).await;
    let _ = io::remove_file(PathBuf::from(cached_input)).await;
}
