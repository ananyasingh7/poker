use crate::{
    arena::{Record, replay},
    cards::Card,
    engine::{Action, Event, Hand},
};
use std::{
    fmt::Write,
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};
fn cards(cs: &[Card]) -> String {
    cs.iter().map(Card::to_string).collect::<Vec<_>>().join(" ")
}
pub fn hand(r: &Record, number: usize, seat: Option<usize>) -> Result<String, String> {
    replay(r)?;
    let mut out = String::new();
    writeln!(
        out,
        "HAND {number} | button {} | blinds {}/{}",
        r.button, r.rules.small_blind, r.rules.big_blind
    )
    .unwrap();
    writeln!(out, "Starting stacks: {:?}", r.initial_stacks).unwrap();
    let mut state = Hand::new(r.rules.clone(), r.initial_stacks.clone(), r.button, r.seed)?;
    if let Some(i) = seat {
        let h = Hand::new(r.rules.clone(), r.initial_stacks.clone(), r.button, r.seed)?;
        writeln!(out, "Seat {i} private cards: {}", cards(&h.view(i)?.hole)).unwrap();
    }
    for e in &r.events {
        match e {
            Event::Blind { seat, amount } => writeln!(out, "  Seat {seat} posts {amount}"),
            Event::Act { seat, action } => {
                let detail = match action {
                    Action::Fold => "folds".into(),
                    Action::Check => "checks".into(),
                    Action::Call => format!("calls {}", state.legal().call.unwrap()),
                    Action::RaiseTo(n) => format!(
                        "raises to {n} (adds {})",
                        n - state.view(*seat)?.players[*seat].street_committed
                    ),
                };
                state.act(*seat, *action)?;
                writeln!(out, "  Seat {seat}: {detail}")
            }
            Event::Board { street, cards: c } => writeln!(out, "{street:?}: {}", cards(c)),
            Event::Reveal { seat, cards: c } => writeln!(out, "  Seat {seat} reveals {}", cards(c)),
            Event::Refund { seat, amount } => writeln!(out, "  Seat {seat} refunded {amount}"),
            Event::Award {
                amount,
                eligible,
                winners,
                shares,
            } => writeln!(
                out,
                "  Pot {amount}, eligible {eligible:?}: seats {winners:?} receive {shares:?}"
            ),
        }
        .unwrap();
    }
    for d in &r.decisions {
        if let Some(e) = &d.error {
            writeln!(
                out,
                "  FALLBACK seat {}: {e}; applied {:?}",
                d.seat, d.applied
            )
            .unwrap();
        }
    }
    let net: Vec<_> = r
        .final_stacks
        .iter()
        .zip(&r.initial_stacks)
        .map(|(a, b)| *a as i128 - *b as i128)
        .collect();
    writeln!(
        out,
        "Final stacks: {:?} | hand net: {net:?}\n",
        r.final_stacks
    )
    .unwrap();
    Ok(out)
}
pub fn directory(dir: &Path, seat: Option<usize>) -> Result<String, Box<dyn std::error::Error>> {
    let mut out = String::new();
    for (i, line) in BufReader::new(File::open(dir.join("evaluator.jsonl"))?)
        .lines()
        .enumerate()
    {
        let record: Record = serde_json::from_str(&line?)?;
        out.push_str(&hand(&record, i + 1, seat)?);
    }
    Ok(out)
}
