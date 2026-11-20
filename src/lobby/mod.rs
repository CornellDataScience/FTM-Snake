//! The standalone (--NONE) game task: owns every connected player and the running game, and is
//! the only code that changes shared state. Connections talk to it by sending
//! `Event`s.

use std::time::Duration;

use rand::{RngCore, SeedableRng, rngs::StdRng};
use tokio::{
    sync::{mpsc, oneshot},
    time::{MissedTickBehavior, interval},
};

use crate::{
    config::GameConfig,
    game::Status,
    player::Direction,
    protocol::{LobbyPlayer, ServerMessage},
    round::Round,
};

mod snapshot;
use snapshot::snapshot;

const MAX_NAME_LENGTH: usize = 20;
/// Original game speed: one cell every 100 ms, independent of heartbeats.
const TICK: Duration = Duration::from_millis(100);

/// Messages waiting to be written to one client's socket.
pub type Outbox = mpsc::Sender<ServerMessage>;

/// Everything a connection can tell the game task.
pub enum Event {
    /// Replies with the new player's ID, or a reason the join was refused.
    Join {
        name: String,
        outbox: Outbox,
        reply: oneshot::Sender<Result<String, String>>,
    },
    Direction {
        player_id: String,
        direction: Direction,
    },
    SetReady {
        player_id: String,
        ready: bool,
    },
    /// The player sent `leave`.
    Leave {
        player_id: String,
    },
    /// The socket closed or failed, or heartbeats stopped, without a `leave`.
    Disconnected {
        player_id: String,
    },
}

/// One joined player, as the game task sees them.
struct Player {
    id: String,
    name: String,
    ready: bool,
    /// Lost during a game; the player stays listed until the game ends.
    disconnected: bool,
    outbox: Outbox,
}

impl Player {
    fn lobby_entry(&self) -> LobbyPlayer {
        LobbyPlayer {
            id: self.id.clone(),
            name: self.name.clone(),
            ready: self.ready,
            disconnected: self.disconnected,
        }
    }

    /// Queues a message without waiting: a client that falls behind loses
    /// messages instead of stalling the game.
    fn notify(&self, message: ServerMessage) {
        let _ = self.outbox.try_send(message);
    }
}

/// Runs until every connection's sender is dropped.
pub async fn run(mut events: mpsc::Receiver<Event>, config: GameConfig) {
    let mut lobby = Lobby {
        players: Vec::new(),
        next_id: 1,
        round: None,
        rng: StdRng::from_entropy(),
        config,
    };
    let mut ticker = tick_clock();

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => {
                    let was_waiting = lobby.round.is_none();
                    lobby.handle(event);
                    if was_waiting && lobby.round.is_some() { ticker.reset(); }
                },
                None => break,
            },
            _ = ticker.tick(), if lobby.round.is_some() => lobby.tick(),
        }
    }
}

fn tick_clock() -> tokio::time::Interval {
    let mut ticker = interval(TICK);
    // Skip missed deadlines instead of racing through a backlog of moves.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}

struct Lobby {
    config: GameConfig,
    players: Vec<Player>,
    next_id: u64,
    /// `None` while players wait in the lobby.
    round: Option<Round>,
    rng: StdRng,
}

impl Lobby {
    fn handle(&mut self, event: Event) {
        match event {
            Event::Join {
                name,
                outbox,
                reply,
            } => {
                let name = name.trim().to_string();
                if let Err(reason) = self.check_join(&name) {
                    let _ = reply.send(Err(reason));
                    return;
                }
                let id = self.next_id.to_string();
                self.next_id += 1;
                let player = Player {
                    id: id.clone(),
                    name,
                    ready: false,
                    disconnected: false,
                    outbox,
                };
                // Welcome first, so the client knows its ID before anything else.
                player.notify(ServerMessage::Welcome {
                    player_id: id.clone(),
                });
                self.players.push(player);
                self.broadcast_lobby();
                let _ = reply.send(Ok(id));
            }
            Event::Direction {
                player_id,
                direction,
            } => {
                if let Some(round) = &mut self.round {
                    round.set_direction(&player_id, direction);
                }
            }
            Event::SetReady { player_id, ready } => {
                if self.round.is_some() {
                    return;
                }
                if let Some(player) = self.player_mut(&player_id) {
                    player.ready = ready;
                }
                self.broadcast_lobby();
                self.start_if_ready();
            }
            Event::Leave { player_id } | Event::Disconnected { player_id } => {
                if self.round.is_some() {
                    // Their snake keeps its last direction until it crashes.
                    if let Some(player) = self.player_mut(&player_id) {
                        player.disconnected = true;
                    }
                } else {
                    self.players.retain(|p| p.id != player_id);
                }
                self.broadcast_lobby();
                self.start_if_ready();
            }
        }
    }

    /// Move every snake once, then publish the authoritative state.
    fn tick(&mut self) {
        let Some(round) = &mut self.round else { return };
        let status = round.tick();
        let state = snapshot(round, &self.players);
        if status == Status::Running {
            self.broadcast(ServerMessage::State { state });
            return;
        }
        let winner_id = round.winner().map(str::to_string);
        self.broadcast(ServerMessage::GameOver { winner_id, state });
        self.round = None;
        self.players.retain(|p| !p.disconnected);
        for player in &mut self.players {
            player.ready = false;
        }
        self.broadcast_lobby();
    }

    /// Starts once at least two players have joined and everyone is ready.
    fn start_if_ready(&mut self) {
        if self.round.is_some() || self.players.len() < 2 || !self.players.iter().all(|p| p.ready) {
            return;
        }
        let owners = self.players.iter().map(|p| p.id.clone()).collect();
        let seed = self.rng.next_u64();
        let round = match Round::new(self.config, owners, seed) {
            Ok(round) => round,
            Err(message) => {
                self.broadcast(ServerMessage::Error { message });
                return;
            }
        };
        let round = self.round.insert(round);
        // Show the starting board before the first move.
        let state = snapshot(round, &self.players);
        self.broadcast(ServerMessage::State { state });
    }

    fn player_mut(&mut self, player_id: &str) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == player_id)
    }

    /// Why a player with this name cannot join right now, if any reason applies.
    fn check_join(&self, name: &str) -> Result<(), String> {
        if self.round.is_some() {
            return Err("A game is in progress, try again when it ends".into());
        }
        if name.is_empty() {
            return Err("Enter a name to join".into());
        }
        if name.chars().count() > MAX_NAME_LENGTH {
            return Err(format!("Names can be at most {MAX_NAME_LENGTH} characters"));
        }
        if self.players.iter().any(|p| p.name == name) {
            return Err(format!("{name} is already taken"));
        }
        if self.players.len() >= self.config.player_capacity() {
            return Err(format!(
                "This board has room for {} players; use a larger board for more",
                self.config.player_capacity()
            ));
        }
        Ok(())
    }

    fn broadcast_lobby(&self) {
        self.broadcast(ServerMessage::Lobby {
            players: self.players.iter().map(Player::lobby_entry).collect(),
        });
    }

    fn broadcast(&self, message: ServerMessage) {
        for player in &self.players {
            player.notify(message.clone());
        }
    }
}

#[cfg(test)]
mod tests;
