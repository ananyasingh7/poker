// ABOUTME: Deterministic Monte Carlo showdown equity against one random hand, for scripted policies.
// ABOUTME: Seeded from the visible cards, so the same view always gives the same estimate.
use crate::cards::{Card, Rng, evaluate};

/// Probability that `hole` wins a showdown against one uniformly random hand (ties count half),
/// with the remaining board cards dealt at random. `samples` run-outs; deterministic.
pub fn equity(hole: [Card; 2], board: &[Card], samples: u32) -> f64 {
    equity_against(hole, board, 1, samples)
}

/// Showdown share against `opponents` uniformly random hands: a win scores 1, a k-way tie
/// for best scores 1/k. With one opponent this is exactly `equity` (same draws, same result).
pub fn equity_against(hole: [Card; 2], board: &[Card], opponents: usize, samples: u32) -> f64 {
    let opponents = opponents.clamp(1, 5);
    let known: Vec<u8> = hole.iter().chain(board).map(|c| u8::from(*c)).collect();
    let mut rest: Vec<Card> = (0..52u8)
        .filter(|c| !known.contains(c))
        .map(|c| Card::try_from(c).expect("valid card"))
        .collect();
    let seed = known.iter().fold(0x51ed_2701u64, |h, c| {
        h.wrapping_mul(131).wrapping_add(u64::from(*c))
    });
    let mut rng = Rng(seed);
    let missing = 5 - board.len();
    let dealt = 2 * opponents;
    let mut score = 0.0;
    for _ in 0..samples {
        // Partial Fisher–Yates: the first `dealt + missing` slots become this sample's draw.
        for i in 0..dealt + missing {
            let j = i + rng.below((rest.len() - i) as u64) as usize;
            rest.swap(i, j);
        }
        let mut full = board.to_vec();
        full.extend_from_slice(&rest[dealt..dealt + missing]);
        let mut mine = full.clone();
        mine.extend(hole);
        let mine = evaluate(&mine);
        let mut best_other = None;
        let mut tied = 0;
        for o in 0..opponents {
            let mut theirs = full.clone();
            theirs.extend_from_slice(&rest[2 * o..2 * o + 2]);
            let theirs = evaluate(&theirs);
            if theirs == mine {
                tied += 1;
            }
            best_other = best_other.max(Some(theirs));
        }
        score += match best_other.expect("at least one opponent").cmp(&mine) {
            std::cmp::Ordering::Less => 1.0,
            std::cmp::Ordering::Equal => 1.0 / f64::from(tied + 1),
            std::cmp::Ordering::Greater => 0.0,
        };
    }
    score / f64::from(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cards(values: &[u8]) -> Vec<Card> {
        values.iter().map(|v| Card::try_from(*v).unwrap()).collect()
    }
    #[test]
    fn equity_is_deterministic_and_ordered() {
        let aces = [cards(&[12])[0], cards(&[25])[0]]; // Ac Ad
        let trash = [cards(&[0])[0], cards(&[18])[0]]; // 2c 7d
        let a = equity(aces, &[], 400);
        assert_eq!(a, equity(aces, &[], 400));
        assert!(a > 0.78 && a < 0.92, "{a}");
        let t = equity(trash, &[], 400);
        assert!(t > 0.25 && t < 0.42, "{t}");
        // A made royal flush on the river cannot lose.
        let royal = [cards(&[12])[0], cards(&[11])[0]];
        assert_eq!(equity(royal, &cards(&[10, 9, 8, 14, 30]), 50), 1.0);
    }
    #[test]
    fn more_opponents_lower_equity() {
        let aces = [cards(&[12])[0], cards(&[25])[0]];
        let one = equity_against(aces, &[], 1, 400);
        assert_eq!(one, equity(aces, &[], 400));
        let five = equity_against(aces, &[], 5, 400);
        assert!(five < one && five > 0.40 && five < 0.62, "{five}");
        let royal = [cards(&[12])[0], cards(&[11])[0]];
        assert_eq!(
            equity_against(royal, &cards(&[10, 9, 8, 14, 30]), 3, 50),
            1.0
        );
    }
}
