//! Game-facing coordinator. No blocking waits: inputs continue to queue while
//! one fresh batch is unresolved. Periodic scheduling belongs to the Tokio host.
use super::{
    agreement::{Agreement, SubmitOutcome},
    application::{AppliedEntry, ApplyError, GameAdapter},
    batcher::InputBatcher,
    types::*,
};

#[derive(Default, Debug)]
pub struct Progress {
    pub outcome: Option<SubmitOutcome>,
    pub applied: Vec<AppliedEntry>,
    /// Host executes network/timer/leadership effects. Chosen effects have already
    /// been consumed by GameAdapter and are represented by `applied` instead.
    pub effects: Vec<Effect>,
}

pub struct TickSession<A: Agreement> {
    pub agreement: A,
    application: GameAdapter,
    batcher: InputBatcher,
    pending: Option<TickBatch>,
    submitted: Option<LogSlot>,
    recovering: bool,
}

impl<A: Agreement> TickSession<A> {
    pub fn new(agreement: A, application: GameAdapter) -> Self {
        Self {
            agreement,
            application,
            batcher: InputBatcher::default(),
            pending: None,
            submitted: None,
            recovering: false,
        }
    }

    pub fn application(&self) -> &GameAdapter {
        &self.application
    }
    pub fn pending(&self) -> Option<&TickBatch> {
        self.pending.as_ref()
    }

    pub fn receive_input(&mut self, input: PlayerInput) -> bool {
        !self.application.has_applied(&input.request_id) && self.batcher.push(input)
    }

    /// One opportunity, not one accumulated obligation, per periodic deadline.
    /// Busy/NotLeader/Recovering/Unsupported never consume the closed batch.
    pub fn on_deadline(&mut self) -> Result<Progress, ApplyError> {
        if self.application.finished() {
            return Ok(Progress::default());
        }
        if self.recovering {
            return Ok(Progress {
                outcome: Some(SubmitOutcome::Recovering),
                ..Progress::default()
            });
        }
        if self.submitted.is_some() {
            return Ok(Progress {
                outcome: Some(SubmitOutcome::Busy),
                ..Progress::default()
            });
        }
        if self.pending.is_none() {
            self.pending = Some(self.batcher.close(self.application.next_tick()));
        }
        let submission = self
            .agreement
            .submit_fresh(self.pending.as_ref().expect("closed batch"));
        if let SubmitOutcome::Submitted(slot) = submission.outcome {
            self.submitted = Some(slot);
        }
        let mut progress = Progress {
            outcome: Some(submission.outcome),
            ..Progress::default()
        };
        for effect in submission.effects {
            match effect {
                Effect::Chosen(entry) => progress.applied.extend(self.on_chosen(entry)?),
                other => progress.effects.push(other),
            }
        }
        Ok(progress)
    }

    pub fn on_chosen(&mut self, entry: ChosenEntry) -> Result<Vec<AppliedEntry>, ApplyError> {
        let applied = self.application.receive_chosen(entry)?;
        for entry in &applied {
            self.agreement.on_applied(entry.slot);
            if self.submitted == Some(entry.slot) {
                self.submitted = None;
            }
        }
        if self.submitted.is_none() {
            self.reconcile_pending();
        }
        Ok(applied)
    }

    /// Call as soon as leadership is lost/attempted. A submitted batch is NOT
    /// safe to resubmit under a new ballot until authoritative recovery finishes.
    pub fn begin_recovery(&mut self) {
        self.recovering = true;
    }

    /// Coordinator calls this only after Group 1's complete Phase 1 recovery
    /// plan has been chosen/applied. This checks ordered application, clears the
    /// obsolete submission marker, retains uncommitted requests, and derives the
    /// next tick from replayed state. Then (and only then) enable Phase 2 Ready.
    pub fn reconcile_after_recovery(
        &mut self,
        through: Option<LogSlot>,
    ) -> Result<(), ProtocolError> {
        if self.application.applied_through() < through {
            return Err(ProtocolError::RecoveryNotApplied);
        }
        self.submitted = None;
        self.reconcile_pending();
        self.recovering = false;
        Ok(())
    }

    fn reconcile_pending(&mut self) {
        self.batcher
            .discard_applied(|id| self.application.has_applied(id));
        if let Some(batch) = &mut self.pending {
            let was_empty = batch.inputs.is_empty();
            batch
                .inputs
                .retain(|input| !self.application.has_applied(&input.request_id));
            if batch.inputs.is_empty()
                && (!was_empty || batch.tick.0 < self.application.next_tick().0)
            {
                self.pending = None;
            } else {
                batch.tick = self.application.next_tick();
            }
        }
    }
}
