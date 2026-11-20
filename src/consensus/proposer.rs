//! Game-facing propose interface and local test double. No distributed runtime.
use super::types::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalOutcome {
    /// Accepted by the local proposer, NOT a commitment or successful execution.
    Proposed(Slot),
    NotLeader,
    Recovering,
    Busy,
    Unsupported,
}

#[derive(Debug)]
pub struct ProposalResult {
    pub outcome: ProposalOutcome,
    pub effects: Vec<Effect>,
}

impl ProposalResult {
    pub fn refused(outcome: ProposalOutcome) -> Self {
        Self {
            outcome,
            effects: vec![],
        }
    }
}

/// Game-facing interface. Borrowed proposals remain owned by the caller on ALL
/// outcomes. Only Effect::Decision may feed the replica.
pub trait Proposer {
    fn propose(&mut self, batch: &TickBatch) -> ProposalResult;
    fn repropose(&mut self, slot: Slot, command: &Command) -> ProposalResult;
    /// Release the unresolved slot only AFTER ordered execution, not on send.
    fn on_executed(&mut self, slot: Slot);
}

/// Volatile single-node log. Immediately decides, but still waits for execution
/// before accepting another proposal. Restart creates a new, unrelated log.
#[derive(Default)]
pub struct LocalProposer {
    next_slot: u64,
    pending: Option<Slot>,
}

impl Proposer for LocalProposer {
    fn propose(&mut self, batch: &TickBatch) -> ProposalResult {
        if self.pending.is_some() {
            return ProposalResult::refused(ProposalOutcome::Busy);
        }
        let slot = Slot(self.next_slot);
        self.next_slot += 1;
        self.pending = Some(slot);
        ProposalResult {
            outcome: ProposalOutcome::Proposed(slot),
            effects: vec![Effect::Decision(Decision {
                slot,
                command: Command::Tick(batch.clone()),
            })],
        }
    }

    fn repropose(&mut self, _slot: Slot, _command: &Command) -> ProposalResult {
        ProposalResult::refused(ProposalOutcome::Unsupported)
    }

    fn on_executed(&mut self, slot: Slot) {
        if self.pending == Some(slot) {
            self.pending = None;
        }
    }
}
