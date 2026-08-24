//! Acquires a *server*-side runnable for each loader family, for locally
//! hosted servers (`state::hosting`). This is the server-side counterpart to
//! the client install flow in `launcher::mod` (`install_minecraft_with_reporter`),
//! reusing the same underlying pieces (version manifest resolution, merged
//! loader "version info", library downloading, Java resolution, and -- for
//! Forge/NeoForge -- the same native install-profile "processor" execution
//! engine, just pointed at the server side of each processor's data instead
//! of the client side) rather than re-deriving any of it.
//!
//! Vanilla and Fabric/Quilt are well-defined, single-file downloads and
//! should be solid (Fabric/Quilt's server-jar route requires an explicit
//! installer-version path segment, fetched from their `/versions/installer`
//! endpoint -- confirmed against a live 404 during testing, see
//! `fetch_latest_installer_version`). Forge/NeoForge server installs are
//! inherently more fragile (their install profiles vary release to release)
//! and this has not been exercised against a live install -- see
//! `acquire_forge_like_server`.

use super::args::ProcessorSide;
use super::{download, get_loader_version_from_profile, resolve_minecraft_manifest};
use crate::install::{
    InstallPhaseDetails, InstallPhaseId, InstallProgress, InstallProgressReporter,
};
use crate::state::{JavaVersion, ModLoader, State};
use crate::util::fetch;
use crate::util::io::{self, IOError};
use daedalus::minecraft::DownloadType;
use daedalus::modded::{LoaderVersion, SidedDataEntry};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// What a hosted server needs in order to be launched: either a plain jar
/// (`java -jar <jar> nogui`, for Vanilla/Fabric/Quilt), or a generated run
/// script (Forge/NeoForge, which -- like the real installers -- write a
/// `run.sh`/`run.bat` wrapping a much larger, version-specific classpath/
/// argument list that isn't worth hand-reconstructing here).
pub enum ServerRunnable {
    Jar(PathBuf),
    RunScript(PathBuf),
}

pub struct AcquiredServerLoader {
    pub runnable: ServerRunnable,
    pub java_major_version: u32,
}

pub async fn acquire_server_loader(
    server_dir: &Path,
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<AcquiredServerLoader> {
    io::create_dir_all(server_dir).await?;

    match loader {
        ModLoader::Vanilla => {
            acquire_vanilla_server(server_dir, game_version, state, reporter)
                .await
        }
        ModLoader::Fabric | ModLoader::Quilt => {
            acquire_fabric_like_server(
                server_dir,
                game_version,
                loader,
                loader_version,
                state,
                reporter,
            )
            .await
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            acquire_forge_like_server(
                server_dir,
                game_version,
                loader,
                loader_version,
                state,
                reporter,
            )
            .await
        }
    }
}

async fn resolve_server_java(
    java_major_version: u32,
) -> crate::Result<JavaVersion> {
    let state = State::get().await?;
    let path = crate::api::jre::auto_install_java(java_major_version).await?;
    let java_version = crate::api::jre::check_jre(path).await?;
    java_version.upsert(&state.pool).await?;
    Ok(java_version)
}

async fn acquire_vanilla_server(
    server_dir: &Path,
    game_version: &str,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<AcquiredServerLoader> {
    reporter
        .update(InstallPhaseId::ResolvingMinecraft, None, InstallPhaseDetails::Empty)
        .await?;
    let (minecraft, version_index) =
        resolve_minecraft_manifest(game_version, state).await?;
    let version = &minecraft.versions[version_index];

    let version_info = download::download_version_info(
        state, version, None, None, None, None,
    )
    .await?;

    let server_download =
        version_info.downloads.get(&DownloadType::Server).ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Minecraft {game_version} does not publish a server download"
            ))
        })?;

    reporter
        .update(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: 0,
                total: server_download.size as u64,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;
    let bytes = fetch::fetch(
        &server_download.url,
        Some(&server_download.sha1),
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    reporter
        .update(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: server_download.size as u64,
                total: server_download.size as u64,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;

    let jar_path = server_dir.join("server.jar");
    io::write(&jar_path, &bytes).await?;

    let java_major_version =
        version_info.java_version.as_ref().map_or(8, |it| it.major_version);
    resolve_server_java(java_major_version).await?;

    Ok(AcquiredServerLoader {
        runnable: ServerRunnable::Jar(jar_path),
        java_major_version,
    })
}

/// Both Fabric's and Quilt's meta APIs (`/v2/versions/installer` and
/// `/v3/versions/installer` respectively) return installer versions
/// newest-first, each optionally flagged `stable`.
#[derive(serde::Deserialize)]
struct MetaInstallerVersion {
    version: String,
    #[serde(default)]
    stable: bool,
}

async fn fetch_latest_installer_version(
    installer_versions_url: &str,
    state: &State,
) -> crate::Result<String> {
    let bytes = fetch::fetch(
        installer_versions_url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    let versions: Vec<MetaInstallerVersion> = serde_json::from_slice(&bytes)?;
    versions
        .iter()
        .find(|v| v.stable)
        .or_else(|| versions.first())
        .map(|v| v.version.clone())
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "No installer versions published at {installer_versions_url}"
            ))
            .into()
        })
}

/// Fabric and Quilt both publish a ready-to-run "server launcher" fat jar
/// directly from their meta APIs -- no installer execution needed. Unlike
/// the client loader-version endpoint, the server jar route requires an
/// explicit installer version segment (there's no "latest" shorthand), so
/// this fetches the newest stable installer version first.
async fn acquire_fabric_like_server(
    server_dir: &Path,
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<AcquiredServerLoader> {
    reporter
        .update(InstallPhaseId::ResolvingLoader, None, InstallPhaseDetails::Empty)
        .await?;
    let resolved_loader_version = get_loader_version_from_profile(
        game_version,
        loader,
        loader_version,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "No {} loader version found for Minecraft {game_version}",
            loader.as_str()
        ))
    })?;

    let (base_url, installer_versions_url) = match loader {
        ModLoader::Fabric => (
            "https://meta.fabricmc.net/v2/versions/loader",
            "https://meta.fabricmc.net/v2/versions/installer",
        ),
        ModLoader::Quilt => (
            "https://meta.quiltmc.org/v3/versions/loader",
            "https://meta.quiltmc.org/v3/versions/installer",
        ),
        _ => unreachable!(),
    };
    let installer_version =
        fetch_latest_installer_version(installer_versions_url, state).await?;
    let url = format!(
        "{base_url}/{game_version}/{}/{installer_version}/server/jar",
        resolved_loader_version.id
    );

    reporter
        .update(
            InstallPhaseId::DownloadingMinecraft,
            None,
            InstallPhaseDetails::Empty,
        )
        .await?;
    let bytes = fetch::fetch(
        &url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await
    .map_err(|err| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to download {} server launcher for Minecraft {game_version}: {err}",
            loader.as_str()
        ))
    })?;

    let jar_path = server_dir.join("server.jar");
    io::write(&jar_path, &bytes).await?;

    // Fabric/Quilt server launchers run fine on the same Java version the
    // client would use for this Minecraft version; resolve it the same way
    // `resolve_java_for_launch` does for the client, minus the instance
    // context we don't have here.
    let (minecraft, version_index) =
        resolve_minecraft_manifest(game_version, state).await?;
    let version = &minecraft.versions[version_index];
    let version_info = download::download_version_info(
        state, version, None, None, None, None,
    )
    .await?;
    let java_major_version =
        version_info.java_version.as_ref().map_or(8, |it| it.major_version);
    resolve_server_java(java_major_version).await?;

    Ok(AcquiredServerLoader {
        runnable: ServerRunnable::Jar(jar_path),
        java_major_version,
    })
}

/// Downloads the real Forge/NeoForge installer jar (from Forge's/NeoForge's
/// own Maven, not Modrinth's launcher-meta mirror) so its path can be used
/// as the `{INSTALLER}` processor substitution below. Some newer install
/// profiles (confirmed on NeoForge 26.2.x) include an `ExtractFiles`
/// processor that opens `{INSTALLER}` as a zip archive to pull the
/// `run.sh`/`run.bat`/`user_jvm_args.txt` files bundled inside the
/// installer's own `data/` folder -- unlike the other built-in substitution
/// vars (`SIDE`, `ROOT`, `MINECRAFT_JAR`, ...), Modrinth's launcher-meta
/// doesn't republish these particular files as a downloadable library (it
/// never needed to, since Modrinth's own client only ever runs the
/// `client`-side processors), so this has to fetch the genuine installer
/// jar directly, exactly like the real Forge/NeoForge installer app would
/// already have its own jar path available for this substitution.
async fn download_forge_like_installer_jar(
    game_version: &str,
    loader: ModLoader,
    resolved_loader_version: &LoaderVersion,
    server_dir: &Path,
    state: &State,
) -> crate::Result<PathBuf> {
    let (url, filename) = match loader {
        ModLoader::Forge => {
            let full_version =
                format!("{game_version}-{}", resolved_loader_version.id);
            (
                format!(
                    "https://maven.minecraftforge.net/net/minecraftforge/forge/{full_version}/forge-{full_version}-installer.jar"
                ),
                format!("forge-{full_version}-installer.jar"),
            )
        }
        ModLoader::NeoForge => {
            let version = &resolved_loader_version.id;
            (
                format!(
                    "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
                ),
                format!("neoforge-{version}-installer.jar"),
            )
        }
        _ => unreachable!("only called for Forge/NeoForge"),
    };

    let bytes = fetch::fetch(
        &url,
        None,
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await
    .map_err(|err| {
        crate::ErrorKind::LauncherError(format!(
            "Failed to download {} installer for Minecraft {game_version}: {err}",
            loader.as_str()
        ))
    })?;

    // Kept in its own subdirectory (rather than the server's own directory
    // tree) so it doesn't linger as a stray file once the install profile
    // has been fully applied.
    let installer_dir = server_dir.join(".installer");
    io::create_dir_all(&installer_dir).await?;
    let installer_path = installer_dir.join(&filename);
    io::write(&installer_path, &bytes).await?;

    Ok(installer_path)
}

/// Forge and NeoForge don't publish a plain server jar. Instead their
/// installer runs an "install profile" (a set of `processors`, each a Java
/// tool invocation, driven by side-specific `data` substitutions) that
/// patches together the server's libraries and, for modern versions, writes
/// a `run.sh`/`run.bat` launch script. This reuses the exact same
/// processor-execution engine the client install path uses
/// (`launcher::args::get_processor_arguments`, now parameterized by
/// `ProcessorSide`), just filtered to `server`-applicable processors with
/// server-side substitution values, instead of client ones.
async fn acquire_forge_like_server(
    server_dir: &Path,
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
    state: &State,
    reporter: &InstallProgressReporter,
) -> crate::Result<AcquiredServerLoader> {
    reporter
        .update(InstallPhaseId::ResolvingLoader, None, InstallPhaseDetails::Empty)
        .await?;
    let (minecraft, version_index) =
        resolve_minecraft_manifest(game_version, state).await?;
    let version = &minecraft.versions[version_index];

    let resolved_loader_version = get_loader_version_from_profile(
        game_version,
        loader,
        loader_version,
    )
    .await?
    .ok_or_else(|| {
        crate::ErrorKind::LauncherError(format!(
            "No {} loader version found for Minecraft {game_version}",
            loader.as_str()
        ))
    })?;

    let mut version_info = download::download_version_info(
        state,
        version,
        Some(&resolved_loader_version),
        None,
        None,
        None,
    )
    .await?;

    let java_major_version =
        version_info.java_version.as_ref().map_or(8, |it| it.major_version);
    let java_version = resolve_server_java(java_major_version).await?;

    // The server jar is required as the `MINECRAFT_JAR` processor
    // substitution below, same as the client jar is for a client install.
    let server_jar_download = version_info
        .downloads
        .get(&DownloadType::Server)
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Minecraft {game_version} does not publish a server download"
            ))
        })?;
    reporter
        .update(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: 0,
                total: server_jar_download.size as u64,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;
    let server_jar_bytes = fetch::fetch(
        &server_jar_download.url,
        Some(&server_jar_download.sha1),
        None,
        None,
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;
    let server_jar_path = server_dir.join(format!("minecraft_server.{game_version}.jar"));
    io::write(&server_jar_path, &server_jar_bytes).await?;

    let libraries_dir = state.directories.libraries_dir();
    reporter
        .update(
            InstallPhaseId::DownloadingMinecraft,
            Some(InstallProgress {
                current: 0,
                total: version_info.libraries.len() as u64,
                secondary: None,
            }),
            InstallPhaseDetails::Empty,
        )
        .await?;
    download::download_libraries(
        state,
        &version_info.libraries,
        &version.id,
        None,
        0.0,
        &java_version.architecture,
        false,
        true,
        None,
    )
    .await?;

    let installer_jar_path = download_forge_like_installer_jar(
        game_version,
        loader,
        &resolved_loader_version,
        server_dir,
        state,
    )
    .await?;

    let processors = version_info.processors.take();
    if let Some(processors) = processors
        && let Some(ref mut data) = version_info.data
    {
        insert_server_processor_vars(
            data,
            &server_jar_path,
            &installer_jar_path,
            game_version,
            server_dir,
            &libraries_dir,
        );

        let server_processor_count = processors
            .iter()
            .filter(|processor| {
                processor
                    .sides
                    .as_ref()
                    .is_none_or(|sides| sides.contains(&String::from("server")))
            })
            .count() as u64;
        let mut completed_processors = 0u64;
        reporter
            .update(
                InstallPhaseId::RunningLoaderProcessors,
                Some(InstallProgress {
                    current: 0,
                    total: server_processor_count,
                    secondary: None,
                }),
                InstallPhaseDetails::Empty,
            )
            .await?;

        for processor in &processors {
            if let Some(sides) = &processor.sides
                && !sides.contains(&String::from("server"))
            {
                continue;
            }

            let cp = {
                let mut cp = processor.classpath.clone();
                cp.push(processor.jar.clone());
                cp
            };

            let output = Command::new(&java_version.path)
                .arg("-cp")
                .arg(super::args::get_class_paths_jar(
                    &libraries_dir,
                    &cp,
                    &java_version.architecture,
                )?)
                .arg(
                    super::args::get_processor_main_class(
                        super::args::get_lib_path(
                            &libraries_dir,
                            &processor.jar,
                            false,
                        )?,
                    )
                    .await?
                    .ok_or_else(|| {
                        crate::ErrorKind::LauncherError(format!(
                            "Could not find processor main class for {}",
                            processor.jar
                        ))
                    })?,
                )
                .args(super::args::get_processor_arguments(
                    &libraries_dir,
                    &processor.args,
                    data,
                    ProcessorSide::Server,
                )?)
                .output()
                .await
                .map_err(|e| IOError::with_path(e, &java_version.path))
                .map_err(|err| {
                    crate::ErrorKind::LauncherError(format!(
                        "Error running {} server installer processor: {err}",
                        loader.as_str()
                    ))
                })?;

            if !output.status.success() {
                return Err(crate::ErrorKind::LauncherError(format!(
                    "{} server installer processor failed: {}",
                    loader.as_str(),
                    String::from_utf8_lossy(&output.stderr)
                ))
                .as_error());
            }

            completed_processors += 1;
            reporter
                .update(
                    InstallPhaseId::RunningLoaderProcessors,
                    Some(InstallProgress {
                        current: completed_processors,
                        total: server_processor_count,
                        secondary: None,
                    }),
                    InstallPhaseDetails::Empty,
                )
                .await?;
        }
    }

    // The installer jar was only needed to drive the processors above (its
    // path was substituted in for `{INSTALLER}`) -- clean it up now rather
    // than leaving a stray multi-megabyte jar sitting in the server's
    // directory.
    if let Some(installer_dir) = installer_jar_path.parent() {
        let _ = io::remove_dir_all(installer_dir).await;
    }

    let run_script_name =
        if cfg!(target_os = "windows") { "run.bat" } else { "run.sh" };
    let run_script = server_dir.join(run_script_name);
    if run_script.exists() {
        return Ok(AcquiredServerLoader {
            runnable: ServerRunnable::RunScript(run_script),
            java_major_version,
        });
    }

    Err(crate::ErrorKind::LauncherError(format!(
        "{} {game_version} server install finished, but no {run_script_name} \
         was generated -- this loader/version combination isn't supported \
         for hosting yet.",
        loader.as_str()
    ))
    .as_error())
}

fn insert_server_processor_vars(
    data: &mut HashMap<String, SidedDataEntry>,
    server_jar_path: &Path,
    installer_jar_path: &Path,
    game_version: &str,
    server_dir: &Path,
    libraries_dir: &Path,
) {
    let mut set = |key: &str, value: String| {
        data.insert(
            key.to_string(),
            SidedDataEntry {
                client: String::new(),
                server: value,
            },
        );
    };

    set("SIDE", "server".to_string());
    set("MINECRAFT_JAR", server_jar_path.to_string_lossy().into_owned());
    set("MINECRAFT_VERSION", game_version.to_string());
    set("ROOT", server_dir.to_string_lossy().into_owned());
    set("LIBRARY_DIR", libraries_dir.to_string_lossy().into_owned());
    set("INSTALLER", installer_jar_path.to_string_lossy().into_owned());
}
