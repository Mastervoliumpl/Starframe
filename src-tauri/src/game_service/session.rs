use super::*;

#[derive(Default)]
pub(super) struct Session {
    pub prepared: Option<serde_json::Value>,
    pub requested: Option<Instant>,
    pub wait: Option<Instant>,
    seen: bool,
}

impl Session {
    pub fn reset(&mut self) {
        self.requested = None;
        self.seen = false;
        self.wait = None;
    }

    pub fn observe(
        &mut self,
        view: &mut LaunchView,
        running: &Running,
        observed: Option<LaunchView>,
        now: Instant,
    ) {
        if let Some(observed) = observed {
            self.seen = true;
            let waited = now.saturating_duration_since(*self.wait.get_or_insert(now));
            *view = observed;
            if waited >= Duration::from_secs(60) && view.phase == Phase::ProcessObserved {
                view.details = vec!["No matching runtime result was confirmed within 60 seconds. Check the game's BepInEx log after it closes.".into()];
            }
        } else if *running == Running::Stopped {
            self.wait = None;
            if self.seen {
                self.seen = false;
                self.requested = None;
                *view = if self.prepared.is_some() {
                    LaunchView::new(
                        Phase::Ready,
                        "Game closed. Ready to prepare the next launch.",
                    )
                } else {
                    LaunchView::new(
                        Phase::SetupRequired,
                        "Game closed. Finish setup to enable the runtime.",
                    )
                };
            } else if self
                .requested
                .is_some_and(|time| now.saturating_duration_since(time) >= Duration::from_secs(30))
            {
                self.requested = None;
                *view = LaunchView::new(
                    Phase::Failed,
                    "Windows accepted the launch request, but no game process was observed within 30 seconds. Retry setup before launching again.",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_timeout_waits_for_a_confirmed_stopped_observation() {
        let now = Instant::now();
        let mut session = Session {
            requested: Some(now),
            ..Default::default()
        };
        let mut view = LaunchView::new(Phase::LaunchRequested, "Waiting");
        session.observe(
            &mut view,
            &Running::Stopped,
            None,
            now + Duration::from_secs(29),
        );
        assert_eq!(view.phase, Phase::LaunchRequested);
        session.observe(
            &mut view,
            &Running::Unknown,
            None,
            now + Duration::from_secs(30),
        );
        assert!(session.requested.is_some());
        session.observe(
            &mut view,
            &Running::Stopped,
            None,
            now + Duration::from_secs(30),
        );
        assert_eq!(view.phase, Phase::Failed);
        assert!(session.requested.is_none());
    }

    #[test]
    fn observed_process_report_timeout_exit_and_reset_keep_launch_state_consistent() {
        let now = Instant::now();
        let mut session = Session {
            prepared: Some(serde_json::json!({})),
            requested: Some(now),
            ..Default::default()
        };
        let mut view = LaunchView::default();
        let observed = || Some(LaunchView::new(Phase::ProcessObserved, "Unverified"));
        session.observe(&mut view, &Running::Running, observed(), now);
        session.observe(
            &mut view,
            &Running::Running,
            observed(),
            now + Duration::from_secs(60),
        );
        assert_eq!(view.details.len(), 1);
        session.observe(
            &mut view,
            &Running::Running,
            Some(LaunchView::new(Phase::RuntimeReady, "Ready")),
            now + Duration::from_secs(61),
        );
        assert_eq!(view.phase, Phase::RuntimeReady);
        session.observe(
            &mut view,
            &Running::Unknown,
            None,
            now + Duration::from_secs(62),
        );
        assert_eq!(view.phase, Phase::RuntimeReady);
        session.observe(
            &mut view,
            &Running::Stopped,
            None,
            now + Duration::from_secs(63),
        );
        assert_eq!(view.phase, Phase::Ready);
        assert!(session.requested.is_none());
        assert!(session.wait.is_none());
        session.observe(
            &mut view,
            &Running::Running,
            observed(),
            now + Duration::from_secs(64),
        );
        assert!(view.details.is_empty());
        session.reset();
        assert!(!session.seen);
        assert!(session.wait.is_none());
    }
}
