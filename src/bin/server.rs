use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use apple_logic::protocol::ClientMessage;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::any,
};
use tokio::time::{MissedTickBehavior, interval};

const ADDRESS: &str = "0.0.0.0:8080";
const HEARTBEAT_TIMEOUT: Duration = Duration::from_millis(50);

// Temporary stand-in until real join assigns stable player ids.
static NEXT_PLAYER_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
struct AppState {
    clients: Arc<Mutex<HashMap<String, ClientRecord>>>,
}

struct ClientRecord {
    last_heartbeat: Instant,
    disconnected: bool,
    /// Wall-clock times of each connected → disconnected transition.
    disconnect_times: Vec<SystemTime>,
}

/// Binds the listener and serves WebSocket upgrades.
#[tokio::main]
async fn main() {
    let state = AppState::default();
    let app = Router::new().route("/", any(upgrade)).with_state(state);
    let listener = tokio::net::TcpListener::bind(ADDRESS)
        .await
        .unwrap_or_else(|error| panic!("cannot bind {ADDRESS}: {error}"));
    println!("listening on ws://{ADDRESS}");
    axum::serve(listener, app)
        .await
        .expect("the server stopped");
}

/// Completes the HTTP upgrade, then hands the socket to `handle`.
async fn upgrade(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| handle(socket, state))
}

/// Runs one client until the socket closes or heartbeats time out.
async fn handle(mut socket: WebSocket, state: AppState) {
    // TODO(joining): replace with real join / player identity.
    let player_id = next_player_id();
    register_client(&state, &player_id);
    println!("client connected: {player_id}");

    let mut timeout_check = interval(HEARTBEAT_TIMEOUT);
    timeout_check.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Text(text))) => {
                        let client_message: ClientMessage =
                            serde_json::from_str(&text).expect("valid ClientMessage JSON");
                        // Other message types are handled by later join/game work.
                        if let ClientMessage::Heartbeat = client_message {
                            refresh_heartbeat(&state, &player_id);
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
            _ = timeout_check.tick() => {
                if heartbeat_timed_out(&state, &player_id) {
                    break;
                }
            }
        }
    }

    mark_disconnected(&state, &player_id);
    println!("client disconnected: {player_id}");
}

/// Allocates the next temporary player id (until real join exists).
fn next_player_id() -> String {
    format!("player-{}", NEXT_PLAYER_ID.fetch_add(1, Ordering::Relaxed))
}

/// Inserts a new client, or clears `disconnected` on reconnect (keeps history).
fn register_client(state: &AppState, player_id: &str) {
    let mut clients = state.clients.lock().expect("clients lock");
    if let Some(record) = clients.get_mut(player_id) {
        // Reconnect: clear live flag, keep disconnect history.
        record.disconnected = false;
        record.last_heartbeat = Instant::now();
    } else {
        clients.insert(
            player_id.to_string(),
            ClientRecord {
                last_heartbeat: Instant::now(),
                disconnected: false,
                disconnect_times: Vec::new(),
            },
        );
    }
}

/// Records that a heartbeat just arrived from this client.
fn refresh_heartbeat(state: &AppState, player_id: &str) {
    let mut clients = state.clients.lock().expect("clients lock");
    if let Some(record) = clients.get_mut(player_id) {
        record.last_heartbeat = Instant::now();
    }
}

/// True when this connected client has been silent longer than `HEARTBEAT_TIMEOUT`.
fn heartbeat_timed_out(state: &AppState, player_id: &str) -> bool {
    let clients = state.clients.lock().expect("clients lock");
    clients.get(player_id).is_none_or(|record| {
        !record.disconnected && record.last_heartbeat.elapsed() > HEARTBEAT_TIMEOUT
    })
}

/// Marks the client disconnected and appends a disconnect time (idempotent).
fn mark_disconnected(state: &AppState, player_id: &str) {
    let mut clients = state.clients.lock().expect("clients lock");
    let Some(record) = clients.get_mut(player_id) else {
        return;
    };
    if record.disconnected {
        return;
    }
    record.disconnected = true;
    record.disconnect_times.push(SystemTime::now());
}
