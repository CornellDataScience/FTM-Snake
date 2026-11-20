//! GROUP 1: leader election and the prepare phase (P1a / P1b).
//! The Scout role gathers a majority; it shares PaxosServer's one acceptor state.
use std::collections::BTreeMap;

use super::{paxos::PaxosServer, types::*};

pub struct Scout {
    pub ballot: Ballot,
    /// Distinct configured acceptors only. Retransmissions do not add votes;
    /// paginated accepted histories must be complete before constructing a plan.
    pub p1b: BTreeMap<NodeId, AcceptedHistory>,
}

impl PaxosServer {
    /// TODO: call self.acceptor.on_p1a, then send P1b with accepted p-values,
    /// or Preempted with the current higher ballot. Both groups use this owner.
    pub fn on_p1a(
        &mut self,
        _from: NodeId,
        _ballot: Ballot,
        _from_slot: Slot,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 1 P1a handling"))
    }

    /// TODO: collect a majority of complete P1b histories for this scout's ballot.
    /// For each slot, select the command with the highest accepted ballot;
    /// fill proven holes with no-ops. Emit Phase1Complete with the recovery plan.
    pub fn on_p1b(
        &mut self,
        _from: NodeId,
        _ballot: Ballot,
        _accepted: AcceptedHistory,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported(
            "Group 1 P1b quorum and recovery plan",
        ))
    }

    /// TODO: a higher ballot revokes leadership and fresh-proposal readiness.
    /// Wait for leader suspicion before starting another election (Section 4).
    pub fn on_preempted(&mut self, _ballot: Ballot) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 1 preemption"))
    }

    /// TODO: suspect failure, choose a higher unique ballot, enter Scouting and
    /// send P1a. A timeout is suspicion, not proof that the leader has crashed.
    pub fn on_phase1_timer(&mut self, _timer: TimerEvent) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported(
            "Group 1 election and P1a retries",
        ))
    }

    /// TODO: emit LeaderReady only after every recovery slot is decided AND
    /// executed. A P2b or a decision beyond a hole is not sufficient.
    pub fn on_recovery_executed(
        &mut self,
        _through: Option<Slot>,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported(
            "Group 1 recovery execution barrier",
        ))
    }
}
