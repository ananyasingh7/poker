use serde::{Deserialize, Serialize};

/// 0..51: rank = value % 13 + 2, suit = value / 13 (clubs, diamonds, hearts, spades).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Card(u8);
impl TryFrom<u8> for Card {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, String> {
        if v < 52 {
            Ok(Self(v))
        } else {
            Err("card must be in 0..52".into())
        }
    }
}
impl From<Card> for u8 {
    fn from(c: Card) -> u8 {
        c.0
    }
}
/// Rank then suit, for example `Ah` or `Tc`.
impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            b"23456789TJQKA"[(self.rank() - 2) as usize] as char,
            b"cdhs"[self.suit() as usize] as char
        )
    }
}
impl Card {
    pub fn rank(self) -> u8 {
        self.0 % 13 + 2
    }
    pub fn suit(self) -> u8 {
        self.0 / 13
    }
}

/// Versioned, portable SplitMix64 generator. Not suitable for real-money gambling.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);
impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        let threshold = n.wrapping_neg() % n;
        loop {
            let v = self.next_u64();
            if v >= threshold {
                return v % n;
            }
        }
    }
}
pub(crate) fn deck(seed: u64) -> Vec<Card> {
    let mut cards: Vec<_> = (0..52).map(Card).collect();
    let mut rng = Rng(seed);
    for i in (1..52).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        cards.swap(i, j);
    }
    cards
}

/// Lexicographically ordered category and kickers; larger is stronger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Rank(pub [u8; 6]);
fn five(cards: [Card; 5]) -> Rank {
    let mut counts = [0u8; 15];
    for c in cards {
        counts[c.rank() as usize] += 1;
    }
    let flush = cards.iter().all(|c| c.suit() == cards[0].suit());
    let mut unique: Vec<u8> = (2..=14).rev().filter(|&r| counts[r as usize] > 0).collect();
    let straight = if unique.len() == 5 {
        if unique[0] - unique[4] == 4 {
            unique[0]
        } else if unique == [14, 5, 4, 3, 2] {
            5
        } else {
            0
        }
    } else {
        0
    };
    let mut groups: Vec<_> = unique.iter().map(|&r| (counts[r as usize], r)).collect();
    groups.sort_unstable_by(|a, b| b.cmp(a));
    let (category, kickers) = if flush && straight > 0 {
        (8, vec![straight])
    } else if groups[0].0 == 4 {
        (7, groups.iter().map(|g| g.1).collect())
    } else if groups[0].0 == 3 && groups[1].0 == 2 {
        (6, groups.iter().map(|g| g.1).collect())
    } else if flush {
        (5, unique.clone())
    } else if straight > 0 {
        (4, vec![straight])
    } else if groups[0].0 == 3 {
        (3, groups.iter().map(|g| g.1).collect())
    } else if groups[0].0 == 2 && groups[1].0 == 2 {
        (2, groups.iter().map(|g| g.1).collect())
    } else if groups[0].0 == 2 {
        (1, groups.iter().map(|g| g.1).collect())
    } else {
        (0, std::mem::take(&mut unique))
    };
    let mut result = [0; 6];
    result[0] = category;
    result[1..1 + kickers.len()].copy_from_slice(&kickers);
    Rank(result)
}
pub fn evaluate(cards: &[Card]) -> Result<Rank, String> {
    if !(5..=7).contains(&cards.len()) {
        return Err("ranking requires 5 to 7 cards".into());
    }
    let mut seen = 0u64;
    for c in cards {
        let bit = 1u64 << c.0;
        if seen & bit != 0 {
            return Err("duplicate card".into());
        }
        seen |= bit;
    }
    let mut best = Rank([0; 6]);
    for a in 0..cards.len() - 4 {
        for b in a + 1..cards.len() - 3 {
            for c in b + 1..cards.len() - 2 {
                for d in c + 1..cards.len() - 1 {
                    for e in d + 1..cards.len() {
                        best = best.max(five([cards[a], cards[b], cards[c], cards[d], cards[e]]));
                    }
                }
            }
        }
    }
    Ok(best)
}
