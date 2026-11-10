import { DEFAULT_SERVER_URL, resolveServerUrl, ServerConnection } from "./connection";
import type { Direction, GameState, LobbyPlayer, ServerMessage } from "./protocol";
import { EMPTY_BOARD, drawGame } from "./render";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const joinView = $<HTMLElement>("join-view");
const joinForm = $<HTMLFormElement>("join-form");
const nameInput = $<HTMLInputElement>("name-input");
const lobbyIpInput = $<HTMLInputElement>("lobby-ip-input");
const gameView = $<HTMLElement>("game-view");
const canvas = $<HTMLCanvasElement>("board");
const score = $<HTMLSpanElement>("score");
const playerName = $<HTMLSpanElement>("player-name");
const overlay = $<HTMLDivElement>("overlay");
const overlayText = $<HTMLParagraphElement>("overlay-text");
const lobby = $<HTMLDivElement>("lobby");
const lobbyList = $<HTMLUListElement>("lobby-list");
const readyButton = $<HTMLButtonElement>("ready-button");
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
/** Whether the server lists this player as ready. */
let ready = false;

lobbyIpInput.value = DEFAULT_SERVER_URL;
lobbyIpInput.addEventListener("input", () => lobbyIpInput.setCustomValidity(""));

joinForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const name = nameInput.value.trim();
  if (!name) return;
  let current: ServerConnection;
  try {
    const serverUrl = resolveServerUrl(lobbyIpInput.value);
    current = new ServerConnection(serverUrl, {
      onOpen: () => connection === current && current.send({ type: "join", name }),
      // Ignore events from a connection the player already left.
      onMessage: (message) => connection === current && handleMessage(message),
      onClose: () => connection === current && showOverlay("Disconnected from the game server"),
    });
  } catch (error) {
    lobbyIpInput.setCustomValidity(
      error instanceof Error ? error.message : "Enter a valid lobby address.",
    );
    lobbyIpInput.reportValidity();
    return;
  }
  connection = current;
  playerName.textContent = name;
  showGame(true);
  showOverlay("Connecting…");
});

readyButton.addEventListener("click", () => connection?.send({ type: "set_ready", ready: !ready }));

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
      showOverlay("Waiting for players", true);
      break;
    case "lobby":
      showLobby(message.players);
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
    case "game_over": {
      state = message.state;
      drawGame(canvas, state, playerId);
      const winner = state.snakes.find((snake) => snake.id === message.winner_id);
      const result = !winner ? "Draw!" : winner.id === playerId ? "You win!" : `${winner.name} wins!`;
      showOverlay(result, true);
      break;
    }
    case "error":
      showOverlay(message.message);
      break;
  }
}

function showLobby(players: LobbyPlayer[]): void {
  lobbyList.replaceChildren(
    ...players.map((player) => {
      const item = document.createElement("li");
      item.textContent = `${player.name}${player.id === playerId ? " (you)" : ""}${player.ready ? " ✓" : ""}`;
      item.classList.toggle("disconnected", player.disconnected);
      return item;
    }),
  );
  ready = players.find((player) => player.id === playerId)?.ready ?? false;
  readyButton.textContent = ready ? "Not ready" : "Ready";
}

/** Shows `text` over the board, with the lobby beneath it between games. */
function showOverlay(text: string, withLobby = false): void {
  overlayText.textContent = text;
  overlayText.hidden = !text;
  lobby.hidden = !withLobby;
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
