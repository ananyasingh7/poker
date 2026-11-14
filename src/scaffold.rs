// ABOUTME: Milestone 9 scaffold treatments: deterministic decision aids computed from one seat's
// ABOUTME: PlayerView only (features, then equity, then a rule-based suggestion) for the model prompt.
use crate::{
    cards::{Card, evaluate},
    engine::{Action, Event, PlayerView, Street},
    equity::equity_against,
    opponents::Style,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Monte Carlo run-outs for the equity aid. Deterministic: seeded from the visible cards.
pub const EQUITY_SAMPLES: u32 = 300;

/// Cumulative treatments: each level includes everything below it, so adjacent levels differ
/// by exactly one added aid and paired differences isolate that aid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scaffold {
    #[default]
    None,
    /// Descriptive facts the model could compute itself: position, pot odds, stack depth,
    /// hand notation, made hand and draws.
    Features,
    /// Features plus Monte Carlo showdown equity against random hands.
    Equity,
    /// Equity plus the action a fixed equity-versus-pot-odds rule would take.
    Advice,
}
impl Scaffold {
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Features => "features",
            Self::Equity => "equity",
            Self::Advice => "advice",
        }
    }
    /// The system-prompt sentence that accompanies the aids, empty for `None`.
    pub fn instruction(self) -> &'static str {
        match self {
            Self::None => "",
            _ => {
                " The observation includes decision_aids computed by the host from your own observation only (no hidden information). They are approximate and may be wrong; the decision is yours."
            }
        }
    }
}

const NOTATION: &[u8; 13] = b"23456789TJQKA";
const CATEGORIES: [&str; 9] = [
    "high_card",
    "pair",
    "two_pair",
    "three_of_a_kind",
    "straight",
    "flush",
    "full_house",
    "four_of_a_kind",
    "straight_flush",
];

/// Conventional seat name from the button offset. Heads-up the button posts the small blind.
pub fn position(seats: usize, seat: usize, button: usize) -> &'static str {
    let offset = (seat + seats - button) % seats;
    if seats == 2 {
        return if offset == 0 {
            "button_small_blind"
        } else {
            "big_blind"
        };
    }
    match (offset, seats - offset) {
        (0, _) => "button",
        (1, _) => "small_blind",
        (2, _) => "big_blind",
        (3, _) => "under_the_gun",
        (_, 1) => "cutoff",
        _ => "hijack",
    }
}

/// "AKs", "T9o", "77": higher rank first.
pub fn notation(hole: [Card; 2]) -> String {
    let (hi, lo) = if hole[0].rank() >= hole[1].rank() {
        (hole[0], hole[1])
    } else {
        (hole[1], hole[0])
    };
    let mut text: String = [hi, lo]
        .iter()
        .map(|c| NOTATION[(c.rank() - 2) as usize] as char)
        .collect();
    if hi.rank() != lo.rank() {
        text.push(if hi.suit() == lo.suit() { 's' } else { 'o' });
    }
    text
}

fn has_straight(ranks: u16) -> bool {
    // Bit r set for rank r (2..=14); the ace also plays low.
    let ranks = ranks | ((ranks >> 14) & 1) << 1;
    (1..=10).any(|low| (ranks >> low) & 0b11111 == 0b11111)
}

/// Draw information on the flop and turn: a four-card flush using a hole card, and the number
/// of ranks that would complete a straight that uses a hole card (2 = open-ended, 1 = gutshot).
fn draws(hole: [Card; 2], board: &[Card]) -> Value {
    let all: Vec<Card> = hole.iter().chain(board).copied().collect();
    let flush_draw = (0..4).any(|suit| {
        all.iter().filter(|c| c.suit() == suit).count() == 4
            && hole.iter().any(|c| c.suit() == suit)
    });
    let mask = |cards: &[Card]| cards.iter().fold(0u16, |m, c| m | 1 << c.rank());
    let (mine, table) = (mask(&all), mask(board));
    let straight_ranks = if has_straight(mine) {
        0
    } else {
        (2..=14)
            .filter(|r| has_straight(mine | 1 << r) && !has_straight(table | 1 << r))
            .count()
    };
    json!({"flush_draw":flush_draw,"straight_completing_ranks":straight_ranks})
}

fn big_blind(view: &PlayerView) -> u64 {
    view.history
        .iter()
        .filter_map(|e| match e {
            Event::Blind { amount, .. } => Some(*amount),
            _ => None,
        })
        .max()
        .unwrap_or(1)
        .max(1)
}

/// Seats acting after this one on later streets get to see this seat act first.
fn in_position(view: &PlayerView) -> bool {
    let n = view.players.len();
    let order = |seat: usize| match (seat + n - view.button) % n {
        0 => n,
        offset => offset,
    };
    view.players
        .iter()
        .filter(|p| !p.folded && p.seat != view.seat)
        .all(|p| order(p.seat) < order(view.seat))
}

fn round(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}

/// Descriptive features available to every scaffold level above `None`.
pub fn features(view: &PlayerView) -> Value {
    let bb = big_blind(view);
    let own = &view.players[view.seat];
    let to_call = view.legal.as_ref().and_then(|l| l.call).unwrap_or(0);
    let opponents: Vec<_> = view
        .players
        .iter()
        .filter(|p| !p.folded && p.seat != view.seat)
        .collect();
    let effective = opponents
        .iter()
        .map(|p| p.stack + p.street_committed)
        .max()
        .unwrap_or(0)
        .min(own.stack + own.street_committed);
    let street_start = view
        .history
        .iter()
        .rposition(|e| matches!(e, Event::Board { .. }))
        .map_or(0, |i| i + 1);
    let mut raises = 0;
    let mut aggressor = None;
    for event in &view.history[street_start..] {
        if let Event::Act {
            seat,
            action: Action::RaiseTo(_),
        } = event
        {
            raises += 1;
            aggressor = Some(*seat);
        }
    }
    let mut out = json!({
        "position": position(view.players.len(), view.seat, view.button),
        "in_position_postflop": in_position(view),
        "big_blind": bb,
        "opponents_in_hand": opponents.len(),
        "to_call": to_call,
        "pot_before_call": view.pot,
        "pot_odds_required_equity": round(to_call as f64 / (view.pot + to_call).max(1) as f64, 3),
        "stack_bb": round(own.stack as f64 / bb as f64, 1),
        "effective_stack_bb": round(effective as f64 / bb as f64, 1),
        "stack_to_pot_ratio": round(effective as f64 / view.pot.max(1) as f64, 2),
        "raises_this_street": raises,
        "last_raiser_this_street": aggressor,
        "hand": notation(view.hole),
    });
    if view.street != Street::Preflop && view.board.len() >= 3 {
        let mut cards = view.hole.to_vec();
        cards.extend(&view.board);
        let made = evaluate(&cards).expect("valid visible cards");
        out["made_hand"] = json!(CATEGORIES[made.0[0] as usize]);
        if view.board.len() == 5 {
            let board = evaluate(&view.board).expect("valid board");
            out["board_alone_makes"] = json!(CATEGORIES[board.0[0] as usize]);
            out["hole_cards_improve_board"] = json!(made > board);
        } else {
            out["draws"] = draws(view.hole, &view.board);
        }
    }
    out
}

/// The decision aids for one decision, or `None` when the scaffold is off or the seat is not
/// acting. Uses only `view`, so it carries no information the seat was not already given.
pub fn aids(scaffold: Scaffold, view: &PlayerView) -> Option<Value> {
    if scaffold == Scaffold::None || view.legal.is_none() {
        return None;
    }
    let mut out = features(view);
    if scaffold >= Scaffold::Equity {
        let opponents = out["opponents_in_hand"].as_u64().unwrap_or(1).max(1) as usize;
        let equity = equity_against(view.hole, &view.board, opponents, EQUITY_SAMPLES);
        out["equity_vs_random_hands"] = json!(round(equity, 3));
        out["equity_note"] = json!(
            "Share of showdowns won against the remaining opponents holding uniformly random cards. Opponents who bet or raise usually hold stronger hands than random."
        );
    }
    if scaffold == Scaffold::Advice {
        out["suggested_action"] = suggestion(view).map(reply).unwrap_or(Value::Null);
        out["suggestion_source"] = json!(
            "A fixed rule: preflop hand-strength thresholds, postflop equity against random hands versus pot odds. It ignores who the opponents are."
        );
    }
    Some(out)
}

/// The reference rule's choice: the default `Solid` scripted policy, deterministic per view.
pub fn suggestion(view: &PlayerView) -> Option<Action> {
    Style::Solid.agent().decide(view).ok()
}

/// An action in the model's reply schema.
pub fn reply(action: Action) -> Value {
    match action {
        Action::Fold => json!({"action":"fold","amount":null}),
        Action::Check => json!({"action":"check","amount":null}),
        Action::Call => json!({"action":"call","amount":null}),
        Action::RaiseTo(n) => json!({"action":"raise_to","amount":n}),
    }
}

/// Coarse action kind for agreement statistics: raise sizes are not compared.
pub fn kind(action: Action) -> &'static str {
    match action {
        Action::Fold => "fold",
        Action::Check | Action::Call => "passive",
        Action::RaiseTo(_) => "raise",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Hand, Rules};

    fn card(text: &str) -> Card {
        let b = text.as_bytes();
        let rank = NOTATION.iter().position(|c| *c == b[0]).unwrap() as u8;
        let suit = b"cdhs".iter().position(|c| *c == b[1]).unwrap() as u8;
        Card::try_from(suit * 13 + rank).unwrap()
    }

    #[test]
    fn positions_follow_the_blinds() {
        for seats in 2..=6 {
            for button in 0..seats {
                let hand = Hand::new(Rules::default(), vec![200; seats], button, 9).unwrap();
                let view = hand.view(0).unwrap();
                let blinds: Vec<usize> = view
                    .history
                    .iter()
                    .filter_map(|e| match e {
                        Event::Blind { seat, .. } => Some(*seat),
                        _ => None,
                    })
                    .collect();
                assert!(position(seats, blinds[0], button).contains("small_blind"));
                assert_eq!(position(seats, blinds[1], button), "big_blind");
            }
        }
        let six: Vec<_> = (0..6).map(|s| position(6, s, 0)).collect();
        assert_eq!(
            six,
            [
                "button",
                "small_blind",
                "big_blind",
                "under_the_gun",
                "hijack",
                "cutoff"
            ]
        );
    }

    #[test]
    fn notation_and_draws() {
        assert_eq!(notation([card("Kd"), card("As")]), "AKo");
        assert_eq!(notation([card("9h"), card("Th")]), "T9s");
        assert_eq!(notation([card("7c"), card("7d")]), "77");
        let flop = [card("8h"), card("7c"), card("2h")];
        let d = draws([card("9h"), card("Th")], &flop);
        assert_eq!(d["flush_draw"], true);
        assert_eq!(d["straight_completing_ranks"], 2); // J or 6
        let d = draws([card("9c"), card("Js")], &flop);
        assert_eq!(d["flush_draw"], false);
        assert_eq!(d["straight_completing_ranks"], 1); // T only
        // A straight on the board alone is not the player's draw.
        let board = [card("5c"), card("6d"), card("7h"), card("8s")];
        let d = draws([card("Ac"), card("Kd")], &board);
        assert_eq!(d["straight_completing_ranks"], 0);
        assert!(has_straight(1 << 14 | 0b111100)); // A-5 wheel
    }

    #[test]
    fn aids_are_cumulative_and_deterministic() {
        let hand = Hand::new(Rules::default(), vec![200; 6], 0, 4).unwrap();
        let actor = (0..6)
            .find(|&s| hand.view(s).unwrap().legal.is_some())
            .unwrap();
        let view = hand.view(actor).unwrap();
        assert!(aids(Scaffold::None, &view).is_none());
        let other = (actor + 1) % 6;
        assert!(aids(Scaffold::Advice, &hand.view(other).unwrap()).is_none());
        let f = aids(Scaffold::Features, &view).unwrap();
        assert_eq!(f["position"], "under_the_gun");
        assert_eq!(f["opponents_in_hand"], 5);
        assert_eq!(f["to_call"], 2);
        assert!(f.get("equity_vs_random_hands").is_none());
        let e = aids(Scaffold::Equity, &view).unwrap();
        assert!(e["equity_vs_random_hands"].as_f64().unwrap() > 0.0);
        assert!(e.get("suggested_action").is_none());
        let a = aids(Scaffold::Advice, &view).unwrap();
        assert_eq!(a, aids(Scaffold::Advice, &view).unwrap());
        assert_eq!(a["suggested_action"], reply(suggestion(&view).unwrap()));
        for key in f.as_object().unwrap().keys() {
            assert_eq!(a[key], f[key], "{key}");
        }
    }
}
