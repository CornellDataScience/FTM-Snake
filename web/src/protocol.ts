//! Messages exchanged with the game server.
//!
//! Mirrored by `src/protocol.rs`; change both files together. The Rust side
//! matches these shapes with `#[serde(tag = "type", rename_all = "snake_case")]`
//! on the message enums and `#[serde(rename_all = "lowercase")]` on
//! `Direction`. `Point` is an `(i16, i16)` tuple, which serde writes as `[x, y]`.

export type Point = [x: number, y: number];

export type Direction = "up" | "down" | "left" | "right";

export interface SnakeState {
  id: string;
  name: string;
  /** Head first, then body, then tail. */
  segments: Point[];
  alive: boolean;
  score: number;
  /** Network disconnect; independent of in-game `alive`. */
  disconnected: boolean;
}

export interface GameState {
  width: number;
  height: number;
  snakes: SnakeState[];
  apples: Point[];
}

export interface LobbyPlayer {
  id: string;
  name: string;
  ready: boolean;
  /** Network disconnect; player stays in the list when true. */
  disconnected: boolean;
}

export interface LeaderboardEntry {
  name: string;
  score: number;
}

export type ClientMessage =
  | { type: "join"; name: string }
  | { type: "direction"; direction: Direction; sequence?: number }
  | { type: "set_ready"; ready: boolean }
  | { type: "get_lobby" }
  | { type: "get_leaderboard" }
  | { type: "leave" }
  | { type: "heartbeat" };

export type ServerMessage =
  | { type: "input_applied"; sequence: number; tick: number; duplicate: boolean }
  | { type: "welcome"; player_id: string }
  | { type: "state"; state: GameState }
  | { type: "player_joined"; player: LobbyPlayer }
  | { type: "lobby"; players: LobbyPlayer[] }
  | {
      type: "player_eliminated";
      player_id: string;
      state: GameState;
    }
  | {
      type: "game_over";
      winner_id: string | null;
      state: GameState;
    }
  | { type: "leaderboard"; entries: LeaderboardEntry[] }
  | { type: "error"; message: string };
