// ABOUTME: Hand-drawn pixel art: card suits, the robot and whale avatars, and button icons.
use crate::canvas::Sprite;

pub const HEART: Sprite = Sprite(&[
    ".##.##.", "#######", "#######", "#######", ".#####.", "..###..", "...#...",
]);
pub const DIAMOND: Sprite = Sprite(&[
    "...#...", "..###..", ".#####.", "#######", ".#####.", "..###..", "...#...",
]);
pub const SPADE: Sprite = Sprite(&[
    "...#...", "..###..", ".#####.", "#######", "##.#.##", "...#...", "..###..",
]);
pub const CLUB: Sprite = Sprite(&[
    "..###..", "..###..", "##.#.##", "#######", "##.#.##", "...#...", "..###..",
]);
/// Suits in the engine's order: clubs, diamonds, hearts, spades.
pub const SUITS: [&Sprite; 4] = [&CLUB, &DIAMOND, &HEART, &SPADE];

/// A scripted opponent. '#' body, 'o' eyes, 'm' mouth.
pub const ROBOT: Sprite = Sprite(&[
    ".....##.....",
    ".....##.....",
    ".##########.",
    "############",
    "##oo####oo##",
    "##oo####oo##",
    "############",
    "###mmmmmm###",
    "############",
    ".##########.",
    "...##..##...",
    "..###..###..",
]);
/// The DeepSeek seat: a whale, after the model's logo. '#' body, 'o' eye.
pub const WHALE: Sprite = Sprite(&[
    "..........#..#",
    "..........####",
    "...........##.",
    "..######..###.",
    ".###########..",
    "############..",
    "##o#########..",
    "############..",
    ".##########...",
    "...######.....",
]);

pub const ICON_MENU: Sprite = Sprite(&[
    "#########",
    "#########",
    ".........",
    "#########",
    "#########",
    ".........",
    "#########",
    "#########",
]);
pub const ICON_LIST: Sprite = Sprite(&[
    "##.######",
    "##.######",
    ".........",
    "##.######",
    "##.######",
    ".........",
    "##.######",
    "##.######",
]);
pub const ICON_EYE: Sprite = Sprite(&[
    "..#####..",
    ".#.....#.",
    "#..###..#",
    "#..###..#",
    "#..###..#",
    ".#.....#.",
    "..#####..",
]);
pub const ICON_PAUSE: Sprite = Sprite(&[
    "###..###", "###..###", "###..###", "###..###", "###..###", "###..###", "###..###", "###..###",
]);
pub const ICON_PLAY: Sprite = Sprite(&[
    "##......", "####....", "######..", "########", "########", "######..", "####....", "##......",
]);
