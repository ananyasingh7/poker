// ABOUTME: The table's fixed colour set. A small palette keeps the picture looking like one machine drew it.
use crate::canvas::Color;

pub const ROOM: Color = 0x16171b;
pub const ROOM_DOT: Color = 0x1b1d22;
pub const FELT: Color = 0x2b8a4f;
pub const FELT_LIGHT: Color = 0x329b59;
pub const FELT_DARK: Color = 0x1f6c3c;
pub const FELT_MARK: Color = 0x247a45;
pub const RAIL: Color = 0x12281b;
pub const RAIL_SIDE: Color = 0x08110c;
pub const SHADOW: Color = 0x0d0e11;
pub const INK: Color = 0xf2efe2;
pub const DIM: Color = 0xa3a294;
pub const FAINT: Color = 0x63655f;
pub const PLATE: Color = 0x0c0d0f;
pub const PLATE_EDGE: Color = 0x3a3d44;
pub const PANEL: Color = 0x0f1013;
pub const PANEL_EDGE: Color = 0x2b2d33;
pub const PANEL_ROW: Color = 0x17191d;
pub const CHIP: Color = 0xf0d84a;
pub const CHIP_EDGE: Color = 0x8d7412;
pub const RED: Color = 0xdb4a40;
pub const CARD: Color = 0xf4f1e4;
pub const CARD_EDGE: Color = 0x25262a;
pub const CARD_INK: Color = 0x1b1c20;
pub const BACK: Color = 0xc8655b;
pub const BACK_DARK: Color = 0xa54b43;
pub const BLUE: Color = 0x4d6bfe;
pub const GOOD: Color = 0x74d391;
pub const BAD: Color = 0xec6a60;

/// Body colours of the scripted opponents' robots, by seat.
pub const ROBOTS: [Color; 6] = [0xd9d6c8, 0x8fb4d9, 0xe3a860, 0x9bd08a, 0xc79be8, 0xe58a86];
