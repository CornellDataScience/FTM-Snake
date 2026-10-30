//! Messages exchanged with the game server.
//!
//! Shapes follow serde's default JSON encoding of the Rust types, so the
//! server can derive `Serialize`/`Deserialize` and stay compatible:
//! `Point` is an `(i16, i16)` tuple and `Direction` is a unit enum.

export type Point = [x: number, y: number];

export type Direction = "up" | "down" | "left" | "right";

export interface SnakeState {
  id: string;
  name: string;
  /** Head first, then body, then tail. */
  segments: Point[];
  alive: boolean;
  score: number;
}

export interface GameState {
  width: number;
  height: number;
  snakes: SnakeState[];
  apples: Point[];
}

export interface LobbyPlayer {
  id: string;
  name : string; 
  ready : boolean;
}

export interface LeaderboardEntry {
  name: string;
  score: number;
}

export type ClientMessage =
  | { type: "join"; name: string }
  | { type: "direction"; direction: Direction }
  | { type: "set_ready"; ready: boolean }
  | { type: "get_lobby" }
  | { type: "get_leaderboard" }
  | { type: "leave" }; 

export type ServerMessage =
  | { type: "welcome"; player_id: string }
  | { type: "state"; state: GameState }
  | { type: "player_joined"; player : LobbyPlayer }
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