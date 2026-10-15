import type { GameState, Point } from "./protocol";

// Google Snake palette.
const LIGHT_TILE = "#aad751";
const DARK_TILE = "#a2d149";
const APPLE = "#e7471d";
const LEAF = "#4a752c";
const PLAYER = "#4e7cf6";
const OPPONENT = "#b048f0";
const EYE = "#ffffff";
const PUPIL = "#1a1a1a";

/** Shown before the server sends its first state. */
export const EMPTY_BOARD: GameState = { width: 25, height: 25, snakes: [], apples: [] };

export function drawGame(canvas: HTMLCanvasElement, state: GameState, playerId: string | null): void {
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const dpr = window.devicePixelRatio || 1;
  const { clientWidth, clientHeight } = canvas;
  canvas.width = clientWidth * dpr;
  canvas.height = clientHeight * dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  const cell = Math.min(clientWidth / state.width, clientHeight / state.height);
  const ox = (clientWidth - cell * state.width) / 2;
  const oy = (clientHeight - cell * state.height) / 2;
  const center = ([x, y]: Point): [number, number] => [ox + (x + 0.5) * cell, oy + (y + 0.5) * cell];

  for (let y = 0; y < state.height; y++) {
    for (let x = 0; x < state.width; x++) {
      ctx.fillStyle = (x + y) % 2 === 0 ? LIGHT_TILE : DARK_TILE;
      ctx.fillRect(ox + x * cell, oy + y * cell, cell, cell);
    }
  }

  for (const apple of state.apples) {
    const [cx, cy] = center(apple);
    ctx.fillStyle = APPLE;
    ctx.beginPath();
    ctx.arc(cx, cy + cell * 0.05, cell * 0.38, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = LEAF;
    ctx.beginPath();
    ctx.ellipse(cx + cell * 0.12, cy - cell * 0.32, cell * 0.14, cell * 0.07, -0.6, 0, Math.PI * 2);
    ctx.fill();
  }

  for (const snake of state.snakes) {
    if (snake.segments.length === 0) continue;
    const color = snake.id === playerId ? PLAYER : OPPONENT;
    ctx.globalAlpha = snake.alive ? 1 : 0.4;
    ctx.strokeStyle = color;
    ctx.lineWidth = cell * 0.7;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    ctx.beginPath();
    for (const [i, segment] of snake.segments.entries()) {
      const [cx, cy] = center(segment);
      if (i === 0) ctx.moveTo(cx, cy);
      else ctx.lineTo(cx, cy);
    }
    if (snake.segments.length === 1) ctx.lineTo(...center(snake.segments[0]));
    ctx.stroke();
    drawEyes(ctx, snake.segments, center, cell);
    ctx.globalAlpha = 1;
  }
}

function drawEyes(
  ctx: CanvasRenderingContext2D,
  segments: Point[],
  center: (p: Point) => [number, number],
  cell: number,
): void {
  const [hx, hy] = center(segments[0]);
  // Face away from the neck; default to facing right.
  const [nx, ny] = segments.length > 1 ? center(segments[1]) : [hx - cell, hy];
  const fx = Math.sign(hx - nx);
  const fy = Math.sign(hy - ny);
  for (const side of [-1, 1]) {
    const ex = hx + fx * cell * 0.12 + -fy * side * cell * 0.17;
    const ey = hy + fy * cell * 0.12 + fx * side * cell * 0.17;
    ctx.fillStyle = EYE;
    ctx.beginPath();
    ctx.arc(ex, ey, cell * 0.12, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = PUPIL;
    ctx.beginPath();
    ctx.arc(ex + fx * cell * 0.04, ey + fy * cell * 0.04, cell * 0.06, 0, Math.PI * 2);
    ctx.fill();
  }
}
