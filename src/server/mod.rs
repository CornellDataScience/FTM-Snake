//! Server entry point. Keep mode selection here, networking in `connection`,
//! and standalone gameplay in `lobby`. Replicated runtimes can be added here
//! without putting consensus branches into the original game loop.
mod connection;
mod options;

pub use options::{ServerMode, ServerOptions};

use axum::{Router, routing::any};
use tokio::{net::TcpListener, sync::mpsc};

use crate::{config::GameConfig, lobby};

const EVENT_CAPACITY: usize = 256;

/// Refuse unfinished modes before binding a port or accepting any players.
pub async fn run(options: ServerOptions, address: &str) -> Result<(), String> {
    match options.mode {
        ServerMode::None => run_standalone(options.game, address).await,
        ServerMode::PrimaryBackup => Err(
            "--PB: primary/backup is a placeholder and is not implemented yet. Use --NONE to play."
                .into(),
        ),
        ServerMode::Paxos => Err(
            "--PAXOS: only the consensus interfaces/test adapter exist; the distributed server is not implemented yet. Use --NONE to play."
                .into(),
        ),
    }
}

async fn run_standalone(config: GameConfig, address: &str) -> Result<(), String> {
    config.validate()?;
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("Cannot bind {address}: {error}"))?;
    let (events, receiver) = mpsc::channel(EVENT_CAPACITY);
    let app = Router::new()
        .route("/", any(connection::upgrade))
        .with_state(events);
    let game_task = tokio::spawn(lobby::run(receiver, config));

    println!("Mode: NONE (standalone, 10 moves/second)");
    println!(
        "Board: {} x {}, apples: {}, player capacity: {}",
        config.board.width,
        config.board.height,
        config.apple_count,
        config.player_capacity()
    );
    println!("listening on ws://{address}");

    let result = axum::serve(listener, app).await;
    game_task.abort();
    result.map_err(|error| format!("Server stopped: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn placeholders_fail_before_attempting_to_bind() {
        for mode in [ServerMode::PrimaryBackup, ServerMode::Paxos] {
            let options = ServerOptions {
                mode,
                ..ServerOptions::default()
            };
            let error = run(options, "invalid address").await.unwrap_err();
            assert!(error.contains("not implemented yet"), "{error}");
            assert!(error.contains("--NONE"), "{error}");
        }
    }
}
