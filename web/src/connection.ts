import type { ClientMessage, ServerMessage } from "./protocol";

// Override with `VITE_SERVER_URL=ws://host:port npm run dev`.
const SERVER_URL = (import.meta.env.VITE_SERVER_URL as string | undefined) ?? "ws://localhost:8080";

export interface ConnectionHandlers {
  onOpen(): void;
  onMessage(message: ServerMessage): void;
  onClose(): void;
}

/** Thin WebSocket wrapper: the server owns all game state and rules. */
export class ServerConnection {
  private socket: WebSocket;

  constructor(handlers: ConnectionHandlers) {
    this.socket = new WebSocket(SERVER_URL);
    this.socket.addEventListener("open", () => handlers.onOpen());
    this.socket.addEventListener("message", (event) => {
      // TODO: validate messages once the server protocol settles.
      handlers.onMessage(JSON.parse(event.data as string) as ServerMessage);
    });
    this.socket.addEventListener("close", () => handlers.onClose());
  }

  send(message: ClientMessage): void {
    if (this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  close(): void {
    this.socket.close();
  }
}
