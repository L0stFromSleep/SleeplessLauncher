//! Starting/stopping a hosted server's process.

use super::{HostedServer, HostedServerInstallStage};
use crate::state::{JavaVersion, ModLoader, State};
use crate::util::io;
use tokio::process::Command;

pub async fn start(server_id: &str, state: &State) -> crate::Result<()> {
    let server = HostedServer::get(server_id, state).await?.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "Unknown hosted server {server_id}"
        ))
    })?;

    if server.install_stage != HostedServerInstallStage::Installed {
        return Err(crate::ErrorKind::InputError(
            "This server hasn't finished installing yet".to_string(),
        )
        .into());
    }
    if !server.eula_accepted {
        return Err(crate::ErrorKind::InputError(
            "You must accept the Minecraft EULA before starting this server"
                .to_string(),
        )
        .into());
    }
    if state.hosting_process_manager.is_running(server_id) {
        return Ok(());
    }

    let server_dir = state.directories.hosted_server_dir(server_id);
    io::write(server_dir.join("eula.txt"), b"eula=true\n").await?;

    // The server was installed against a specific Java major version (it
    // may need a much newer JRE than whatever `java` happens to resolve to
    // on PATH -- e.g. a bundled/older client JRE) -- resolve the exact
    // executable that was downloaded for it during install rather than
    // trusting PATH.
    let java_major_version = server.java_major_version.ok_or_else(|| {
        crate::ErrorKind::LauncherError(
            "This server has no resolved Java version -- try reinstalling it"
                .to_string(),
        )
    })?;
    let java_version = JavaVersion::get(java_major_version, &state.pool)
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::LauncherError(format!(
                "Java {java_major_version} isn't installed -- try reinstalling this server"
            ))
        })?;

    let command = match server.loader {
        ModLoader::Forge | ModLoader::NeoForge => {
            build_run_script_command(&server, &server_dir, &java_version)
                .await?
        }
        _ => build_jar_command(&server, &server_dir, &java_version),
    };

    state.hosting_process_manager.launch(server_id, command).await?;
    Ok(())
}

fn build_jar_command(
    server: &HostedServer,
    server_dir: &std::path::Path,
    java_version: &JavaVersion,
) -> Command {
    let jar_path = server_dir.join("server.jar");
    let mut command = Command::new(&java_version.path);
    command
        .current_dir(server_dir)
        .arg(format!("-Xmx{}M", server.max_memory_mb));
    if let Some(extra_args) = &server.extra_java_args
        && let Some(parts) = shlex::split(extra_args)
    {
        command.args(parts);
    }
    command.arg("-jar").arg(jar_path).arg("nogui");
    command
}

async fn build_run_script_command(
    server: &HostedServer,
    server_dir: &std::path::Path,
    java_version: &JavaVersion,
) -> crate::Result<Command> {
    // Modern Forge/NeoForge run scripts read JVM args from this file by
    // convention -- write it unconditionally so the memory/extra-args
    // settings actually take effect.
    let mut jvm_args = format!("-Xmx{}M", server.max_memory_mb);
    if let Some(extra_args) = &server.extra_java_args {
        jvm_args.push(' ');
        jvm_args.push_str(extra_args);
    }
    io::write(server_dir.join("user_jvm_args.txt"), jvm_args.as_bytes())
        .await?;

    let script_name =
        if cfg!(target_os = "windows") { "run.bat" } else { "run.sh" };
    let script_path = server_dir.join(script_name);
    if !script_path.exists() {
        return Err(crate::ErrorKind::LauncherError(format!(
            "{script_name} not found in {} -- this server wasn't installed \
             successfully",
            server_dir.display()
        ))
        .as_error());
    }

    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(&script_path).arg("nogui");
        command
    } else {
        let mut command = Command::new("sh");
        command.arg(&script_path).arg("nogui");
        command
    };
    command.current_dir(server_dir);

    // The generated run script invokes a bare `java` itself rather than
    // taking a path, so the resolved JRE's `bin` directory is prepended to
    // PATH for this process rather than trusting whatever `java` the user's
    // own environment resolves to.
    if let Some(java_bin_dir) = std::path::Path::new(&java_version.path).parent() {
        let path_var = if cfg!(target_os = "windows") {
            "Path"
        } else {
            "PATH"
        };
        let existing_path = std::env::var_os(path_var).unwrap_or_default();
        if let Ok(new_path) = std::env::join_paths(
            std::iter::once(java_bin_dir.to_path_buf())
                .chain(std::env::split_paths(&existing_path)),
        ) {
            command.env(path_var, new_path);
        }
    }

    Ok(command)
}

pub async fn stop(server_id: &str, state: &State) -> crate::Result<()> {
    state
        .hosting_process_manager
        .stop(server_id, std::time::Duration::from_secs(30))
        .await
}

pub async fn send_command(
    server_id: &str,
    line: &str,
    state: &State,
) -> crate::Result<()> {
    state.hosting_process_manager.send_command(server_id, line).await
}
