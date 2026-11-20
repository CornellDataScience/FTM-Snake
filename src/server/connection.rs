//! One WebSocket connection: joining, message routing and heartbeat timeout.
use std::time::Duration;

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::Response,
};
use tokio::{
    sync::{mpsc, oneshot},
    time::{Instant, sleep_until},
};

use crate::{
    lobby::{self, Event},
    protocol::{ClientMessage, ServerMessage},
};

/// Browser heartbeats arrive every 25 ms; only they extend this deadline.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(1);
const OUTBOX_CAPACITY: usize = 64;
type GameHandle = mpsc::Sender<Event>;

/// Completes the HTTP handshake, then `handle` owns the socket
pub(super) async fn upgrade(upgrade: WebSocketUpgrade, State(game): State<GameHandle>) -> Response {
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
                match route(message, &player_id) {
                    Route::Leave => break true,
                    Route::Heartbeat => last_heartbeat = Instant::now(),
                    Route::Game(event) => {
                        if game.send(event).await.is_err() {
                            break false;
                        }
                    }
                    Route::Ignore => {}
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

/// What the connection does with a message from a joined player.
enum Route {
    Leave,
    Heartbeat,
    /// Forward to the game task.
    Game(Event),
    Ignore,
}

/// Decides every client message in one place; add new messages here.
fn route(message: ClientMessage, player_id: &str) -> Route {
    let player_id = player_id.to_string();
    match message {
        ClientMessage::Leave => Route::Leave,
        ClientMessage::Heartbeat => Route::Heartbeat,
        // NONE applies directions directly; sequence receipts belong to the
        // future replicated modes. Accept the field for wire compatibility.
        ClientMessage::Direction { direction, .. } => Route::Game(Event::Direction {
            player_id,
            direction,
        }),
        ClientMessage::SetReady { ready } => Route::Game(Event::SetReady { player_id, ready }),
        // Already joined.
        ClientMessage::Join { .. } => Route::Ignore,
    }
}

/// The next text frame, skipping other frames. `None` once the socket closes.
async fn next_text(socket: &mut WebSocket) -> Option<String> {
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(text) => return Some(text.to_string()),
            Message::Close(_) => return None,
            _ => {}
        }
    }
    None
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    let json = serde_json::to_string(message).expect("server messages always serialize");
    socket.send(Message::text(json)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::Direction;

    #[test]
    fn standalone_accepts_old_and_sequenced_inputs_for_the_joined_player() {
        for json in [
            r#"{"type":"direction","direction":"up"}"#,
            r#"{"type":"direction","direction":"up","sequence":7,"player_id":"other"}"#,
            r#"{"type":"direction","direction":"up","sequence":18446744073709551615}"#,
        ] {
            let message = serde_json::from_str(json).unwrap();
            assert!(matches!(
                route(message, "joined-player"),
                Route::Game(Event::Direction { player_id, direction: Direction::Up })
                    if player_id == "joined-player"
            ));
        }
    }
}
