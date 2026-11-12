//! GROUP 1: implement elections, Phase 1 quorum collection, and recovery here.
//! No election or quorum algorithm is implemented by this scaffold.
use std::collections::BTreeMap;

use super::{acceptor::AcceptorState, types::*};

pub struct Leadership {
    pub node_id: NodeId,
    pub members: Vec<NodeId>,
    /// Key by DISTINCT member ID. Never count a retransmitted Promise twice;
    /// partial histories must be completed before constructing Established.
    pub promises: BTreeMap<NodeId, AcceptedHistory>,
    state: LeadershipState,
}

impl Leadership {
    pub fn new(node_id: NodeId, members: Vec<NodeId>) -> Self {
        Self {
            node_id,
            members,
            promises: BTreeMap::new(),
            state: LeadershipState::Follower,
        }
    }

    pub fn state(&self) -> LeadershipState {
        self.state
    }

    /// TODO Group 1: process heartbeats, Prepare/Promise/Reject, use the SAME
    /// acceptor as Group 2, and emit Send/Schedule/Established effects.
    pub fn on_message(
        &mut self,
        _from: NodeId,
        _message: PeerMessage,
        _acceptor: &mut AcceptorState,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 1 message handling"))
    }

    /// TODO: suspect failure (not proof), choose a higher unique ballot, enter
    /// Attempting, emit Prepare, and collect a fixed-membership majority.
    pub fn on_timer(
        &mut self,
        _timer: TimerEvent,
        _acceptor: &mut AcceptorState,
    ) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 1 election timers"))
    }

    /// TODO: after Established enter Recovering. Emit Ready ONLY when the game
    /// adapter reports every slot through the recovery boundary applied. Seeing
    /// Accepted/Decision messages alone is not sufficient. Higher ballots revoke
    /// readiness immediately. This stub can never promote itself to Ready.
    pub fn on_applied(&mut self, _through: Option<LogSlot>) -> Result<Vec<Effect>, ProtocolError> {
        Err(ProtocolError::Unsupported("Group 1 recovery completion"))
    }
}
