//! Messages exchanged with the web client; must match web/src/protocol.ts.
use serde::{Deserialize, Serialize};

use crate::{Point, player::Direction};

/// Browser → server.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Join { name: String },              // "join"
    Direction { direction: Direction }, // "direction"
    SetReady { ready: bool },           // "set_ready"
    GetLobby,                           // "get_lobby"
    GetLeaderboard,                     // "get_leaderboard"
    Leave,                              // "leave"
    Heartbeat,                          // "heartbeat"
}

/// Server → browser.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Welcome {
        player_id: String,
    }, // "welcome"
    State {
        state: GameState,
    }, // "state"
    PlayerJoined {
        player: LobbyPlayer,
    }, // "player_joined"
    Lobby {
        players: Vec<LobbyPlayer>,
    }, // "lobby"
    PlayerEliminated {
        player_id: String,
        state: GameState,
    }, // "player_eliminated"
    /// `winner_id` is `None` (JSON `null`) for a draw.
    GameOver {
        winner_id: Option<String>,
        state: GameState,
    }, // "game_over"
    Leaderboard {
        entries: Vec<LeaderboardEntry>,
    }, // "leaderboard"
    Error {
        message: String,
    }, // "error"
}

#[derive(Debug, Clone, Serialize)]
pub struct GameState {
    pub width: i16,
    pub height: i16,
    pub snakes: Vec<SnakeState>,
    pub apples: Vec<Point>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SnakeState {
    pub id: String,
    pub name: String,
    /// Head first, then body, then tail.
    pub segments: Vec<Point>,
    pub alive: bool,
    pub score: u32,
    /// Network disconnect; independent of in-game `alive`.
    pub disconnected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LobbyPlayer {
    pub id: String,
    pub name: String,
    pub ready: bool,
    /// Network disconnect; player stays in the list when true.
    pub disconnected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeaderboardEntry {
    pub name: String,
    pub score: u32,
}
