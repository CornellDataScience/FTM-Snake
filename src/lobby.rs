//! The game task: owns every connected player and is the only code that
//! changes shared state. Connections talk to it by sending `Event`s.

use tokio::sync::{mpsc, oneshot};

use crate::{
    player::Direction,
    protocol::{LobbyPlayer, ServerMessage},
};

const MAX_PLAYERS: usize = 10;
const MAX_NAME_LENGTH: usize = 20;

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
    /// Lost without a `leave`; the player stays listed so others can see it.
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
pub async fn run(mut events: mpsc::Receiver<Event>) {
    let mut players: Vec<Player> = Vec::new();
    let mut next_id: u64 = 1;

    while let Some(event) = events.recv().await {
        match event {
            Event::Join {
                name,
                outbox,
                reply,
            } => {
                let name = name.trim().to_string();
                if let Err(reason) = check_join(&players, &name) {
                    let _ = reply.send(Err(reason));
                    continue;
                }
                let player = Player {
                    id: next_id.to_string(),
                    name,
                    ready: false,
                    disconnected: false,
                    outbox,
                };
                next_id += 1;

                // Welcome first, so the client knows its ID before anything else.
                player.notify(ServerMessage::Welcome {
                    player_id: player.id.clone(),
                });
                for other in &players {
                    other.notify(ServerMessage::PlayerJoined {
                        player: player.lobby_entry(),
                    });
                }
                players.push(player);
                let joined = players.last().expect("just pushed");
                joined.notify(lobby(&players));
                let _ = reply.send(Ok(joined.id.clone()));
                // TODO: if a round is running, keep the player in the waiting room.
            }
            Event::Direction { .. } => {
                // TODO: forward to `Game::set_direction` once rounds run.
            }
            Event::SetReady { player_id, ready } => {
                if let Some(player) = players.iter_mut().find(|p| p.id == player_id) {
                    player.ready = ready;
                }
                broadcast(&players, lobby(&players));
                // TODO: start the countdown when everyone is ready and there are at least 2.
            }
            Event::GetLobby { player_id } => {
                if let Some(player) = players.iter().find(|p| p.id == player_id) {
                    player.notify(lobby(&players));
                }
            }
            Event::Leave { player_id } => {
                players.retain(|p| p.id != player_id);
                broadcast(&players, lobby(&players));
            }
            Event::Disconnected { player_id } => {
                // TODO: mid-round, a disconnected player's snake keeps moving until it dies.
                if let Some(player) = players.iter_mut().find(|p| p.id == player_id) {
                    player.disconnected = true;
                }
                broadcast(&players, lobby(&players));
            }
        }
    }
}

/// Why a player with this name cannot join right now, if any reason applies.
fn check_join(players: &[Player], name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Enter a name to join".into());
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(format!("Names can be at most {MAX_NAME_LENGTH} characters"));
    }
    if players.iter().any(|p| p.name == name) {
        return Err(format!("{name} is already taken"));
    }
    if players.len() >= MAX_PLAYERS {
        return Err("The game is full".into());
    }
    Ok(())
}

fn lobby(players: &[Player]) -> ServerMessage {
    ServerMessage::Lobby {
        players: players.iter().map(Player::lobby_entry).collect(),
    }
}

fn broadcast(players: &[Player], message: ServerMessage) {
    for player in players {
        player.notify(message.clone());
    }
}
