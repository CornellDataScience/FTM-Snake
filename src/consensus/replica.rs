//! Replica execution (Week 6): state, slot_num and decisions.
//! Decisions may arrive out of order; execute only the contiguous prefix.
use std::collections::{BTreeMap, HashMap};

use super::types::*;
use crate::{
    config::GameConfig,
    game::{Game, Status},
    round::Round,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    ConflictingSlot(Slot),
    WrongTick {
        expected: GameTick,
        received: GameTick,
    },
    UnknownPlayer(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientResult {
    pub request_id: ClientRequestId,
    /// Original application tick, including for a repeated request.
    pub tick: GameTick,
    pub duplicate: bool,
}

#[derive(Debug)]
pub struct ExecutedSlot {
    pub slot: Slot,
    pub tick: Option<GameTick>,
    pub results: Vec<ClientResult>,
}

pub struct Replica {
    round: Round,
    slot_num: u64,
    next_tick: u64,
    /// Keep executed values too, so a conflicting duplicate is detectable.
    decisions: BTreeMap<Slot, Command>,
    requests: HashMap<ClientRequestId, GameTick>,
}

impl Replica {
    /// All replicas must start with the SAME config, ordered roster and seed,
    /// and the same binary/dependency versions (StdRng is not a stable wire RNG).
    /// Match setup is external metadata, not inferred from local arrival times.
    pub fn new(config: GameConfig, owners: Vec<String>, seed: u64) -> Result<Self, String> {
        Ok(Self {
            round: Round::new(config, owners, seed)?,
            slot_num: 0,
            next_tick: 0,
            decisions: BTreeMap::new(),
            requests: HashMap::new(),
        })
    }

    pub fn game(&self) -> &Game {
        self.round.game()
    }
    pub fn owners(&self) -> &[String] {
        self.round.owners()
    }
    /// Next slot to execute (the notes' slot_num / slot_out).
    pub fn slot_num(&self) -> Slot {
        Slot(self.slot_num)
    }

    pub fn next_tick(&self) -> GameTick {
        GameTick(self.next_tick)
    }
    pub fn executed_through(&self) -> Option<Slot> {
        self.slot_num.checked_sub(1).map(Slot)
    }
    pub fn has_executed(&self, request: &ClientRequestId) -> bool {
        self.requests.contains_key(request)
    }
    /// A retry may get a cached success only after the original was executed.
    pub fn result_for(&self, request: &ClientRequestId) -> Option<ClientResult> {
        self.requests.get(request).map(|&tick| ClientResult {
            request_id: request.clone(),
            tick,
            duplicate: true,
        })
    }
    pub fn finished(&self) -> bool {
        self.game().status() != Status::Running
    }

    /// Buffer gaps, ignore identical duplicates, reject conflicting decisions.
    /// Validate the contiguous prefix before changing the game, so errors cannot
    /// hide receipts from entries executed earlier in the same call.
    pub fn on_decision(&mut self, entry: Decision) -> Result<Vec<ExecutedSlot>, ExecutionError> {
        if let Some(command) = self.decisions.get(&entry.slot) {
            if command != &entry.command {
                return Err(ExecutionError::ConflictingSlot(entry.slot));
            }
        } else {
            self.decisions.insert(entry.slot, entry.command);
        }
        let mut prefix = Vec::new();
        let mut expected_tick = self.next_tick;
        let mut slot = self.slot_num;
        while let Some(command) = self.decisions.get(&Slot(slot)) {
            if let Command::Tick(batch) = command {
                if batch.tick != GameTick(expected_tick) {
                    return Err(ExecutionError::WrongTick {
                        expected: GameTick(expected_tick),
                        received: batch.tick,
                    });
                }
                for input in &batch.inputs {
                    if self.round.snake_of(&input.request_id.client_id).is_none() {
                        return Err(ExecutionError::UnknownPlayer(
                            input.request_id.client_id.clone(),
                        ));
                    }
                }
                expected_tick += 1;
            }
            prefix.push((Slot(slot), command.clone()));
            slot += 1;
        }
        let mut executed = Vec::new();
        for (slot, command) in prefix {
            let mut results = Vec::new();
            let tick = match command {
                Command::Noop => None,
                Command::Tick(batch) => {
                    for input in batch.inputs {
                        let old_tick = self.requests.get(&input.request_id).copied();
                        if old_tick.is_none() {
                            // The roster was validated above.
                            self.round
                                .set_direction(&input.request_id.client_id, input.direction);
                            self.requests.insert(input.request_id.clone(), batch.tick);
                        }
                        results.push(ClientResult {
                            request_id: input.request_id,
                            tick: old_tick.unwrap_or(batch.tick),
                            duplicate: old_tick.is_some(),
                        });
                    }
                    self.round.tick();
                    self.next_tick += 1;
                    Some(batch.tick)
                }
            };
            self.slot_num += 1;
            executed.push(ExecutedSlot {
                slot,
                tick,
                results,
            });
        }
        Ok(executed)
    }
}
