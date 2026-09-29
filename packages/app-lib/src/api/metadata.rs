use crate::State;
use crate::state::CachedEntry;
pub use daedalus::minecraft::VersionManifest;
pub use daedalus::modded::Manifest;

#[tracing::instrument]
pub async fn get_minecraft_versions() -> crate::Result<VersionManifest> {
    let state = State::get().await?;
    let minecraft_versions = CachedEntry::get_minecraft_manifest(
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor("minecraft versions".to_string())
    })?;

    Ok(minecraft_versions)
}

// #[tracing::instrument]
pub async fn get_loader_versions(loader: &str) -> crate::Result<Manifest> {
    let state = State::get().await?;
    let cache_key =
        daedalus::modded::loader_manifest_metadata(loader).cache_key;
    let loaders = CachedEntry::get_loader_manifest(
        &cache_key,
        None,
        &state.pool,
        &state.api_semaphore,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::NoValueFor(format!("{loader} loader versions"))
    })?;

    let mut manifest = loaders.manifest;

    // Modrinth's hosted manifest can be missing game versions it should
    // cover (see `launcher::loader_fallback`); patch any gaps in from the
    // loader's own upstream metadata so consumers of this manifest (e.g. the
    // instance version/loader pickers) see the full picture rather than
    // whatever subset happened to make it into the last generated manifest.
    if let Some(mod_loader) = crate::data::ModLoader::from_meta_str(loader)
        && let Err(err) = crate::launcher::loader_fallback::patch_manifest_gaps(
            mod_loader,
            &mut manifest,
        )
        .await
    {
        // The upstream fallback source being unreachable (offline, host
        // down) shouldn't take down the whole manifest -- callers still get
        // whatever Modrinth's own (possibly incomplete) manifest has.
        tracing::warn!(
            loader,
            %err,
            "Failed to patch loader manifest gaps from upstream metadata"
        );
    }

    Ok(manifest)
}
