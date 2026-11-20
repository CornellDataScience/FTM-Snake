use super::{
    acceptor::{AcceptorError, AcceptorState},
    batcher::InputBatcher,
    paxos::PaxosServer,
    proposer::{LocalProposer, ProposalOutcome, ProposalResult, Proposer},
    replica::{ExecutionError, Replica},
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
fn decision(slot: u64, tick: u64, inputs: Vec<PlayerInput>) -> Decision {
    Decision {
        slot: Slot(slot),
        command: Command::Tick(batch(tick, inputs)),
    }
}
fn replica() -> Replica {
    Replica::new(GameConfig::default(), vec!["a".into(), "b".into()], 42).unwrap()
}

struct DeferredProposer {
    outcome: ProposalOutcome,
    attempts: Vec<TickBatch>,
}
impl Proposer for DeferredProposer {
    fn propose(&mut self, batch: &TickBatch) -> ProposalResult {
        self.attempts.push(batch.clone());
        ProposalResult::refused(self.outcome.clone())
    }
    fn repropose(&mut self, _: Slot, _: &Command) -> ProposalResult {
        ProposalResult::refused(ProposalOutcome::Unsupported)
    }
    fn on_executed(&mut self, _: Slot) {}
}
fn session(outcome: ProposalOutcome) -> TickSession<DeferredProposer> {
    TickSession::new(
        DeferredProposer {
            outcome,
            attempts: vec![],
        },
        replica(),
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
        ProposalOutcome::Busy,
        ProposalOutcome::NotLeader,
        ProposalOutcome::Recovering,
        ProposalOutcome::Unsupported,
    ] {
        let mut session = session(refusal.clone());
        let first = input("a", 1, Direction::Down);
        let later = input("b", 1, Direction::Up);
        session.receive_input(first.clone());
        assert_eq!(session.on_deadline().unwrap().outcome, Some(refusal));
        session.receive_input(later.clone());
        session.on_deadline().unwrap();
        assert_eq!(
            session.proposer.attempts,
            vec![batch(0, vec![first.clone()]); 2]
        );
        assert_eq!(session.replica().next_tick(), GameTick(0));
        session.proposer.outcome = ProposalOutcome::Proposed(Slot(0));
        assert!(session.on_deadline().unwrap().executed.is_empty());
        let executed = session.on_decision(decision(0, 0, vec![first])).unwrap();
        assert_eq!(executed[0].results.len(), 1);
        session.proposer.outcome = ProposalOutcome::Proposed(Slot(1));
        session.on_deadline().unwrap();
        assert_eq!(session.pending().unwrap(), &batch(1, vec![later]));
    }
}

#[test]
fn unresolved_tick_blocks_new_proposals_but_not_input_buffering() {
    let mut session = session(ProposalOutcome::Proposed(Slot(0)));
    session.on_deadline().unwrap(); // Empty ticks are still proposals.
    for sequence in 0..100 {
        session.receive_input(input("a", sequence, Direction::Down));
        assert_eq!(
            session.on_deadline().unwrap().outcome,
            Some(ProposalOutcome::Busy)
        );
    }
    assert_eq!(session.proposer.attempts.len(), 1);
    assert_eq!(session.replica().next_tick(), GameTick(0));
    session.on_decision(decision(0, 0, vec![])).unwrap();
    session.proposer.outcome = ProposalOutcome::Proposed(Slot(1));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap().inputs.len(), 100);
    assert_eq!(session.pending().unwrap().tick, GameTick(1));
}

#[test]
fn gaps_noops_and_duplicate_decisions_apply_in_log_order_once() {
    let mut app = replica();
    let start = app.game().snakes()[0].head();
    let tick = decision(1, 0, vec![input("a", 0, Direction::Down)]);
    assert!(app.on_decision(tick.clone()).unwrap().is_empty());
    assert_eq!(app.game().snakes()[0].head(), start);
    let executed = app
        .on_decision(Decision {
            slot: Slot(0),
            command: Command::Noop,
        })
        .unwrap();
    assert_eq!(
        executed.iter().map(|entry| entry.slot).collect::<Vec<_>>(),
        vec![Slot(0), Slot(1)]
    );
    assert_eq!(executed[0].tick, None);
    assert_eq!(executed[1].results.len(), 1);
    assert_eq!(app.next_tick(), GameTick(1));
    let head = app.game().snakes()[0].head();
    assert!(app.on_decision(tick).unwrap().is_empty());
    assert_eq!(app.game().snakes()[0].head(), head);
    assert_eq!(
        app.on_decision(Decision {
            slot: Slot(1),
            command: Command::Noop
        })
        .unwrap_err(),
        ExecutionError::ConflictingSlot(Slot(1))
    );
}

#[test]
fn first_valid_turn_wins_and_duplicate_requests_never_execute_twice() {
    let mut app = replica();
    let down = input("a", 1, Direction::Down);
    let executed = app
        .on_decision(decision(
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
    assert_eq!(executed[0].results.len(), 3);
    app.on_decision(decision(1, 1, vec![input("a", 3, Direction::Right)]))
        .unwrap();
    let repeated = app.on_decision(decision(2, 2, vec![down])).unwrap();
    assert_eq!(app.game().snakes()[0].head(), (4, 3));
    assert!(repeated[0].results[0].duplicate);
    assert_eq!(repeated[0].results[0].tick, GameTick(0));
}

#[test]
fn invalid_tick_does_not_advance_the_simulation() {
    let mut app = replica();
    assert!(matches!(
        app.on_decision(decision(0, 5, vec![])),
        Err(ExecutionError::WrongTick { .. })
    ));
    assert_eq!(app.executed_through(), None);
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
    let make = || Replica::new(config, vec!["a".into(), "b".into()], 123).unwrap();
    let mut first = make();
    let mut second = make();
    let entries = vec![
        Decision {
            slot: Slot(0),
            command: Command::Noop,
        },
        decision(1, 0, vec![input("a", 0, Direction::Down)]),
        decision(2, 1, vec![]),
        Decision {
            slot: Slot(3),
            command: Command::Noop,
        },
        decision(4, 2, vec![input("b", 0, Direction::Up)]),
    ];
    for entry in &entries {
        first.on_decision(entry.clone()).unwrap();
    }
    for entry in entries.iter().rev() {
        second.on_decision(entry.clone()).unwrap();
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
    let mut session = session(ProposalOutcome::Proposed(Slot(2)));
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
        Some(ProposalOutcome::Recovering)
    );
    session
        .on_decision(decision(1, 0, vec![committed_elsewhere]))
        .unwrap();
    assert_eq!(
        session.reconcile_after_recovery(Some(Slot(1))),
        Err(ProtocolError::RecoveryNotExecuted)
    );
    session
        .on_decision(Decision {
            slot: Slot(0),
            command: Command::Noop,
        })
        .unwrap();
    session.reconcile_after_recovery(Some(Slot(1))).unwrap();
    session.on_deadline().unwrap();
    assert_eq!(
        session.pending().unwrap(),
        &batch(1, vec![uncommitted.clone()])
    );
    session
        .on_decision(decision(2, 1, vec![uncommitted]))
        .unwrap();
    session.proposer.outcome = ProposalOutcome::Proposed(Slot(3));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap(), &batch(2, vec![later]));
}

#[test]
fn recovery_drops_a_pending_request_already_executed_elsewhere() {
    let mut session = session(ProposalOutcome::NotLeader);
    let request = input("a", 0, Direction::Down);
    session.receive_input(request.clone());
    session.on_deadline().unwrap();
    session.begin_recovery();
    session.on_decision(decision(0, 0, vec![request])).unwrap();
    session.reconcile_after_recovery(Some(Slot(0))).unwrap();
    assert!(session.pending().is_none());
    session.proposer.outcome = ProposalOutcome::Proposed(Slot(1));
    session.on_deadline().unwrap();
    assert_eq!(session.pending().unwrap(), &batch(1, vec![]));
}

#[test]
fn local_proposal_is_not_execution_and_cannot_pipeline() {
    let mut local = LocalProposer::default();
    assert_eq!(
        local.propose(&batch(0, vec![])).outcome,
        ProposalOutcome::Proposed(Slot(0))
    );
    assert_eq!(
        local.propose(&batch(1, vec![])).outcome,
        ProposalOutcome::Busy
    );
    local.on_executed(Slot(9));
    assert_eq!(
        local.propose(&batch(1, vec![])).outcome,
        ProposalOutcome::Busy
    );
    local.on_executed(Slot(0));
    assert_eq!(
        local.propose(&batch(1, vec![])).outcome,
        ProposalOutcome::Proposed(Slot(1))
    );
    let mut session = TickSession::new(LocalProposer::default(), replica());
    assert_eq!(session.on_deadline().unwrap().executed.len(), 1);
    assert_eq!(session.on_deadline().unwrap().executed.len(), 1);
    assert_eq!(session.replica().next_tick(), GameTick(2));
}

#[test]
fn acceptor_adopts_ballots_in_p1a_and_requires_an_exact_match_in_p2a() {
    let mut acceptor = AcceptorState::default();
    let low = Ballot {
        counter: 1,
        node_id: 1,
    };
    let high = Ballot {
        counter: 1,
        node_id: 2,
    };
    let higher = Ballot {
        counter: 2,
        node_id: 1,
    };
    assert!(low < high && high < higher);
    let pvalue = PValue {
        ballot: high,
        slot: Slot(0),
        command: Command::Noop,
    };

    // A P2a cannot establish a ballot, even on a fresh acceptor.
    assert_eq!(
        acceptor.on_p2a(pvalue.clone()),
        Err(AcceptorError::BallotMismatch { current: None })
    );
    assert!(acceptor.accepted(Slot(0)).is_none());
    assert_eq!(acceptor.on_p1a(high), Ok(AcceptedHistory::Complete(vec![])));
    assert_eq!(
        acceptor.on_p1a(low),
        Err(AcceptorError::BallotMismatch {
            current: Some(high)
        })
    );
    acceptor.on_p2a(pvalue.clone()).unwrap();
    acceptor.on_p2a(pvalue.clone()).unwrap(); // Idempotent retry.
    assert_eq!(
        acceptor.on_p1a(high),
        Ok(AcceptedHistory::Complete(vec![pvalue.clone()]))
    );

    let conflict = PValue {
        command: Command::Tick(batch(0, vec![])),
        ..pvalue.clone()
    };
    assert_eq!(
        acceptor.on_p2a(conflict),
        Err(AcceptorError::ConflictingCommand)
    );
    for ballot in [low, higher] {
        assert_eq!(
            acceptor.on_p2a(PValue {
                ballot,
                ..pvalue.clone()
            }),
            Err(AcceptorError::BallotMismatch {
                current: Some(high)
            })
        );
        assert_eq!(acceptor.ballot(), Some(high));
        assert_eq!(acceptor.accepted(Slot(0)), Some(&pvalue));
    }
    // Phase 1 under the new ballot still reports the old accepted p-value.
    assert_eq!(
        acceptor.on_p1a(higher),
        Ok(AcceptedHistory::Complete(vec![pvalue.clone()]))
    );
    let recovered = PValue {
        ballot: higher,
        ..pvalue
    };
    acceptor.on_p2a(recovered.clone()).unwrap();
    assert_eq!(acceptor.accepted(Slot(0)), Some(&recovered));
}

#[test]
fn p1b_reports_accepted_pvalues_for_all_slots_under_the_adopted_ballot() {
    let mut acceptor = AcceptorState::default();
    let ballot = Ballot {
        counter: 1,
        node_id: 1,
    };
    acceptor.on_p1a(ballot).unwrap();
    let first = PValue {
        ballot,
        slot: Slot(1),
        command: Command::Noop,
    };
    let second = PValue {
        ballot,
        slot: Slot(4),
        command: Command::Tick(batch(0, vec![])),
    };
    acceptor.on_p2a(second.clone()).unwrap();
    acceptor.on_p2a(first.clone()).unwrap();
    assert_eq!(
        acceptor.on_p1a(Ballot {
            counter: 2,
            node_id: 2
        }),
        Ok(AcceptedHistory::Complete(vec![first, second]))
    );
}

#[test]
fn replica_slot_num_waits_for_holes_and_never_decreases() {
    let mut replica = replica();
    let later = decision(1, 0, vec![]);
    assert_eq!(replica.slot_num(), Slot(0));
    assert!(replica.on_decision(later.clone()).unwrap().is_empty());
    assert_eq!(replica.slot_num(), Slot(0));
    let no_op = Decision {
        slot: Slot(0),
        command: Command::Noop,
    };
    assert_eq!(replica.on_decision(no_op.clone()).unwrap().len(), 2);
    assert_eq!(replica.slot_num(), Slot(2));
    assert_eq!(replica.next_tick(), GameTick(1));
    assert!(replica.on_decision(later).unwrap().is_empty());
    assert!(replica.on_decision(no_op).unwrap().is_empty());
    assert_eq!(replica.slot_num(), Slot(2));
}

fn lecture_messages() -> Vec<PaxosMessage> {
    let ballot = Ballot {
        counter: 1,
        node_id: 2,
    };
    let pvalue = PValue {
        ballot,
        slot: Slot(0),
        command: Command::Noop,
    };
    let decision = Decision {
        slot: Slot(0),
        command: Command::Noop,
    };
    vec![
        PaxosMessage::P1a {
            ballot,
            from_slot: Slot(0),
        },
        PaxosMessage::P1b {
            ballot,
            accepted: AcceptedHistory::Complete(vec![pvalue.clone()]),
        },
        PaxosMessage::P1b {
            ballot,
            accepted: AcceptedHistory::Page {
                entries: vec![pvalue.clone()],
                from_slot: Slot(0),
                next_slot: Some(Slot(1)),
            },
        },
        PaxosMessage::P2a(pvalue.clone()),
        PaxosMessage::P2b(pvalue),
        PaxosMessage::Preempted { ballot },
        PaxosMessage::Decision(decision.clone()),
        PaxosMessage::Heartbeat {
            ballot,
            executed_through: None,
        },
        PaxosMessage::HeartbeatReply {
            ballot,
            executed_through: Some(Slot(0)),
        },
        PaxosMessage::CatchUp { from_slot: Slot(0) },
        PaxosMessage::CatchUpReply {
            decisions: vec![decision],
            complete: true,
        },
    ]
}

#[test]
fn peer_messages_use_the_lecture_names_on_the_wire() {
    let messages = lecture_messages();
    for message in &messages {
        let json = serde_json::to_string(message).unwrap();
        assert_eq!(
            serde_json::from_str::<PaxosMessage>(&json).unwrap(),
            *message
        );
    }
    let p2a = serde_json::to_value(&messages[3]).unwrap();
    assert_eq!(p2a["P2a"]["slot"], 0);
    assert_eq!(p2a["P2a"]["command"], "Noop");
    assert_eq!(p2a["P2a"]["ballot"]["node_id"], 2);
    assert!(p2a["P2a"].get("value").is_none());
}

#[test]
fn distributed_handlers_are_still_placeholders_and_never_decide() {
    let mut paxos = PaxosServer::new(1, vec![1, 2, 3]);
    for message in lecture_messages() {
        assert!(matches!(
            paxos.on_message(2, message),
            Err(ProtocolError::Unsupported(_))
        ));
    }
    for timer in [
        TimerEvent::Heartbeat,
        TimerEvent::HeartbeatCheck,
        TimerEvent::P1aRetry,
        TimerEvent::P2aRetry,
    ] {
        assert!(matches!(
            paxos.on_timer(timer),
            Err(ProtocolError::Unsupported(_))
        ));
    }
    let ballot = Ballot {
        counter: 1,
        node_id: 1,
    };
    let plan = RecoveryPlan {
        ballot,
        proposals: vec![(Slot(0), Command::Noop)],
        through: Some(Slot(0)),
    };
    assert!(matches!(
        paxos.begin_recovery(&plan),
        Err(ProtocolError::Unsupported(_))
    ));
    assert!(matches!(
        paxos.on_recovery_executed(plan.through),
        Err(ProtocolError::Unsupported(_))
    ));
    assert!(matches!(
        paxos.activate(ballot),
        Err(ProtocolError::Unsupported(_))
    ));
    assert_eq!(paxos.leader_state(), LeaderState::Follower);
    assert_eq!(paxos.acceptor().ballot(), None);
    assert!(paxos.scout().is_none());
    assert!(paxos.commander().is_none());
    let result = paxos.propose(&batch(0, vec![]));
    assert_eq!(result.outcome, ProposalOutcome::NotLeader);
    assert!(result.effects.is_empty());
    let result = paxos.repropose(Slot(0), &Command::Noop);
    assert_eq!(result.outcome, ProposalOutcome::Unsupported);
    assert!(result.effects.is_empty());
}
