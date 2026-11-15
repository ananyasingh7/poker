// ABOUTME: Draws one picture of the table: the oval felt, six seats with name plates, cards, bets,
// ABOUTME: the dealer button, the board and the pot. The watched seat is always at the bottom.
use crate::{
    canvas::{Canvas, Color, Font, Rect},
    frames::{Frame, Label},
    sprites, theme,
};
use poker_lab::{cards::Card, engine::Chips};

/// Width of the table area; the side panel starts here.
pub const TABLE_W: i32 = 470;
const CX: i32 = 235;
const CY: i32 = 196;
const RX: i32 = 170;
const RY: i32 = 108;

pub struct Scene<'a> {
    pub frame: &'a Frame,
    pub names: &'a [String],
    /// Hole cards by seat; empty before the first deal.
    pub holes: &'a [[Card; 2]],
    pub hero: usize,
    pub button: usize,
    /// Show every seat's hole cards (the spectator's choice).
    pub reveal: bool,
    /// Milliseconds since start, for blinking.
    pub ms: u64,
}

pub fn card_text(card: Card) -> String {
    format!(
        "{}{}",
        b"23456789TJQKA"[(card.rank() - 2) as usize] as char,
        b"cdhs"[card.suit() as usize] as char
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardSize {
    Small,
    Board,
    Large,
}
impl CardSize {
    pub fn dims(self) -> (i32, i32) {
        match self {
            Self::Small => (20, 28),
            Self::Board => (24, 32),
            Self::Large => (26, 36),
        }
    }
}

/// A card with its top-left corner at (x, y); `None` draws the back.
pub fn draw_card(c: &mut Canvas, x: i32, y: i32, card: Option<Card>, size: CardSize, dim: bool) {
    let (w, h) = size.dims();
    let r = Rect::new(x, y, w, h);
    let Some(card) = card else {
        c.plate(r, theme::BACK, theme::CARD_EDGE);
        c.outline(r.inset(2), theme::BACK_DARK);
        c.dither(r.inset(4), theme::BACK_DARK);
        return;
    };
    c.plate(
        r,
        if dim { theme::DIM } else { theme::CARD },
        theme::CARD_EDGE,
    );
    let ink = if matches!(card.suit(), 1 | 2) {
        theme::RED
    } else {
        theme::CARD_INK
    };
    let rank = (b"23456789TJQKA"[(card.rank() - 2) as usize] as char).to_string();
    let suit = sprites::SUITS[card.suit() as usize];
    let pip = |ch| (ch == '#').then_some(ink);
    match size {
        CardSize::Small => {
            c.text(x + 3, y + 2, &rank, Font::Bold, ink);
            c.sprite(x + 3, y + 17, suit, 1, pip);
        }
        CardSize::Board => {
            c.text(x + 3, y + 2, &rank, Font::Head, ink);
            c.sprite(x + 8, y + 16, suit, 2, pip);
        }
        CardSize::Large => {
            c.text(x + 3, y + 2, &rank, Font::Head, ink);
            c.sprite(x + 5, y + 19, suit, 2, pip);
        }
    }
}

/// The room and the empty table.
pub fn draw_felt(c: &mut Canvas) {
    c.fill(Rect::new(0, 0, TABLE_W, c.h), theme::ROOM);
    for y in (0..c.h).step_by(4) {
        for x in (((y / 4) % 2) * 2..TABLE_W).step_by(4) {
            c.set(x, y, theme::ROOM_DOT);
        }
    }
    c.ellipse(CX, CY + 15, RX + 12, RY + 9, theme::SHADOW, false);
    c.ellipse(CX, CY + 8, RX + 9, RY + 9, theme::RAIL_SIDE, false);
    c.ellipse(CX, CY, RX + 9, RY + 9, theme::RAIL, false);
    c.ellipse(CX, CY, RX, RY, theme::FELT_DARK, false);
    c.ellipse(CX, CY, RX - 3, RY - 3, theme::FELT, false);
    c.ellipse(CX, CY - 8, RX - 44, RY - 40, theme::FELT_LIGHT, true);
    let mark = "POKER LAB";
    let width = Font::Big.width(mark) * 2;
    c.text_scaled(CX - width / 2, 214, mark, Font::Big, theme::FELT_MARK, 2);
    c.text_center(
        CX,
        255,
        "no limit texas hold'em",
        Font::Body,
        theme::FELT_MARK,
    );
}

struct Slot {
    /// Top-left of the seat's cards-and-plate cluster.
    seat: (i32, i32),
    /// Centre of the seat's bet.
    bet: (i32, i32),
    /// Centre of the dealer button when this seat has it.
    dealer: (i32, i32),
}
/// Clockwise from the watched seat at the bottom.
const SLOTS: [Slot; 6] = [
    Slot {
        seat: (170, 296),
        bet: (235, 280),
        dealer: (316, 282),
    },
    Slot {
        seat: (4, 232),
        bet: (150, 224),
        dealer: (138, 250),
    },
    Slot {
        seat: (4, 128),
        bet: (150, 166),
        dealer: (138, 138),
    },
    Slot {
        seat: (173, 56),
        bet: (235, 112),
        dealer: (306, 106),
    },
    Slot {
        seat: (342, 128),
        bet: (320, 166),
        dealer: (332, 138),
    },
    Slot {
        seat: (342, 232),
        bet: (320, 224),
        dealer: (332, 250),
    },
];

/// Which slots a table of `seats` uses, in clockwise order from the watched seat.
fn slots_for(seats: usize) -> &'static [usize] {
    match seats {
        2 => &[0, 3],
        3 => &[0, 2, 4],
        4 => &[0, 1, 3, 5],
        5 => &[0, 1, 2, 4, 5],
        _ => &[0, 1, 2, 3, 4, 5],
    }
}

fn label_colours(label: Label) -> (Color, Color) {
    match label {
        Label::SmallBlind | Label::BigBlind => (theme::PLATE_EDGE, theme::INK),
        Label::Fold => (theme::FAINT, theme::PLATE),
        Label::Check | Label::Call => (theme::GOOD, theme::PLATE),
        Label::Bet | Label::Raise => (theme::RED, theme::INK),
        Label::AllIn => (theme::CHIP, theme::PLATE),
    }
}

fn tag(c: &mut Canvas, right: i32, y: i32, text: &str, fill: Color, ink: Color) {
    let w = Font::Small.width(text) + 5;
    c.fill(Rect::new(right - w, y, w, 10), fill);
    c.text(right - w + 3, y + 1, text, Font::Small, ink);
}

pub fn chips(c: &mut Canvas, cx: i32, cy: i32, amount: Chips) {
    let text = amount.to_string();
    let w = (Font::Small.width(&text) + 8).max(19);
    let r = Rect::new(cx - w / 2, cy - 6, w, 12);
    c.plate(r, theme::CHIP, theme::CHIP_EDGE);
    c.text_center(cx, cy - 4, &text, Font::Small, theme::CARD_INK);
}

pub fn draw(c: &mut Canvas, scene: &Scene) {
    draw_felt(c);
    let frame = scene.frame;
    let n = frame.seats.len();
    let slots = slots_for(n);

    // Pot and board.
    let pot = Rect::new(CX - 31, 143, 62, 17);
    c.plate(pot, 0x18522f, theme::RAIL);
    c.text_center(CX, 145, &frame.pot.to_string(), Font::Bold, theme::INK);
    if frame.total_pot() != frame.pot {
        c.text_center(
            CX,
            131,
            &format!("total {}", frame.total_pot()),
            Font::Small,
            theme::RAIL,
        );
    }
    let (card_w, _) = CardSize::Board.dims();
    let x0 = CX - (5 * card_w + 4 * 4) / 2;
    for (i, card) in frame.board.iter().enumerate() {
        draw_card(
            c,
            x0 + i as i32 * (card_w + 4),
            172,
            Some(*card),
            CardSize::Board,
            false,
        );
    }

    for seat in 0..n {
        let slot = &SLOTS[slots[(seat + n - scene.hero % n) % n]];
        let state = &frame.seats[seat];
        let is_hero = seat == scene.hero;
        let (ax, ay) = slot.seat;
        let size = if is_hero {
            CardSize::Large
        } else {
            CardSize::Small
        };
        let (card_w, _) = size.dims();
        let overlap = card_w - 6;
        let plate = if is_hero {
            Rect::new(ax + 38, ay + 6, 92, 28)
        } else {
            Rect::new(ax + 28, ay, 96, 28)
        };
        let acting = frame.actor == Some(seat);
        let won = state.won > 0;
        if acting {
            let lit = (scene.ms / 320).is_multiple_of(2);
            c.dither(plate.inset(-4), theme::INK);
            c.fill(plate.inset(-2), if lit { theme::INK } else { theme::DIM });
        }
        let edge = if won {
            theme::CHIP
        } else if acting {
            theme::INK
        } else {
            theme::PLATE_EDGE
        };
        c.plate(plate, theme::PLATE, edge);
        let (name_ink, stack_ink) = if state.folded {
            (theme::FAINT, theme::FAINT)
        } else {
            (theme::DIM, theme::INK)
        };
        let name = scene.names.get(seat).map_or("", String::as_str);
        let avatar_x = plate.x + 10;
        if name.starts_with("DEEPSEEK") {
            let body = if state.folded {
                theme::FAINT
            } else {
                theme::BLUE
            };
            c.sprite(
                avatar_x - 1,
                plate.y + 9,
                &sprites::WHALE,
                1,
                |ch| match ch {
                    '#' => Some(body),
                    'o' => Some(theme::INK),
                    _ => None,
                },
            );
        } else {
            let body = if state.folded {
                theme::FAINT
            } else {
                theme::ROBOTS[seat % theme::ROBOTS.len()]
            };
            c.sprite(avatar_x, plate.y + 8, &sprites::ROBOT, 1, |ch| match ch {
                '#' => Some(body),
                'o' | 'm' => Some(theme::PLATE),
                _ => None,
            });
        }
        let text_x = plate.x + 27;
        c.text(text_x, plate.y + 3, name, Font::Body, name_ink);
        c.text(
            text_x,
            plate.y + 13,
            &state.stack.to_string(),
            Font::Bold,
            stack_ink,
        );

        // Cards sit over the plate's left edge, as dealt hands do at a real table.
        if let Some(hole) = scene.holes.get(seat) {
            let face_up = is_hero || scene.reveal || state.shown;
            if !state.folded || is_hero || scene.reveal {
                for (i, card) in hole.iter().enumerate() {
                    draw_card(
                        c,
                        ax + i as i32 * overlap,
                        ay + 1,
                        face_up.then_some(*card),
                        size,
                        state.folded,
                    );
                }
            }
        }
        if won {
            tag(
                c,
                plate.right() - 2,
                plate.y - 9,
                &format!("+{}", state.won),
                theme::CHIP,
                theme::PLATE,
            );
        } else if let Some(label) = state.label {
            let (fill, ink) = label_colours(label);
            tag(c, plate.right() - 2, plate.y - 9, label.text(), fill, ink);
        }
        if state.bet > 0 {
            chips(c, slot.bet.0, slot.bet.1, state.bet);
        }
        if seat == scene.button {
            let (dx, dy) = slot.dealer;
            c.ellipse(dx, dy, 6, 6, theme::CARD_EDGE, false);
            c.ellipse(dx, dy, 5, 5, theme::INK, false);
            c.text(dx - 2, dy - 4, "D", Font::Small, theme::CARD_INK);
        }
    }
}
