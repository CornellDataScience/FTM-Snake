use super::{
    acceptor::{AcceptorError, AcceptorState},
    agreement::{Agreement, LocalAgreement, PaxosAgreement, Submission, SubmitOutcome},
    application::{ApplyError, GameAdapter},
    batcher::InputBatcher,
    leadership::Leadership,
    session::TickSession,
    types::*,
};
use crate::{Board, config::GameConfig, player::Direction};

fn input(client: &str, sequence: u64, direction: Direction) -> PlayerInput {
    PlayerInput {
        request_id: ClientRequestId {
            client_id: client.into(),
            sequence,
        },
        direction,
    }
}
fn batch(tick: u64, inputs: Vec<PlayerInput>) -> TickBatch {
    TickBatch {
        tick: GameTick(tick),
        inputs,
    }
}
fn chosen(slot: u64, tick: u64, inputs: Vec<PlayerInput>) -> ChosenEntry {
    ChosenEntry {
        slot: LogSlot(slot),
        value: LogValue::Tick(batch(tick, inputs)),
    }
}
fn application() -> GameAdapter {
    GameAdapter::new(GameConfig::default(), vec!["a".into(), "b".into()], 42).unwrap()
}

struct DeferredAgreement {
    outcome: SubmitOutcome,
    attempts: Vec<TickBatch>,
}
impl Agreement for DeferredAgreement {
    fn submit_fresh(&mut self, batch: &TickBatch) -> Submission {
        self.attempts.push(batch.clone());
        Submission::refused(self.outcome.clone())
    }
    fn submit_recovered(&mut self, _: LogSlot, _: &LogValue) -> Submission {
        Submission::refused(SubmitOutcome::Unsupported)
    }
    fn on_applied(&mut self, _: LogSlot) {}
}
fn session(outcome: SubmitOutcome) -> TickSession<DeferredAgreement> {
    TickSession::new(
        DeferredAgreement {
            outcome,
            attempts: vec![],
        },
        application(),
    )
}

#[test]
fn closing_batch_preserves_order_ids_and_later_inputs() {
    let mut batcher = InputBatcher::default();
    let a = input("a", 1, Direction::Down);
    let b = input("a", 2, Direction::Left);
    assert!(batcher.push(a.clone()));
    assert!(!batcher.push(a.clone()));
    let closed = batcher.close(GameTick(0));
    assert!(batcher.push(b.clone()));
    assert!(!batcher.push(a.clone()));
    assert_eq!(closed.inputs, vec![a]);
    assert_eq!(batcher.close(GameTick(1)).inputs, vec![b]);
    assert!(batcher.close(GameTick(2)).inputs.is_empty());
}

#[test]
fn every_refusal_retains_closed_batch_and_buffers_later_inputs() {
    for refusal in [
        SubmitOutcome::Busy,
        SubmitOutcome::NotLeader,
        SubmitOutcome::Recovering,
        SubmitOutcome::Unsupported,
    ] {
        let mut session = session(refusal.clone());
        let first = input("a", 1, Direction::Down);
        let later = input("b", 1, Direction::Up);
        session.receive_input(first.clone());
        assert_eq!(session.on_deadline().unwrap().outcome, Some(refusal));
        session.receive_input(later.clone());
        session.on_deadline().unwrap();
        assert_eq!(
            session.agreement.attempts,
            vec![batch(0, vec![first.clone()]); 2]
        );
        assert_eq!(session.application().next_tick(), GameTick(0));
        session.agreement.outcome = SubmitOutcome::Submitted(LogSlot(0));
        assert!(session.on_deadline().unwrap().applied.is_empty());
        let applied = session.on_chosen(chosen(0, 0, vec![first])).unwrap();
        assert_eq!(applied[0].results.len(), 1);
        session.agreement.outcome = SubmitOutcome::Submitted(LogSlot(1));
        session.on_deadline().unwrap();
        assert_eq!(session.pending().unwrap(), &batch(1, vec![later]));
    }
}

#[test]
fn unresolved_tick_blocks_new_proposals_but_not_input_buffering() {
    let mut session = session(SubmitOutcome::Submitted(LogSlot(0)));
    session.on_deadline().unwrap(); // Empty ticks are still proposals.
    for sequence in 0..100 {
        session.receive_input(input("a", sequence, Direction::Down));
        assert_eq!(
            session.on_deadline().unwrap().outcome,
            Some(SubmitOutcome::Busy)
        );
    }
    assert_eq!(session.agreement.attempts.len(), 1);
    assert_eq!(session.application().next_tick(), GameTick(0));
    session.on_chosen(chosen(0, 0, vec![])).unwrap();
    session.agreement.outcome = SubmitOutcome::Submitted(LogSlot(1));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap().inputs.len(), 100);
    assert_eq!(session.pending().unwrap().tick, GameTick(1));
}

#[test]
fn gaps_noops_and_duplicate_decisions_apply_in_log_order_once() {
    let mut app = application();
    let start = app.game().snakes()[0].head();
    let tick = chosen(1, 0, vec![input("a", 0, Direction::Down)]);
    assert!(app.receive_chosen(tick.clone()).unwrap().is_empty());
    assert_eq!(app.game().snakes()[0].head(), start);
    let applied = app
        .receive_chosen(ChosenEntry {
            slot: LogSlot(0),
            value: LogValue::Noop,
        })
        .unwrap();
    assert_eq!(
        applied.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        vec![LogSlot(0), LogSlot(1)]
    );
    assert_eq!(applied[0].tick, None);
    assert_eq!(applied[1].results.len(), 1);
    assert_eq!(app.next_tick(), GameTick(1));
    let head = app.game().snakes()[0].head();
    assert!(app.receive_chosen(tick).unwrap().is_empty());
    assert_eq!(app.game().snakes()[0].head(), head);
    assert_eq!(
        app.receive_chosen(ChosenEntry {
            slot: LogSlot(1),
            value: LogValue::Noop
        })
        .unwrap_err(),
        ApplyError::ConflictingSlot(LogSlot(1))
    );
}

#[test]
fn first_valid_turn_wins_and_duplicate_requests_never_execute_twice() {
    let mut app = application();
    let down = input("a", 1, Direction::Down);
    let applied = app
        .receive_chosen(chosen(
            0,
            0,
            vec![
                input("a", 0, Direction::Left),  // Invalid reversal.
                down.clone(),                    // First valid turn.
                input("a", 2, Direction::Right), // Committed but ignored this tick.
            ],
        ))
        .unwrap();
    assert_eq!(app.game().snakes()[0].head(), (2, 3));
    assert_eq!(applied[0].results.len(), 3);
    app.receive_chosen(chosen(1, 1, vec![input("a", 3, Direction::Right)]))
        .unwrap();
    let repeated = app.receive_chosen(chosen(2, 2, vec![down])).unwrap();
    assert_eq!(app.game().snakes()[0].head(), (4, 3));
    assert!(repeated[0].results[0].duplicate);
    assert_eq!(repeated[0].results[0].tick, GameTick(0));
}

#[test]
fn invalid_tick_does_not_advance_the_simulation() {
    let mut app = application();
    assert!(matches!(
        app.receive_chosen(chosen(0, 5, vec![])),
        Err(ApplyError::WrongTick { .. })
    ));
    assert_eq!(app.applied_through(), None);
    assert_eq!(app.next_tick(), GameTick(0));
}

#[test]
fn deterministic_replay_includes_growth_food_and_out_of_order_delivery() {
    let config = GameConfig {
        board: Board {
            width: 10,
            height: 10,
        },
        apple_count: 100,
    };
    let make = || GameAdapter::new(config, vec!["a".into(), "b".into()], 123).unwrap();
    let mut first = make();
    let mut second = make();
    let entries = vec![
        ChosenEntry {
            slot: LogSlot(0),
            value: LogValue::Noop,
        },
        chosen(1, 0, vec![input("a", 0, Direction::Down)]),
        chosen(2, 1, vec![]),
        ChosenEntry {
            slot: LogSlot(3),
            value: LogValue::Noop,
        },
        chosen(4, 2, vec![input("b", 0, Direction::Up)]),
    ];
    for entry in &entries {
        first.receive_chosen(entry.clone()).unwrap();
    }
    for entry in entries.iter().rev() {
        second.receive_chosen(entry.clone()).unwrap();
    }
    assert_eq!(first.next_tick(), GameTick(3));
    assert_eq!(first.next_tick(), second.next_tick());
    assert_eq!(first.game().status(), second.game().status());
    assert_eq!(first.game().apples(), second.game().apples());
    for (a, b) in first.game().snakes().iter().zip(second.game().snakes()) {
        assert_eq!(
            a.segments().collect::<Vec<_>>(),
            b.segments().collect::<Vec<_>>()
        );
        assert_eq!(a.is_alive(), b.is_alive());
        assert!(a.length() > 3); // Food randomness and growth were actually exercised.
    }
}

#[test]
fn recovery_reconciles_pending_and_queued_requests_after_authoritative_replay() {
    let mut session = session(SubmitOutcome::Submitted(LogSlot(2)));
    let uncommitted = input("a", 0, Direction::Down);
    let committed_elsewhere = input("b", 0, Direction::Up);
    let later = input("a", 1, Direction::Right);
    session.receive_input(uncommitted.clone());
    session.on_deadline().unwrap();
    session.receive_input(committed_elsewhere.clone());
    session.receive_input(later.clone());
    session.begin_recovery();
    assert_eq!(
        session.on_deadline().unwrap().outcome,
        Some(SubmitOutcome::Recovering)
    );
    session
        .on_chosen(chosen(1, 0, vec![committed_elsewhere]))
        .unwrap();
    assert_eq!(
        session.reconcile_after_recovery(Some(LogSlot(1))),
        Err(ProtocolError::RecoveryNotApplied)
    );
    session
        .on_chosen(ChosenEntry {
            slot: LogSlot(0),
            value: LogValue::Noop,
        })
        .unwrap();
    session.reconcile_after_recovery(Some(LogSlot(1))).unwrap();
    session.on_deadline().unwrap();
    assert_eq!(
        session.pending().unwrap(),
        &batch(1, vec![uncommitted.clone()])
    );
    session.on_chosen(chosen(2, 1, vec![uncommitted])).unwrap();
    session.agreement.outcome = SubmitOutcome::Submitted(LogSlot(3));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap(), &batch(2, vec![later]));
}

#[test]
fn recovery_drops_a_pending_request_already_applied_elsewhere() {
    let mut session = session(SubmitOutcome::NotLeader);
    let request = input("a", 0, Direction::Down);
    session.receive_input(request.clone());
    session.on_deadline().unwrap();
    session.begin_recovery();
    session.on_chosen(chosen(0, 0, vec![request])).unwrap();
    session.reconcile_after_recovery(Some(LogSlot(0))).unwrap();
    assert!(session.pending().is_none());
    session.agreement.outcome = SubmitOutcome::Submitted(LogSlot(1));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap(), &batch(1, vec![]));
}

#[test]
fn shared_acceptor_promise_and_accept_guards_agree() {
    let mut acceptor = AcceptorState::default();
    let low = Ballot {
        counter: 1,
        node_id: 1,
    };
    let high = Ballot {
        counter: 1,
        node_id: 2,
    };
    assert!(low < high);
    acceptor.promise(high).unwrap();
    assert_eq!(acceptor.promise(low), Err(AcceptorError::Preempted(high)));
    let entry = AcceptedEntry {
        slot: LogSlot(0),
        ballot: high,
        value: LogValue::Noop,
    };
    acceptor.accept(entry.clone()).unwrap();
    acceptor.accept(entry.clone()).unwrap();
    let conflict = AcceptedEntry {
        value: LogValue::Tick(batch(0, vec![])),
        ..entry.clone()
    };
    assert_eq!(
        acceptor.accept(conflict),
        Err(AcceptorError::ConflictingValue)
    );
    assert_eq!(
        acceptor.promise(high).unwrap(),
        AcceptedHistory::Complete(vec![entry])
    );
    let higher = Ballot {
        counter: 2,
        node_id: 1,
    };
    acceptor
        .accept(AcceptedEntry {
            slot: LogSlot(1),
            ballot: higher,
            value: LogValue::Noop,
        })
        .unwrap();
    assert_eq!(acceptor.promised(), Some(higher));
    assert!(acceptor.promise(high).is_err());
}

#[test]
fn local_submission_is_not_application_and_cannot_pipeline() {
    let mut local = LocalAgreement::default();
    assert_eq!(
        local.submit_fresh(&batch(0, vec![])).outcome,
        SubmitOutcome::Submitted(LogSlot(0))
    );
    assert_eq!(
        local.submit_fresh(&batch(1, vec![])).outcome,
        SubmitOutcome::Busy
    );
    local.on_applied(LogSlot(9));
    assert_eq!(
        local.submit_fresh(&batch(1, vec![])).outcome,
        SubmitOutcome::Busy
    );
    local.on_applied(LogSlot(0));
    assert_eq!(
        local.submit_fresh(&batch(1, vec![])).outcome,
        SubmitOutcome::Submitted(LogSlot(1))
    );
    let mut session = TickSession::new(LocalAgreement::default(), application());
    assert_eq!(session.on_deadline().unwrap().applied.len(), 1);
    assert_eq!(session.on_deadline().unwrap().applied.len(), 1);
    assert_eq!(session.application().next_tick(), GameTick(2));
}

#[test]
fn unimplemented_protocol_paths_never_claim_commitment() {
    let mut acceptor = AcceptorState::default();
    let mut leadership = Leadership::new(1, vec![1, 2, 3]);
    assert!(matches!(
        leadership.on_timer(TimerEvent::ElectionTimeout, &mut acceptor),
        Err(ProtocolError::Unsupported(_))
    ));
    assert_eq!(leadership.state(), LeadershipState::Follower);
    let mut paxos = PaxosAgreement::new(vec![1, 2, 3]);
    let result = paxos.submit_fresh(&batch(0, vec![]));
    assert_eq!(result.outcome, SubmitOutcome::NotLeader);
    assert!(result.effects.is_empty());
    assert_eq!(
        paxos.submit_recovered(LogSlot(0), &LogValue::Noop).outcome,
        SubmitOutcome::Unsupported
    );
    assert!(
        paxos
            .on_message(
                2,
                PeerMessage::CatchUp {
                    from_slot: LogSlot(0)
                },
                &mut acceptor
            )
            .is_err()
    );
}
