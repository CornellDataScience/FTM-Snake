use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::player::Direction;

pub type NodeId = u64;

/// Lexicographic ordering: first counter, then node ID to break ties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ballot {
    pub counter: u64,
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LogSlot(pub u64);

/// Separate from log slots: a recovery no-op does not consume a game tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameTick(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientRequestId {
    pub client_id: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub request_id: ClientRequestId,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickBatch {
    pub tick: GameTick,
    /// Leader receipt order is part of the chosen value and must not be sorted.
    pub inputs: Vec<PlayerInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogValue {
    Tick(TickBatch),
    Noop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedEntry {
    pub slot: LogSlot,
    pub ballot: Ballot,
    pub value: LogValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChosenEntry {
    pub slot: LogSlot,
    pub value: LogValue,
}

/// A page is NEVER evidence that unlisted slots are empty. No compaction yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptedHistory {
    Complete(Vec<AcceptedEntry>),
    Page {
        entries: Vec<AcceptedEntry>,
        from_slot: LogSlot,
        /// None ends this page chain, not proof that earlier pages were received.
        next_slot: Option<LogSlot>,
    },
}

/// Peer messages, not browser messages. Transport supplies the sender's NodeId.
/// All messages belong to one agreed match/log; the future transport must fence
/// other matches and use a fixed, identical acceptor membership on every node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerMessage {
    Prepare {
        ballot: Ballot,
        /// Start at zero; resume at a Promise page's next_slot when paginating.
        from_slot: LogSlot,
    },
    Promise {
        ballot: Ballot,
        history: AcceptedHistory,
    },
    Accept(AcceptedEntry),
    /// Full value makes the ballot/slot/value equality contract explicit.
    Accepted(AcceptedEntry),
    Reject {
        promised: Ballot,
    },
    Decision(ChosenEntry),
    Heartbeat {
        ballot: Ballot,
        applied_through: Option<LogSlot>,
    },
    CatchUp {
        from_slot: LogSlot,
    },
    CatchUpReply {
        entries: Vec<ChosenEntry>,
        complete: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    LeaderHeartbeat,
    ElectionTimeout,
    RetryPhaseOne,
    RetryPhaseTwo,
}

/// Group 1 produces this only after a Phase 1 majority with COMPLETE histories.
/// Select the highest accepted ballot per slot, including values already known
/// chosen. Fill holes through `through` with no-ops; never renumber recovered ticks.
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub ballot: Ballot,
    pub values: Vec<(LogSlot, LogValue)>,
    pub through: Option<LogSlot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadershipState {
    Follower,
    Attempting(Ballot),
    Recovering {
        ballot: Ballot,
        through: Option<LogSlot>,
    },
    Ready(Ballot),
}

/// Caller executes effects outside protocol code; no sockets or tasks owned here.
#[derive(Debug, Clone)]
pub enum Effect {
    Send { to: NodeId, message: PeerMessage },
    Schedule { timer: TimerEvent, after: Duration },
    Established(RecoveryPlan),
    Ready(Ballot),
    Chosen(ChosenEntry),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Unsupported(&'static str),
    RecoveryNotApplied,
}
