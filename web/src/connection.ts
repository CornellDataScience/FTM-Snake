import type { ClientMessage, ServerMessage } from "./protocol";

// Override with `VITE_SERVER_URL=ws://host:port npm run dev`.
const SERVER_URL = (import.meta.env.VITE_SERVER_URL as string | undefined) ?? "ws://localhost:8080";

const HEARTBEAT_INTERVAL_MS = 25;

export interface ConnectionHandlers {
  onOpen(): void;
  onMessage(message: ServerMessage): void;
  onClose(): void;
}

/** Thin WebSocket wrapper: the server owns all game state and rules. */
export class ServerConnection {
  private socket: WebSocket;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;

  constructor(handlers: ConnectionHandlers) {
    this.socket = new WebSocket(SERVER_URL);
    this.socket.addEventListener("open", () => {
      this.startHeartbeat();
      handlers.onOpen();
    });
    this.socket.addEventListener("message", (event) => {
      // TODO: validate messages once the server protocol settles.
      handlers.onMessage(JSON.parse(event.data as string) as ServerMessage);
    });
    this.socket.addEventListener("close", () => {
      this.stopHeartbeat();
      handlers.onClose();
    });
  }

  /** Sends a protocol message if the socket is still open. */
  send(message: ClientMessage): void {
    if (this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  /** Stops heartbeats and closes the WebSocket. */
  close(): void {
    this.stopHeartbeat();
    this.socket.close();
  }

  /** Starts sending heartbeats every HEARTBEAT_INTERVAL_MS. */
  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeatTimer = setInterval(() => {
      this.send({ type: "heartbeat" });
    }, HEARTBEAT_INTERVAL_MS);
  }

  /** Clears the heartbeat interval if one is running. */
  private stopHeartbeat(): void {
    if (this.heartbeatTimer !== null) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }
}
