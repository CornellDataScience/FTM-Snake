//! Group 2: batch formation. Closing transfers inputs to the caller, who MUST
//! retain that batch until applied or reconciled after authoritative recovery.
use std::collections::{HashSet, VecDeque};

use super::types::{ClientRequestId, GameTick, PlayerInput, TickBatch};

#[derive(Default)]
pub struct InputBatcher {
    inputs: VecDeque<PlayerInput>,
    /// Retained for this match; production snapshots must preserve dedup state.
    seen: HashSet<ClientRequestId>,
}

impl InputBatcher {
    pub fn push(&mut self, input: PlayerInput) -> bool {
        if !self.seen.insert(input.request_id.clone()) {
            return false;
        }
        self.inputs.push_back(input);
        true
    }

    /// Keep every distinct request in receipt order. Execution calls
    /// Game::set_direction in that order: the first VALID turn per player wins;
    /// reversals, repeated directions, and later turns are committed but ignored.
    /// Inputs arriving after close stay queued for the following tick.
    pub fn close(&mut self, tick: GameTick) -> TickBatch {
        TickBatch {
            tick,
            inputs: self.inputs.drain(..).collect(),
        }
    }

    pub fn discard_applied(&mut self, mut is_applied: impl FnMut(&ClientRequestId) -> bool) {
        self.inputs.retain(|input| !is_applied(&input.request_id));
    }
}
