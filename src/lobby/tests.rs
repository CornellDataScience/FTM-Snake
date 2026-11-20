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
    assert_eq!(round.game().snakes().len(), 6);
    assert_eq!(
        (round.game().board().width, round.game().board().height),
        (30, 20)
    );
    assert_eq!(round.game().apples().len(), 8);
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
    assert_eq!(lobby.round.as_ref().unwrap().owners(), [a, b]);
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
    assert_eq!(lobby.round.as_ref().unwrap().game().snakes().len(), 4);
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
    assert_eq!(lobby.round.as_ref().unwrap().game().snakes().len(), 3);
}
#[tokio::test(start_paused = true)]
async fn clock_moves_ten_times_per_second_and_skips_backlog() {
    let mut ticker = tick_clock();
    ticker.reset();
    let start = tokio::time::Instant::now();
    ticker.tick().await;
    assert_eq!(
        tokio::time::Instant::now() - start,
        Duration::from_millis(100)
    );
    // Work takes half a period; the next deadline stays anchored.
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
fn direction_changes_the_next_move_without_a_consensus_batch() {
    let mut lobby = lobby();
    let (a, mut a_messages) = join(&mut lobby, "A");
    let (b, _b_messages) = join(&mut lobby, "B");
    ready(&mut lobby, &a);
    ready(&mut lobby, &b);
    while a_messages.try_recv().is_ok() {}
    let head = lobby.round.as_ref().unwrap().game().snakes()[0].head();
    lobby.handle(Event::Direction {
        player_id: a,
        direction: Direction::Down,
    });
    let snake = &lobby.round.as_ref().unwrap().game().snakes()[0];
    assert_eq!(snake.head(), head);
    assert_eq!(snake.next_head(), (head.0, head.1 + 1));
    assert!(a_messages.try_recv().is_err());
    lobby.tick();
    assert_eq!(
        lobby.round.as_ref().unwrap().game().snakes()[0].head(),
        (head.0, head.1 + 1)
    );
    // NONE broadcasts state, without claiming a chosen/committed input receipt.
    assert!(matches!(
        a_messages.try_recv().unwrap(),
        ServerMessage::State { .. }
    ));
    assert!(a_messages.try_recv().is_err());
}
