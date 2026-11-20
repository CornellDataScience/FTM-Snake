//! One PaxosServer owns the protocol state, following Section 4's Lab 3 layout.
//! Scout and Commander are records, not separate actors. Group 1 and Group 2
//! implement methods on this same owner in phase1.rs and phase2.rs.
use super::{acceptor::AcceptorState, phase1::Scout, phase2::Commander, types::*};

pub struct PaxosServer {
    pub node_id: NodeId,
    pub members: Vec<NodeId>,
    pub(super) acceptor: AcceptorState,
    pub(super) leader_state: LeaderState,
    pub(super) scout: Option<Scout>,
    pub(super) commander: Option<Commander>,
}

impl PaxosServer {
    pub fn new(node_id: NodeId, members: Vec<NodeId>) -> Self {
        Self {
            node_id,
            members,
            acceptor: AcceptorState::default(),
            leader_state: LeaderState::Follower,
            scout: None,
            commander: None,
        }
    }

    pub fn leader_state(&self) -> LeaderState {
        self.leader_state
    }
    pub fn acceptor(&self) -> &AcceptorState {
        &self.acceptor
    }
    pub fn scout(&self) -> Option<&Scout> {
        self.scout.as_ref()
    }
    pub fn commander(&self) -> Option<&Commander> {
        self.commander.as_ref()
    }

    /// Dispatch only; all distributed handlers still return Unsupported.
    /// TODO: validate sender membership and match identity before handling peers.
    pub fn on_message(
        &mut self,
        from: NodeId,
        message: PaxosMessage,
    ) -> Result<Vec<Effect>, ProtocolError> {
        match message {
            PaxosMessage::P1a { ballot, from_slot } => self.on_p1a(from, ballot, from_slot),
            PaxosMessage::P1b { ballot, accepted } => self.on_p1b(from, ballot, accepted),
            PaxosMessage::P2a(pvalue) => self.on_p2a(from, pvalue),
            PaxosMessage::P2b(pvalue) => self.on_p2b(from, pvalue),
            PaxosMessage::Preempted { ballot } => self.on_preempted(ballot),
            // Future host: validate decisions, emit Effect::Decision, then let
            // TickSession deliver them to the replica. Never execute a P2a.
            PaxosMessage::Decision(_) => Err(ProtocolError::Unsupported("decision delivery")),
            PaxosMessage::Heartbeat { .. } | PaxosMessage::HeartbeatReply { .. } => Err(
                ProtocolError::Unsupported("leader heartbeats and execution progress"),
            ),
            PaxosMessage::CatchUp { .. } | PaxosMessage::CatchUpReply { .. } => {
                Err(ProtocolError::Unsupported("replica catch-up"))
            }
        }
    }

    pub fn on_timer(&mut self, timer: TimerEvent) -> Result<Vec<Effect>, ProtocolError> {
        match timer {
            TimerEvent::HeartbeatCheck | TimerEvent::P1aRetry => self.on_phase1_timer(timer),
            TimerEvent::P2aRetry => self.on_phase2_timer(),
            TimerEvent::Heartbeat => Err(ProtocolError::Unsupported("leader heartbeat timer")),
        }
    }
}
