//! Group 2 execution boundary: the ONLY path from chosen log values to Game.
use std::collections::{BTreeMap, HashMap};

use rand::{SeedableRng, rngs::StdRng};

use super::types::*;
use crate::{
    config::GameConfig,
    game::{Game, Status},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    ConflictingSlot(LogSlot),
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
pub struct AppliedEntry {
    pub slot: LogSlot,
    pub tick: Option<GameTick>,
    pub results: Vec<ClientResult>,
}

pub struct GameAdapter {
    game: Game,
    owners: Vec<String>,
    rng: StdRng,
    next_slot: u64,
    next_tick: u64,
    /// Keep applied values too, so a conflicting duplicate is detectable.
    log: BTreeMap<LogSlot, LogValue>,
    requests: HashMap<ClientRequestId, GameTick>,
}

impl GameAdapter {
    /// All replicas must start with the SAME config, ordered roster and seed,
    /// and the same binary/dependency versions (StdRng is not a stable wire RNG).
    /// Match setup is external metadata, not inferred from local arrival times.
    pub fn new(config: GameConfig, owners: Vec<String>, seed: u64) -> Result<Self, String> {
        if owners
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len()
            != owners.len()
        {
            return Err("Player IDs must be unique".into());
        }
        let mut rng = StdRng::seed_from_u64(seed);
        let game = Game::with_config(config, owners.len(), &mut rng)?;
        Ok(Self {
            game,
            owners,
            rng,
            next_slot: 0,
            next_tick: 0,
            log: BTreeMap::new(),
            requests: HashMap::new(),
        })
    }

    pub fn game(&self) -> &Game {
        &self.game
    }
    pub fn owners(&self) -> &[String] {
        &self.owners
    }
    pub fn next_tick(&self) -> GameTick {
        GameTick(self.next_tick)
    }
    pub fn applied_through(&self) -> Option<LogSlot> {
        self.next_slot.checked_sub(1).map(LogSlot)
    }
    pub fn has_applied(&self, request: &ClientRequestId) -> bool {
        self.requests.contains_key(request)
    }
    /// A retry may get a cached success only after the original was applied.
    pub fn result_for(&self, request: &ClientRequestId) -> Option<ClientResult> {
        self.requests.get(request).map(|&tick| ClientResult {
            request_id: request.clone(),
            tick,
            duplicate: true,
        })
    }
    pub fn finished(&self) -> bool {
        self.game.status() != Status::Running
    }

    /// Buffer gaps, ignore identical duplicates, reject conflicting decisions.
    /// Validate the contiguous prefix before changing the game, so errors cannot
    /// hide receipts from entries applied earlier in the same call.
    pub fn receive_chosen(&mut self, entry: ChosenEntry) -> Result<Vec<AppliedEntry>, ApplyError> {
        if let Some(value) = self.log.get(&entry.slot) {
            if value != &entry.value {
                return Err(ApplyError::ConflictingSlot(entry.slot));
            }
        } else {
            self.log.insert(entry.slot, entry.value);
        }
        let mut prefix = Vec::new();
        let mut expected_tick = self.next_tick;
        let mut slot = self.next_slot;
        while let Some(value) = self.log.get(&LogSlot(slot)) {
            if let LogValue::Tick(batch) = value {
                if batch.tick != GameTick(expected_tick) {
                    return Err(ApplyError::WrongTick {
                        expected: GameTick(expected_tick),
                        received: batch.tick,
                    });
                }
                for input in &batch.inputs {
                    if !self.owners.contains(&input.request_id.client_id) {
                        return Err(ApplyError::UnknownPlayer(
                            input.request_id.client_id.clone(),
                        ));
                    }
                }
                expected_tick += 1;
            }
            prefix.push((LogSlot(slot), value.clone()));
            slot += 1;
        }
        let mut applied = Vec::new();
        for (slot, value) in prefix {
            let mut results = Vec::new();
            let tick = match value {
                LogValue::Noop => None,
                LogValue::Tick(batch) => {
                    for input in batch.inputs {
                        let old_tick = self.requests.get(&input.request_id).copied();
                        if old_tick.is_none() {
                            let player = self
                                .owners
                                .iter()
                                .position(|id| *id == input.request_id.client_id)
                                .expect("validated roster");
                            self.game.set_direction(player, input.direction);
                            self.requests.insert(input.request_id.clone(), batch.tick);
                        }
                        results.push(ClientResult {
                            request_id: input.request_id,
                            tick: old_tick.unwrap_or(batch.tick),
                            duplicate: old_tick.is_some(),
                        });
                    }
                    self.game.tick(&mut self.rng);
                    self.next_tick += 1;
                    Some(batch.tick)
                }
            };
            self.next_slot += 1;
            applied.push(AppliedEntry {
                slot,
                tick,
                results,
            });
        }
        Ok(applied)
    }
}
