//! Game-facing coordinator. No blocking waits: inputs continue to queue while
//! one fresh batch is unresolved. Periodic scheduling belongs to the Tokio host.
use super::{
    batcher::InputBatcher,
    proposer::{ProposalOutcome, Proposer},
    replica::{ExecutedSlot, ExecutionError, Replica},
    types::*,
};

#[derive(Default, Debug)]
pub struct Progress {
    pub outcome: Option<ProposalOutcome>,
    pub executed: Vec<ExecutedSlot>,
    /// Host executes network/timer/leadership effects. Decision effects have already
    /// been consumed by Replica and are represented by `executed` instead.
    pub effects: Vec<Effect>,
}

pub struct TickSession<P: Proposer> {
    pub proposer: P,
    replica: Replica,
    batcher: InputBatcher,
    pending: Option<TickBatch>,
    proposed_slot: Option<Slot>,
    recovering: bool,
}

impl<P: Proposer> TickSession<P> {
    pub fn new(proposer: P, replica: Replica) -> Self {
        Self {
            proposer,
            replica,
            batcher: InputBatcher::default(),
            pending: None,
            proposed_slot: None,
            recovering: false,
        }
    }

    pub fn replica(&self) -> &Replica {
        &self.replica
    }
    pub fn pending(&self) -> Option<&TickBatch> {
        self.pending.as_ref()
    }

    pub fn receive_input(&mut self, input: PlayerInput) -> bool {
        !self.replica.has_executed(&input.request_id) && self.batcher.push(input)
    }

    /// One opportunity, not one accumulated obligation, per periodic deadline.
    /// Busy/NotLeader/Recovering/Unsupported never consume the closed batch.
    pub fn on_deadline(&mut self) -> Result<Progress, ExecutionError> {
        if self.replica.finished() {
            return Ok(Progress::default());
        }
        if self.recovering {
            return Ok(Progress {
                outcome: Some(ProposalOutcome::Recovering),
                ..Progress::default()
            });
        }
        if self.proposed_slot.is_some() {
            return Ok(Progress {
                outcome: Some(ProposalOutcome::Busy),
                ..Progress::default()
            });
        }
        if self.pending.is_none() {
            self.pending = Some(self.batcher.close(self.replica.next_tick()));
        }
        let proposal = self
            .proposer
            .propose(self.pending.as_ref().expect("closed batch"));
        if let ProposalOutcome::Proposed(slot) = proposal.outcome {
            self.proposed_slot = Some(slot);
        }
        let mut progress = Progress {
            outcome: Some(proposal.outcome),
            ..Progress::default()
        };
        for effect in proposal.effects {
            match effect {
                Effect::Decision(entry) => progress.executed.extend(self.on_decision(entry)?),
                other => progress.effects.push(other),
            }
        }
        Ok(progress)
    }

    pub fn on_decision(&mut self, entry: Decision) -> Result<Vec<ExecutedSlot>, ExecutionError> {
        let executed = self.replica.on_decision(entry)?;
        for entry in &executed {
            self.proposer.on_executed(entry.slot);
            if self.proposed_slot == Some(entry.slot) {
                self.proposed_slot = None;
            }
        }
        if self.proposed_slot.is_none() {
            self.reconcile_pending();
        }
        Ok(executed)
    }

    /// Call as soon as leadership is lost/attempted. A proposed batch is NOT
    /// safe to resubmit under a new ballot until authoritative recovery finishes.
    pub fn begin_recovery(&mut self) {
        self.recovering = true;
    }

    /// Coordinator calls this only after Group 1's complete Phase 1 recovery
    /// plan has been decided/executed. This checks ordered execution, clears the
    /// obsolete submission marker, retains uncommitted requests, and derives the
    /// next tick from replayed state. Then (and only then) enable Phase 2 Active.
    pub fn reconcile_after_recovery(&mut self, through: Option<Slot>) -> Result<(), ProtocolError> {
        if self.replica.executed_through() < through {
            return Err(ProtocolError::RecoveryNotExecuted);
        }
        self.proposed_slot = None;
        self.reconcile_pending();
        self.recovering = false;
        Ok(())
    }

    fn reconcile_pending(&mut self) {
        self.batcher
            .discard_executed(|id| self.replica.has_executed(id));
        if let Some(batch) = &mut self.pending {
            let was_empty = batch.inputs.is_empty();
            batch
                .inputs
                .retain(|input| !self.replica.has_executed(&input.request_id));
            if batch.inputs.is_empty() && (!was_empty || batch.tick.0 < self.replica.next_tick().0)
            {
                self.pending = None;
            } else {
                batch.tick = self.replica.next_tick();
            }
        }
    }
}
