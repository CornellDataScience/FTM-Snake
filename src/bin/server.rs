use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::Response,
    routing::any,
};

const ADDRESS: &str = "0.0.0.0:8080";

/// Sets up the server
#[tokio::main]
async fn main() {
    let app = Router::new().route("/", any(upgrade));
    let listener = tokio::net::TcpListener::bind(ADDRESS)
        .await
        .unwrap_or_else(|error| panic!("cannot bind {ADDRESS}: {error}"));
    println!("listening on ws://{ADDRESS}");
    axum::serve(listener, app)
        .await
        .expect("the server stopped");
}

/// Completes the HTTP handshake, then `handle` then owns the socket
async fn upgrade(upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(handle)
}

/// Serves one client until it disconnects, echoing every message back
async fn handle(mut socket: WebSocket) {
    println!("client connected");
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            continue;
        };
        println!("received: {text}");
        if socket.send(Message::text(text)).await.is_err() {
            break;
        }
    }
    println!("client disconnected");
}
