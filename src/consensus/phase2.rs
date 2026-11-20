//! GROUP 2: the propose phase (P2a / P2b), decisions and recovery proposals.
use std::collections::BTreeSet;

use super::{
    paxos::PaxosServer,
    proposer::{ProposalOutcome, ProposalResult, Proposer},
    types::*,
};

/// The Commander role tracks one proposed p-value and distinct acceptor replies.
/// This project's game loop allows only one unresolved fresh tick at a time.
pub struct Commander {
    pub pvalue: PValue,
    pub p2b: BTreeSet<NodeId>,
}

impl PaxosServer {
    /// TODO: install Group 1's recovery plan and enter Recovering. For each slot,
    /// re-propose the command selected from the highest accepted ballot, under
    /// the NEW ballot. Preserve original slots and game-tick numbers.
    pub fn begin_recovery(&mut self, _plan: &RecoveryPlan) -> Result<(), ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 recovery installation"))
    }

    /// TODO: enter Active only after Group 1's LeaderReady, executed-through
    /// covers recovery, and TickSession::reconcile_after_recovery has run.
    pub fn activate(&mut self, _ballot: Ballot) -> Result<(), ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 leader activation"))
    }

    /// TODO: call self.acceptor.on_p2a. Reply P2b only after accepting the p-value
    /// at the EXACT adopted ballot. On a lower ballot, reply Preempted; a higher
    /// ballot needs a successful P1a first. Never acknowledge a rejected P2a.
    pub fn on_p2a(&mut self, _from: NodeId, _pvalue: PValue) -> Result<Vec<Effect>, ProtocolError> {
        if self.node_id > _from {
            let mut ret = Vec::new();
            ret.push( Effect::Send {
                to: _from,
                message: PaxosMessage::Preempted { ballot: Ballot { counter: 0, node_id: self.node_id } },
            });
            return Ok(ret)
        }

        if self.acceptor.on_p2a(_pvalue.clone()) == Ok(()){
            let mut ret = Vec::new();
            ret.push( Effect::Send {
                to: _from,
                message: PaxosMessage::P2b(_pvalue)}
            );
            return Ok(ret)
        }

        let mut ret = Vec::new();
            ret.push( Effect::Send {
                to: _from,
                message: PaxosMessage::Preempted { ballot: Ballot { counter: 0, node_id: self.node_id } },
            });
        return Ok(ret)
    }

    /// TODO: match ballot + slot + command to the commander's proposal. Count
    /// only distinct configured acceptors. After a majority, send Decision to
    /// replicas and emit Effect::Decision locally. Ignore stale/outsider replies.
    pub fn on_p2b(&mut self, _from: NodeId, _pvalue: PValue) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported(
            "Group 2 P2b quorum and decision",
        ))
    }

    pub fn on_phase2_timer(&mut self) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 2 P2a retries"))
    }
}

impl Proposer for PaxosServer {
    fn propose(&mut self, _batch: &TickBatch) -> ProposalResult {
        ProposalResult::refused(match self.leader_state {
            LeaderState::Follower | LeaderState::Scouting(_) => ProposalOutcome::NotLeader,
            LeaderState::Recovering { .. } => ProposalOutcome::Recovering,
            LeaderState::Active(_) if self.commander.is_some() => ProposalOutcome::Busy,
            LeaderState::Active(_) => ProposalOutcome::Unsupported,
        })
    }

    fn repropose(&mut self, _slot: Slot, _command: &Command) -> ProposalResult {
        ProposalResult::refused(ProposalOutcome::Unsupported)
    }

    fn on_executed(&mut self, _slot: Slot) {
        // TODO: release the unresolved proposal only after ordered execution.
    }
}
