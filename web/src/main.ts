import { DEFAULT_SERVER_URL, resolveServerUrl, ServerConnection } from "./connection";
import type { Direction, GameState, LobbyPlayer, ServerMessage } from "./protocol";
import { EMPTY_BOARD, drawGame, playerColor } from "./render";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const joinView = $<HTMLElement>("join-view");
const joinForm = $<HTMLFormElement>("join-form");
const nameInput = $<HTMLInputElement>("name-input");
const lobbyIpInput = $<HTMLInputElement>("lobby-ip-input");
const gameView = $<HTMLElement>("game-view");
const canvas = $<HTMLCanvasElement>("board");
const score = $<HTMLSpanElement>("score");
const playerName = $<HTMLSpanElement>("player-name");
const matchStatus = $<HTMLParagraphElement>("match-status");
const overlay = $<HTMLDivElement>("overlay");
const overlayText = $<HTMLParagraphElement>("overlay-text");
const lobby = $<HTMLDivElement>("lobby");
const lobbyList = $<HTMLUListElement>("lobby-list");
const lobbySummary = $<HTMLParagraphElement>("lobby-summary");
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
let inRound = false;

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
  if (!connection || !direction || !inRound) return;
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
      inRound = true;
      showState(message.state);
      // Eliminated players can watch until the whole round finishes.
      overlay.hidden = true;
      break;
    }
    case "game_over": {
      inRound = false;
      showState(message.state);
      const winner = state.snakes.find((snake) => snake.id === message.winner_id);
      const result = !winner ? "Draw!" : winner.id === playerId ? "You win!" : `${winner.name} wins!`;
      matchStatus.textContent = result;
      showOverlay(result, true);
      break;
    }
    case "error":
      showOverlay(message.message);
      break;
  }
}

function showState(next: GameState): void {
  state = next;
  const me = state.snakes.find((snake) => snake.id === playerId);
  const alive = state.snakes.filter((snake) => snake.alive).length;
  score.textContent = String(me?.score ?? 0);
  const status = `${alive}/${state.snakes.length} alive · ${state.width} × ${state.height} board`;
  matchStatus.textContent = me && !me.alive ? `${status} · Eliminated — watching` : status;
  drawGame(canvas, state, playerId);
}

function showLobby(players: LobbyPlayer[]): void {
  const readyCount = players.filter((player) => player.ready).length;
  lobbySummary.textContent = `${players.length} players · ${readyCount} ready. Starts when everyone is ready (minimum 2).`;
  lobbyList.replaceChildren(
    ...players.map((player) => {
      const item = document.createElement("li");
      item.textContent = `${player.name}${player.id === playerId ? " (you)" : ""}${player.ready ? " ✓" : ""}`;
      item.classList.toggle("disconnected", player.disconnected);
      item.style.borderInlineStartColor = playerColor(player.id);
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
  inRound = false;
  ready = false;
  matchStatus.textContent = "Join the lobby, then get ready. Your snake has a white outline.";
  score.textContent = "0";
  if (inGame) drawGame(canvas, state, playerId);
  else nameInput.focus();
}
