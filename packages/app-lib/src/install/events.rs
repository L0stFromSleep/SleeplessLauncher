use super::model::{
    InstallErrorContext, InstallErrorView, InstallJobEventKind,
    InstallJobSnapshot, InstallJobState, InstallJobStatus, InstallPhaseDetails,
    InstallPhaseId, InstallProgress,
};
use super::store;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

const PROGRESS_PERSIST_INTERVAL: Duration = Duration::from_millis(750);
const CONTENT_PROGRESS_PERSIST_STEPS: u64 = 25;

#[derive(Clone, Debug)]
pub struct InstallProgressReporter {
    job_id: Uuid,
    state: Arc<Mutex<InstallProgressReporterState>>,
}

#[derive(Debug)]
struct InstallProgressReporterState {
    job: InstallJobState,
    last_persisted_at: Instant,
    last_persisted_progress: Option<(InstallPhaseId, u64)>,
}

impl InstallProgressReporter {
    pub fn new(job_id: Uuid, state: InstallJobState) -> Self {
        Self {
            job_id,
            state: Arc::new(Mutex::new(InstallProgressReporterState {
                job: state,
                last_persisted_at: Instant::now(),
                last_persisted_progress: None,
            })),
        }
    }

    pub async fn update(
        &self,
        phase: InstallPhaseId,
        progress: Option<InstallProgress>,
        details: InstallPhaseDetails,
    ) -> crate::Result<()> {
        self.update_with_events(phase, progress, details, Vec::new())
            .await
    }

    pub async fn set_context(
        &self,
        context: InstallErrorContext,
    ) -> crate::Result<()> {
        self.update_context(Some(context), true).await
    }

    pub async fn set_transient_context(
        &self,
        context: InstallErrorContext,
    ) -> crate::Result<()> {
        self.update_context(Some(context), false).await
    }

    pub async fn clear_context(&self) -> crate::Result<()> {
        self.update_context(None, true).await
    }

    async fn update_context(
        &self,
        context: Option<InstallErrorContext>,
        persist: bool,
    ) -> crate::Result<()> {
        let app_state = if persist {
            Some(crate::State::get().await?)
        } else {
            None
        };
        let mut state = self.state.lock().await;
        state.job.set_context(context);

        let Some(app_state) = app_state else {
            return Ok(());
        };

        let record =
            store::update_state(self.job_id, &state.job, &app_state).await?;
        state.mark_persisted();
        emit_install_job(&record.snapshot()).await
    }

    pub async fn persist(&self) -> crate::Result<InstallJobSnapshot> {
        let app_state = crate::State::get().await?;
        let mut state = self.state.lock().await;

        let record =
            store::update_state(self.job_id, &state.job, &app_state).await?;
        state.mark_persisted();
        let snapshot = record.snapshot();
        emit_install_job(&snapshot).await?;
        Ok(snapshot)
    }

    pub async fn persist_failure_context(&self, context: InstallErrorContext) {
        if let Err(error) = self.update_context(Some(context), true).await {
            tracing::warn!(
                "Failed to persist install context for failed operation: {error}"
            );
        }
    }

    pub async fn preserve_failure_context<T>(
        &self,
        context: InstallErrorContext,
        result: crate::Result<T>,
    ) -> crate::Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.persist_failure_context(context).await;
                Err(error)
            }
        }
    }

    pub async fn update_with_events(
        &self,
        phase: InstallPhaseId,
        progress: Option<InstallProgress>,
        details: InstallPhaseDetails,
        events: Vec<InstallJobEventKind>,
    ) -> crate::Result<()> {
        let app_state = crate::State::get().await?;
        let mut state = self.state.lock().await;
        let phase_started = state.job.progress.phase != phase
            || matches!(
                &state.job.progress.details,
                InstallPhaseDetails::Empty
            ) && !matches!(&details, InstallPhaseDetails::Empty);

        state.job.set_progress(phase, progress, details);
        for event in events {
            state.job.record_event(event);
        }

        if !state.should_persist(phase_started) {
            return Ok(());
        }

        let record =
            store::update_state(self.job_id, &state.job, &app_state).await?;
        state.mark_persisted();
        emit_install_job(&record.snapshot()).await
    }

    /// Updates the notification's icon mid-job, once it becomes known --
    /// for a hosted server created from a modpack/mod, the real icon isn't
    /// resolved until partway through `state::hosting::install`, well after
    /// the notification (and its job record) already exist with `icon:
    /// None`. Without this, the notification stays blank-icon for the
    /// entire install even though the server list itself picks up the icon
    /// as soon as it's set.
    pub async fn set_display_icon(
        &self,
        icon: Option<String>,
    ) -> crate::Result<()> {
        let app_state = crate::State::get().await?;
        let mut state = self.state.lock().await;
        let Some(display) = state.job.display.as_mut() else {
            return Ok(());
        };
        display.icon = icon;

        let record =
            store::update_state(self.job_id, &state.job, &app_state).await?;
        state.mark_persisted();
        emit_install_job(&record.snapshot()).await
    }

    /// Marks the job finished successfully. Only for jobs whose target
    /// isn't a real client `Instance` (e.g. hosted server installs, see
    /// `state::hosting::install`) -- those go through
    /// `install::runner`'s `run_job`/`store::complete_success` instead,
    /// which also updates the `instances` table in the same transaction.
    /// This calls the instance-agnostic `store::finish_active` directly.
    pub async fn succeed(&self) -> crate::Result<InstallJobSnapshot> {
        let app_state = crate::State::get().await?;
        let mut state = self.state.lock().await;

        state.job.record_event(InstallJobEventKind::JobSucceeded {
            instance_id: None,
        });
        state.job.progress.phase = InstallPhaseId::Finalizing;
        state.job.progress.progress = None;
        state.job.progress.details = InstallPhaseDetails::Empty;
        state.job.error = None;

        let record = store::finish_active(
            self.job_id,
            InstallJobStatus::Succeeded,
            &state.job,
            &app_state,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Install job {} is no longer active",
                self.job_id
            ))
        })?;
        state.mark_persisted();
        let snapshot = record.snapshot();
        emit_install_job(&snapshot).await?;
        Ok(snapshot)
    }

    /// Marks the job failed, using the reporter's own last-known phase
    /// (whatever was last passed to `update`/`update_with_events`) rather
    /// than requiring the caller to track/pass it separately. See
    /// [`Self::succeed`] for why this bypasses `install::runner`.
    pub async fn fail(
        &self,
        code: &str,
        message: impl Into<String>,
    ) -> crate::Result<InstallJobSnapshot> {
        let app_state = crate::State::get().await?;
        let mut state = self.state.lock().await;

        let phase = state.job.progress.phase;
        let error_view = InstallErrorView::from_message(code, phase, message);
        state.job.record_event(InstallJobEventKind::Failed {
            phase,
            code: error_view.code.clone(),
            message: error_view.message.clone(),
        });
        state.job.error = Some(error_view);

        let record = store::finish_active(
            self.job_id,
            InstallJobStatus::Failed,
            &state.job,
            &app_state,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Install job {} is no longer active",
                self.job_id
            ))
        })?;
        state.mark_persisted();
        let snapshot = record.snapshot();
        emit_install_job(&snapshot).await?;
        Ok(snapshot)
    }
}

impl InstallProgressReporterState {
    fn should_persist(&self, phase_started: bool) -> bool {
        if phase_started {
            return true;
        }

        let Some(progress) = &self.job.progress.progress else {
            return true;
        };

        if progress.current >= progress.total {
            return true;
        }

        let progressed_enough =
            if self.job.progress.phase == InstallPhaseId::DownloadingContent {
                self.last_persisted_progress
                    .map(|(phase, current)| {
                        phase != self.job.progress.phase
                            || progress.current.saturating_sub(current)
                                >= CONTENT_PROGRESS_PERSIST_STEPS
                    })
                    .unwrap_or(true)
            } else {
                false
            };

        progressed_enough
            || self.last_persisted_at.elapsed() >= PROGRESS_PERSIST_INTERVAL
    }

    fn mark_persisted(&mut self) {
        self.last_persisted_at = Instant::now();
        self.last_persisted_progress = self
            .job
            .progress
            .progress
            .as_ref()
            .map(|progress| (self.job.progress.phase, progress.current));
    }
}

#[allow(unused_variables)]
pub async fn emit_install_job(
    snapshot: &InstallJobSnapshot,
) -> crate::Result<()> {
    #[cfg(feature = "tauri")]
    {
        let result: crate::Result<()> = (|| {
            let event_state = crate::EventState::get();
            event_state.send(crate::event::AppEvent::InstallJob(
                std::sync::Arc::new(snapshot.clone()),
            ))?;
            Ok(())
        })();
        if let Err(error) = result {
            tracing::warn!(
                "Failed to emit install job {} update: {error}",
                snapshot.job_id
            );
        }
    }

    Ok(())
}
