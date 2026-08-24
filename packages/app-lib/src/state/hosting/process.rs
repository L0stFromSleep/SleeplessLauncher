//! Process tracking for locally hosted server processes. Deliberately
//! separate from `state::process::ProcessManager`, which is entangled with
//! client-launch-specific concerns (RPC keep-alive wrapper, auth, Discord
//! RPC, window focus) that don't apply to a headless dedicated server.

use crate::state::process::{push_log_line, remove_log_buffer};
use crate::util::io::IOError;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::process::ExitStatus;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use uuid::Uuid;

pub struct HostedServerProcessManager {
    processes: DashMap<String, HostedServerProcess>,
}

impl Default for HostedServerProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

struct HostedServerProcess {
    metadata: HostedServerProcessMetadata,
    child: Child,
    stdin: Option<ChildStdin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostedServerProcessMetadata {
    pub uuid: Uuid,
    pub server_id: String,
    pub start_time: DateTime<Utc>,
}

impl HostedServerProcessManager {
    pub fn new() -> Self {
        Self {
            processes: DashMap::new(),
        }
    }

    /// Spawns the server process, piping stdout/stderr into the same
    /// log-buffer + event-emission mechanism the client launch path uses
    /// (`state::process::push_log_line` / `emit_process`), so the frontend
    /// console can reuse the exact same log-streaming plumbing.
    pub async fn launch(
        &self,
        server_id: &str,
        mut command: Command,
    ) -> crate::Result<HostedServerProcessMetadata> {
        if self.processes.contains_key(server_id) {
            return Err(crate::ErrorKind::InputError(format!(
                "Hosted server {server_id} is already running"
            ))
            .into());
        }

        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        command.stdin(std::process::Stdio::piped());

        let mut child = command.spawn().map_err(IOError::from)?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();

        let metadata = HostedServerProcessMetadata {
            uuid: Uuid::new_v4(),
            server_id: server_id.to_string(),
            start_time: Utc::now(),
        };

        remove_log_buffer(server_id);

        if let Some(stdout) = stdout {
            let id = server_id.to_string();
            tokio::spawn(async move {
                pipe_output_to_log(&id, stdout).await;
            });
        }
        if let Some(stderr) = stderr {
            let id = server_id.to_string();
            tokio::spawn(async move {
                pipe_output_to_log(&id, stderr).await;
            });
        }

        self.processes.insert(
            server_id.to_string(),
            HostedServerProcess {
                metadata: metadata.clone(),
                child,
                stdin,
            },
        );

        let id = server_id.to_string();
        tokio::spawn(async move {
            wait_for_exit(id).await;
        });

        Ok(metadata)
    }

    pub fn is_running(&self, server_id: &str) -> bool {
        self.processes.contains_key(server_id)
    }

    pub fn get(
        &self,
        server_id: &str,
    ) -> Option<HostedServerProcessMetadata> {
        self.processes.get(server_id).map(|p| p.metadata.clone())
    }

    /// Writes a line to the server's stdin, as if typed into its console
    /// (e.g. `/op player`, `stop`). A newline is appended automatically.
    pub async fn send_command(
        &self,
        server_id: &str,
        line: &str,
    ) -> crate::Result<()> {
        let mut process =
            self.processes.get_mut(server_id).ok_or_else(|| {
                crate::ErrorKind::InputError(format!(
                    "Hosted server {server_id} is not running"
                ))
            })?;

        let Some(stdin) = process.stdin.as_mut() else {
            return Err(crate::ErrorKind::OtherError(
                "Hosted server process has no stdin".to_string(),
            )
            .into());
        };

        stdin
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(IOError::from)?;
        stdin.flush().await.map_err(IOError::from)?;

        Ok(())
    }

    /// Asks the server to shut down gracefully via the `stop` console
    /// command, falling back to killing the process if it hasn't exited
    /// within `timeout`.
    pub async fn stop(
        &self,
        server_id: &str,
        timeout: std::time::Duration,
    ) -> crate::Result<()> {
        if !self.is_running(server_id) {
            return Ok(());
        }

        let _ = self.send_command(server_id, "stop").await;

        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if !self.is_running(server_id) {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }

        self.kill(server_id).await
    }

    pub async fn kill(&self, server_id: &str) -> crate::Result<()> {
        if let Some(mut process) = self.processes.get_mut(server_id) {
            process.child.kill().await.map_err(IOError::from)?;
        }
        Ok(())
    }

    fn remove(&self, server_id: &str) {
        self.processes.remove(server_id);
    }
}

async fn pipe_output_to_log<R>(server_id: &str, reader: R)
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    while let Ok(bytes_read) = buf_reader.read_line(&mut line).await {
        if bytes_read == 0 {
            break;
        }
        if !line.is_empty() {
            push_log_line(server_id, line.trim_end().to_string());

            #[cfg(feature = "tauri")]
            {
                let event_state = crate::EventState::get();
                let _ = event_state.send(crate::event::AppEvent::Log(
                    crate::event::LogPayload {
                        instance_id: server_id.to_string(),
                        event: crate::event::LogEvent::Legacy {
                            message: line.trim_end().to_string(),
                        },
                    },
                ));
            }
        }
        line.clear();
    }
}

async fn wait_for_exit(server_id: String) {
    loop {
        let Ok(state) = crate::State::get().await else {
            return;
        };

        let exit_status: Option<ExitStatus> = {
            let process =
                state.hosting_process_manager.processes.get_mut(&server_id);
            match process {
                Some(mut process) => match process.child.try_wait() {
                    Ok(status) => status,
                    Err(_) => Some(ExitStatus::default()),
                },
                None => return,
            }
        };

        if exit_status.is_some() {
            state.hosting_process_manager.remove(&server_id);
            #[cfg(feature = "tauri")]
            {
                let event_state = crate::EventState::get();
                let _ = event_state.send(crate::event::AppEvent::Log(
                    crate::event::LogPayload {
                        instance_id: server_id.clone(),
                        event: crate::event::LogEvent::Legacy {
                            message: "# Server process exited".to_string(),
                        },
                    },
                ));
            }
            return;
        }

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}
