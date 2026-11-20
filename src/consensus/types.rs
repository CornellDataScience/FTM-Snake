//! Vocabulary from CS5414 Section 4 (Multi-Paxos) and Week 6 (replicas).
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::player::Direction;

pub type NodeId = u64;

/// Proposal number: (counter, node ID), ordered lexicographically.
/// One ballot is shared across all slots, as in Multi-Paxos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ballot {
    pub counter: u64,
    pub node_id: NodeId,
}

/// One position in the replicated log. This project uses zero-based slots;
/// the lecture pseudocode starts at one. A slot is not a game tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Slot(pub u64);

/// Separate from slots: deciding a no-op does not advance the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameTick(pub u64);

/// The lecture's (client, cid). Browser `sequence` is the client's command ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientRequestId {
    pub client_id: String,
    pub sequence: u64,
}

/// The lecture's <client, cid, op>; our operation is a snake direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerInput {
    pub request_id: ClientRequestId,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickBatch {
    pub tick: GameTick,
    /// Receipt order is part of the command and must not be sorted.
    pub inputs: Vec<PlayerInput>,
}

/// Our application command batches player operations and advances one game tick.
/// A recovery no-op fills a log hole without changing the game or its RNG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Tick(TickBatch),
    Noop,
}

/// A p-value is <ballot, slot, command>. P2a proposes it; an acceptor may store it.
/// Its existence alone does not mean a majority has accepted or decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PValue {
    pub ballot: Ballot,
    pub slot: Slot,
    pub command: Command,
}

/// A decided <slot, command>, ready for ordered execution by a replica.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub slot: Slot,
    pub command: Command,
}

/// Accepted p-values returned in P1b. A partial page never proves a hole empty.
/// There is no log garbage collection yet, so a complete history starts at zero.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptedHistory {
    Complete(Vec<PValue>),
    Page {
        entries: Vec<PValue>,
        from_slot: Slot,
        /// None ends this page chain, not proof that earlier pages were received.
        next_slot: Option<Slot>,
    },
}

/// Peer messages, separate from browser messages. The transport supplies sender
/// identity and must fence other matches and validate the fixed membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaxosMessage {
    /// Prepare: adopt this ballot across all slots and report accepted p-values.
    P1a {
        ballot: Ballot,
        /// Start at zero; resume at a P1b page's next_slot when paginating.
        from_slot: Slot,
    },
    /// Successful prepare response, including accepted p-values.
    P1b {
        ballot: Ballot,
        accepted: AcceptedHistory,
    },
    /// Propose <ballot, slot, command>.
    P2a(PValue),
    /// Successful accept response. Echo the whole p-value so the commander can
    /// match ballot, slot AND command; this is more explicit than the slides.
    P2b(PValue),
    /// A higher ballot has preempted the sender's leadership attempt.
    Preempted {
        ballot: Ballot,
    },
    Decision(Decision),
    Heartbeat {
        ballot: Ballot,
        executed_through: Option<Slot>,
    },
    /// Execution progress for future catch-up/garbage collection (Section 4).
    /// A majority is NOT enough for the notes' everyone-executed GC rule.
    HeartbeatReply {
        ballot: Ballot,
        executed_through: Option<Slot>,
    },
    CatchUp {
        from_slot: Slot,
    },
    CatchUpReply {
        decisions: Vec<Decision>,
        complete: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerEvent {
    Heartbeat,
    HeartbeatCheck,
    P1aRetry,
    P2aRetry,
}

/// Group 1's output after a P1b majority with COMPLETE accepted histories.
/// For each slot, preserve the command at the highest accepted ballot. Fill
/// proven holes through `through` with no-ops; never renumber recovered ticks.
#[derive(Debug, Clone)]
pub struct RecoveryPlan {
    pub ballot: Ballot,
    pub proposals: Vec<(Slot, Command)>,
    pub through: Option<Slot>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderState {
    Follower,
    Scouting(Ballot),
    Recovering {
        ballot: Ballot,
        through: Option<Slot>,
    },
    /// Project policy: fresh game ticks wait until recovery has been executed.
    Active(Ballot),
}

/// The host executes effects; protocol code owns no sockets or timer tasks.
#[derive(Debug, Clone)]
pub enum Effect {
    Send {
        to: NodeId,
        message: PaxosMessage,
    },
    Schedule {
        timer: TimerEvent,
        after: Duration,
    },
    Phase1Complete(RecoveryPlan),
    LeaderReady(Ballot),
    /// Only a validated decision can be delivered to Replica::on_decision.
    Decision(Decision),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Unsupported(&'static str),
    RecoveryNotExecuted,
}
