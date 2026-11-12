use std::time::Duration;

use apple_logic::{
    config::GameConfig,
    consensus::types::{ClientRequestId, PlayerInput},
    lobby::{self, Event},
    protocol::{ClientMessage, ServerMessage},
};
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
    routing::any,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{Instant, sleep_until},
};

const ADDRESS: &str = "0.0.0.0:8080";
/// A joined client with no heartbeat for this long is marked disconnected.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(1);
/// Messages queued for one client before the game task starts dropping them.
const OUTBOX_CAPACITY: usize = 64;
const EVENT_CAPACITY: usize = 256;

/// How connections reach the game task.
type GameHandle = mpsc::Sender<Event>;

/// Sets up the server
#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Usage: server [--width 25] [--height 25] [--apples 3]\n\
                  Dimensions are in cells. Each player gets a 5 x 5 starting area.\n\
                  A round starts when at least two players join and everyone is ready."
        );
        return;
    }
    let config = GameConfig::from_args(args).unwrap_or_else(|error| {
        eprintln!("{error}\nRun with --help for usage.");
        std::process::exit(2);
    });
    println!(
        "Board: {} x {}, apples: {}, player capacity: {}",
        config.board.width,
        config.board.height,
        config.apple_count,
        config.player_capacity()
    );
    println!(
        "Agreement: local single-node adapter (no distributed fault tolerance), target 30 ticks/s"
    );
    let (events, event_receiver) = mpsc::channel(EVENT_CAPACITY);
    tokio::spawn(lobby::run(event_receiver, config));

    let app = Router::new().route("/", any(upgrade)).with_state(events);
    let address = std::env::var("SNAKE_BIND_ADDRESS").unwrap_or_else(|_| ADDRESS.to_string());
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| panic!("cannot bind {address}: {error}"));
    println!("listening on ws://{address}");
    axum::serve(listener, app)
        .await
        .expect("the server stopped");
}

/// Completes the HTTP handshake, then `handle` owns the socket
async fn upgrade(upgrade: WebSocketUpgrade, State(game): State<GameHandle>) -> Response {
    upgrade.on_upgrade(move |socket| handle(socket, game))
}

/// Serves one client: waits for a join, then relays messages until it leaves.
async fn handle(mut socket: WebSocket, game: GameHandle) {
    println!("client connected");
    let (outbox, mut outbox_receiver) = mpsc::channel(OUTBOX_CAPACITY);

    let Some(player_id) = join(&mut socket, &game, outbox).await else {
        println!("client disconnected before joining");
        return;
    };
    println!("player {player_id} joined");

    let mut last_heartbeat = Instant::now();
    let mut next_sequence = 0;

    // Each pass reads one message from the client, writes one queued for it,
    // or checks that its heartbeats are still arriving.
    let left = loop {
        tokio::select! {
            text = next_text(&mut socket) => {
                let Some(text) = text else { break false };
                // Unreadable messages are skipped so a bad client cannot crash this task.
                let Ok(message) = serde_json::from_str::<ClientMessage>(&text) else {
                    continue;
                };
                match message {
                    ClientMessage::Leave => break true,
                    ClientMessage::Heartbeat => {
                        last_heartbeat = Instant::now();
                        continue;
                    }
                    _ => {}
                }
                let Some(event) = to_event(message, &player_id, &mut next_sequence) else {
                    continue;
                };
                if game.send(event).await.is_err() {
                    break false;
                }
            }
            Some(message) = outbox_receiver.recv() => {
                if send(&mut socket, &message).await.is_err() {
                    break false;
                }
            }
            // Only a heartbeat extends this deadline; other traffic does not.
            _ = sleep_until(last_heartbeat + HEARTBEAT_TIMEOUT) => {
                println!("player {player_id} timed out");
                break false;
            }
        }
    };

    let event = if left {
        Event::Leave { player_id }
    } else {
        Event::Disconnected { player_id }
    };
    // Fails only if the game task has stopped, and then there is nothing to tell.
    let _ = game.send(event).await;
    println!("client disconnected");
}

/// Waits for a `join` the game task accepts. Returns the new player's ID, or
/// `None` if the socket closes first. Refused joins get an `error` and may retry.
async fn join(socket: &mut WebSocket, game: &GameHandle, outbox: lobby::Outbox) -> Option<String> {
    while let Some(text) = next_text(socket).await {
        // Before joining, every other message is ignored.
        let Ok(ClientMessage::Join { name }) = serde_json::from_str(&text) else {
            continue;
        };
        let (reply, answer) = oneshot::channel();
        let event = Event::Join {
            name,
            outbox: outbox.clone(),
            reply,
        };
        game.send(event).await.ok()?;
        match answer.await.ok()? {
            Ok(player_id) => return Some(player_id),
            Err(message) => send(socket, &ServerMessage::Error { message }).await.ok()?,
        }
    }
    None
}

/// The game event for a joined player's message, or `None` for messages the
/// game task does not handle yet.
fn to_event(message: ClientMessage, player_id: &str, next_sequence: &mut u64) -> Option<Event> {
    let player_id = player_id.to_string();
    match message {
        ClientMessage::Direction {
            direction,
            sequence,
        } => {
            let sequence = sequence.unwrap_or(*next_sequence);
            *next_sequence = (*next_sequence).max(sequence.checked_add(1)?);
            Some(Event::Input(PlayerInput {
                request_id: ClientRequestId {
                    client_id: player_id,
                    sequence,
                },
                direction,
            }))
        }
        ClientMessage::SetReady { ready } => Some(Event::SetReady { player_id, ready }),
        ClientMessage::GetLobby => Some(Event::GetLobby { player_id }),
        // Already joined; `handle` deals with `leave` and `heartbeat`; the
        // leaderboard is not decided yet.
        ClientMessage::Join { .. }
        | ClientMessage::Leave
        | ClientMessage::Heartbeat
        | ClientMessage::GetLeaderboard => None,
    }
}

/// The next text frame, skipping other frames. `None` once the socket closes.
async fn next_text(socket: &mut WebSocket) -> Option<String> {
    while let Some(Ok(message)) = socket.recv().await {
        if let Message::Text(text) = message {
            return Some(text.to_string());
        }
    }
    None
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).expect("server messages always serialize");
    socket.send(Message::text(json)).await
}
