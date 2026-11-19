// ABOUTME: The spectator's screen: the table with its controls, the side panel (mind, hand analysis,
// ABOUTME: session statistics, hand list), replay of earlier hands, and the settings sheet.
use crate::{
    canvas::{Canvas, Color, Font, H, Rect, Sprite, W, wrap},
    frames::{Frame, Kind, SeatState},
    session::{Brain, HandLog, Notes, Phase, Settings, Shared, State, Thought},
    sprites,
    tableview::{self, Scene, TABLE_W, card_text},
    theme,
};
use poker_lab::{
    analyze,
    cards::Card,
    engine::{Action, Street},
};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Space,
    Left,
    Right,
    Up,
    Down,
    Tab,
    Enter,
    Escape,
    Backspace,
}

/// What the spectator did since the last picture, in canvas coordinates.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub mouse: (i32, i32),
    pub clicked: bool,
    /// Lines scrolled; positive scrolls the content down.
    pub wheel: i32,
    pub keys: Vec<Key>,
    pub text: String,
}
impl Input {
    fn hit(&self, r: Rect) -> bool {
        self.clicked && r.contains(self.mouse.0, self.mouse.1)
    }
    fn over(&self, r: Rect) -> bool {
        r.contains(self.mouse.0, self.mouse.1)
    }
    /// The same pointer position with nothing pressed, for layers under an open sheet.
    fn inert(&self) -> Input {
        Input {
            mouse: (-1, -1),
            ..Input::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Mind,
    Hand,
    Session,
    Hands,
    Notes,
}
const TABS: [(Tab, &str); 5] = [
    (Tab::Mind, "MIND"),
    (Tab::Hand, "HAND"),
    (Tab::Session, "SESSION"),
    (Tab::Hands, "HANDS"),
    (Tab::Notes, "NOTES"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Live,
    Replay {
        hand: usize,
        frame: usize,
        playing: bool,
        stepped_ms: u64,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Model,
    Seed,
}

#[derive(Clone, Copy)]
enum Click {
    Hand(usize),
    Decision(usize, usize),
}

struct Line {
    text: String,
    font: Font,
    ink: Color,
    right: Option<(String, Color)>,
    click: Option<Click>,
    marked: bool,
}

/// A scrolling column of text for the side panel.
struct Doc {
    lines: Vec<Line>,
    cols: usize,
}
impl Doc {
    fn new() -> Self {
        Self {
            lines: vec![],
            cols: (PANEL.w - 14) as usize / Font::Body.w() as usize,
        }
    }
    fn push(&mut self, text: impl Into<String>, font: Font, ink: Color) -> &mut Line {
        self.lines.push(Line {
            text: text.into(),
            font,
            ink,
            right: None,
            click: None,
            marked: false,
        });
        self.lines.last_mut().expect("just pushed")
    }
    fn title(&mut self, text: impl Into<String>) {
        self.push(text, Font::Head, theme::INK);
    }
    fn head(&mut self, text: impl Into<String>) {
        if !self.lines.is_empty() {
            self.gap();
        }
        self.push(text, Font::Bold, theme::CHIP);
    }
    fn gap(&mut self) {
        self.push("", Font::Tiny, theme::INK);
    }
    fn line(&mut self, text: impl Into<String>, ink: Color) -> &mut Line {
        self.push(text, Font::Body, ink)
    }
    /// A label on the left and a value at the right edge; the value moves to its own line
    /// when the two would collide.
    fn pair(&mut self, label: &str, value: impl Into<String>, ink: Color) {
        let value = value.into();
        if label.len() + value.len() + 1 > self.cols {
            self.push(label, Font::Body, theme::DIM);
            self.push("", Font::Body, theme::DIM).right = Some((value, ink));
        } else {
            self.push(label, Font::Body, theme::DIM).right = Some((value, ink));
        }
    }
    fn para(&mut self, text: &str, ink: Color) {
        for line in wrap(text, self.cols) {
            self.push(line, Font::Body, ink);
        }
    }
}

const PANEL: Rect = Rect::new(TABLE_W, 0, W - TABLE_W, H);
const SPEEDS: [f32; 4] = [0.5, 1.0, 2.0, 4.0];
const REPLAY_STEP_MS: u64 = 900;
const TYPING_CHARS_PER_SECOND: u128 = 400;

/// What the lab measured, one block per setup so results from different eras and models are
/// never read side by side as if comparable: (heading, unit, rows of label, value, 95% interval).
/// Thinking-on rows are DeepSeek with low effort on the deals of docs/results.md §5 and §6; the older
/// thinking-off rows are docs/results.md §2. Model rows are §6's lineup.
/// One measured row: label, value and 95% interval.
type LabRow = (&'static str, &'static str, &'static str);
const LAB_REFERENCE: [(&str, &str, &[LabRow]); 3] = [
    (
        "GLM, varied bots (11-19)",
        "chips vs no help, bb/100",
        &[
            ("facts sheet", "+151", "[+0, +301]"),
            ("cheat sheet", "+76", "[-105, +258]"),
            ("tracker + sharp rule", "+76", "[-68, +219]"),
            ("own notes, every 5", "+82", "[-90, +254]"),
            ("DeepSeek instead", "+126", "[+23, +228]"),
        ],
    ),
    (
        "DeepSeek, thinking on (11-17)",
        "river mistakes vs no help, bb/100",
        &[
            ("tracker + sharp rule", "-142", "[-265, -18]"),
            ("tracker + plain rule", "-136", "[-213, -59]"),
            ("cheat sheet", "-278", "[-437, -118]"),
            ("own notes, every 5", "-140", "[-178, -101]"),
            ("tracker alone", "-72", "[-130, -13]"),
        ],
    ),
    (
        "Other models, no help (11-17)",
        "chips won, bb/100",
        &[
            ("DeepSeek, think on", "+175", "[+87, +264]"),
            ("GLM-5.3 Flash, on", "+180", "[+79, +280]"),
            ("DeepSeek, think off", "+15", "[-63, +93]"),
            ("Gemma 4 26B, off", "-10", "[-64, +44]"),
            ("gpt-oss-20b, on", "-744", "[-984, -504]"),
        ],
    ),
];

fn signed(value: f64, places: usize) -> String {
    format!("{value:+.places$}")
}
fn tone(value: f64) -> Color {
    if value > 0.0 {
        theme::GOOD
    } else if value < 0.0 {
        theme::BAD
    } else {
        theme::INK
    }
}
fn percent(value: &Value) -> String {
    value
        .as_f64()
        .map_or("-".to_owned(), |v| format!("{:.0}%", v * 100.0))
}
fn cards_text(cards: &[Card]) -> String {
    cards
        .iter()
        .map(|c| card_text(*c))
        .collect::<Vec<_>>()
        .join(" ")
}
fn street_name(street: Street) -> &'static str {
    match street {
        Street::Preflop => "PREFLOP",
        Street::Flop => "FLOP",
        Street::Turn => "TURN",
        Street::River => "RIVER",
        Street::Complete => "DONE",
    }
}
fn action_name(action: Action) -> String {
    match action {
        Action::Fold => "fold".into(),
        Action::Check => "check".into(),
        Action::Call => "call".into(),
        Action::RaiseTo(n) => format!("raise to {n}"),
    }
}
/// "Seat N: text" lines of a notebook or cheat sheet, as (seat, text).
fn seat_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Seat ")?;
            let (seat, text) = rest.split_once(':')?;
            Some((seat.trim().parse().ok()?, text.trim().to_owned()))
        })
        .collect()
}

/// A compact table of tracker statistics (the stats condition's `completed_hand_stats`).
fn tracker_lines(stats: &Value, names: &[String], hero: usize) -> Vec<String> {
    let rate = |a: &Value, b: &Value| match (a.as_u64(), b.as_u64()) {
        (Some(a), Some(b)) if b > 0 => format!("{:>4.0}%", a as f64 / b as f64 * 100.0),
        _ => "    -".to_owned(),
    };
    let mut lines = vec![format!(
        "{:<8}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}",
        "", "HANDS", "VPIP", "PFR", "AF", "FOLD", "RIV"
    )];
    for (seat, s) in stats.as_array().into_iter().flatten().enumerate() {
        if seat == hero {
            continue;
        }
        let af = match (
            s["postflop_bets_raises"].as_u64(),
            s["postflop_calls"].as_u64(),
        ) {
            (Some(b), Some(0)) if b > 0 => "  inf".to_owned(),
            (Some(b), Some(c)) if c > 0 => format!("{:>5.1}", b as f64 / c as f64),
            _ => "    -".to_owned(),
        };
        let name: String = names
            .get(seat)
            .cloned()
            .unwrap_or_else(|| format!("SEAT {seat}"))
            .chars()
            .take(7)
            .collect();
        lines.push(format!(
            "{:<8}{:>5}{}{}{}{}{}",
            name,
            s["dealt"].to_string(),
            rate(&s["vpip"], &s["preflop_opportunities"]),
            rate(&s["pfr"], &s["preflop_opportunities"]),
            af,
            rate(&s["folded_to_postflop_bets"], &s["faced_postflop_bets"]),
            rate(&s["folded_to_river_bets"], &s["faced_river_bets"]),
        ));
    }
    lines
}

fn short_position(position: &str) -> &'static str {
    match position {
        "under_the_gun" => "UTG",
        "hijack" => "HJ",
        "cutoff" => "CO",
        "button" | "button_small_blind" => "BTN",
        "small_blind" => "SB",
        "big_blind" => "BB",
        _ => "MP",
    }
}
fn money(usd: f64) -> String {
    if usd == 0.0 {
        "$0".into()
    } else if usd < 0.01 {
        format!("${usd:.5}")
    } else {
        format!("${usd:.3}")
    }
}

fn text_button(c: &mut Canvas, input: &Input, r: Rect, label: &str, lit: bool) -> bool {
    let hover = input.over(r);
    let (fill, edge, ink) = if lit {
        (theme::INK, theme::INK, theme::PLATE)
    } else if hover {
        (theme::PANEL_ROW, theme::INK, theme::INK)
    } else {
        (theme::PLATE, theme::PLATE_EDGE, theme::DIM)
    };
    c.plate(r, fill, edge);
    c.text_center(
        r.x + r.w / 2,
        r.y + (r.h - Font::Body.h()) / 2,
        label,
        Font::Body,
        ink,
    );
    input.hit(r)
}

fn icon_button(c: &mut Canvas, input: &Input, r: Rect, icon: &Sprite, lit: bool) -> bool {
    let hover = input.over(r);
    c.plate(
        r,
        if lit { theme::INK } else { theme::PLATE },
        if hover || lit {
            theme::INK
        } else {
            theme::PLATE_EDGE
        },
    );
    let ink = if lit {
        theme::PLATE
    } else if hover {
        theme::INK
    } else {
        theme::DIM
    };
    c.sprite(
        r.x + (r.w - icon.w()) / 2,
        r.y + (r.h - icon.h()) / 2,
        icon,
        1,
        |ch| (ch == '#').then_some(ink),
    );
    input.hit(r)
}

/// A square that is filled when `on`; returns true when its row was clicked.
fn check(c: &mut Canvas, input: &Input, x: i32, y: i32, row_w: i32, on: bool, round: bool) -> bool {
    let row = Rect::new(x - 2, y - 2, row_w, 13);
    let hover = input.over(row);
    let b = Rect::new(x, y, 9, 9);
    let edge = if hover { theme::INK } else { theme::DIM };
    if round {
        c.plate(b, theme::PLATE, edge);
    } else {
        c.fill(b, edge);
        c.fill(b.inset(1), theme::PLATE);
    }
    if on {
        c.fill(b.inset(2), theme::CHIP);
    }
    input.hit(row)
}

pub struct App {
    shared: Arc<Shared>,
    /// An OpenRouter key was found, so the model can be seated.
    has_key: bool,
    /// The model is the local mock: nothing is spent.
    mock: bool,
    draft: Settings,
    seed_text: String,
    setup_open: bool,
    setup_error: Option<String>,
    focus: Option<Field>,
    pub tab: Tab,
    pub view: View,
    reveal: bool,
    scroll: [i32; TABS.len()],
    resume_after_replay: bool,
    seen_hands: usize,
}
impl App {
    pub fn new(shared: Arc<Shared>, has_key: bool, mock: bool) -> Self {
        let (draft, review, hands) = {
            let state = shared.lock();
            (
                state.settings.clone(),
                state.phase == Phase::Review,
                state.hands.len(),
            )
        };
        Self {
            shared,
            has_key,
            mock,
            seed_text: draft.seed.to_string(),
            draft,
            setup_open: !review,
            setup_error: None,
            focus: None,
            tab: if review { Tab::Hand } else { Tab::Mind },
            view: if review {
                View::Replay {
                    hand: 0,
                    frame: 0,
                    playing: false,
                    stepped_ms: 0,
                }
            } else {
                View::Live
            },
            reveal: false,
            scroll: [0; TABS.len()],
            resume_after_replay: false,
            seen_hands: hands,
        }
    }

    /// Draws one picture and applies what the spectator did.
    pub fn frame(&mut self, c: &mut Canvas, input: &Input, ms: u64) {
        let shared = self.shared.clone();
        let mut guard = shared.lock();
        let state = &mut *guard;
        self.keys(input, state, ms);
        self.advance_replay(state, ms);
        let under = if self.setup_open {
            input.inert()
        } else {
            input.clone()
        };
        self.draw_table(c, &under, state, ms);
        self.draw_panel(c, &under, state);
        if self.setup_open {
            self.draw_setup(c, input, state);
        }
        drop(guard);
        shared.wake.notify_all();
    }

    fn open_setup(&mut self, state: &mut State) {
        if state.phase == Phase::Review {
            return;
        }
        self.draft = state.settings.clone();
        self.seed_text = self.draft.seed.to_string();
        self.setup_error = None;
        self.focus = None;
        self.setup_open = true;
        if state.phase == Phase::Playing {
            state.paused = true;
        }
    }

    fn show_live(&mut self, state: &mut State) {
        if state.phase == Phase::Review {
            return;
        }
        self.view = View::Live;
        if self.resume_after_replay {
            state.paused = false;
            state.notice = None;
        }
        self.resume_after_replay = false;
    }

    fn show_replay(&mut self, state: &mut State, hand: usize, frame: usize) {
        if hand >= state.hands.len() {
            return;
        }
        if self.view == View::Live && state.phase == Phase::Playing {
            self.resume_after_replay = !state.paused;
            state.paused = true;
        }
        let last = state.hands[hand].frames.len() - 1;
        self.view = View::Replay {
            hand,
            frame: frame.min(last),
            playing: false,
            stepped_ms: 0,
        };
        self.scroll[Tab::Mind as usize] = 0;
        self.scroll[Tab::Hand as usize] = 0;
    }

    fn step(&mut self, state: &mut State, by: i32) {
        match self.view {
            View::Replay { hand, frame, .. } => {
                let target = frame as i32 + by;
                let frames = state.hands[hand].frames.len() as i32;
                if target < 0 && hand > 0 {
                    let previous = state.hands[hand - 1].frames.len() - 1;
                    self.show_replay(state, hand - 1, previous);
                } else if target >= frames && hand + 1 < state.hands.len() {
                    self.show_replay(state, hand + 1, 0);
                } else {
                    self.show_replay(state, hand, target.clamp(0, frames - 1) as usize);
                }
            }
            View::Live if by < 0 && !state.hands.is_empty() => {
                let hand = state.hands.len() - 1;
                self.show_replay(state, hand, usize::MAX);
            }
            View::Live => {}
        }
    }

    fn toggle_pause(&mut self, state: &mut State, ms: u64) {
        match &mut self.view {
            View::Replay {
                playing,
                stepped_ms,
                ..
            } => {
                *playing = !*playing;
                *stepped_ms = ms;
            }
            View::Live if state.phase == Phase::Playing => {
                state.paused = !state.paused;
                if !state.paused {
                    state.notice = None;
                }
            }
            View::Live => {}
        }
    }

    fn keys(&mut self, input: &Input, state: &mut State, ms: u64) {
        if self.setup_open {
            for key in &input.keys {
                match key {
                    Key::Escape if state.phase != Phase::Setup => self.close_setup(state, false),
                    Key::Enter => self.close_setup(state, true),
                    Key::Backspace => match self.focus {
                        Some(Field::Model) => {
                            self.draft.model.pop();
                        }
                        Some(Field::Seed) => {
                            self.seed_text.pop();
                        }
                        None => {}
                    },
                    Key::Tab => {
                        self.focus = match self.focus {
                            Some(Field::Model) => Some(Field::Seed),
                            _ => Some(Field::Model),
                        }
                    }
                    _ => {}
                }
            }
            for ch in input.text.chars() {
                match self.focus {
                    Some(Field::Model) if ch.is_ascii_graphic() && self.draft.model.len() < 60 => {
                        self.draft.model.push(ch)
                    }
                    Some(Field::Seed) if ch.is_ascii_digit() && self.seed_text.len() < 12 => {
                        self.seed_text.push(ch)
                    }
                    _ => {}
                }
            }
            return;
        }
        for key in &input.keys {
            match key {
                Key::Space => self.toggle_pause(state, ms),
                Key::Left => self.step(state, -1),
                Key::Right => self.step(state, 1),
                Key::Up | Key::Down => {
                    let by: i32 = if *key == Key::Up { -1 } else { 1 };
                    let current = match self.view {
                        View::Replay { hand, .. } => hand as i32,
                        View::Live => state.hands.len() as i32,
                    };
                    let target = current + by;
                    if target >= 0 && (target as usize) < state.hands.len() {
                        self.show_replay(state, target as usize, 0);
                    }
                }
                Key::Tab => {
                    let at = TABS
                        .iter()
                        .position(|(tab, _)| *tab == self.tab)
                        .unwrap_or(0);
                    self.tab = TABS[(at + 1) % TABS.len()].0;
                }
                Key::Escape => self.show_live(state),
                _ => {}
            }
        }
        for ch in input.text.chars() {
            match ch.to_ascii_lowercase() {
                'c' => self.open_setup(state),
                'h' => self.reveal = !self.reveal,
                'l' => self.show_live(state),
                '1'..='5' => self.tab = TABS[ch as usize - '1' as usize].0,
                '+' | '=' => self.change_speed(state, 1),
                '-' => self.change_speed(state, -1),
                _ => {}
            }
        }
    }

    fn change_speed(&self, state: &mut State, by: i32) {
        let at = SPEEDS.iter().position(|s| *s == state.speed).unwrap_or(1) as i32;
        state.speed = SPEEDS[(at + by).rem_euclid(SPEEDS.len() as i32) as usize];
    }

    fn advance_replay(&mut self, state: &mut State, ms: u64) {
        if let View::Replay {
            hand,
            frame,
            playing,
            stepped_ms,
        } = &mut self.view
        {
            *hand = (*hand).min(state.hands.len().saturating_sub(1));
            let frames = state.hands.get(*hand).map_or(1, |h| h.frames.len());
            *frame = (*frame).min(frames - 1);
            let wait = (REPLAY_STEP_MS as f32 / state.speed.max(0.25)) as u64;
            if *playing && ms.saturating_sub(*stepped_ms) >= wait {
                *stepped_ms = ms;
                if *frame + 1 < frames {
                    *frame += 1;
                } else {
                    *playing = false;
                }
            }
        }
        // A new hand resets the live reading position.
        if state.hands.len() != self.seen_hands {
            self.seen_hands = state.hands.len();
            if self.view == View::Live {
                self.scroll[Tab::Hand as usize] = 0;
            }
        }
    }

    /// The hand on screen, when it is a finished one.
    fn shown_hand<'a>(&self, state: &'a State) -> Option<&'a HandLog> {
        match self.view {
            View::Replay { hand, .. } => state.hands.get(hand),
            View::Live => state.hands.last(),
        }
    }

    fn draw_table(&mut self, c: &mut Canvas, input: &Input, state: &mut State, ms: u64) {
        let seats = state.names.len().max(2);
        let placeholder = Frame {
            kind: Kind::Deal,
            street: Street::Preflop,
            board: vec![],
            pot: 0,
            seats: vec![
                SeatState {
                    stack: 200,
                    bet: 0,
                    folded: false,
                    label: None,
                    shown: false,
                    won: 0
                };
                seats
            ],
            actor: None,
            decision: None,
            decisions_made: 0,
            caption: String::new(),
        };
        let no_cards: Vec<[Card; 2]> = vec![];
        let (frame, holes, button, number, help) = match self.view {
            View::Replay { hand, frame, .. } => {
                let log = &state.hands[hand];
                (
                    &log.frames[frame],
                    &log.analysis.holes,
                    log.record.button,
                    log.number,
                    log.help.as_str(),
                )
            }
            View::Live => match &state.live {
                Some(live) => (
                    live.frames.last().unwrap_or(&placeholder),
                    &live.holes,
                    live.start.button,
                    live.number,
                    live.help.as_str(),
                ),
                None => (&placeholder, &no_cards, usize::MAX, 0, ""),
            },
        };
        tableview::draw(
            c,
            &Scene {
                frame,
                names: &state.names,
                holes,
                hero: state.hero,
                button,
                reveal: self.reveal,
                ms,
            },
        );

        // Bottom: what just happened, where we are, and the watched seat's move.
        let caption = frame.caption.clone();
        let hero = &frame.seats[state.hero.min(frame.seats.len() - 1)];
        let asking =
            self.view == View::Live && state.live.as_ref().is_some_and(|l| l.asking.is_some());
        let closing = self.view == View::Live
            && state.live.as_ref().is_some_and(|l| l.closing)
            && state
                .applied
                .as_ref()
                .is_some_and(|s| s.notes == Notes::Own && s.brain == Brain::Model);
        let hero_line = if asking {
            format!("THINKING{}", ".".repeat((ms / 400 % 4) as usize))
        } else if closing {
            "WRITING NOTES".to_owned()
        } else if hero.won > 0 {
            format!("WINS {}", hero.won)
        } else if hero.folded {
            "FOLDED".to_owned()
        } else if frame.kind == Kind::Showdown {
            "SHOWDOWN".to_owned()
        } else if frame.kind == Kind::Payout {
            "LOSES".to_owned()
        } else {
            match hero.label {
                Some(label) if hero.bet > 0 => format!("{} {}", label.text(), hero.bet),
                Some(label) => label.text().to_owned(),
                None if number == 0 => "NOT DEALT".to_owned(),
                None => "WAITING".to_owned(),
            }
        };
        let step_text = match self.view {
            View::Replay { hand, frame, .. } => {
                format!("step {}/{}", frame + 1, state.hands[hand].frames.len())
            }
            View::Live => String::new(),
        };
        let help = help.to_owned();
        c.text(8, 350, &caption, Font::Body, theme::INK);
        let (status, fill, ink) = match self.view {
            View::Replay { .. } => ("REPLAY", theme::BLUE, theme::INK),
            View::Live if state.phase == Phase::Setup => {
                ("NOT STARTED", theme::FAINT, theme::PLATE)
            }
            View::Live if state.paused => ("PAUSED", theme::CHIP, theme::PLATE),
            View::Live => ("LIVE", theme::GOOD, theme::PLATE),
        };
        let tag = Rect::new(8, 365, Font::Bold.width(status) + 8, 15);
        c.fill(tag, fill);
        c.text(tag.x + 4, tag.y + 1, status, Font::Bold, ink);
        let mut x = tag.right() + 8;
        if number > 0 {
            x = c.text(x, 366, &format!("HAND {number}"), Font::Bold, theme::INK) + 8;
        }
        x = c.text(x, 368, &step_text, Font::Body, theme::DIM) + 8;
        c.text(x, 368, &help, Font::Body, theme::DIM);

        let name = state.names.get(state.hero).cloned().unwrap_or_default();
        let hero_box = Rect::new(346, 365, 118, 32);
        let lit = asking && (ms / 320).is_multiple_of(2);
        c.plate(
            hero_box,
            theme::PLATE,
            if lit { theme::INK } else { theme::DIM },
        );
        c.text_center(
            hero_box.x + hero_box.w / 2,
            hero_box.y + 4,
            &name,
            Font::Small,
            theme::DIM,
        );
        c.text_center(
            hero_box.x + hero_box.w / 2,
            hero_box.y + 15,
            &hero_line,
            Font::Bold,
            theme::INK,
        );

        if let View::Replay {
            hand,
            frame,
            playing,
            ..
        } = self.view
        {
            let frames = state.hands[hand].frames.len();
            let mut bx = 8;
            let mut button = |c: &mut Canvas, label: &str, w: i32, lit: bool| {
                let hit = text_button(c, input, Rect::new(bx, 384, w, 15), label, lit);
                bx += w + 3;
                hit
            };
            if button(c, "|<", 22, false) {
                self.show_replay(state, hand, 0);
            }
            if button(c, "<", 22, false) {
                self.step(state, -1);
            }
            if button(c, if playing { "STOP" } else { "PLAY" }, 34, playing) {
                if frame + 1 >= frames {
                    self.show_replay(state, hand, 0);
                }
                self.toggle_pause(state, ms);
            }
            if button(c, ">", 22, false) {
                self.step(state, 1);
            }
            if button(c, ">|", 22, false) {
                self.show_replay(state, hand, usize::MAX);
            }
            if button(c, "HAND-", 40, false) && hand > 0 {
                self.show_replay(state, hand - 1, 0);
            }
            if button(c, "HAND+", 40, false) {
                self.show_replay(state, hand + 1, 0);
            }
            if state.phase != Phase::Review && button(c, "BACK TO LIVE", 82, false) {
                self.show_live(state);
            }
        } else {
            let spent = state.stats.lab.reported_cost_usd + state.stats.lab.assumed_cost_usd;
            let spend = match &state.applied {
                Some(applied) if applied.brain == Brain::Model && self.mock => {
                    "mock model: nothing is spent".to_owned()
                }
                Some(applied) if applied.brain == Brain::Model => {
                    format!(
                        "spent {} of ${:.2} cap",
                        money(spent),
                        applied.spend_cap_usd
                    )
                }
                Some(_) => "stand-in bot: free".to_owned(),
                None => String::new(),
            };
            c.text(8, 386, &spend, Font::Body, theme::DIM);
        }
        if let Some(notice) = &state.notice {
            let lines = wrap(notice, 54);
            let h = lines.len() as i32 * 11 + 8;
            let r = Rect::new(60, 100 - h / 2, 350, h);
            c.plate(r, theme::PLATE, theme::CHIP);
            for (i, line) in lines.iter().enumerate() {
                c.text(
                    r.x + 8,
                    r.y + 4 + i as i32 * 11,
                    line,
                    Font::Body,
                    theme::INK,
                );
            }
        }

        // Left column, as on a card-room client: settings, hand list, card visibility.
        let review = state.phase == Phase::Review;
        if !review {
            if icon_button(
                c,
                input,
                Rect::new(6, 6, 24, 18),
                &sprites::ICON_MENU,
                false,
            ) {
                self.open_setup(state);
            }
            c.text(7, 26, "CONFIG", Font::Tiny, theme::DIM);
        }
        if icon_button(
            c,
            input,
            Rect::new(6, 36, 24, 18),
            &sprites::ICON_LIST,
            self.tab == Tab::Hands,
        ) {
            self.tab = Tab::Hands;
        }
        c.text(9, 56, "HANDS", Font::Tiny, theme::DIM);
        if icon_button(
            c,
            input,
            Rect::new(6, 66, 24, 18),
            &sprites::ICON_EYE,
            self.reveal,
        ) {
            self.reveal = !self.reveal;
        }
        c.text(9, 86, "CARDS", Font::Tiny, theme::DIM);

        // Top right: the game being played, pause and speed.
        let top = if review {
            "SAVED RUN".to_owned()
        } else {
            format!(
                "SEED {} - 6 MAX",
                state.applied.as_ref().map_or(self.draft.seed, |s| s.seed)
            )
        };
        c.text_right(TABLE_W - 8, 5, &top, Font::Tiny, theme::DIM);
        c.text_right(TABLE_W - 8, 13, "NLH ~ 1 / 2", Font::Bold, theme::INK);
        let running = match self.view {
            View::Replay { playing, .. } => playing,
            View::Live => state.phase == Phase::Playing && !state.paused,
        };
        let icon = if running {
            &sprites::ICON_PAUSE
        } else {
            &sprites::ICON_PLAY
        };
        if icon_button(c, input, Rect::new(TABLE_W - 32, 32, 24, 20), icon, false) {
            if state.phase == Phase::Setup {
                self.open_setup(state);
            } else {
                self.toggle_pause(state, ms);
            }
        }
        let speed = if state.speed < 1.0 {
            "1/2x".to_owned()
        } else {
            format!("{}x", state.speed as u32)
        };
        if text_button(c, input, Rect::new(TABLE_W - 32, 56, 24, 16), &speed, false) {
            self.change_speed(state, 1);
        }
    }

    fn draw_panel(&mut self, c: &mut Canvas, input: &Input, state: &mut State) {
        c.fill(PANEL, theme::PANEL);
        c.fill(Rect::new(PANEL.x, 0, 1, H), theme::PANEL_EDGE);
        let tab_w = (PANEL.w - 1) / TABS.len() as i32;
        for (i, (tab, label)) in TABS.iter().enumerate() {
            let r = Rect::new(PANEL.x + 1 + i as i32 * tab_w, 0, tab_w, 17);
            let active = *tab == self.tab;
            c.fill(
                r,
                if active {
                    theme::PANEL
                } else {
                    theme::PANEL_ROW
                },
            );
            let ink = if active {
                theme::INK
            } else if input.over(r) {
                theme::DIM
            } else {
                theme::FAINT
            };
            c.text_center(r.x + r.w / 2, 4, label, Font::Body, ink);
            if active {
                c.fill(Rect::new(r.x + 4, 15, r.w - 8, 2), theme::CHIP);
            }
            if input.hit(r) {
                self.tab = *tab;
            }
        }
        let doc = match self.tab {
            Tab::Mind => self.mind_doc(state),
            Tab::Hand => self.hand_doc(state),
            Tab::Session => self.session_doc(state),
            Tab::Hands => self.hands_doc(state),
            Tab::Notes => self.notes_doc(state),
        };
        let area = Rect::new(PANEL.x + 7, 23, PANEL.w - 14, H - 27);
        let height: i32 = doc.lines.iter().map(|l| l.font.h() + 1).sum();
        let scroll = &mut self.scroll[self.tab as usize];
        if input.over(PANEL) {
            *scroll += input.wheel * 11;
        }
        *scroll = (*scroll).clamp(0, (height - area.h).max(0));
        let previous = c.clip(Some(area));
        let mut y = area.y - *scroll;
        let mut clicked = None;
        for line in &doc.lines {
            let h = line.font.h() + 1;
            if y + h > area.y && y < area.bottom() {
                let row = Rect::new(area.x - 3, y - 1, area.w + 6, h);
                let hover = line.click.is_some() && input.over(row) && input.over(area);
                if line.marked || hover {
                    c.fill(
                        row,
                        if hover {
                            theme::PANEL_EDGE
                        } else {
                            theme::PANEL_ROW
                        },
                    );
                }
                c.text(area.x, y, &line.text, line.font, line.ink);
                if let Some((text, ink)) = &line.right {
                    c.text_right(area.right(), y, text, line.font, *ink);
                }
                if hover && input.clicked {
                    clicked = line.click;
                }
            }
            y += h;
        }
        c.restore_clip(previous);
        if height > area.h {
            let track = Rect::new(PANEL.right() - 4, area.y, 2, area.h);
            c.fill(track, theme::PANEL_ROW);
            let thumb = (area.h * area.h / height).max(8);
            let at = (area.h - thumb) * *scroll / (height - area.h);
            c.fill(Rect::new(track.x, area.y + at, 2, thumb), theme::DIM);
        }
        match clicked {
            Some(Click::Hand(hand)) => self.show_replay(state, hand, 0),
            Some(Click::Decision(hand, decision)) => {
                // Stop on the picture where the seat is about to make this decision.
                let frame = state.hands[hand]
                    .frames
                    .iter()
                    .rposition(|f| f.decisions_made <= decision)
                    .unwrap_or(0);
                self.show_replay(state, hand, frame);
                self.tab = Tab::Mind;
            }
            None => {}
        }
    }

    /// What the watched seat was shown, what it thought and what it did, for the decision on
    /// screen.
    fn mind_doc(&self, state: &State) -> Doc {
        let mut doc = Doc::new();
        let name = state.names.get(state.hero).cloned().unwrap_or_default();
        // (hand number, which of the seat's decisions, live)
        let target = match self.view {
            View::Replay { hand, frame, .. } => {
                let log = &state.hands[hand];
                let picture = &log.frames[frame];
                let made = log.record.decisions[..picture.decisions_made]
                    .iter()
                    .filter(|d| d.seat == state.hero)
                    .count();
                let index = if picture.actor == Some(state.hero) {
                    Some(made)
                } else {
                    made.checked_sub(1)
                };
                (log.number, index, false)
            }
            View::Live => match &state.live {
                Some(live) => {
                    let latest = state
                        .thoughts
                        .iter()
                        .rev()
                        .find(|t| t.hand == live.number && t.is_action())
                        .and_then(|t| t.decision);
                    (live.number, latest, true)
                }
                None => (0, None, true),
            },
        };
        let (number, index, live) = target;
        doc.title(format!("{name}'S MIND"));
        if number == 0 {
            doc.para(
                "The game has not started. Choose the player and the help it may use, then start.",
                theme::DIM,
            );
            return doc;
        }
        let Some(index) = index else {
            doc.line(format!("HAND {number}"), theme::DIM);
            doc.gap();
            doc.para(
                &format!("{name} has not had to act yet in this hand."),
                theme::DIM,
            );
            self.notes_section(&mut doc, state, number, live);
            return doc;
        };
        let thoughts = state.thoughts_for(number, index);
        if thoughts.is_empty() {
            doc.line(format!("HAND {number}  DECISION {}", index + 1), theme::DIM);
            doc.gap();
            let model = state
                .applied
                .as_ref()
                .is_none_or(|s| s.brain == Brain::Model);
            doc.para(
                if model && state.phase != Phase::Review {
                    "Waiting for the model."
                } else if state.phase == Phase::Review {
                    "This run has no saved model log for this decision."
                } else {
                    "The stand-in bot follows the lab's solid rule. It makes no requests and has no thoughts to show. See the HAND tab for how each move scores."
                },
                theme::DIM,
            );
            return doc;
        }
        for (part, thought) in thoughts.iter().enumerate() {
            if part > 0 {
                doc.head("ASKED AGAIN (ILLEGAL MOVE)");
            }
            self.thought_lines(&mut doc, state, thought, number, index, live);
        }
        self.notes_section(&mut doc, state, number, live);
        doc
    }

    fn thought_lines(
        &self,
        doc: &mut Doc,
        state: &State,
        thought: &Thought,
        number: usize,
        index: usize,
        live: bool,
    ) {
        let seen = &thought.observation;
        let street = seen["street"].as_str().unwrap_or("").to_uppercase();
        doc.line(
            format!("HAND {number}  {street}  DECISION {}", index + 1),
            theme::DIM,
        );
        doc.head("SEES");
        let list = |value: &Value| -> String {
            value
                .as_array()
                .map(|cards| {
                    cards
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
        };
        doc.pair("cards", list(&seen["hole"]), theme::INK);
        let board = list(&seen["board"]);
        doc.pair(
            "board",
            if board.is_empty() {
                "none yet".to_owned()
            } else {
                board
            },
            theme::INK,
        );
        doc.pair("pot", seen["pot"].to_string(), theme::INK);
        let legal = &seen["legal"];
        let mut may = vec![];
        if legal["check"] == true {
            may.push("check".to_owned());
        }
        if let Some(call) = legal["call"].as_u64() {
            may.push(format!("call {call}"));
            may.push("fold".to_owned());
        }
        if let (Some(lo), Some(hi)) = (
            legal["min_raise_to"].as_u64(),
            legal["max_raise_to"].as_u64(),
        ) {
            may.push(format!("raise {lo}-{hi}"));
        }
        if let Some(all_in) = legal["short_all_in_to"].as_u64() {
            may.push(format!("all in {all_in}"));
        }
        doc.pair("may", may.join(", "), theme::INK);
        if let Some(sizes) = thought.observation["bet_sizes"].as_array() {
            doc.head("BET SIZES IT WAS SHOWN");
            let menu: Vec<String> = sizes
                .iter()
                .map(|m| format!("{} {}", m["size"].as_str().unwrap_or("?"), m["raise_to"]))
                .collect();
            doc.para(&menu.join(", "), theme::DIM);
            doc.para(
                "With BET SIZING its instructions also say: size bets to what each opponent will call; against players who rarely fold, bet at least the pot on the river with strong hands, usually all-in, and never bluff them.",
                theme::FAINT,
            );
        }
        if let Some(aids) = thought.aids() {
            doc.head("FACTS SHEET");
            let word = |key: &str| aids[key].as_str().unwrap_or("-").replace('_', " ");
            doc.pair("position", word("position"), theme::INK);
            doc.pair("hand", word("hand"), theme::INK);
            if aids.get("made_hand").is_some() {
                doc.pair("made hand", word("made_hand"), theme::INK);
            }
            if let Some(draws) = aids.get("draws") {
                let mut found = vec![];
                if draws["flush_draw"] == true {
                    found.push("flush draw".to_owned());
                }
                match draws["straight_completing_ranks"].as_u64() {
                    Some(0) | None => {}
                    Some(1) => found.push("gutshot".to_owned()),
                    Some(n) => found.push(format!("straight draw ({n} ranks)")),
                }
                doc.pair(
                    "draws",
                    if found.is_empty() {
                        "none".to_owned()
                    } else {
                        found.join(", ")
                    },
                    theme::INK,
                );
            }
            if aids["to_call"].as_u64().unwrap_or(0) > 0 {
                doc.pair(
                    "to call",
                    format!(
                        "{} (needs {:.0}% to win)",
                        aids["to_call"],
                        aids["pot_odds_required_equity"].as_f64().unwrap_or(0.0) * 100.0
                    ),
                    theme::INK,
                );
            }
            doc.pair(
                "stack",
                format!(
                    "{} bb, {}x the pot",
                    aids["effective_stack_bb"], aids["stack_to_pot_ratio"]
                ),
                theme::INK,
            );
            doc.pair(
                "opponents still in",
                aids["opponents_in_hand"].to_string(),
                theme::INK,
            );
            doc.pair(
                "raises this street",
                aids["raises_this_street"].to_string(),
                theme::INK,
            );
        }
        if let Some(notes) = thought.notes_shown() {
            let kind = if notes.starts_with("Opponent profiles supplied") {
                "cheat sheet"
            } else {
                "own notes"
            };
            doc.pair(
                kind,
                format!("{} characters, below", notes.chars().count()),
                theme::INK,
            );
        }

        doc.head("THINKS");
        if thought.waiting {
            let waited = state
                .live
                .as_ref()
                .and_then(|l| l.asking)
                .map_or(0.0, |since| since.elapsed().as_secs_f32());
            doc.line(format!("thinking... {waited:.1}s"), theme::INK);
            if thought.attempts > 1 {
                doc.line(
                    format!("attempt {} (provider trouble)", thought.attempts),
                    theme::BAD,
                );
            }
            return;
        }
        match &thought.reasoning {
            Some(reasoning) => {
                // Fresh reasoning is typed out; replays show it whole.
                let shown = match thought.received.filter(|_| live) {
                    Some(at) => (at.elapsed().as_millis() * TYPING_CHARS_PER_SECOND / 1000) as usize,
                    None => usize::MAX,
                };
                let text: String = reasoning.chars().take(shown).collect();
                doc.para(text.trim(), theme::INK);
            }
            None if seen.is_null() => doc.para("(nothing recorded)", theme::DIM),
            None => doc.para(
                "No reasoning came back. With thinking off the model sends only its move, as in the lab's experiments.",
                theme::DIM,
            ),
        }

        doc.head("DOES");
        let reply: Value = thought
            .reply
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or(Value::Null);
        if thought.outcome == "ok" {
            let action = reply["action"]
                .as_str()
                .unwrap_or("?")
                .replace('_', " ")
                .to_uppercase();
            let text = match reply["amount"].as_u64() {
                Some(amount) => format!("{action} {amount}"),
                None => action,
            };
            doc.push(text, Font::Head, theme::INK);
        } else {
            doc.push(
                format!("REJECTED: {}", thought.outcome),
                Font::Bold,
                theme::BAD,
            );
            doc.para(
                "The referee played the fallback instead (check if free, else fold).",
                theme::DIM,
            );
            if let Some(text) = &thought.reply {
                doc.para(&format!("It answered: {text}"), theme::DIM);
            }
        }
        doc.line(
            format!(
                "{:.1}s  {}  {}",
                thought.latency_ms as f64 / 1000.0,
                thought.cost_usd.map_or("cost unknown".to_owned(), money),
                thought.provider.as_deref().unwrap_or("")
            ),
            theme::DIM,
        );
        doc.line(
            format!(
                "{} tokens in, {} out ({} reasoning)",
                thought.prompt_tokens, thought.completion_tokens, thought.reasoning_tokens
            ),
            theme::DIM,
        );
        if let Some(stats) = thought.observation.get("completed_hand_stats") {
            doc.head("TRACKER STATS IT WAS SHOWN");
            for line in tracker_lines(stats, &state.names, state.hero) {
                doc.line(line, theme::DIM);
            }
        }
        if let Some(notes) = thought.notes_shown().filter(|n| !n.trim().is_empty()) {
            doc.head(if notes.starts_with("Opponent profiles supplied") {
                "CHEAT SHEET IT WAS SHOWN"
            } else {
                "ITS OWN NOTES IT WAS SHOWN"
            });
            doc.para(notes, theme::DIM);
        }
    }

    /// The note-writing request after the hand, when the model keeps its own notes.
    fn notes_section(&self, doc: &mut Doc, state: &State, number: usize, live: bool) {
        let finished = match self.view {
            View::Replay { hand, frame, .. } => frame + 1 == state.hands[hand].frames.len(),
            View::Live => true,
        };
        let Some(thought) = state.notebook_thought(number).filter(|_| finished) else {
            return;
        };
        doc.head("AFTER THE HAND: NOTES");
        if thought.waiting && live {
            doc.line("rewriting its notes...", theme::INK);
            return;
        }
        if thought.outcome != "ok" {
            doc.line(format!("update failed: {}", thought.outcome), theme::BAD);
        }
        if let Some(reasoning) = &thought.reasoning {
            doc.para(reasoning.trim(), theme::INK);
            doc.gap();
        }
        match thought.notes_after.as_deref() {
            Some(notes) if !notes.trim().is_empty() => doc.para(notes, theme::DIM),
            _ => doc.para("(notes are empty)", theme::DIM),
        }
        doc.line(
            format!(
                "{:.1}s  {}",
                thought.latency_ms as f64 / 1000.0,
                thought.cost_usd.map_or(String::new(), money)
            ),
            theme::FAINT,
        );
    }

    fn hand_doc(&self, state: &State) -> Doc {
        let mut doc = Doc::new();
        let Some(log) = self.shown_hand(state) else {
            doc.title("HAND ANALYSIS");
            doc.para("No hand has finished yet.", theme::DIM);
            return doc;
        };
        let hand_index = log.number - 1;
        let a = &log.analysis;
        let name = state.names.get(state.hero).cloned().unwrap_or_default();
        doc.title(format!("HAND {}", log.number));
        if self.view == View::Live {
            doc.line("the last finished hand", theme::DIM);
        }
        doc.pair("help allowed", log.help.clone(), theme::INK);
        doc.pair(
            &name.to_lowercase(),
            format!(
                "{}  {}",
                cards_text(&a.holes[a.seat]),
                a.position.replace('_', " ")
            ),
            theme::INK,
        );
        let board = log
            .frames
            .last()
            .map(|f| cards_text(&f.board))
            .unwrap_or_default();
        doc.pair(
            "board",
            if board.is_empty() {
                "no flop".to_owned()
            } else {
                board
            },
            theme::INK,
        );

        doc.head("RESULT");
        doc.pair(
            "won or lost",
            format!("{} bb", signed(a.net_bb(), 1)),
            tone(a.net_bb()),
        );
        match a.adjusted_net {
            Some(_) => {
                doc.pair(
                    "with luck removed",
                    format!("{} bb", signed(a.adjusted_bb(), 1)),
                    tone(a.adjusted_bb()),
                );
                doc.para("All the money went in before the last card. This is the average over every way the board could finish.", theme::FAINT);
            }
            None => doc.pair("with luck removed", "same", theme::DIM),
        }
        doc.pair(
            "showdown",
            if a.reached_showdown { "yes" } else { "no" },
            theme::INK,
        );
        let graded = a.decisions.iter().filter(|d| d.graded.is_some()).count();
        doc.pair(
            "call/fold mistakes cost",
            format!("{:.1} bb", a.mistake_cost / a.big_blind as f64),
            if a.mistake_cost > 0.0 {
                theme::BAD
            } else {
                theme::INK
            },
        );
        let best = a
            .decisions
            .iter()
            .filter_map(|d| d.graded.as_ref())
            .filter(|g| g.loss == 0.0)
            .count();
        if graded > 0 {
            doc.pair(
                "took the best EV option",
                format!("{best} of {graded}"),
                theme::INK,
            );
        }
        for (street, label) in [
            (Street::River, "river choices cost"),
            (Street::Turn, "turn choices cost (approx)"),
        ] {
            let grades: Vec<_> = log.river.iter().filter(|g| g.street == street).collect();
            if !grades.is_empty() {
                let lost: f64 = grades.iter().map(|g| g.loss).sum();
                doc.pair(
                    label,
                    format!("{:.1} bb", lost / a.big_blind as f64),
                    if lost > 0.0 { theme::BAD } else { theme::INK },
                );
            }
        }

        doc.head("DECISIONS");
        if a.decisions.is_empty() {
            doc.line("none: the hand ended before its turn", theme::DIM);
        }
        let current = match self.view {
            View::Replay { frame, .. } => log.frames[frame].decision,
            View::Live => None,
        };
        for (i, d) in a.decisions.iter().enumerate() {
            let click = Some(Click::Decision(hand_index, d.index));
            let marked = current == Some(d.index);
            let mut action = action_name(d.action);
            if d.action == Action::Call {
                action = format!("call {}", d.to_call);
            }
            let verdict = match &d.graded {
                Some(g) => (
                    g.verdict().to_owned(),
                    if g.loss > 0.0 {
                        theme::BAD
                    } else {
                        theme::GOOD
                    },
                ),
                None => (String::new(), theme::DIM),
            };
            let first = doc.line(
                format!("{} {:<8}{action}", i + 1, street_name(d.street)),
                theme::INK,
            );
            first.right = Some(verdict);
            first.click = click;
            first.marked = marked;
            let second = doc.line(
                format!(
                    "  pot {}  equity {:.0}% vs {}",
                    d.pot,
                    d.equity * 100.0,
                    d.opponents
                ),
                theme::DIM,
            );
            second.click = click;
            second.marked = marked;
            if let Some(g) = &d.graded {
                let third = doc.line(
                    format!(
                        "  needs {:.0}%  EV call {}",
                        g.required * 100.0,
                        signed(g.ev_call, 1)
                    ),
                    theme::DIM,
                );
                third.right = Some((format!("best: {}", g.best().0), theme::DIM));
                third.click = click;
                third.marked = marked;
            }
            let rule = match (d.rule, d.agrees_with_rule()) {
                (Some(rule), Some(true)) => format!("  rule also says {}", action_name(rule)),
                (Some(rule), _) => format!("  rule says {}", action_name(rule)),
                (None, _) => "  rule: -".to_owned(),
            };
            let fourth = doc.line(rule, theme::FAINT);
            fourth.click = click;
            fourth.marked = marked;
            if let Some(g) = log.river.iter().find(|g| g.decision == d.index) {
                // The turn grade is an estimate (later play stood in for): marked with "~".
                let street = if g.street == Street::Turn {
                    "turn ~"
                } else {
                    "river"
                };
                let text = if g.loss > 0.0 {
                    format!(
                        "  {street}: {} (+{:.1} bb)",
                        action_name(g.best),
                        g.loss / a.big_blind as f64
                    )
                } else {
                    format!("  {street}: the best choice")
                };
                let line = doc.line(
                    text,
                    if g.loss > 0.0 {
                        theme::BAD
                    } else {
                        theme::GOOD
                    },
                );
                line.right = Some((g.kind.name().to_owned(), theme::DIM));
                line.click = click;
                line.marked = marked;
            }
            if let Some(reason) = &d.fallback {
                doc.line(format!("  FALLBACK played: {reason}"), theme::BAD);
            }
        }
        doc.head("HOW TO READ THIS");
        doc.para(
            "River choices are graded by replaying every option (check, call, fold, bet sizes, all-in) against every hand each bot could hold given how it plays and what it did, with the bots' real answers: the best a player who knows how each bot plays could do, without seeing its cards.",
            theme::FAINT,
        );
        doc.para(
            "Equity is the share of showdowns it would win against the cards the others really held. EV call = equity x (pot + call) - call, in chips; folding is 0. Only calls and folds are graded: the lab has no model yet of how opponents answer a bet or raise. The rule is the lab's solid bot. Click a decision to see what it was thinking.",
            theme::FAINT,
        );
        doc
    }

    fn session_doc(&self, state: &State) -> Doc {
        let mut doc = Doc::new();
        let name = state.names.get(state.hero).cloned().unwrap_or_default();
        doc.title("SESSION");
        let hands = state.hands.len();
        if hands == 0 {
            doc.para("No hand has finished yet.", theme::DIM);
            self.reference(&mut doc);
            return doc;
        }
        doc.pair("hands played", hands.to_string(), theme::INK);
        if let Some(applied) = &state.applied {
            doc.pair("help allowed now", applied.help_label(), theme::INK);
        }
        let row = state
            .stats
            .rows
            .get(state.hero)
            .cloned()
            .unwrap_or(Value::Null);
        let per_hand: Vec<f64> = state
            .hands
            .iter()
            .map(|h| h.analysis.net_bb() * 100.0)
            .collect();
        let total_bb: f64 = state.hands.iter().map(|h| h.analysis.net_bb()).sum();
        let number = |key: &str| row[key].as_f64().unwrap_or(0.0);

        doc.head(name.clone());
        doc.pair(
            "won or lost",
            format!("{} bb", signed(total_bb, 1)),
            tone(total_bb),
        );
        doc.pair(
            "win rate",
            format!("{} bb/100", signed(number("bb_per_100"), 0)),
            tone(number("bb_per_100")),
        );
        let range = analyze::interval(&per_hand);
        if let (Some(low), Some(high)) = (range["ci95"][0].as_f64(), range["ci95"][1].as_f64()) {
            doc.pair(
                "  95% range",
                format!("{} to {}", signed(low, 0), signed(high, 0)),
                theme::DIM,
            );
        }
        doc.pair(
            "with luck removed",
            format!("{} bb/100", signed(number("adjusted_bb_per_100"), 0)),
            tone(number("adjusted_bb_per_100")),
        );
        doc.pair(
            "call/fold mistakes cost",
            format!("{:.0} bb/100", number("mistake_cost_bb_per_100")),
            theme::INK,
        );
        doc.pair(
            "  graded calls and folds",
            format!(
                "{}: {} bad calls, {} bad folds",
                row["graded_decisions"], row["bad_calls"], row["bad_folds"]
            ),
            theme::DIM,
        );
        if row["river_graded"].as_u64().unwrap_or(0) > 0 {
            doc.pair(
                "river choices cost",
                format!("{:.0} bb/100", number("river_cost_bb_per_100")),
                theme::INK,
            );
            doc.pair(
                "  graded river decisions",
                row["river_graded"].to_string(),
                theme::DIM,
            );
        }
        if row["turn_graded"].as_u64().unwrap_or(0) > 0 {
            doc.pair(
                "turn choices cost (approx)",
                format!("{:.0} bb/100", number("turn_cost_bb_per_100")),
                theme::INK,
            );
            doc.pair(
                "  graded turn decisions",
                row["turn_graded"].to_string(),
                theme::DIM,
            );
        }
        doc.pair("plays a hand (VPIP)", percent(&row["vpip"]), theme::INK);
        doc.pair("raises before flop (PFR)", percent(&row["pfr"]), theme::INK);
        doc.pair(
            "bets per call (AF)",
            row["aggression_factor"]
                .as_f64()
                .map_or("-".to_owned(), |v| format!("{v:.2}")),
            theme::INK,
        );
        doc.pair(
            "folds when bet at",
            percent(&row["fold_to_bet"]),
            theme::INK,
        );
        doc.pair("goes to showdown", percent(&row["showdown"]), theme::INK);
        doc.pair(
            "agrees with the rule",
            percent(&row["reference_agreement"]),
            theme::INK,
        );
        let lab = &state.stats.lab;
        doc.pair(
            "fallbacks / repaired",
            format!("{} / {}", lab.fallbacks, lab.repaired_actions),
            if lab.fallbacks > 0 {
                theme::BAD
            } else {
                theme::INK
            },
        );
        doc.head("LEAKS");
        let kind = |k: &str| {
            let e = &row["river_kinds"][k];
            match e["count"].as_u64() {
                Some(n) => format!("{n}, {:.0} bb/100", e["bb_per_100"].as_f64().unwrap_or(0.0)),
                None => "none".to_owned(),
            }
        };
        doc.pair("river: missed bets", kind("missed bet"), theme::INK);
        doc.pair("river: bet the wrong size", kind("sizing"), theme::INK);
        doc.pair("river: bets that lost", kind("bad bet"), theme::INK);
        let turn_kind = |k: &str| {
            let e = &row["turn_kinds"][k];
            match e["count"].as_u64() {
                Some(n) => format!("{n}, {:.0} bb/100", e["bb_per_100"].as_f64().unwrap_or(0.0)),
                None => "none".to_owned(),
            }
        };
        if row["turn_graded"].as_u64().unwrap_or(0) > 0 {
            doc.pair("turn ~: missed bets", turn_kind("missed bet"), theme::INK);
            doc.pair(
                "turn ~: bet the wrong size",
                turn_kind("sizing"),
                theme::INK,
            );
        }
        let bluffs = row["bluffs"].as_u64().unwrap_or(0);
        doc.pair(
            "bluffs called down",
            if bluffs > 0 {
                format!("{} of {bluffs}", row["bluffs_called_down"])
            } else {
                "no bluffs".to_owned()
            },
            theme::INK,
        );
        doc.pair(
            "answers cut off (fallback)",
            row["truncations"].to_string(),
            if row["truncations"].as_u64().unwrap_or(0) > 0 {
                theme::BAD
            } else {
                theme::INK
            },
        );
        doc.para(
            "A bluff is a bet with under 30% equity against the cards really held; called down means it went to showdown anyway, as it does against a calling station.",
            theme::FAINT,
        );
        if hands < 200 {
            doc.para(
                "A session this short is mostly card luck: the range above is wide. The lab needed thousands of hands to separate the help settings.",
                theme::FAINT,
            );
        }

        if lab.requests > 0 {
            doc.head("SPEND");
            let spent = lab.reported_cost_usd + lab.assumed_cost_usd;
            doc.pair("spent", money(spent), theme::INK);
            doc.pair(
                "requests",
                format!("{} ({} for notes)", lab.requests, lab.memory_requests),
                theme::INK,
            );
            doc.pair("per hand", money(spent / hands as f64), theme::INK);
            doc.pair(
                "tokens in / out",
                format!("{} / {}", lab.prompt_tokens, lab.completion_tokens),
                theme::INK,
            );
            if lab.infra_failures > 0 {
                doc.pair(
                    "provider failures retried",
                    lab.infra_failures.to_string(),
                    theme::BAD,
                );
            }
        }

        if !state.stats.by_help.is_empty() {
            doc.head("BY HELP SETTING");
            for totals in &state.stats.by_help {
                doc.line(totals.help.clone(), theme::INK);
                doc.pair(
                    &format!("  {} hands", totals.hands),
                    format!(
                        "{} bb, luck removed {}",
                        signed(totals.net_bb, 1),
                        signed(totals.adjusted_bb, 1)
                    ),
                    tone(totals.net_bb),
                );
                if totals.river_bb > 0.0 || totals.turn_bb > 0.0 {
                    doc.pair(
                        "  given up: river / turn ~",
                        format!("{:.1} / {:.1} bb", totals.river_bb, totals.turn_bb),
                        theme::DIM,
                    );
                }
            }
        }

        doc.head("THE TABLE");
        doc.line(
            format!(
                "{:<11}{:>7}{:>6}{:>5}{:>5}",
                "player", "bb/100", "VPIP", "PFR", "fold"
            ),
            theme::DIM,
        );
        for row in &state.stats.rows {
            let rate = row["bb_per_100"].as_f64().unwrap_or(0.0);
            doc.line(
                format!(
                    "{:<11}{:>7}{:>6}{:>5}{:>5}",
                    row["label"].as_str().unwrap_or(""),
                    signed(rate, 0),
                    percent(&row["vpip"]),
                    percent(&row["pfr"]),
                    percent(&row["fold_to_bet"])
                ),
                if row["is_model"] == true {
                    theme::INK
                } else {
                    theme::DIM
                },
            );
        }
        self.reference(&mut doc);
        if let Some(dir) = &state.run_dir {
            doc.head("SAVED");
            doc.para(&format!("{}", dir.display()), theme::DIM);
            doc.para(
                "Readable by poker-lab analyze and poker-lab report.",
                theme::FAINT,
            );
        }
        doc
    }

    fn reference(&self, doc: &mut Doc) {
        doc.head("WHAT THE LAB MEASURED");
        for (heading, unit, rows) in LAB_REFERENCE {
            doc.line(heading, theme::INK);
            doc.line(unit, theme::DIM);
            for (label, value, range) in rows {
                doc.pair(label, format!("{value:>5} {range:>12}"), theme::INK);
            }
        }
        doc.para("Lower river mistakes is better. Each block is its own setup: do not compare numbers across blocks. 60-hand sessions, 10-20 deals each; the GLM block faced varied bots, the others the fixed five.", theme::FAINT);
    }

    /// What the watched seat knows about its opponents: the cheat sheet, its own notes and how
    /// they changed, or the tracker statistics, depending on the help in force.
    fn notes_doc(&self, state: &State) -> Doc {
        let mut doc = Doc::new();
        doc.title("NOTES");
        let help = match &state.applied {
            Some(settings) => settings.notes,
            // A saved lab run: read its condition.
            None => match state.hands.last().map(|h| h.help.as_str()).unwrap_or("") {
                h if h.contains("ORACLE") => Notes::CheatSheet,
                h if h.contains("NOTES") => Notes::Own,
                h if h.contains("STATS") => Notes::Tracker,
                _ => Notes::Off,
            },
        };
        let name = |seat: usize| {
            state
                .names
                .get(seat)
                .cloned()
                .unwrap_or_else(|| format!("SEAT {seat}"))
        };
        match help {
            Notes::CheatSheet => {
                doc.para(
                    "The cheat sheet: an exact description of how each bot plays. Real poker has no such thing; it measures the most that knowing your opponents can be worth.",
                    theme::FAINT,
                );
                for (seat, text) in seat_lines(&state.notebook) {
                    doc.head(name(seat));
                    doc.para(&text, theme::INK);
                }
            }
            Notes::Own => {
                doc.para(
                    "Notes the model writes about each opponent after every hand, as a player would. This is the realistic way to learn who is who.",
                    theme::FAINT,
                );
                // In a replay, the notes it had during that hand: those written after earlier
                // hands. Live, the latest.
                let before_hand = match self.view {
                    View::Replay { hand, .. } => Some(state.hands[hand].number),
                    View::Live => None,
                };
                if let Some(number) = before_hand {
                    doc.line(format!("as it had them during hand {number}"), theme::DIM);
                }
                // Every notes update in order, to see when each opponent's profile changed.
                let mut current: Vec<(usize, String)> = vec![];
                let mut changes: Vec<(usize, usize, String)> = vec![];
                for t in state.thoughts.iter().filter(|t| {
                    t.purpose.starts_with("notebook") && before_hand.is_none_or(|n| t.hand < n)
                }) {
                    let Some(after) = &t.notes_after else {
                        continue;
                    };
                    for (seat, text) in seat_lines(after) {
                        let before = current.iter().find(|(s, _)| *s == seat).map(|(_, x)| x);
                        if before != Some(&text) {
                            changes.push((t.hand, seat, text.clone()));
                            current.retain(|(s, _)| *s != seat);
                            current.push((seat, text));
                        }
                    }
                }
                let shown = if before_hand.is_some() || state.notebook.trim().is_empty() {
                    current.clone()
                } else {
                    seat_lines(&state.notebook)
                };
                if shown.is_empty() {
                    doc.para("No notes yet: they are written after each hand.", theme::DIM);
                }
                for (seat, text) in &shown {
                    doc.head(name(*seat));
                    if let Some((hand, _, _)) = changes.iter().rev().find(|(_, s, _)| s == seat) {
                        doc.line(format!("since hand {hand}"), theme::FAINT);
                    }
                    doc.para(text, theme::INK);
                }
                if !changes.is_empty() {
                    doc.head("HOW THEY CHANGED");
                    for (hand, seat, text) in &changes {
                        doc.line(format!("hand {hand}  {}", name(*seat)), theme::DIM);
                        doc.para(text, theme::FAINT);
                    }
                }
            }
            Notes::Tracker => {
                doc.para(
                    "Statistics of every seat from the hands so far, like the trackers online players use. The model sees these before each move.",
                    theme::FAINT,
                );
                // In a replay, the statistics it was shown during that hand.
                let upto = match self.view {
                    View::Replay { hand, .. } => state.hands[hand].number,
                    View::Live => usize::MAX,
                };
                if upto != usize::MAX {
                    doc.line(format!("as shown during hand {upto}"), theme::DIM);
                }
                let latest = state
                    .thoughts
                    .iter()
                    .rev()
                    .filter(|t| t.hand <= upto)
                    .find_map(|t| t.observation.get("completed_hand_stats"));
                match latest {
                    Some(stats) => {
                        for line in tracker_lines(stats, &state.names, state.hero) {
                            doc.line(line, theme::INK);
                        }
                        doc.para(
                            "VPIP: plays a hand. PFR: raises first. AF: bets per call after the flop. FOLD: folds when bet into after the flop; RIV: the same on the river.",
                            theme::FAINT,
                        );
                    }
                    None => doc.para("No statistics yet: they count finished hands.", theme::DIM),
                }
            }
            Notes::Off => doc.para(
                "No notes at this help setting. Press C and choose CHEAT SHEET, OWN NOTES or TRACKER.",
                theme::DIM,
            ),
        }
        doc
    }

    fn hands_doc(&self, state: &State) -> Doc {
        let mut doc = Doc::new();
        doc.title("HANDS");
        if state.hands.is_empty() {
            doc.para(
                "Finished hands appear here. Click one to replay it.",
                theme::DIM,
            );
            return doc;
        }
        doc.line("click a hand to replay it", theme::DIM);
        doc.gap();
        let current = match self.view {
            View::Replay { hand, .. } => Some(hand),
            View::Live => None,
        };
        for (index, log) in state.hands.iter().enumerate().rev() {
            let a = &log.analysis;
            let mut flags = String::new();
            if a.reached_showdown {
                flags.push_str(" SD");
            }
            if a.mistake_cost > 0.0 {
                flags.push_str(" !");
            }
            let line = doc.line(
                format!(
                    "#{:<4}{}  {:<4}{flags}",
                    log.number,
                    cards_text(&a.holes[a.seat]),
                    short_position(a.position)
                ),
                theme::INK,
            );
            line.right = Some((signed(a.net_bb(), 1), tone(a.net_bb())));
            line.click = Some(Click::Hand(index));
            line.marked = current == Some(index);
        }
        doc.gap();
        doc.para(
            "SD: went to showdown.  !: a graded call or fold lost chips.",
            theme::FAINT,
        );
        doc
    }

    fn close_setup(&mut self, state: &mut State, apply: bool) {
        if !apply {
            self.setup_open = false;
            return;
        }
        if self.draft.brain == Brain::Model {
            if !self.has_key {
                self.setup_error = Some("No OpenRouter key found. Set OPENROUTER_API_KEY or create .openrouter-api-key, or pick the stand-in.".into());
                return;
            }
            if self.draft.model.trim().is_empty() {
                self.setup_error = Some("Enter a model ID.".into());
                return;
            }
        }
        if state.phase == Phase::Setup {
            self.draft.seed = self.seed_text.parse().unwrap_or(42);
        }
        state.settings = self.draft.clone();
        state.phase = Phase::Playing;
        state.paused = false;
        state.notice = None;
        self.setup_open = false;
        self.view = View::Live;
        self.resume_after_replay = false;
    }

    fn draw_setup(&mut self, c: &mut Canvas, input: &Input, state: &mut State) {
        let started = state.phase != Phase::Setup;
        c.dither(Rect::new(0, 0, W, H), theme::ROOM);
        let sheet = Rect::new(100, 36, 520, 332);
        c.fill(
            Rect::new(sheet.x + 5, sheet.y + 5, sheet.w, sheet.h),
            theme::SHADOW,
        );
        c.plate(sheet, theme::PANEL, theme::DIM);
        let x = sheet.x + 16;
        let mut y = sheet.y + 12;
        c.text(x, y, "POKER LAB TABLE", Font::Head, theme::INK);
        c.text_right(
            sheet.right() - 16,
            y + 4,
            "NLH 1/2  six seats  you spectate",
            Font::Body,
            theme::DIM,
        );
        y += 24;

        c.text(x, y, "PLAYER", Font::Bold, theme::CHIP);
        y += 16;
        let model_label = if self.mock {
            "MODEL     local mock, nothing is spent"
        } else {
            "MODEL     live through OpenRouter; costs money"
        };
        if check(c, input, x, y, 470, self.draft.brain == Brain::Model, true) {
            self.draft.brain = Brain::Model;
        }
        c.text(x + 16, y, model_label, Font::Body, theme::INK);
        y += 14;
        if check(
            c,
            input,
            x,
            y,
            470,
            self.draft.brain == Brain::StandIn,
            true,
        ) {
            self.draft.brain = Brain::StandIn;
        }
        c.text(
            x + 16,
            y,
            "STAND-IN  the lab's solid rule; free, offline, no thoughts",
            Font::Body,
            theme::INK,
        );
        y += 16;
        let is_model = self.draft.brain == Brain::Model;
        c.text(x + 16, y + 3, "model", Font::Body, theme::DIM);
        let field = Rect::new(x + 56, y, 300, 15);
        let focused = self.focus == Some(Field::Model);
        c.plate(
            field,
            theme::PLATE,
            if focused {
                theme::INK
            } else {
                theme::PLATE_EDGE
            },
        );
        let end = c.text(
            field.x + 5,
            y + 3,
            &self.draft.model,
            Font::Body,
            if is_model { theme::INK } else { theme::FAINT },
        );
        if focused {
            c.fill(Rect::new(end + 1, y + 3, 5, 9), theme::CHIP);
        }
        if input.hit(field) {
            self.focus = Some(Field::Model);
        } else if input.clicked {
            self.focus = None;
        }
        y += 20;
        if check(c, input, x + 16, y, 454, self.draft.thinking, false) {
            self.draft.thinking = !self.draft.thinking;
        }
        c.text(x + 32, y, "SHOW THINKING", Font::Body, theme::INK);
        c.text(
            x + 118,
            y,
            "it reasons before each move, and you read it",
            Font::Body,
            theme::DIM,
        );
        y += 12;
        c.text(
            x + 32,
            y,
            "Lab numbers below: DeepSeek with thinking on (low effort). Off is cheaper.",
            Font::Body,
            theme::FAINT,
        );
        y += 14;

        c.text(x, y, "HELP ALLOWED", Font::Bold, theme::CHIP);
        c.text_right(
            sheet.right() - 16,
            y + 2,
            "river mistakes vs no help, bb/100",
            Font::Body,
            theme::DIM,
        );
        y += 14;
        /// One row of the HELP ALLOWED list.
        #[derive(Clone, Copy, PartialEq)]
        enum Help {
            Facts,
            Sizing,
            Cheat,
            Own,
            Quick,
            Tracker,
            Baseline,
        }
        let rows: [(Help, &str, &str, &str, &str); 7] = [
            (
                Help::Facts,
                "FACTS SHEET",
                "position, pot odds, hand strength",
                "   -",
                "untested with thinking",
            ),
            (
                Help::Sizing,
                "BET SIZING",
                "sharp rule (tested with tracker)",
                "-142",
                "held on new cards",
            ),
            (
                Help::Cheat,
                "CHEAT SHEET",
                "true profile of every opponent",
                "-278",
                "biggest cut",
            ),
            (
                Help::Own,
                "OWN NOTES",
                "writes notes after each hand",
                " -98",
                "helps; costs 3x",
            ),
            (
                Help::Quick,
                "  QUICK",
                "own notes written without thinking",
                "   -",
                "cheaper; not yet tested",
            ),
            (
                Help::Tracker,
                "TRACKER",
                "stats per opponent, like a HUD",
                " -72",
                "small help",
            ),
            (
                Help::Baseline,
                "BASELINE",
                "no help at all",
                "   0",
                "the reference",
            ),
        ];
        for (help, name, what, gain, verdict) in rows {
            let draft = &mut self.draft;
            let on = match help {
                Help::Facts => draft.facts_sheet,
                Help::Sizing => draft.bet_sizing,
                Help::Cheat => draft.notes == Notes::CheatSheet,
                Help::Own => draft.notes == Notes::Own,
                Help::Quick => draft.notes == Notes::Own && draft.quick_notes,
                Help::Tracker => draft.notes == Notes::Tracker,
                Help::Baseline => draft.is_baseline(),
            };
            if check(c, input, x, y, 488, on, false) {
                match help {
                    Help::Facts => draft.facts_sheet = !on,
                    Help::Sizing => draft.bet_sizing = !on,
                    Help::Cheat => draft.notes = if on { Notes::Off } else { Notes::CheatSheet },
                    Help::Own => draft.notes = if on { Notes::Off } else { Notes::Own },
                    // Quick notes are own notes written without thinking.
                    Help::Quick => {
                        draft.quick_notes = !on;
                        if !on {
                            draft.notes = Notes::Own;
                        }
                    }
                    Help::Tracker => draft.notes = if on { Notes::Off } else { Notes::Tracker },
                    Help::Baseline => {
                        draft.facts_sheet = false;
                        draft.bet_sizing = false;
                        draft.notes = Notes::Off;
                    }
                }
            }
            c.text(x + 16, y, name, Font::Body, theme::INK);
            c.text(x + 92, y, what, Font::Body, theme::DIM);
            c.text(x + 306, y, gain, Font::Body, theme::INK);
            c.text(
                x + 340,
                y,
                verdict,
                Font::Body,
                if matches!(help, Help::Sizing | Help::Cheat) {
                    theme::GOOD
                } else {
                    theme::DIM
                },
            );
            y += 12;
        }
        c.text(
            x + 16,
            y,
            "Cheat sheet, own notes and tracker are one at a time, as the lab measures them.",
            Font::Body,
            theme::FAINT,
        );
        y += 14;

        c.text(x, y, "TABLE", Font::Bold, theme::CHIP);
        y += 16;
        c.text(x, y + 3, "seed", Font::Body, theme::DIM);
        let seed = Rect::new(x + 30, y, 86, 15);
        let seed_focus = self.focus == Some(Field::Seed) && !started;
        c.plate(
            seed,
            theme::PLATE,
            if seed_focus {
                theme::INK
            } else {
                theme::PLATE_EDGE
            },
        );
        let end = c.text(
            seed.x + 5,
            y + 3,
            &self.seed_text,
            Font::Body,
            if started { theme::FAINT } else { theme::INK },
        );
        if seed_focus {
            c.fill(Rect::new(end + 1, y + 3, 5, 9), theme::CHIP);
        }
        if input.hit(seed) && !started {
            self.focus = Some(Field::Seed);
        }
        c.text(x + 132, y + 3, "stacks", Font::Body, theme::DIM);
        if text_button(
            c,
            input,
            Rect::new(x + 174, y, 76, 15),
            "CARRY OVER",
            !self.draft.reset_stacks,
        ) {
            self.draft.reset_stacks = false;
        }
        if text_button(
            c,
            input,
            Rect::new(x + 252, y, 76, 15),
            "RESET EACH",
            self.draft.reset_stacks,
        ) {
            self.draft.reset_stacks = true;
        }
        c.text(x + 344, y + 3, "cap", Font::Body, theme::DIM);
        if text_button(c, input, Rect::new(x + 368, y, 15, 15), "-", false) {
            self.draft.spend_cap_usd =
                ((self.draft.spend_cap_usd - 0.05) * 100.0).round().max(5.0) / 100.0;
        }
        c.text_center(
            x + 408,
            y + 3,
            &format!("${:.2}", self.draft.spend_cap_usd),
            Font::Body,
            theme::INK,
        );
        if text_button(c, input, Rect::new(x + 434, y, 15, 15), "+", false) {
            self.draft.spend_cap_usd = ((self.draft.spend_cap_usd + 0.05) * 100.0)
                .round()
                .min(2000.0)
                / 100.0;
        }
        y += 20;
        c.text(x, y + 3, "bots", Font::Body, theme::DIM);
        if text_button(
            c,
            input,
            Rect::new(x + 30, y, 60, 15),
            "FIXED",
            !self.draft.varied_bots,
        ) && !started
        {
            self.draft.varied_bots = false;
        }
        if text_button(
            c,
            input,
            Rect::new(x + 92, y, 60, 15),
            "VARIED",
            self.draft.varied_bots,
        ) && !started
        {
            self.draft.varied_bots = true;
        }
        c.text(
            x + 162,
            y + 3,
            if self.draft.varied_bots {
                "5 bots, thresholds drawn per seed, 10% surprises"
            } else {
                "5 bots exactly as in the earlier experiments"
            },
            Font::Body,
            theme::DIM,
        );
        y += 17;
        c.text(
            x,
            y,
            if started {
                "Changes apply from the next hand; seed and bots are fixed per game."
            } else {
                "RESET EACH is the lab's setting: 100 bb in every seat, every hand."
            },
            Font::Body,
            theme::FAINT,
        );

        let by = sheet.bottom() - 30;
        if let Some(error) = &self.setup_error {
            for (i, line) in wrap(error, 50).iter().take(2).enumerate() {
                c.text(x, by - 4 + i as i32 * 11, line, Font::Body, theme::BAD);
            }
        } else if is_model && !self.has_key {
            c.text(
                x,
                by + 4,
                "No OpenRouter key found: only the stand-in can play.",
                Font::Body,
                theme::BAD,
            );
        } else if !started {
            c.text(
                x,
                by + 4,
                "SPACE pause   C this sheet   H show cards   arrows replay",
                Font::Body,
                theme::FAINT,
            );
        }
        let go = Rect::new(sheet.right() - 136, by, 120, 19);
        if text_button(
            c,
            input,
            go,
            if started {
                "APPLY AND RESUME"
            } else {
                "START GAME"
            },
            true,
        ) {
            self.close_setup(state, true);
        }
        if started && text_button(c, input, Rect::new(go.x - 66, by, 60, 19), "CANCEL", false) {
            self.close_setup(state, false);
        }
    }
}
