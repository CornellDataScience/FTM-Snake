import { ServerConnection } from "./connection";
import type { Direction, GameState, ServerMessage } from "./protocol";
import { EMPTY_BOARD, drawGame } from "./render";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const joinView = $<HTMLElement>("join-view");
const joinForm = $<HTMLFormElement>("join-form");
const nameInput = $<HTMLInputElement>("name-input");
const gameView = $<HTMLElement>("game-view");
const canvas = $<HTMLCanvasElement>("board");
const score = $<HTMLSpanElement>("score");
const playerName = $<HTMLSpanElement>("player-name");
const overlay = $<HTMLDivElement>("overlay");
const overlayText = $<HTMLParagraphElement>("overlay-text");
const leaveButton = $<HTMLButtonElement>("leave-button");

const KEY_DIRECTIONS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  w: "up",
  s: "down",
  a: "left",
  d: "right",
};

let connection: ServerConnection | null = null;
let playerId: string | null = null;
let state: GameState = EMPTY_BOARD;

joinForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const name = nameInput.value.trim();
  if (!name) return;
  playerName.textContent = name;
  showGame(true);
  showOverlay("Connecting…");
  const current: ServerConnection = new ServerConnection({
    onOpen: () => current.send({ type: "join", name }),
    // Ignore events from a connection the player already left.
    onMessage: (message) => connection === current && handleMessage(message),
    onClose: () => connection === current && showOverlay("Disconnected from the game server"),
  });
  connection = current;
});

leaveButton.addEventListener("click", () => {
  connection?.close();
  connection = null;
  showGame(false);
});

window.addEventListener("keydown", (event) => {
  const direction = KEY_DIRECTIONS[event.key];
  if (!connection || !direction) return;
  event.preventDefault();
  connection.send({ type: "direction", direction });
});

window.addEventListener("resize", () => drawGame(canvas, state, playerId));

function handleMessage(message: ServerMessage): void {
  switch (message.type) {
    case "welcome":
      playerId = message.player_id;
      break;
    case "state": {
      state = message.state;
      const me = state.snakes.find((snake) => snake.id === playerId);
      score.textContent = String(me?.score ?? 0);
      if (me && !me.alive) showOverlay("Game over");
      else overlay.hidden = true;
      drawGame(canvas, state, playerId);
      break;
    }
    case "error":
      showOverlay(message.message);
      break;
  }
}

function showOverlay(text: string): void {
  overlayText.textContent = text;
  overlay.hidden = false;
}

function showGame(inGame: boolean): void {
  joinView.hidden = inGame;
  gameView.hidden = !inGame;
  playerId = null;
  state = EMPTY_BOARD;
  score.textContent = "0";
  if (inGame) drawGame(canvas, state, playerId);
  else nameInput.focus();
}
