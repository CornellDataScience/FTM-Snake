//! Messages exchanged with the game server.
//!
//! Shapes follow serde's default JSON encoding of the Rust types, so the
//! server can derive `Serialize`/`Deserialize` and stay compatible:
//! `Point` is an `(i16, i16)` tuple and `Direction` is a unit enum.

export type Point = [x: number, y: number];

export type Direction = "Up" | "Down" | "Left" | "Right";

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

export type ClientMessage =
  | { type: "join"; name: string }
  | { type: "direction"; direction: Direction };

export type ServerMessage =
  | { type: "welcome"; playerId: string }
  | { type: "state"; state: GameState }
  | { type: "error"; message: string };
