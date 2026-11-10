import type { ClientMessage, ServerMessage } from "./protocol";

// Use the page's host for LAN clients, unless a deployment supplies an override.
export const DEFAULT_SERVER_URL =
  (import.meta.env.VITE_SERVER_URL as string | undefined) ??
  `${window.location.protocol === "https:" ? "wss:" : "ws:"}//${window.location.hostname}:8080`;

/** Accept a host, host:port, or a full WebSocket URL. */
export function resolveServerUrl(address: string): string {
  const value = address.trim();
  if (!value) throw new Error("Enter a lobby IP or hostname.");

  const hasScheme = value.includes("://");
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const url = new URL(hasScheme ? value : `${protocol}//${value}`);
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error("Use an IP, hostname, or a ws:// or wss:// URL.");
  }
  if (url.username || url.password || url.hash || value.includes("#")) {
    throw new Error("Lobby addresses cannot include credentials or fragments.");
  }
  const hasPort = /:\d+$/.test(value.split(/[/?]/, 1)[0]);
  if (!hasScheme && !hasPort) url.port = "8080";
  return url.href;
}

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

  constructor(serverUrl: string, handlers: ConnectionHandlers) {
    this.socket = new WebSocket(serverUrl);
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
