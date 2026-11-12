//! GROUP 2: Phase 2 implementation belongs here. The local adapter below is a
//! development shortcut, NOT Paxos and NOT distributed fault tolerance.
use std::collections::BTreeSet;

use super::{acceptor::AcceptorState, types::*};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitOutcome {
    /// Accepted by the local proposer, NOT a commitment or successful execution.
    Submitted(LogSlot),
    NotLeader,
    Recovering,
    Busy,
    Unsupported,
}

#[derive(Debug)]
pub struct Submission {
    pub outcome: SubmitOutcome,
    pub effects: Vec<Effect>,
}

impl Submission {
    pub fn refused(outcome: SubmitOutcome) -> Self {
        Self {
            outcome,
            effects: vec![],
        }
    }
}

/// Game-facing interface. Borrowed proposals remain owned by the caller on ALL
/// outcomes. Only Effect::Chosen may feed the application adapter.
pub trait Agreement {
    fn submit_fresh(&mut self, batch: &TickBatch) -> Submission;
    fn submit_recovered(&mut self, slot: LogSlot, value: &LogValue) -> Submission;
    /// Release the unresolved slot only AFTER ordered application, not on send.
    fn on_applied(&mut self, slot: LogSlot);
}

/// At most one unresolved proposal. Students must match acknowledgements against
/// entry.ballot, entry.slot AND entry.value, and count distinct configured members.
pub struct PhaseTwoProposal {
    pub entry: AcceptedEntry,
    pub accepted_by: BTreeSet<NodeId>,
}

pub struct PaxosAgreement {
    pub members: Vec<NodeId>,
    pub pending: Option<PhaseTwoProposal>,
    state: LeadershipState,
}

impl PaxosAgreement {
    pub fn new(members: Vec<NodeId>) -> Self {
        Self {
            members,
            pending: None,
            state: LeadershipState::Follower,
        }
    }

    /// TODO: install Group 1's authoritative recovery plan and enter Recovering.
    /// Re-propose exact recovered values in their original slots; no fresh ticks.
    pub fn begin_recovery(&mut self, _plan: &RecoveryPlan) -> Result<(), ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 recovery installation"))
    }

    /// TODO: accept Group 1's Ready only after applied-through covers recovery,
    /// and after TickSession::reconcile_after_recovery. Never skip the latter.
    pub fn ready_for_fresh(&mut self, _ballot: Ballot) -> Result<(), ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 readiness handoff"))
    }

    /// TODO: Accept uses shared acceptor.accept(); Accepted updates accepted_by.
    /// Emit Chosen only after > members.len()/2 DISTINCT acceptors accept the
    /// identical ballot+slot+value. Ignore outsiders, stale/mismatched replies.
    /// Decision and catch-up paths must validate their trusted peer/log context.
    pub fn on_message(
        &mut self,
        _from: NodeId,
        _message: PeerMessage,
        _acceptor: &mut AcceptorState,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 Phase 2 messages"))
    }

    pub fn on_timer(&mut self, _timer: TimerEvent) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 Phase 2 retries"))
    }
}

impl Agreement for PaxosAgreement {
    fn submit_fresh(&mut self, _batch: &TickBatch) -> Submission {
        Submission::refused(match self.state {
            LeadershipState::Follower | LeadershipState::Attempting(_) => SubmitOutcome::NotLeader,
            LeadershipState::Recovering { .. } => SubmitOutcome::Recovering,
            LeadershipState::Ready(_) if self.pending.is_some() => SubmitOutcome::Busy,
            LeadershipState::Ready(_) => SubmitOutcome::Unsupported,
        })
    }

    fn submit_recovered(&mut self, _slot: LogSlot, _value: &LogValue) -> Submission {
        Submission::refused(SubmitOutcome::Unsupported)
    }

    fn on_applied(&mut self, _slot: LogSlot) {
        // TODO Group 2: resolve pending/recovery progress. No fake commitment.
    }
}

/// Volatile single-node log. Immediately chooses, but still waits for application
/// before accepting another proposal. Restart creates a new, unrelated log.
#[derive(Default)]
pub struct LocalAgreement {
    next_slot: u64,
    pending: Option<LogSlot>,
}

impl Agreement for LocalAgreement {
    fn submit_fresh(&mut self, batch: &TickBatch) -> Submission {
        if self.pending.is_some() {
            return Submission::refused(SubmitOutcome::Busy);
        }
        let slot = LogSlot(self.next_slot);
        self.next_slot += 1;
        self.pending = Some(slot);
        Submission {
            outcome: SubmitOutcome::Submitted(slot),
            effects: vec![Effect::Chosen(ChosenEntry {
                slot,
                value: LogValue::Tick(batch.clone()),
            })],
        }
    }

    fn submit_recovered(&mut self, _slot: LogSlot, _value: &LogValue) -> Submission {
        Submission::refused(SubmitOutcome::Unsupported)
    }

    fn on_applied(&mut self, slot: LogSlot) {
        if self.pending == Some(slot) {
            self.pending = None;
        }
    }
}
