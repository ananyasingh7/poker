use poker_lab::{
    arena::*,
    cards::{Card, Rng, evaluate},
    engine::*,
};
fn passive(h: &mut Hand) {
    while let Some(i) = h.actor() {
        let l = h.legal();
        h.act(i, if l.check { Action::Check } else { Action::Call })
            .unwrap();
        assert!(h.conserved());
    }
}
#[test]
fn heads_up_order_and_atomic_rejection() {
    let mut h = Hand::new(Rules::default(), vec![200; 2], 0, 7).unwrap();
    assert_eq!(h.actor(), Some(0));
    assert_eq!(h.legal().call, Some(1));
    assert_eq!(h.legal().min_raise_to, Some(4));
    let before = h.view(0).unwrap();
    assert!(h.act(1, Action::Call).is_err());
    assert!(h.act(0, Action::Check).is_err());
    assert!(h.act(0, Action::RaiseTo(3)).is_err());
    assert_eq!(before, h.view(0).unwrap());
    h.act(0, Action::Call).unwrap();
    assert_eq!(h.actor(), Some(1));
    h.act(1, Action::Check).unwrap();
    assert_eq!(h.view(0).unwrap().street, Street::Flop);
    assert_eq!(h.actor(), Some(1));
    passive(&mut h);
    assert!(h.is_complete());
}
#[test]
fn folds_and_information_boundary() {
    let mut h = Hand::new(Rules::default(), vec![200; 2], 0, 9).unwrap();
    let view = serde_json::to_value(h.view(0).unwrap()).unwrap();
    assert!(view.get("seed").is_none());
    assert!(view.get("deck").is_none());
    for player in view["players"].as_array().unwrap() {
        assert!(player.get("hole").is_none());
    }
    h.act(0, Action::Fold).unwrap();
    assert_eq!(h.stacks(), vec![199, 201]);
    for i in 0..2 {
        let v = h.view(i).unwrap();
        assert!(v.history.iter().all(|e| !matches!(e, Event::Reveal { .. })));
        assert!(v.legal.is_none());
    }
}
#[test]
fn short_raise_does_not_reopen_but_cumulative_raises_do() {
    let mut h = Hand::new(Rules::default(), vec![200, 15, 200], 0, 1).unwrap();
    h.act(0, Action::RaiseTo(10)).unwrap();
    h.act(1, Action::RaiseTo(15)).unwrap();
    h.act(2, Action::Call).unwrap();
    assert_eq!(h.actor(), Some(0));
    assert_eq!(h.legal().min_raise_to, None);
    assert_eq!(h.legal().call, Some(5));
    passive(&mut h);
    let mut h = Hand::new(Rules::default(), vec![200, 15, 20, 200], 0, 1).unwrap();
    h.act(3, Action::RaiseTo(10)).unwrap();
    h.act(0, Action::Call).unwrap();
    h.act(1, Action::RaiseTo(15)).unwrap();
    h.act(2, Action::RaiseTo(20)).unwrap();
    assert_eq!(h.actor(), Some(3));
    assert_eq!(h.legal().min_raise_to, Some(28));
    passive(&mut h);
}
#[test]
fn multiple_all_ins_and_refund() {
    let mut h = Hand::new(Rules::default(), vec![20, 50, 100], 0, 123).unwrap();
    h.act(0, Action::RaiseTo(20)).unwrap();
    h.act(1, Action::RaiseTo(50)).unwrap();
    h.act(2, Action::Call).unwrap();
    assert!(h.is_complete());
    assert!(h.conserved());
    let pots: Vec<_> = h
        .events()
        .iter()
        .filter_map(|e| {
            if let Event::Award {
                amount, eligible, ..
            } = e
            {
                Some((*amount, eligible.clone()))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(pots, vec![(60, vec![0, 1, 2]), (60, vec![1, 2])]);
    let mut h = Hand::new(Rules::default(), vec![200, 20], 0, 2).unwrap();
    h.act(0, Action::RaiseTo(200)).unwrap();
    h.act(1, Action::Call).unwrap();
    assert!(h.events().contains(&Event::Refund {
        seat: 0,
        amount: 180
    }));
    assert!(h.conserved());
}
#[test]
fn seeded_sessions_and_replay() {
    for seats in 2..=6 {
        let mut session = Session::new(seats, 42).unwrap();
        let mut agents: Vec<Box<dyn Agent>> = (0..seats)
            .map(|i| Box::new(Scripted::Random(Rng(i as u64))) as Box<dyn Agent>)
            .collect();
        for index in 0..500 {
            let played = session.next_hand(&mut agents).unwrap();
            assert_eq!(played.record.button, index % seats);
            replay(&played.record).unwrap();
            assert_eq!(
                session.stacks.iter().sum::<u64>(),
                200 * seats as u64 + session.rebuys.iter().sum::<u64>()
            );
            for (seat, views) in played.private_views.iter().enumerate() {
                for view in views {
                    assert_eq!(view.seat, seat);
                    for e in &view.history {
                        if let Event::Reveal { seat: revealed, .. } = e {
                            assert!(!view.players[*revealed].folded);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn reproducible_and_fallback() {
    struct Bad;
    impl Agent for Bad {
        fn decide(&mut self, _: &PlayerView) -> Result<Action, String> {
            Ok(Action::RaiseTo(u64::MAX))
        }
    }
    let run = || {
        play(
            Rules::default(),
            vec![200; 2],
            0,
            81,
            &mut [Box::new(Bad), Box::new(Scripted::Caller)],
        )
        .unwrap()
    };
    let a = run();
    let b = run();
    assert_eq!(a.record, b.record);
    assert!(a.record.decisions[0].error.is_some());
    replay(&a.record).unwrap();
    let mut tampered = a.record;
    tampered.final_stacks[0] += 1;
    assert!(replay(&tampered).is_err());
}
fn cards(s: &str) -> Vec<Card> {
    s.split_whitespace()
        .map(|s| {
            let b = s.as_bytes();
            let r = "23456789TJQKA".find(b[0] as char).unwrap();
            let suit = "cdhs".find(b[1] as char).unwrap();
            Card::try_from((suit * 13 + r) as u8).unwrap()
        })
        .collect()
}
#[test]
fn rank_categories_wheel_and_best_five() {
    let hands = [
        "Ac Jd 9h 7s 3c",
        "Ac Ad 9h 7s 3c",
        "Ac Ad 9h 9s 3c",
        "Ac Ad Ah 7s 3c",
        "Ac 2d 3h 4s 5c",
        "Ac Jc 9c 7c 3c",
        "Ac Ad Ah 7s 7c",
        "Ac Ad Ah As 3c",
        "9c Tc Jc Qc Kc",
    ];
    for (category, hand) in hands.iter().enumerate() {
        assert_eq!(evaluate(&cards(hand)).unwrap().0[0], category as u8);
    }
    assert!(evaluate(&cards("2c 3d 4h 5s 6c")).unwrap() > evaluate(&cards(hands[4])).unwrap());
    assert_eq!(
        evaluate(&cards("Ac Ad Ah Ks Kc Kd 2c")).unwrap().0,
        [6, 14, 13, 0, 0, 0]
    );
    assert_eq!(
        evaluate(&cards("Ac Kc Qc Jc Tc 2d 3h")).unwrap(),
        evaluate(&cards("Ac Kc Qc Jc Tc 4d 5h")).unwrap()
    );
    assert!(evaluate(&cards("Ac Ac Qc Jc Tc")).is_err());
    assert!(serde_json::from_str::<Card>("52").is_err());
}
/// Independent combinatorial census; run explicitly in release mode.
#[test]
#[ignore = "enumerates all 2,598,960 five-card hands"]
fn exhaustive_five_card_distribution() {
    let deck: Vec<_> = (0..52).map(|i| Card::try_from(i).unwrap()).collect();
    let mut counts = [0u64; 9];
    for a in 0..48 {
        for b in a + 1..49 {
            for c in b + 1..50 {
                for d in c + 1..51 {
                    for e in d + 1..52 {
                        counts[evaluate(&[deck[a], deck[b], deck[c], deck[d], deck[e]])
                            .unwrap()
                            .0[0] as usize] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        counts,
        [
            1_302_540, 1_098_240, 123_552, 54_912, 10_200, 5_108, 3_744, 624, 40
        ]
    );
}

#[test]
fn short_blind_runs_out_without_phantom_call() {
    let h = Hand::new(Rules::default(), vec![200, 1], 0, 1).unwrap();
    assert!(h.is_complete());
    assert!(h.conserved());
    assert!(!h.events().iter().any(|e| matches!(e, Event::Act { .. })));
}
#[test]
fn random_unequal_stacks_and_buttons() {
    let mut rng = Rng(918);
    for _ in 0..2000 {
        let n = 2 + rng.below(5) as usize;
        let stacks = (0..n).map(|_| 1 + rng.below(200)).collect();
        let mut h = Hand::new(
            Rules::default(),
            stacks,
            rng.below(n as u64) as usize,
            rng.next_u64(),
        )
        .unwrap();
        let mut agent = Scripted::Random(Rng(rng.next_u64()));
        let mut steps = 0;
        while let Some(i) = h.actor() {
            let action = agent.decide(&h.view(i).unwrap()).unwrap();
            h.act(i, action).unwrap();
            assert!(h.conserved());
            steps += 1;
            assert!(steps < 10000);
        }
        assert!(h.is_complete());
    }
}
