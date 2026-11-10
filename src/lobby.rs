//! The game task: owns every connected player and the running game, and is
//! the only code that changes shared state. Connections talk to it by sending
//! `Event`s.

use std::time::Duration;

use rand::{SeedableRng, rngs::StdRng};
use tokio::{
    sync::{mpsc, oneshot},
    time::{MissedTickBehavior, interval},
};

use crate::{
    PLAYER_COUNT,
    game::{Game, Status},
    player::Direction,
    protocol::{GameState, LobbyPlayer, ServerMessage, SnakeState},
};

const MAX_NAME_LENGTH: usize = 20;
/// How often the snakes move, matching the desktop game.
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
    GetLobby {
        player_id: String,
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

/// A game in progress.
struct Round {
    game: Game,
    /// Player IDs by snake index: `owners[i]` steers snake `i`.
    owners: Vec<String>,
}

impl Round {
    /// The game as clients see it.
    fn snapshot(&self, players: &[Player]) -> GameState {
        let board = self.game.board();
        let snakes = self.game.snakes().iter().zip(&self.owners);
        GameState {
            width: board.width,
            height: board.height,
            snakes: snakes
                .map(|(snake, id)| {
                    let player = players.iter().find(|p| p.id == *id);
                    SnakeState {
                        id: id.clone(),
                        name: player.map(|p| p.name.clone()).unwrap_or_default(),
                        segments: snake.segments().collect(),
                        alive: snake.is_alive(),
                        score: snake.length() as u32,
                        disconnected: player.is_none_or(|p| p.disconnected),
                    }
                })
                .collect(),
            apples: self.game.apples().to_vec(),
        }
    }
}

/// Runs until every connection's sender is dropped.
pub async fn run(mut events: mpsc::Receiver<Event>) {
    let mut lobby = Lobby {
        players: Vec::new(),
        next_id: 1,
        round: None,
        rng: StdRng::from_entropy(),
    };
    let mut ticker = interval(TICK);
    // Ticks are not polled between games; don't replay them when one starts.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => lobby.handle(event),
                None => break,
            },
            _ = ticker.tick(), if lobby.round.is_some() => lobby.tick(),
        }
    }
}

struct Lobby {
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
                if let Some(round) = &mut self.round
                    && let Some(snake) = round.owners.iter().position(|id| *id == player_id)
                {
                    round.game.set_direction(snake, direction);
                }
            }
            Event::SetReady { player_id, ready } => {
                if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                    player.ready = ready;
                }
                self.broadcast_lobby();
                self.start_if_ready();
            }
            Event::GetLobby { player_id } => {
                if let Some(player) = self.players.iter().find(|p| p.id == player_id) {
                    player.notify(self.lobby_message());
                }
            }
            Event::Leave { player_id } | Event::Disconnected { player_id } => {
                if self.round.is_some() {
                    // Their snake keeps its last direction until it crashes.
                    if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                        player.disconnected = true;
                    }
                } else {
                    self.players.retain(|p| p.id != player_id);
                }
                self.broadcast_lobby();
            }
        }
    }

    /// Moves every snake once and sends the result, ending the round when
    /// the game is decided.
    fn tick(&mut self) {
        let Some(round) = &mut self.round else {
            return;
        };
        let status = round.game.tick(&mut self.rng);
        let state = round.snapshot(&self.players);
        let winner_id = match status {
            Status::Running => {
                self.broadcast(ServerMessage::State { state });
                return;
            }
            Status::Winner(snake) => Some(round.owners[snake].clone()),
            Status::Draw => None,
        };
        self.broadcast(ServerMessage::GameOver { winner_id, state });

        self.round = None;
        self.players.retain(|p| !p.disconnected);
        for player in &mut self.players {
            player.ready = false;
        }
        self.broadcast_lobby();
    }

    /// Starts a round once the lobby is full and everyone is ready.
    fn start_if_ready(&mut self) {
        if self.round.is_some()
            || self.players.len() != PLAYER_COUNT
            || !self.players.iter().all(|p| p.ready)
        {
            return;
        }
        let round = self.round.insert(Round {
            game: Game::new(&mut self.rng),
            owners: self.players.iter().map(|p| p.id.clone()).collect(),
        });
        // Show the starting board before the first move.
        let state = round.snapshot(&self.players);
        self.broadcast(ServerMessage::State { state });
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
        // TODO: allow up to 4 once `Game` supports more than two snakes.
        if self.players.len() >= PLAYER_COUNT {
            return Err("The game is full".into());
        }
        Ok(())
    }

    fn lobby_message(&self) -> ServerMessage {
        ServerMessage::Lobby {
            players: self.players.iter().map(Player::lobby_entry).collect(),
        }
    }

    fn broadcast_lobby(&self) {
        self.broadcast(self.lobby_message());
    }

    fn broadcast(&self, message: ServerMessage) {
        for player in &self.players {
            player.notify(message.clone());
        }
    }
}
