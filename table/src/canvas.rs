// ABOUTME: Fixed-resolution software framebuffer for the retro table: rectangles, ellipses, dithering,
// ABOUTME: pixel sprites and X11 bitmap-font text. Everything on screen is drawn through this.
use embedded_graphics::{
    Drawable, Pixel,
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point, Size},
    mono_font::{MonoFont, MonoTextStyle, ascii},
    pixelcolor::{Rgb888, RgbColor},
    text::{Baseline, Text},
};
use std::{convert::Infallible, path::Path};

/// Logical screen size. The window shows it at the largest whole-number scale that fits.
pub const W: i32 = 720;
pub const H: i32 = 405;

/// 0x00RRGGBB, the layout the window surface expects.
pub type Color = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
    pub fn inset(&self, n: i32) -> Self {
        Self::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n)
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Font {
    /// 4x6: chip amounts and other labels that must fit in a few pixels.
    Tiny,
    /// 5x8: secondary labels.
    Small,
    /// 6x10: body text.
    Body,
    /// 7x13 bold: names, emphasised values.
    Bold,
    /// 9x15 bold: headings.
    Head,
    /// 10x20: the largest face, used scaled for the felt wordmark.
    Big,
}
impl Font {
    fn mono(self) -> &'static MonoFont<'static> {
        match self {
            Self::Tiny => &ascii::FONT_4X6,
            Self::Small => &ascii::FONT_5X8,
            Self::Body => &ascii::FONT_6X10,
            Self::Bold => &ascii::FONT_7X13_BOLD,
            Self::Head => &ascii::FONT_9X15_BOLD,
            Self::Big => &ascii::FONT_10X20,
        }
    }
    /// Advance per character.
    pub fn w(self) -> i32 {
        self.mono().character_size.width as i32
    }
    pub fn h(self) -> i32 {
        self.mono().character_size.height as i32
    }
    pub fn width(self, text: &str) -> i32 {
        text.chars().count() as i32 * self.w()
    }
}

/// A pixel drawing: one character per pixel, coloured through a palette at draw time.
pub struct Sprite(pub &'static [&'static str]);
impl Sprite {
    pub fn w(&self) -> i32 {
        self.0.first().map_or(0, |row| row.len() as i32)
    }
    pub fn h(&self) -> i32 {
        self.0.len() as i32
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Color>,
    clip: Rect,
}
impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h) as usize],
            clip: Rect::new(0, 0, w, h),
        }
    }
    /// Restricts drawing to `rect` (the whole canvas for `None`); returns the previous clip.
    pub fn clip(&mut self, rect: Option<Rect>) -> Rect {
        let full = Rect::new(0, 0, self.w, self.h);
        let wanted = rect.unwrap_or(full);
        let x0 = wanted.x.max(0);
        let y0 = wanted.y.max(0);
        let x1 = wanted.right().min(self.w);
        let y1 = wanted.bottom().min(self.h);
        std::mem::replace(
            &mut self.clip,
            Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0)),
        )
    }
    pub fn restore_clip(&mut self, previous: Rect) {
        self.clip = previous;
    }
    pub fn set(&mut self, x: i32, y: i32, c: Color) {
        if self.clip.contains(x, y) {
            self.px[(y * self.w + x) as usize] = c;
        }
    }
    pub fn get(&self, x: i32, y: i32) -> Color {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        self.px[(y * self.w + x) as usize]
    }
    pub fn fill(&mut self, r: Rect, c: Color) {
        let x0 = r.x.max(self.clip.x);
        let x1 = r.right().min(self.clip.right());
        for y in r.y.max(self.clip.y)..r.bottom().min(self.clip.bottom()) {
            if x0 < x1 {
                let row = (y * self.w) as usize;
                self.px[row + x0 as usize..row + x1 as usize].fill(c);
            }
        }
    }
    /// One-pixel outline just inside `r`.
    pub fn outline(&mut self, r: Rect, c: Color) {
        self.fill(Rect::new(r.x, r.y, r.w, 1), c);
        self.fill(Rect::new(r.x, r.bottom() - 1, r.w, 1), c);
        self.fill(Rect::new(r.x, r.y, 1, r.h), c);
        self.fill(Rect::new(r.right() - 1, r.y, 1, r.h), c);
    }
    /// A filled box with its four corner pixels cut, the era's rounded rectangle.
    pub fn plate(&mut self, r: Rect, fill: Color, border: Color) {
        self.fill(Rect::new(r.x + 1, r.y, r.w - 2, r.h), border);
        self.fill(Rect::new(r.x, r.y + 1, r.w, r.h - 2), border);
        self.fill(r.inset(1), fill);
    }
    /// Every other pixel of `r` in a checkerboard: a two-colour blend without new colours.
    pub fn dither(&mut self, r: Rect, c: Color) {
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                if (x + y) & 1 == 0 {
                    self.set(x, y, c);
                }
            }
        }
    }
    /// Filled axis-aligned ellipse; `checker` draws only every other pixel.
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Color, checker: bool) {
        for dy in -ry..=ry {
            let t = 1.0 - (dy as f64 / ry as f64).powi(2);
            let half = (rx as f64 * t.max(0.0).sqrt()).round() as i32;
            if checker {
                for x in cx - half..=cx + half {
                    if (x + cy + dy) & 1 == 0 {
                        self.set(x, cy + dy, c);
                    }
                }
            } else {
                self.fill(Rect::new(cx - half, cy + dy, 2 * half + 1, 1), c);
            }
        }
    }
    /// Draws `text` with its top-left corner at (x, y); returns the x just past it.
    pub fn text(&mut self, x: i32, y: i32, text: &str, font: Font, c: Color) -> i32 {
        self.text_scaled(x, y, text, font, c, 1)
    }
    pub fn text_scaled(
        &mut self,
        x: i32,
        y: i32,
        text: &str,
        font: Font,
        c: Color,
        scale: i32,
    ) -> i32 {
        let style = MonoTextStyle::new(font.mono(), rgb(c));
        let mut target = Scaled {
            canvas: self,
            x,
            y,
            scale,
        };
        let _ = Text::with_baseline(text, Point::zero(), style, Baseline::Top).draw(&mut target);
        x + font.width(text) * scale
    }
    pub fn text_right(&mut self, right: i32, y: i32, text: &str, font: Font, c: Color) {
        self.text(right - font.width(text), y, text, font, c);
    }
    pub fn text_center(&mut self, cx: i32, y: i32, text: &str, font: Font, c: Color) {
        self.text(cx - font.width(text) / 2, y, text, font, c);
    }
    /// Draws a sprite; `palette` maps each character to a colour, `None` for transparent.
    pub fn sprite(
        &mut self,
        x: i32,
        y: i32,
        sprite: &Sprite,
        scale: i32,
        palette: impl Fn(char) -> Option<Color>,
    ) {
        for (row, line) in sprite.0.iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if let Some(c) = palette(ch) {
                    self.fill(
                        Rect::new(x + col as i32 * scale, y + row as i32 * scale, scale, scale),
                        c,
                    );
                }
            }
        }
    }
    /// Writes the picture as a PNG, each pixel enlarged `scale` times.
    pub fn save_png(&self, path: &Path, scale: i32) -> Result<(), String> {
        let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut encoder = png::Encoder::new(
            std::io::BufWriter::new(file),
            (self.w * scale) as u32,
            (self.h * scale) as u32,
        );
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        let mut bytes = Vec::with_capacity((self.w * self.h * scale * scale * 3) as usize);
        for y in 0..self.h * scale {
            for x in 0..self.w * scale {
                let c = self.px[((y / scale) * self.w + x / scale) as usize];
                bytes.extend([(c >> 16) as u8, (c >> 8) as u8, c as u8]);
            }
        }
        writer.write_image_data(&bytes).map_err(|e| e.to_string())
    }
}

fn rgb(c: Color) -> Rgb888 {
    Rgb888::new((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Adapter that lets the bitmap-font renderer draw onto the canvas at a whole-number scale.
struct Scaled<'a> {
    canvas: &'a mut Canvas,
    x: i32,
    y: i32,
    scale: i32,
}
impl OriginDimensions for Scaled<'_> {
    fn size(&self) -> Size {
        Size::new(self.canvas.w as u32, self.canvas.h as u32)
    }
}
impl DrawTarget for Scaled<'_> {
    type Color = Rgb888;
    type Error = Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Infallible> {
        for Pixel(point, colour) in pixels {
            let c = (u32::from(colour.r()) << 16)
                | (u32::from(colour.g()) << 8)
                | u32::from(colour.b());
            self.canvas.fill(
                Rect::new(
                    self.x + point.x * self.scale,
                    self.y + point.y * self.scale,
                    self.scale,
                    self.scale,
                ),
                c,
            );
        }
        Ok(())
    }
}

/// The bitmap fonts cover printable ASCII only: common typographic characters are replaced
/// with their plain equivalents and anything else with '?'.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' | '\u{2032}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{2192}' => out.push_str("->"),
            '\u{2248}' => out.push('~'),
            '\u{2265}' => out.push_str(">="),
            '\u{2264}' => out.push_str("<="),
            '\u{00d7}' => out.push('x'),
            '\u{2660}' => out.push('s'),
            '\u{2665}' => out.push('h'),
            '\u{2666}' => out.push('d'),
            '\u{2663}' => out.push('c'),
            '\u{00a0}' | '\t' => out.push(' '),
            '\n' => out.push('\n'),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// Breaks text into lines of at most `cols` characters at word boundaries; newlines are kept
/// and a word longer than a line is split.
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    let mut lines = vec![];
    for paragraph in plain(text).split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            let mut word = word;
            loop {
                let used = line.len() + usize::from(!line.is_empty());
                if used + word.len() <= cols {
                    if !line.is_empty() {
                        line.push(' ');
                    }
                    line.push_str(word);
                    break;
                }
                if line.is_empty() {
                    lines.push(word[..cols].to_owned());
                    word = &word[cols..];
                } else {
                    lines.push(std::mem::take(&mut line));
                }
            }
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_keeps_words_whole_and_splits_overlong_ones() {
        assert_eq!(wrap("call the bet now", 8), ["call the", "bet now"]);
        assert_eq!(wrap("abcdefghij kl", 4), ["abcd", "efgh", "ij", "kl"]);
        assert_eq!(wrap("a\n\nb", 10), ["a", "", "b"]);
    }

    #[test]
    fn plain_replaces_typographic_characters() {
        assert_eq!(
            plain("it\u{2019}s 5\u{2013}6 \u{2192} ok\u{2026} \u{4e2d}"),
            "it's 5-6 -> ok... ?"
        );
    }

    #[test]
    fn drawing_outside_the_clip_is_ignored() {
        let mut canvas = Canvas::new(8, 8);
        let previous = canvas.clip(Some(Rect::new(2, 2, 2, 2)));
        canvas.fill(Rect::new(-5, -5, 30, 30), 0xffffff);
        canvas.restore_clip(previous);
        assert_eq!(canvas.px.iter().filter(|c| **c != 0).count(), 4);
        assert_eq!(canvas.get(2, 2), 0xffffff);
        assert_eq!(canvas.get(4, 4), 0);
    }
}
