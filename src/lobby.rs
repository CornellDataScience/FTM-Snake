//! The game task: owns every connected player and the running game, and is
//! the only code that changes shared state. Connections talk to it by sending
//! `Event`s.

use std::time::Duration;

use rand::{RngCore, SeedableRng, rngs::StdRng};
use tokio::{
    sync::{mpsc, oneshot},
    time::{MissedTickBehavior, interval},
};

use crate::{
    config::GameConfig,
    consensus::{
        agreement::LocalAgreement,
        application::{AppliedEntry, GameAdapter},
        session::TickSession,
        types::PlayerInput,
    },
    game::Status,
    protocol::{GameState, LobbyPlayer, ServerMessage, SnakeState},
};

const MAX_NAME_LENGTH: usize = 20;
/// Periodic opportunities to propose, not accumulated game-tick obligations.
const TICK: Duration = Duration::from_nanos(1_000_000_000 / 30);

/// Messages waiting to be written to one client's socket.
pub type Outbox = mpsc::Sender<ServerMessage>;

/// Everything a connection can tell the game task.
pub enum Event {
    /// Replies with the new player's ID, or a reason the join was refused.
    Join {
        name: String,
        outbox: Outbox,
        reply: oneshot::Sender<Result<String, String>>,
    },
    Input(PlayerInput),
    SetReady {
        player_id: String,
        ready: bool,
    },
    GetLobby {
        player_id: String,
    },
    /// The player sent `leave`.
    Leave {
        player_id: String,
    },
    /// The socket closed or failed, or heartbeats stopped, without a `leave`.
    Disconnected {
        player_id: String,
    },
}

/// One joined player, as the game task sees them.
struct Player {
    id: String,
    name: String,
    ready: bool,
    /// Lost during a game; the player stays listed until the game ends.
    disconnected: bool,
    outbox: Outbox,
}

impl Player {
    fn lobby_entry(&self) -> LobbyPlayer {
        LobbyPlayer {
            id: self.id.clone(),
            name: self.name.clone(),
            ready: self.ready,
            disconnected: self.disconnected,
        }
    }

    /// Queues a message without waiting: a client that falls behind loses
    /// messages instead of stalling the game.
    fn notify(&self, message: ServerMessage) {
        let _ = self.outbox.try_send(message);
    }
}

/// A game in progress.
struct Round {
    session: TickSession<LocalAgreement>,
    /// Fail closed on a conflicting/invalid chosen log; networking stays alive.
    failed: bool,
}

impl Round {
    /// The game as clients see it.
    fn snapshot(&self, players: &[Player]) -> GameState {
        let application = self.session.application();
        let board = application.game().board();
        let snakes = application.game().snakes().iter().zip(application.owners());
        GameState {
            width: board.width,
            height: board.height,
            snakes: snakes
                .map(|(snake, id)| {
                    let player = players.iter().find(|p| p.id == *id);
                    SnakeState {
                        id: id.clone(),
                        name: player.map(|p| p.name.clone()).unwrap_or_default(),
                        segments: snake.segments().collect(),
                        alive: snake.is_alive(),
                        score: snake.length() as u32,
                        disconnected: player.is_none_or(|p| p.disconnected),
                    }
                })
                .collect(),
            apples: application.game().apples().to_vec(),
        }
    }
}

/// Runs until every connection's sender is dropped.
pub async fn run(mut events: mpsc::Receiver<Event>, config: GameConfig) {
    let mut lobby = Lobby {
        players: Vec::new(),
        next_id: 1,
        round: None,
        rng: StdRng::from_entropy(),
        config,
    };
    let mut ticker = tick_clock();

    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => {
                    let was_waiting = lobby.round.is_none();
                    lobby.handle(event);
                    if was_waiting && lobby.round.is_some() { ticker.reset(); }
                },
                None => break,
            },
            _ = ticker.tick(), if lobby.round.is_some() => lobby.tick(),
        }
    }
}

fn tick_clock() -> tokio::time::Interval {
    let mut ticker = interval(TICK);
    // Stay on periodic deadlines; do not sleep a fresh period after commitment.
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker
}

struct Lobby {
    config: GameConfig,
    players: Vec<Player>,
    next_id: u64,
    /// `None` while players wait in the lobby.
    round: Option<Round>,
    rng: StdRng,
}

impl Lobby {
    fn handle(&mut self, event: Event) {
        match event {
            Event::Join {
                name,
                outbox,
                reply,
            } => {
                let name = name.trim().to_string();
                if let Err(reason) = self.check_join(&name) {
                    let _ = reply.send(Err(reason));
                    return;
                }
                let id = self.next_id.to_string();
                self.next_id += 1;
                let player = Player {
                    id: id.clone(),
                    name,
                    ready: false,
                    disconnected: false,
                    outbox,
                };
                // Welcome first, so the client knows its ID before anything else.
                player.notify(ServerMessage::Welcome {
                    player_id: id.clone(),
                });
                self.players.push(player);
                self.broadcast_lobby();
                let _ = reply.send(Ok(id));
            }
            Event::Input(input) => {
                if let Some(round) = &mut self.round {
                    if let Some(result) = round.session.application().result_for(&input.request_id)
                    {
                        if let Some(player) = self
                            .players
                            .iter()
                            .find(|p| p.id == result.request_id.client_id)
                        {
                            player.notify(ServerMessage::InputApplied {
                                sequence: result.request_id.sequence,
                                tick: result.tick.0,
                                duplicate: true,
                            });
                        }
                    } else {
                        round.session.receive_input(input);
                    }
                }
            }
            Event::SetReady { player_id, ready } => {
                if self.round.is_some() {
                    return;
                }
                if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                    player.ready = ready;
                }
                self.broadcast_lobby();
                self.start_if_ready();
            }
            Event::GetLobby { player_id } => {
                if let Some(player) = self.players.iter().find(|p| p.id == player_id) {
                    player.notify(self.lobby_message());
                }
            }
            Event::Leave { player_id } | Event::Disconnected { player_id } => {
                if self.round.is_some() {
                    // Their snake keeps its last direction until it crashes.
                    if let Some(player) = self.players.iter_mut().find(|p| p.id == player_id) {
                        player.disconnected = true;
                    }
                } else {
                    self.players.retain(|p| p.id != player_id);
                }
                self.broadcast_lobby();
                self.start_if_ready();
            }
        }
    }

    /// Never await agreement here. The actor continues to handle player inputs
    /// between periodic deadlines, even while a future Paxos adapter is pending.
    fn tick(&mut self) {
        let Some(round) = &mut self.round else { return };
        if round.failed {
            return;
        }
        match round.session.on_deadline() {
            Ok(progress) => {
                // LocalAgreement has no network/timer effects. A future peer
                // host must dispatch progress.effects without blocking this actor.
                debug_assert!(progress.effects.is_empty());
                self.publish_applied(progress.applied);
            }
            Err(error) => {
                round.failed = true;
                self.broadcast(ServerMessage::Error {
                    message: format!("Simulation paused: invalid chosen log: {error:?}"),
                });
            }
        }
    }

    /// A receipt means chosen AND applied, including turns ignored by game rules.
    fn publish_applied(&mut self, applied: Vec<AppliedEntry>) {
        for entry in &applied {
            for result in &entry.results {
                if let Some(player) = self
                    .players
                    .iter()
                    .find(|p| p.id == result.request_id.client_id)
                {
                    player.notify(ServerMessage::InputApplied {
                        sequence: result.request_id.sequence,
                        tick: result.tick.0,
                        duplicate: result.duplicate,
                    });
                }
            }
        }
        if !applied.iter().any(|entry| entry.tick.is_some()) {
            return;
        }
        let round = self.round.as_ref().expect("applied within a round");
        let application = round.session.application();
        let state = round.snapshot(&self.players);
        let winner_id = match application.game().status() {
            Status::Running => {
                self.broadcast(ServerMessage::State { state });
                return;
            }
            Status::Winner(snake) => Some(application.owners()[snake].clone()),
            Status::Draw => None,
        };
        self.broadcast(ServerMessage::GameOver { winner_id, state });
        self.round = None;
        self.players.retain(|p| !p.disconnected);
        for player in &mut self.players {
            player.ready = false;
        }
        self.broadcast_lobby();
    }

    /// Starts once at least two players have joined and everyone is ready.
    fn start_if_ready(&mut self) {
        if self.round.is_some() || self.players.len() < 2 || !self.players.iter().all(|p| p.ready) {
            return;
        }
        let owners = self.players.iter().map(|p| p.id.clone()).collect();
        let seed = self.rng.next_u64();
        let application = match GameAdapter::new(self.config, owners, seed) {
            Ok(application) => application,
            Err(message) => {
                self.broadcast(ServerMessage::Error { message });
                return;
            }
        };
        println!(
            "Local-only match: seed={seed}, owners={:?}",
            application.owners()
        );
        let round = self.round.insert(Round {
            session: TickSession::new(LocalAgreement::default(), application),
            failed: false,
        });
        // Show the starting board before the first move.
        let state = round.snapshot(&self.players);
        self.broadcast(ServerMessage::State { state });
    }

    /// Why a player with this name cannot join right now, if any reason applies.
    fn check_join(&self, name: &str) -> Result<(), String> {
        if self.round.is_some() {
            return Err("A game is in progress, try again when it ends".into());
        }
        if name.is_empty() {
            return Err("Enter a name to join".into());
        }
        if name.chars().count() > MAX_NAME_LENGTH {
            return Err(format!("Names can be at most {MAX_NAME_LENGTH} characters"));
        }
        if self.players.iter().any(|p| p.name == name) {
            return Err(format!("{name} is already taken"));
        }
        if self.players.len() >= self.config.player_capacity() {
            return Err(format!(
                "This board has room for {} players; use a larger board for more",
                self.config.player_capacity()
            ));
        }
        Ok(())
    }

    fn lobby_message(&self) -> ServerMessage {
        ServerMessage::Lobby {
            players: self.players.iter().map(Player::lobby_entry).collect(),
        }
    }

    fn broadcast_lobby(&self) {
        self.broadcast(self.lobby_message());
    }

    fn broadcast(&self, message: ServerMessage) {
        for player in &self.players {
            player.notify(message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Board;

    fn lobby() -> Lobby {
        Lobby {
            config: GameConfig {
                board: Board {
                    width: 30,
                    height: 20,
                },
                apple_count: 8,
            },
            players: Vec::new(),
            next_id: 1,
            round: None,
            rng: StdRng::seed_from_u64(5),
        }
    }

    fn join(lobby: &mut Lobby, name: &str) -> (String, mpsc::Receiver<ServerMessage>) {
        let (outbox, receiver) = mpsc::channel(128);
        let (reply, mut answer) = oneshot::channel();
        lobby.handle(Event::Join {
            name: name.into(),
            outbox,
            reply,
        });
        (answer.try_recv().unwrap().unwrap(), receiver)
    }

    fn ready(lobby: &mut Lobby, id: &str) {
        lobby.handle(Event::SetReady {
            player_id: id.into(),
            ready: true,
        });
    }

    #[test]
    fn waits_for_every_player_then_starts_with_configured_board() {
        let mut lobby = lobby();
        let joined: Vec<_> = (0..6)
            .map(|i| join(&mut lobby, &format!("Player{i}")))
            .collect();
        for (id, _) in &joined[..5] {
            ready(&mut lobby, id);
            assert!(lobby.round.is_none());
        }
        ready(&mut lobby, &joined[5].0);
        let round = lobby.round.as_ref().unwrap();
        assert_eq!(round.session.application().game().snakes().len(), 6);
        assert_eq!(
            (
                round.session.application().game().board().width,
                round.session.application().game().board().height
            ),
            (30, 20)
        );
        assert_eq!(round.session.application().game().apples().len(), 8);
        assert!(lobby.check_join("Late player").is_err());
    }

    #[test]
    fn one_ready_player_waits_for_company() {
        let mut lobby = lobby();
        let (id, _receiver) = join(&mut lobby, "Solo");
        ready(&mut lobby, &id);
        assert!(lobby.round.is_none());
    }

    #[test]
    fn leaving_the_lobby_can_unblock_ready_players() {
        let mut lobby = lobby();
        let (a, _a) = join(&mut lobby, "A");
        let (b, _b) = join(&mut lobby, "B");
        let (c, _c) = join(&mut lobby, "C");
        ready(&mut lobby, &a);
        ready(&mut lobby, &b);
        lobby.handle(Event::Leave { player_id: c });
        assert_eq!(
            lobby.round.as_ref().unwrap().session.application().owners(),
            vec![a, b]
        );
    }

    #[test]
    fn capacity_depends_on_board_size() {
        let mut lobby = lobby();
        let joined: Vec<_> = (0..lobby.config.player_capacity())
            .map(|i| join(&mut lobby, &format!("Player{i}")))
            .collect();
        assert_eq!(joined.len(), 24);
        assert!(
            lobby
                .check_join("Extra")
                .unwrap_err()
                .contains("larger board")
        );
    }

    #[test]
    fn round_end_clears_readiness_and_removes_disconnected_players() {
        let mut lobby = lobby();
        let joined: Vec<_> = (0..4)
            .map(|i| join(&mut lobby, &format!("Player{i}")))
            .collect();
        for (id, _) in &joined {
            ready(&mut lobby, id);
        }
        lobby.handle(Event::Disconnected {
            player_id: joined[0].0.clone(),
        });
        assert!(lobby.players[0].disconnected);
        assert_eq!(
            lobby
                .round
                .as_ref()
                .unwrap()
                .session
                .application()
                .game()
                .snakes()
                .len(),
            4
        );
        for _ in 0..100 {
            lobby.tick();
            if lobby.round.is_none() {
                break;
            }
        }
        assert!(lobby.round.is_none());
        assert_eq!(lobby.players.len(), 3);
        assert!(lobby.players.iter().all(|p| !p.ready && !p.disconnected));
        for player in &mut lobby.players {
            player.ready = true;
        }
        lobby.start_if_ready();
        assert_eq!(
            lobby
                .round
                .as_ref()
                .unwrap()
                .session
                .application()
                .game()
                .snakes()
                .len(),
            3
        );
    }
    #[tokio::test(start_paused = true)]
    async fn periodic_deadlines_skip_backlog_without_waiting_a_period_after_commit() {
        let mut ticker = tick_clock();
        ticker.reset();
        let start = tokio::time::Instant::now();
        ticker.tick().await;
        // A commitment takes half a period; the next deadline stays anchored.
        tokio::time::advance(TICK / 2).await;
        ticker.tick().await;
        assert!(tokio::time::Instant::now() - start < TICK * 2 + TICK / 4);
        // Simulate a long scheduling stall. One overdue opportunity is delivered;
        // the following opportunity must wait, rather than replaying ten ticks.
        tokio::time::advance(TICK * 10 + TICK / 2).await;
        ticker.tick().await;
        let after_stall = tokio::time::Instant::now();
        ticker.tick().await;
        assert!(tokio::time::Instant::now() > after_stall);
        assert!(tokio::time::Instant::now() - after_stall <= TICK);
    }
    #[test]
    fn input_success_is_sent_after_application_and_cached_for_retries() {
        use crate::{consensus::types::ClientRequestId, player::Direction};
        let mut lobby = lobby();
        let (a, mut a_messages) = join(&mut lobby, "A");
        let (b, _b_messages) = join(&mut lobby, "B");
        ready(&mut lobby, &a);
        ready(&mut lobby, &b);
        while a_messages.try_recv().is_ok() {}
        let input = PlayerInput {
            request_id: ClientRequestId {
                client_id: a,
                sequence: 7,
            },
            direction: Direction::Down,
        };
        lobby.handle(Event::Input(input.clone()));
        assert!(a_messages.try_recv().is_err());
        lobby.tick();
        assert!(matches!(
            a_messages.try_recv().unwrap(),
            ServerMessage::InputApplied {
                sequence: 7,
                tick: 0,
                duplicate: false,
            }
        ));
        while a_messages.try_recv().is_ok() {}
        lobby.handle(Event::Input(input));
        assert!(matches!(
            a_messages.try_recv().unwrap(),
            ServerMessage::InputApplied {
                sequence: 7,
                tick: 0,
                duplicate: true,
            }
        ));
    }
}
