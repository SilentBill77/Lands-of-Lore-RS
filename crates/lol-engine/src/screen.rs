//! The screen as the original drives it (ScummVM's `Screen`, `Screen_v2`, `Screen_LoL`): 16
//! pages of 320×200 (in VGA mode two page numbers share one buffer: 0/1, 2/3 …), page 0 being
//! what the monitor shows after `update_screen`; windows (`ScreenDim`) to draw into; four
//! palettes and the screen's; fonts; the mouse pointer; fades that take time.

#![allow(clippy::needless_range_loop)]

use lol_data::compression::lcw;

use crate::platform::{Cursor, System};
use crate::shape::{self, Dim, ShapeArgs};

pub const SCREEN_W: usize = 320;
pub const SCREEN_H: usize = 200;
pub const PAGE_SIZE: usize = SCREEN_W * SCREEN_H + 1024;
pub const PAGE_NUM: usize = 16;

/// `copyRegion` without the check for colour 0.
pub const CR_NO_P_CHECK: i32 = 1;

/// A palette of 256 colours, 6-bit VGA values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pal(pub Vec<u8>);

impl Default for Pal {
    fn default() -> Self {
        Pal(vec![0; 768])
    }
}

impl Pal {
    pub fn from_vga(bytes: &[u8]) -> Self {
        let mut p = Pal::default();
        p.load(bytes);
        p
    }

    /// Loads 6-bit values (as many as given) from colour 0.
    pub fn load(&mut self, bytes: &[u8]) {
        let n = bytes.len().min(768);
        self.0[..n].copy_from_slice(&bytes[..n]);
    }

    /// `Palette::copy`: colours `first..first + count` of `src` to `dst_start`.
    pub fn copy_from(&mut self, src: &Pal, first: usize, count: usize, dst_start: usize) {
        let count = count.min(256 - first).min(256 - dst_start);
        self.0[dst_start * 3..(dst_start + count) * 3]
            .copy_from_slice(&src.0[first * 3..(first + count) * 3]);
    }

    pub fn fill(&mut self, first: usize, count: usize, value: u8) {
        let end = (first + count).min(256);
        self.0[first * 3..end * 3].fill(value);
    }
}

/// Westwood's fonts of this time (ScummVM's `DOSFont`, signature `0500`).
#[derive(Debug, Clone)]
pub struct Font {
    data: Vec<u8>,
    pub width: u8,
    pub height: u8,
    glyphs: usize,
    offsets_at: usize,
    widths_at: usize,
    heights_at: usize,
}

impl Font {
    pub fn load(bytes: &[u8]) -> Option<Self> {
        let word = |i: usize| -> Option<usize> {
            Some(u16::from_le_bytes([*bytes.get(i)?, *bytes.get(i + 1)?]) as usize)
        };
        if word(2)? != 0x0500 {
            return None;
        }
        let desc = word(4)?;
        Some(Self {
            width: *bytes.get(desc + 5)?,
            height: *bytes.get(desc + 4)?,
            glyphs: *bytes.get(desc + 3)? as usize + 1,
            offsets_at: word(6)?,
            widths_at: word(8)?,
            heights_at: word(12)?,
            data: bytes.to_vec(),
        })
    }

    pub fn char_width(&self, c: u8) -> i32 {
        if c as usize >= self.glyphs {
            return 0;
        }
        self.data
            .get(self.widths_at + c as usize)
            .copied()
            .unwrap_or(0) as i32
    }

    /// `DOSFont::drawChar`: two pixels a byte (low half first), the rows above and below the
    /// glyph's own in colour 0 of the map.
    fn draw_char(&self, c: u8, page: &mut [u8], at: usize, color_map: &[u8; 16]) {
        let c = c as usize;
        if c >= self.glyphs {
            return;
        }
        let offset = self
            .data
            .get(self.offsets_at + 2 * c..self.offsets_at + 2 * c + 2)
            .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
        if offset == 0 {
            return;
        }
        let w = self.char_width(c as u8) as usize;
        if w == 0 {
            return;
        }
        let h1 = self.data.get(self.heights_at + 2 * c).copied().unwrap_or(0) as usize;
        let h2 = self
            .data
            .get(self.heights_at + 2 * c + 1)
            .copied()
            .unwrap_or(0) as usize;
        let h0 = (self.height as usize).saturating_sub(h1 + h2);
        let mut src = offset;
        let mut row = at;
        let put = |page: &mut [u8], i: usize, col: u8| {
            if col != 0
                && let Some(p) = page.get_mut(i)
            {
                *p = col;
            }
        };
        for _ in 0..h1 {
            for i in 0..w {
                put(page, row + i, color_map[0]);
            }
            row += SCREEN_W;
        }
        for _ in 0..h2 {
            let mut b = 0u8;
            for i in 0..w {
                let col = if i & 1 == 1 {
                    color_map[(b >> 4) as usize]
                } else {
                    b = self.data.get(src).copied().unwrap_or(0);
                    src += 1;
                    color_map[(b & 0xf) as usize]
                };
                put(page, row + i, col);
            }
            row += SCREEN_W;
        }
        for _ in 0..h0 {
            for i in 0..w {
                put(page, row + i, color_map[0]);
            }
            row += SCREEN_W;
        }
    }
}

/// The fonts the game loads (`Screen::FontId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontId {
    F6 = 0,
    F8,
    F9,
    Cred6,
    Cred8,
    Book,
    Gold,
    Intro,
}

const FONTS: usize = 8;

/// The windows of Lands of Lore in 256 colours (`_screenDimTable256C`).
const DIMS: [[i32; 8]; 25] = [
    [0x00, 0x00, 0x28, 0xC8, 0xC7, 0xCF, 0x00, 0x00],
    [0x08, 0x48, 0x18, 0x38, 0xFE, 0x01, 0x00, 0x00],
    [0x0E, 0x00, 0x16, 0x78, 0xFE, 0x01, 0x00, 0x00],
    [0x0B, 0x7B, 0x1C, 0x12, 0xFE, 0xFC, 0x00, 0x00],
    [0x0B, 0x7B, 0x1C, 0x2D, 0xFE, 0xFC, 0x00, 0x00],
    [0x55, 0x7B, 0xE9, 0x37, 0xFE, 0xFC, 0x00, 0x00],
    [0x0B, 0x8C, 0x10, 0x2B, 0x3D, 0x01, 0x00, 0x00],
    [0x04, 0x59, 0x20, 0x3C, 0x00, 0x00, 0x00, 0x00],
    [0x05, 0x6E, 0x1E, 0x0C, 0xFE, 0x01, 0x00, 0x00],
    [0x07, 0x19, 0x1A, 0x97, 0x00, 0x00, 0x00, 0x00],
    [0x03, 0x1E, 0x22, 0x8C, 0x00, 0x00, 0x00, 0x00],
    [0x02, 0x48, 0x24, 0x34, 0x00, 0x00, 0x00, 0x00],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x0E, 0x00, 0x16, 0x78, 0xFE, 0x01, 0x00, 0x00],
    [0x0D, 0xA2, 0x18, 0x0C, 0xFE, 0x01, 0x00, 0x00],
    [0x0F, 0x06, 0x14, 0x6E, 0x01, 0x00, 0x00, 0x00],
    [0x1A, 0xBE, 0x0A, 0x07, 0xFE, 0x01, 0x00, 0x00],
    [0x07, 0x21, 0x1A, 0x85, 0x00, 0x00, 0x00, 0x00],
    [0x03, 0x32, 0x22, 0x62, 0x00, 0x00, 0x00, 0x00],
    [0x0B, 0x8C, 0x10, 0x33, 0x3D, 0x01, 0x00, 0x00],
    [0x0B, 0x8C, 0x10, 0x23, 0x3D, 0x01, 0x00, 0x00],
    [0x01, 0x20, 0x26, 0x80, 0xDC, 0xFD, 0x00, 0x00],
    [0x09, 0x29, 0x08, 0x2C, 0x00, 0x00, 0x00, 0x00],
    [0x19, 0x29, 0x08, 0x2C, 0x00, 0x00, 0x00, 0x00],
    [0x01, 0x02, 0x26, 0x14, 0x00, 0x0F, 0x0E, 0x00],
];

pub struct Screen {
    pub system: System,
    pages: Vec<Vec<u8>>,
    pub cur_page: usize,
    dims: Vec<Dim>,
    custom_dims: Vec<Option<Dim>>,
    pub cur_dim_index: usize,
    palettes: Vec<Pal>,
    pub screen_palette: Pal,
    fonts: Vec<Option<Font>>,
    pub current_font: FontId,
    text_colors: [u8; 16],
    pub char_spacing: i32,
    pub line_spacing: i32,
    pub text_margin_right: i32,
    mouse_lock: i32,
    pub fade_flag: i32,
    /// `_paletteOverlay1/2`, `_grayOverlay`, the levels' darkening tables.
    pub palette_overlay1: Vec<u8>,
    pub palette_overlay2: Vec<u8>,
    pub gray_overlay: Vec<u8>,
    pub level_overlays: Vec<Vec<u8>>,
    /// `_wsaFrameAnimBuffer`.
    wsa_buffer: Vec<u8>,
    pub cursor_key: u8,
}

impl Screen {
    pub fn new(system: System) -> Self {
        let dims = DIMS
            .iter()
            .map(|d| Dim {
                sx: d[0],
                sy: d[1],
                w: d[2],
                h: d[3],
                col1: d[4],
                col2: d[5],
                line: d[6],
                column: d[7],
            })
            .collect::<Vec<_>>();
        let n = dims.len();
        Self {
            system,
            pages: (0..PAGE_NUM / 2).map(|_| vec![0; PAGE_SIZE]).collect(),
            cur_page: 0,
            dims,
            custom_dims: vec![None; n],
            cur_dim_index: 0,
            palettes: vec![Pal::default(); 4],
            screen_palette: Pal::default(),
            fonts: vec![None; FONTS],
            current_font: FontId::F6,
            text_colors: std::array::from_fn(|i| i as u8),
            char_spacing: 0,
            line_spacing: 0,
            text_margin_right: SCREEN_W as i32,
            mouse_lock: 1,
            fade_flag: 2,
            palette_overlay1: vec![0; 256],
            palette_overlay2: vec![0; 256],
            gray_overlay: vec![0; 256],
            level_overlays: vec![vec![0; 256]; 8],
            wsa_buffer: vec![0; 1024],
            cursor_key: 0xff,
        }
    }

    // ---- pages

    pub fn page(&self, num: usize) -> &[u8] {
        &self.pages[(num & (PAGE_NUM - 1)) >> 1]
    }

    pub fn page_mut(&mut self, num: usize) -> &mut [u8] {
        &mut self.pages[(num & (PAGE_NUM - 1)) >> 1]
    }

    /// Two pages at once (they must not share a buffer).
    fn two_pages(&mut self, a: usize, b: usize) -> Option<(&mut [u8], &mut [u8])> {
        let (ia, ib) = ((a & 15) >> 1, (b & 15) >> 1);
        if ia == ib {
            return None;
        }
        if ia < ib {
            let (x, y) = self.pages.split_at_mut(ib);
            Some((&mut x[ia], &mut y[0]))
        } else {
            let (x, y) = self.pages.split_at_mut(ia);
            Some((&mut y[0], &mut x[ib]))
        }
    }

    pub fn same_buffer(a: usize, b: usize) -> bool {
        (a & 15) >> 1 == (b & 15) >> 1
    }

    pub fn set_cur_page(&mut self, num: usize) -> usize {
        std::mem::replace(&mut self.cur_page, num)
    }

    pub fn clear_page(&mut self, num: usize) {
        self.page_mut(num).fill(0);
    }

    pub fn clear_cur_page(&mut self) {
        let p = self.cur_page;
        self.clear_page(p);
    }

    pub fn get_page_pixel(&self, page: usize, x: i32, y: i32) -> u8 {
        if !(0..SCREEN_W as i32).contains(&x) || !(0..SCREEN_H as i32).contains(&y) {
            return 0;
        }
        self.page(page)[y as usize * SCREEN_W + x as usize]
    }

    pub fn set_page_pixel(&mut self, page: usize, x: i32, y: i32, color: u8) {
        if (0..SCREEN_W as i32).contains(&x) && (0..SCREEN_H as i32).contains(&y) {
            self.page_mut(page)[y as usize * SCREEN_W + x as usize] = color;
        }
    }

    /// Shows page 0 on the monitor (`updateScreen`).
    pub fn update_screen(&mut self) {
        let page = &self.pages[0];
        self.system.copy_to_screen(page);
    }

    // ---- windows

    pub fn dim(&self, index: usize) -> Dim {
        self.custom_dims
            .get(index)
            .copied()
            .flatten()
            .or_else(|| self.dims.get(index).copied())
            .unwrap_or_default()
    }

    pub fn dim_count(&self) -> usize {
        self.dims.len()
    }

    /// `checkedPageUpdate`: what differs between the pages copied, onto the screen as well
    /// (compared and copied four pixels at a time, as the original does).
    pub fn checked_page_update(&mut self, src: usize, dst: usize) {
        let s = self.page(src)[..SCREEN_W * SCREEN_H].to_vec();
        let d = self.page(dst)[..SCREEN_W * SCREEN_H].to_vec();
        let mut p0 = self.page(0)[..SCREEN_W * SCREEN_H].to_vec();
        let mut dd = d.clone();
        for i in (0..SCREEN_W * SCREEN_H).step_by(4) {
            if s[i..i + 4] != d[i..i + 4] {
                dd[i..i + 4].copy_from_slice(&s[i..i + 4]);
                p0[i..i + 4].copy_from_slice(&s[i..i + 4]);
            }
        }
        self.page_mut(dst)[..SCREEN_W * SCREEN_H].copy_from_slice(&dd);
        self.page_mut(0)[..SCREEN_W * SCREEN_H].copy_from_slice(&p0);
    }

    pub fn cur_dim(&self) -> Dim {
        self.dim(self.cur_dim_index)
    }

    pub fn set_screen_dim(&mut self, index: usize) {
        self.cur_dim_index = index;
    }

    /// `modifyScreenDim` (Lands of Lore makes the window current as well).
    pub fn modify_screen_dim(&mut self, index: usize, x: i32, y: i32, w: i32, h: i32) {
        let mut d = self.dims.get(index).copied().unwrap_or_default();
        d.sx = x;
        d.sy = y;
        d.w = w;
        d.h = h;
        if let Some(slot) = self.custom_dims.get_mut(index) {
            *slot = Some(d);
        }
        self.set_screen_dim(index);
    }

    /// Changes the text position of a window (`line`, `column`).
    pub fn set_dim_cursor(&mut self, index: usize, line: i32, column: i32) {
        let mut d = self.dim(index);
        d.line = line;
        d.column = column;
        if let Some(slot) = self.custom_dims.get_mut(index) {
            *slot = Some(d);
        }
    }

    pub fn set_dim_colors(&mut self, index: usize, col1: i32, col2: i32) {
        let mut d = self.dim(index);
        d.col1 = col1;
        d.col2 = col2;
        if let Some(slot) = self.custom_dims.get_mut(index) {
            *slot = Some(d);
        }
    }

    // ---- palettes

    pub fn palette(&self, num: usize) -> &Pal {
        &self.palettes[num]
    }

    pub fn palette_mut(&mut self, num: usize) -> &mut Pal {
        &mut self.palettes[num]
    }

    pub fn copy_palette(&mut self, dst: usize, src: usize) {
        let p = self.palettes[src].clone();
        self.palettes[dst] = p;
    }

    pub fn set_screen_palette(&mut self, pal: &Pal) {
        self.screen_palette = pal.clone();
        self.system.set_palette(&pal.0, 0);
    }

    pub fn set_palette_index(&mut self, index: usize, r: u8, g: u8, b: u8) {
        let pal = &mut self.palettes[0];
        if pal.0[index * 3..index * 3 + 3] == [r, g, b] {
            return;
        }
        pal.0[index * 3..index * 3 + 3].copy_from_slice(&[r, g, b]);
        let p = self.palettes[0].clone();
        self.set_screen_palette(&p);
    }

    /// `Screen_v2::getFadeParams`.
    pub fn get_fade_params(&self, pal: &Pal, delay: i32) -> (i32, i32) {
        self.fade_params(pal, delay)
    }

    fn fade_params(&self, pal: &Pal, delay: i32) -> (i32, i32) {
        let mut max_diff = 0;
        for i in 0..768 {
            max_diff = max_diff.max((pal.0[i] as i32 - self.screen_palette.0[i] as i32).abs());
        }
        let mut delay_inc = delay << 8;
        if max_diff != 0 {
            delay_inc /= max_diff;
            delay_inc = delay_inc.min(0x7fff);
        }
        let d = delay_inc;
        let mut diff = 1;
        while diff <= max_diff {
            if delay_inc >= 256 {
                break;
            }
            delay_inc += d;
            diff += 1;
        }
        (delay_inc, diff)
    }

    /// `fadePalStep`: one step towards `pal`; false when there.
    pub fn fade_pal_step(&mut self, pal: &Pal, diff: i32) -> bool {
        self.fade_step(pal, diff)
    }

    fn fade_step(&mut self, pal: &Pal, diff: i32) -> bool {
        let mut fade = self.screen_palette.clone();
        let mut refresh = false;
        for i in 0..768 {
            let c1 = pal.0[i] as i32;
            let mut c2 = fade.0[i] as i32;
            if c1 != c2 {
                refresh = true;
                if c1 > c2 {
                    c2 += diff;
                    if c1 < c2 {
                        c2 = c1;
                    }
                }
                if c1 < c2 {
                    c2 -= diff;
                    if c1 > c2 {
                        c2 = c1;
                    }
                }
                fade.0[i] = c2 as u8;
            }
        }
        if refresh {
            self.set_screen_palette(&fade);
        }
        refresh
    }

    /// `fadePalette`: towards `pal` within `delay` ticks.
    pub fn fade_palette(&mut self, pal: &Pal, delay: i32) {
        self.update_screen();
        let (delay_inc, diff) = self.fade_params(pal, delay);
        let mut acc = 0;
        while !self.system.should_quit() {
            acc += delay_inc;
            let refreshed = self.fade_step(pal, diff);
            if !refreshed {
                break;
            }
            self.system.delay_millis(((acc >> 8) * 1000 / 60) as u32);
            acc &= 0xff;
        }
        if self.system.should_quit() {
            self.set_screen_palette(pal);
        }
    }

    /// `fadePalette` with an update function (`UpdateFunctor`), called once after each step,
    /// the last one as well.
    pub fn fade_palette_with(&mut self, pal: &Pal, delay: i32, f: &mut dyn FnMut(&mut Screen)) {
        self.update_screen();
        let (delay_inc, diff) = self.fade_params(pal, delay);
        let mut acc = 0;
        while !self.system.should_quit() {
            acc += delay_inc;
            let refreshed = self.fade_step(pal, diff);
            f(self);
            if !refreshed {
                break;
            }
            self.system.delay_millis(((acc >> 8) * 1000 / 60) as u32);
            acc &= 0xff;
        }
        if self.system.should_quit() {
            self.set_screen_palette(pal);
        }
    }

    /// `copyToPage0`: the lines that changed since the last call onto the screen.
    pub fn copy_to_page0(&mut self, y: i32, h: i32, page: usize, seq_buf: &mut [u8]) {
        let src = self.page(page).to_vec();
        let dst = self.page_mut(0);
        for r in 0..h {
            for x in 0..SCREEN_W {
                let at = ((y + r) as usize) * SCREEN_W + x;
                let sb = r as usize * SCREEN_W + x;
                if sb < seq_buf.len() && at < src.len() && seq_buf[sb] != src[at] {
                    seq_buf[sb] = src[at];
                    dst[at] = src[at];
                }
            }
        }
    }

    pub fn fade_from_black(&mut self, delay: i32) {
        let p = self.palettes[0].clone();
        self.fade_palette(&p, delay);
    }

    /// `Screen_LoL::fadeToBlack`.
    pub fn fade_to_black(&mut self, delay: i32) {
        self.fade_palette(&Pal::default(), delay);
        self.fade_flag = 2;
    }

    /// `fadeToPalette1`.
    pub fn fade_to_palette1(&mut self, delay: i32) {
        let mut p = self.palettes[1].clone();
        self.load_special_colors(&mut p);
        self.palettes[1] = p.clone();
        self.fade_palette(&p, delay);
        self.fade_flag = 0;
    }

    /// `loadSpecialColors`: colours 192–195 of the screen's palette.
    pub fn load_special_colors(&self, dst: &mut Pal) {
        dst.copy_from(&self.screen_palette, 192, 4, 192);
    }

    /// `fadeClearSceneWindow`.
    pub fn fade_clear_scene_window(&mut self, delay: i32) {
        if self.fade_flag == 1 {
            return;
        }
        let mut tpal = Pal::default();
        tpal.copy_from(&self.palettes[0], 128, 128, 128);
        self.load_special_colors(&mut tpal);
        self.fade_palette(&tpal, delay);
        self.fill_rect(112, 0, 288, 120, 0, None);
        self.fade_flag = 1;
    }

    /// `copyColor`: a colour of the screen's palette onto another, at once.
    pub fn copy_color(&mut self, dst: usize, src: usize) {
        let rgb: [u8; 3] = self.screen_palette.0[src * 3..src * 3 + 3]
            .try_into()
            .unwrap();
        self.screen_palette.0[dst * 3..dst * 3 + 3].copy_from_slice(&rgb);
        self.system.set_palette(&rgb, dst);
    }

    /// `Screen_LoL::fadeColor`.
    pub fn fade_color(
        &mut self,
        dst_index: usize,
        src_index: usize,
        elapsed: u32,
        total: u32,
    ) -> bool {
        let mut res = false;
        let mut entry = [0u8; 3];
        for (i, e) in entry.iter_mut().enumerate() {
            let src = self.screen_palette.0[src_index * 3 + i];
            let dst = self.screen_palette.0[dst_index * 3 + i];
            let out = if elapsed < total {
                let s = (src & 0x3f) as i32;
                let d = (dst & 0x3f) as i32;
                let o = s - d;
                if o != 0 {
                    res = true;
                }
                d + ((((o << 8) / total as i32) * elapsed as i32) >> 8)
            } else {
                self.palettes[1].0[dst_index * 3 + i] = src;
                res = false;
                src as i32
            };
            *e = out as u8;
        }
        let mut p = self.screen_palette.clone();
        p.0[dst_index * 3..dst_index * 3 + 3].copy_from_slice(&entry);
        self.set_screen_palette(&p);
        self.update_screen();
        res
    }

    /// `Screen_v2::timedPaletteFadeStep`.
    pub fn timed_palette_fade_step(
        &mut self,
        pal1: &[u8],
        pal2: &[u8],
        elapsed: u32,
        total: u32,
    ) -> bool {
        let mut res = false;
        let mut out = Pal::default();
        for i in 0..768 {
            out.0[i] = if elapsed < total {
                let d = (pal2[i] & 0x3f) as i32 - (pal1[i] & 0x3f) as i32;
                if d != 0 {
                    res = true;
                }
                let val = (((d << 8) / total as i32) * elapsed as i32) >> 8;
                ((pal1[i] & 0x3f) as i32 + val as i8 as i32) as u8
            } else {
                let v = pal2[i] & 0x3f;
                self.palettes[1].0[i] = v;
                res = false;
                v
            };
        }
        self.set_screen_palette(&out);
        self.update_screen();
        res
    }

    /// `generateFadeTable`: `num` palettes stepping from `src1` (the screen's) to `src2`.
    pub fn generate_fade_table(&self, src1: Option<&Pal>, src2: &Pal, num: usize) -> Vec<Pal> {
        let src1 = src1.unwrap_or(&self.screen_palette).clone();
        let mut out = vec![Pal::default(); num.max(2)];
        let diff: Vec<i8> = (0..768)
            .map(|i| (src2.0[i] as i8).wrapping_sub(src1.0[i] as i8))
            .collect();
        let mut t: i16 = 0;
        let d = (256 / num.max(1)) as i16;
        for p in out.iter_mut().take(num.max(2) - 1).skip(1) {
            t += d;
            for i in 0..768 {
                p.0[i] = ((((diff[i] as i16) * t) >> 8) + src1.0[i] as i8 as i16) as u8;
            }
        }
        out[0] = src1;
        let last = out.len() - 1;
        out[last] = src2.clone();
        out
    }

    // ---- copying and filling

    pub fn copy_region(
        &mut self,
        mut x1: i32,
        mut y1: i32,
        mut x2: i32,
        mut y2: i32,
        mut w: i32,
        mut h: i32,
        src_page: usize,
        dst_page: usize,
        flags: i32,
    ) {
        let sw = SCREEN_W as i32;
        let sh = SCREEN_H as i32;
        if x2 < 0 {
            if x2 <= -w {
                return;
            }
            w += x2;
            x1 -= x2;
            x2 = 0;
        } else if x2 + w >= sw {
            if x2 > sw {
                return;
            }
            w = sw - x2;
        }
        if y2 < 0 {
            if y2 <= -h {
                return;
            }
            h += y2;
            y1 -= y2;
            y2 = 0;
        } else if y2 + h >= sh {
            if y2 > sh {
                return;
            }
            h = sh - y2;
        }
        if w <= 0 || h <= 0 || x1 < 0 || y1 < 0 {
            return;
        }
        let transparent = flags & CR_NO_P_CHECK == 0;
        let copy_row = |src: &[u8], dst: &mut [u8], s: usize, d: usize, w: usize| {
            for i in 0..w {
                let (Some(&v), true) = (src.get(s + i), d + i < dst.len()) else {
                    continue;
                };
                if !transparent || v != 0 {
                    dst[d + i] = v;
                }
            }
        };
        if Self::same_buffer(src_page, dst_page) {
            if x1 == x2 && y1 == y2 {
                return;
            }
            // Within one buffer the original goes from the top line down: `memmove` a line
            // without the check for colour 0, pixel by pixel from the left with it.
            let page = self.page_mut(src_page);
            for r in 0..h {
                let s = ((y1 + r) * sw + x1) as usize;
                let d = ((y2 + r) * sw + x2) as usize;
                if transparent {
                    for i in 0..w as usize {
                        let (Some(&v), true) = (page.get(s + i), d + i < page.len()) else {
                            continue;
                        };
                        if v != 0 {
                            page[d + i] = v;
                        }
                    }
                } else {
                    let line: Vec<u8> = page
                        .get(s..s + w as usize)
                        .map(<[u8]>::to_vec)
                        .unwrap_or_default();
                    copy_row(&line, page, 0, d, line.len());
                }
            }
            return;
        }
        let Some((src, dst)) = self.two_pages(src_page, dst_page) else {
            return;
        };
        for r in 0..h {
            let s = ((y1 + r) * sw + x1) as usize;
            let d = ((y2 + r) * sw + x2) as usize;
            copy_row(src, dst, s, d, w as usize);
        }
    }

    pub fn copy_page(&mut self, src: usize, dst: usize) {
        if Self::same_buffer(src, dst) {
            return;
        }
        if let Some((s, d)) = self.two_pages(src, dst) {
            d.copy_from_slice(s);
        }
    }

    pub fn copy_region_to_buffer(&self, page: usize, x: i32, y: i32, w: i32, h: i32) -> Vec<u8> {
        let mut out = vec![0u8; (w.max(0) * h.max(0)) as usize];
        for r in 0..h {
            for c in 0..w {
                out[(r * w + c) as usize] = self.get_page_pixel(page, x + c, y + r);
            }
        }
        out
    }

    pub fn copy_block_to_page(&mut self, page: usize, x: i32, y: i32, w: i32, h: i32, src: &[u8]) {
        for r in 0..h {
            for c in 0..w {
                if let Some(&v) = src.get((r * w + c) as usize) {
                    self.set_page_pixel(page, x + c, y + r, v);
                }
            }
        }
    }

    /// `fillRect`: inclusive corners, on the current page unless given.
    pub fn fill_rect(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u8,
        page: Option<usize>,
    ) {
        let page = page.unwrap_or(self.cur_page);
        let x1 = x1.max(0);
        let y1 = y1.max(0);
        let x2 = x2.min(SCREEN_W as i32 - 1);
        let y2 = y2.min(SCREEN_H as i32 - 1);
        if x1 > x2 || y1 > y2 {
            return;
        }
        let p = self.page_mut(page);
        for y in y1..=y2 {
            let at = y as usize * SCREEN_W;
            p[at + x1 as usize..=at + x2 as usize].fill(color);
        }
    }

    pub fn fill_rect_xor(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        color: u8,
        page: Option<usize>,
    ) {
        let page = page.unwrap_or(self.cur_page);
        for y in y1.max(0)..=y2.min(199) {
            for x in x1.max(0)..=x2.min(319) {
                let v = self.get_page_pixel(page, x, y) ^ color;
                self.set_page_pixel(page, x, y, v);
            }
        }
    }

    fn draw_line(&mut self, vertical: bool, x: i32, y: i32, length: i32, color: u8) {
        let page = self.cur_page;
        for i in 0..length {
            if vertical {
                self.set_page_pixel(page, x, y + i, color);
            } else {
                self.set_page_pixel(page, x + i, y, color);
            }
        }
    }

    pub fn draw_clipped_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: u8) {
        let x1 = x1.clamp(0, 319);
        let x2 = x2.clamp(0, 319);
        let y1 = y1.clamp(0, 199);
        let y2 = y2.clamp(0, 199);
        if x1 == x2 {
            if y1 > y2 {
                self.draw_line(true, x1, y2, y1 - y2 + 1, color);
            } else {
                self.draw_line(true, x1, y1, y2 - y1 + 1, color);
            }
        } else if x1 > x2 {
            self.draw_line(false, x2, y1, x1 - x2 + 1, color);
        } else {
            self.draw_line(false, x1, y1, x2 - x1 + 1, color);
        }
    }

    pub fn draw_box(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: u8) {
        self.draw_clipped_line(x1, y1, x2, y1, color);
        self.draw_clipped_line(x1, y1, x1, y2, color);
        self.draw_clipped_line(x2, y1, x2, y2, color);
        self.draw_clipped_line(x1, y2, x2, y2, color);
    }

    /// `Screen_LoL::drawShadedBox`.
    pub fn draw_shaded_box(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color1: u8, color2: u8) {
        self.fill_rect(x1, y1, x2, y1 + 1, color1, None);
        self.fill_rect(x1, y1, x1 + 1, y2, color1, None);
        self.draw_clipped_line(x2, y1, x2, y2, color2);
        self.draw_clipped_line(x2 - 1, y1 + 1, x2 - 1, y2 - 1, color2);
        self.draw_clipped_line(x1 + 1, y2 - 1, x2, y2 - 1, color2);
        self.draw_clipped_line(x1, y2, x2, y2, color2);
    }

    /// `drawGridBox`: every other pixel, as a chessboard. The original starts each line at
    /// `(x + y + line) & 1` after cutting the box to the screen, which always marks the pixels
    /// whose `x + y` is even.
    pub fn draw_grid_box(&mut self, x: i32, y: i32, w: i32, h: i32, col: u8) {
        if w <= 0 || x >= 320 || h <= 0 || y >= 200 {
            return;
        }
        let page = self.cur_page;
        for r in 0..h {
            for c in 0..w {
                let (px, py) = (x + c, y + r);
                if (px + py) & 1 != 0 {
                    continue;
                }
                self.set_page_pixel(page, px, py, col);
            }
        }
    }

    pub fn apply_overlay(&mut self, x: i32, y: i32, w: i32, h: i32, page: usize, overlay: &[u8]) {
        for r in 0..h {
            for c in 0..w {
                let v = self.get_page_pixel(page, x + c, y + r);
                let n = overlay.get(v as usize).copied().unwrap_or(v);
                self.set_page_pixel(page, x + c, y + r, n);
            }
        }
    }

    /// `crossFadeRegion`: the region of one page onto another, pixel by pixel in a random
    /// order: the columns and the lines shuffled (the tables kept at the start of page 14), a
    /// line of the region at least 3 ms, the screen updated every tenth line.
    pub fn cross_fade_region(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        w: i32,
        h: i32,
        src: usize,
        dst: usize,
        rnd: &mut crate::util::Rng,
    ) {
        if src > 13 || dst > 13 || w <= 0 || h <= 0 {
            return;
        }
        self.hide_mouse();
        let mut wb: Vec<u16> = (0..w as u16).collect();
        let mut hb: Vec<u8> = (0..h).map(|i| i as u8).collect();
        for i in 0..w as usize {
            let j = rnd.get((w - 1) as u32) as usize;
            wb.swap(j, i);
        }
        for i in 0..h as usize {
            let j = rnd.get((h - 1) as u32) as usize;
            hb.swap(j, i);
        }
        {
            let tmp = self.page_mut(14);
            for (i, v) in wb.iter().enumerate() {
                if let Some(b) = tmp.get_mut(i * 2..i * 2 + 2) {
                    b.copy_from_slice(&v.to_le_bytes());
                }
            }
            for (i, &v) in hb.iter().enumerate() {
                if let Some(b) = tmp.get_mut(640 + i) {
                    *b = v;
                }
            }
        }
        for i in 0..h as usize {
            let mut ih = i;
            let end = self.system.millis() + 3;
            for &c in &wb {
                let (c, r) = (c as i32, hb[ih] as i32);
                ih += 1;
                if ih >= h as usize {
                    ih = 0;
                }
                let v = self.get_page_pixel(src, x1 + c, y1 + r);
                self.set_page_pixel(dst, x2 + c, y2 + r, v);
            }
            if i % 10 == 0 {
                self.update_screen();
            }
            let cur = self.system.millis();
            if end > cur {
                self.system.delay_millis(end - cur);
            }
        }
        self.update_screen();
        self.show_mouse();
    }

    // ---- figures

    /// `drawShape` into a page.
    pub fn draw_shape(
        &mut self,
        page: usize,
        shape_data: &[u8],
        x: i32,
        y: i32,
        sd: usize,
        flags: i32,
        args: ShapeArgs,
    ) {
        let dim = self.dim(sd);
        let p = self.page_mut(page);
        shape::draw_shape(p, shape_data, x, y, dim, flags, args);
    }

    // ---- bitmaps and palettes from files

    /// `loadBitmap`: a picture file (with its header) unpacked into `dst_page`; its palette (if
    /// any) into `pal`.
    pub fn load_bitmap(&mut self, bytes: &[u8], dst_page: usize, pal: Option<&mut Pal>) {
        if bytes.len() < 10 {
            return;
        }
        let comp = bytes[2];
        let size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let pal_size = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        if let Some(pal) = pal
            && pal_size > 0
        {
            pal.load(&bytes[10..(10 + pal_size).min(bytes.len())]);
        }
        let body = bytes.get(10 + pal_size..).unwrap_or(&[]);
        let data = match comp {
            0 => body.to_vec(),
            3 => lol_data::compression::rle("Bild", body, size).unwrap_or_default(),
            4 => lcw("Bild", body, size).unwrap_or_default(),
            _ => Vec::new(),
        };
        // The page is cleared first.
        let p = self.page_mut(dst_page);
        p.fill(0);
        let n = data.len().min(p.len());
        p[..n].copy_from_slice(&data[..n]);
    }

    // ---- text

    pub fn load_font(&mut self, id: FontId, bytes: &[u8]) -> bool {
        match Font::load(bytes) {
            Some(f) => {
                self.fonts[id as usize] = Some(f);
                true
            }
            None => false,
        }
    }

    pub fn set_font(&mut self, id: FontId) -> FontId {
        std::mem::replace(&mut self.current_font, id)
    }

    fn font(&self) -> Option<&Font> {
        self.fonts[self.current_font as usize].as_ref()
    }

    pub fn font_height(&self) -> i32 {
        self.font().map_or(8, |f| f.height as i32)
    }

    pub fn font_width(&self) -> i32 {
        self.font().map_or(8, |f| f.width as i32)
    }

    pub fn char_width(&self, c: u8) -> i32 {
        self.font().map_or(0, |f| f.char_width(c)) + self.char_spacing
    }

    /// `getTextWidth`: the widest line (lines end at `\r`).
    pub fn text_width(&self, s: &[u8]) -> i32 {
        let mut cur = 0;
        let mut max = 0;
        for &c in s {
            if c == 0 {
                break;
            }
            if c == b'\r' {
                if cur > max {
                    max = cur;
                } else {
                    cur = 0;
                }
            } else {
                cur += self.char_width(c);
            }
        }
        cur.max(max)
    }

    pub fn set_text_color_map(&mut self, cmap: &[u8]) {
        let n = cmap.len().min(16);
        self.text_colors[..n].copy_from_slice(&cmap[..n]);
    }

    /// `printText`.
    pub fn print_text(&mut self, s: &[u8], x: i32, y: i32, color1: u8, color2: u8) {
        self.text_colors[0] = color2;
        self.text_colors[1] = color1;
        let Some(font) = self.fonts[self.current_font as usize].clone() else {
            return;
        };
        let mut x = x.max(0);
        if x >= SCREEN_W as i32 {
            return;
        }
        let x_start = x;
        let mut y = y.max(0);
        if y >= SCREEN_H as i32 {
            return;
        }
        let height = font.height as i32;
        let map = self.text_colors;
        let page = self.cur_page;
        for &c in s {
            if c == 0 {
                break;
            }
            if c == b'\r' {
                x = x_start;
                y += height + self.line_spacing;
                continue;
            }
            let w = font.char_width(c) + self.char_spacing;
            if x + w > self.text_margin_right {
                x = x_start;
                y += height + self.line_spacing;
                if y >= SCREEN_H as i32 {
                    break;
                }
            }
            let cw = font.char_width(c);
            if x >= 0 && y >= 0 && x + cw <= SCREEN_W as i32 && y + height <= SCREEN_H as i32 {
                let at = y as usize * SCREEN_W + x as usize;
                font.draw_char(c, self.page_mut(page), at, &map);
            }
            x += w;
        }
    }

    /// `Screen_LoL::fprintString`: flags 1 centred, 2 right-aligned, 4/8 shadowed.
    pub fn fprint_string(&mut self, s: &[u8], mut x: i32, y: i32, col1: u8, col2: u8, flags: i32) {
        if flags & 1 != 0 {
            x -= self.text_width(s) >> 1;
        }
        if flags & 2 != 0 {
            x -= self.text_width(s);
        }
        if flags & 4 != 0 {
            self.print_text(s, x - 1, y, 1, col2);
            self.print_text(s, x, y + 1, 1, col2);
        }
        if flags & 8 != 0 {
            self.print_text(s, x - 1, y, 227, col2);
            self.print_text(s, x, y + 1, 227, col2);
        }
        self.print_text(s, x, y, col1, col2);
    }

    /// `fprintStringIntro`.
    pub fn fprint_string_intro(
        &mut self,
        s: &[u8],
        mut x: i32,
        y: i32,
        c1: u8,
        c2: u8,
        c3: u8,
        flags: i32,
    ) {
        if flags & 0x0f00 == 0x100 {
            x -= self.text_width(s) >> 1;
        }
        if flags & 0x0f00 == 0x200 {
            x -= self.text_width(s);
        }
        if flags & 0x00f0 == 0x20 {
            self.print_text(s, x - 1, y, c3, c2);
            self.print_text(s, x, y + 1, c3, c2);
        }
        self.print_text(s, x, y, c1, c2);
    }

    // ---- mouse

    pub fn hide_mouse(&mut self) {
        self.mouse_lock += 1;
        self.system.show_cursor(false);
    }

    pub fn show_mouse(&mut self) {
        if self.mouse_lock == 1 {
            self.system.show_cursor(true);
        }
        if self.mouse_lock > 0 {
            self.mouse_lock -= 1;
        }
    }

    pub fn is_mouse_visible(&self) -> bool {
        self.mouse_lock == 0
    }

    /// `setMouseCursor`: a figure as the pointer, its hot spot at `(x, y)`.
    pub fn set_mouse_cursor(&mut self, x: i32, y: i32, shape_data: &[u8]) {
        if shape_data.len() < 10 {
            return;
        }
        let h = shape::height(shape_data) as usize;
        let w = shape::width(shape_data) as usize + 2;
        let key = self.cursor_key;
        let mut buffer = vec![key; SCREEN_W * (h + 1)];
        let dim = Dim {
            sx: 0,
            sy: 0,
            w: 40,
            h: (h + 1) as i32,
            ..Dim::default()
        };
        shape::draw_shape(&mut buffer, shape_data, 0, 0, dim, 0, ShapeArgs::none());
        let mut pixels = Vec::with_capacity(w * h);
        for r in 0..h {
            pixels.extend_from_slice(&buffer[r * SCREEN_W..r * SCREEN_W + w]);
        }
        self.system.set_cursor(Some(Cursor {
            width: w,
            height: h,
            pixels,
            key,
            hot_x: x,
            hot_y: y,
        }));
        self.system.show_cursor(self.is_mouse_visible());
    }

    // ---- Lands of Lore's own

    /// `backupSceneWindow`: the view into the page's spare memory at `0xA500`.
    pub fn backup_scene_window(&mut self, src: usize, dst: usize) {
        let block: Vec<u8> = (0..120)
            .flat_map(|r| self.page(src)[r * 320 + 112..r * 320 + 288].to_vec())
            .collect();
        self.page_mut(dst)[0xa500..0xa500 + block.len()].copy_from_slice(&block);
    }

    pub fn restore_scene_window(&mut self, src: usize, dst: usize) {
        let block = self.page(src)[0xa500..0xa500 + 176 * 120].to_vec();
        let p = self.page_mut(dst);
        for r in 0..120 {
            p[r * 320 + 112..r * 320 + 288].copy_from_slice(&block[r * 176..(r + 1) * 176]);
        }
    }

    /// `smoothScrollZoomStepTop` / `…Bottom`: the backed-up view, its top (46 rows) or bottom
    /// (74 rows) zoomed by `(x, y)`.
    pub fn smooth_scroll_zoom_step(
        &mut self,
        src_page: usize,
        dst_page: usize,
        x: i32,
        y: i32,
        top: bool,
    ) {
        let (start, rows) = if top {
            (0xa500usize, 46i32)
        } else {
            (0xc4a0usize, 74i32)
        };
        let src_data = self.page(src_page).to_vec();
        let dst = self.page_mut(dst_page);
        let mut s = start
            + if top {
                (y * 176 + x) as usize
            } else {
                x as usize
            };
        let mut d = start;
        let x2 = x << 1;
        let width = (176 - x2) as u16;
        let scale_x = ((((x2 + 1) << 8) / width as i32) + 0x100) as u16;
        let cnt_w = (scale_x >> 8) as usize;
        // 16 bits in the original: the shift drops the whole part.
        let scale_x = (scale_x << 8) as u32;
        let width = width - 1;
        let mut height = (rows - y) as u16;
        let scale_y = ((((y + 1) << 8) / height as i32) + 0x100) as u16;
        let scale_y = (scale_y << 8) as u32;
        let mut scale_yc: u32 = 0;
        while height > 0 {
            let mut scale_xc: u32 = 0;
            for _ in 0..width {
                scale_xc += scale_x;
                let n = cnt_w + (scale_xc >> 16) as usize;
                scale_xc &= 0xffff;
                let v = src_data.get(s).copied().unwrap_or(0);
                s += 1;
                for _ in 0..n {
                    if let Some(p) = dst.get_mut(d) {
                        *p = v;
                    }
                    d += 1;
                }
            }
            if let Some(p) = dst.get_mut(d) {
                *p = src_data.get(s).copied().unwrap_or(0);
            }
            d += 1;
            s += 1;
            s += x2 as usize;
            scale_yc += scale_y;
            if scale_yc >> 16 != 0 {
                scale_yc = 0;
                s -= 176;
                continue;
            }
            height -= 1;
        }
    }

    /// `smoothScrollHorizontalStep`.
    pub fn smooth_scroll_horizontal_step(&mut self, page: usize, src_x: i32, dst_x: i32, w: i32) {
        let p = self.page_mut(page);
        let mut d: isize = 0;
        let mut s: isize = 112 + src_x as isize;
        let w2 = (src_x + w - dst_x) as isize;
        let pitch_s = 320 + w2 - (w as isize * 2);
        let pitch_d = 320 - w as isize;
        for _ in 0..120 {
            for _ in 0..w {
                p[d as usize] = p[s as usize];
                d += 1;
                s += 1;
            }
            d -= w as isize;
            s -= w2;
            for _ in 0..w {
                p[s as usize] = p[d as usize];
                s += 1;
                d += 1;
            }
            s += pitch_s;
            d += pitch_d;
        }
    }

    /// `smoothScrollTurnStep1/2/3`: the view while turning, made of the old and new view
    /// stretched.
    pub fn smooth_scroll_turn_step(
        &mut self,
        step: i32,
        src1: usize,
        src2: usize,
        dst_page: usize,
    ) {
        let a = self.page(src1).to_vec();
        let b = self.page(src2).to_vec();
        let dst = self.page_mut(dst_page);
        let mut put = |d: &mut usize, v: u8| {
            if let Some(p) = dst.get_mut(*d) {
                *p = v;
            }
            *d += 1;
        };
        match step {
            1 => {
                let (mut s, mut d) = (273usize, 0xa500usize);
                for _ in 0..120 {
                    let v = a[s];
                    s += 1;
                    put(&mut d, v);
                    put(&mut d, v);
                    for _ in 0..14 {
                        let v = a[s];
                        s += 1;
                        put(&mut d, v);
                        put(&mut d, v);
                        put(&mut d, v);
                    }
                    s += 305;
                    d += 132;
                }
                let (mut s, mut d) = (112usize, 0xa52cusize);
                for _ in 0..120 {
                    for _ in 0..33 {
                        put(&mut d, b[s]);
                        put(&mut d, b[s + 1]);
                        let v = b[s + 2];
                        put(&mut d, v);
                        put(&mut d, v);
                        s += 3;
                    }
                    s += 221;
                    d += 44;
                }
            }
            2 => {
                let (mut s, mut d) = (244usize, 0xa500usize);
                let mut src = &a;
                for k in 0..2 {
                    for _ in 0..120 {
                        for _ in 0..44 {
                            let v = src[s];
                            s += 1;
                            put(&mut d, v);
                            put(&mut d, v);
                        }
                        s += 276;
                        d += 88;
                    }
                    if k == 0 {
                        src = &b;
                        s = 112;
                        d = 0xa558;
                    }
                }
            }
            _ => {
                let (mut s, mut d) = (189usize, 0xa500usize);
                for _ in 0..120 {
                    for _ in 0..33 {
                        put(&mut d, a[s]);
                        put(&mut d, a[s + 1]);
                        let v = a[s + 2];
                        put(&mut d, v);
                        put(&mut d, v);
                        s += 3;
                    }
                    s += 221;
                    d += 44;
                }
                let (mut s, mut d) = (112usize, 0xa584usize);
                for _ in 0..120 {
                    for _ in 0..14 {
                        let v = b[s];
                        s += 1;
                        put(&mut d, v);
                        put(&mut d, v);
                        put(&mut d, v);
                    }
                    let v = b[s];
                    s += 1;
                    put(&mut d, v);
                    put(&mut d, v);
                    s += 305;
                    d += 132;
                }
            }
        }
    }

    /// `copyBlockAndApplyOverlay`.
    pub fn copy_block_and_apply_overlay(
        &mut self,
        page1: usize,
        x1: i32,
        y1: i32,
        page2: usize,
        x2: i32,
        y2: i32,
        w: i32,
        h: i32,
        dim: usize,
        ovl: &[u8],
    ) {
        let d = self.dim(dim);
        let (ix, iy) = (d.sx << 3, d.sy);
        for r in 0..h {
            for c in 0..w {
                let (dx, dy) = (x2 + c, y2 + r);
                if dx < 0 || dy < 0 || dx >= d.w << 3 || dy >= d.h {
                    continue;
                }
                let v = self.get_page_pixel(page1, x1 + c, y1 + r);
                let p = ovl.get(v as usize).copied().unwrap_or(v);
                if p != 0 {
                    self.set_page_pixel(page2, dx + ix, dy + iy, p);
                }
            }
        }
    }

    /// `applyOverlaySpecial`.
    pub fn apply_overlay_special(
        &mut self,
        page1: usize,
        x1: i32,
        y1: i32,
        page2: usize,
        x2: i32,
        y2: i32,
        w: i32,
        h: i32,
        dim: usize,
        flag: bool,
        ovl: &[u8],
    ) {
        let d = self.dim(dim);
        let (ix, iy) = (d.sx << 3, d.sy);
        for r in 0..h {
            for c in 0..w {
                let (dx, dy) = (x2 + c, y2 + r);
                if dx < 0 || dy < 0 || dx >= d.w << 3 || dy >= d.h {
                    continue;
                }
                if self.get_page_pixel(page1, x1 + c, y1 + r) == 0 {
                    continue;
                }
                let px = dx + ix + if flag { r >> 1 } else { 0 };
                let v = self.get_page_pixel(page2, px, dy + iy);
                let n = ovl.get(v as usize).copied().unwrap_or(v);
                self.set_page_pixel(page2, px, dy + iy, n);
            }
        }
    }

    /// `copyRegionSpecial`: a region from a buffer `w1×h1` wide and high into one `w2×h2`
    /// (pages used as buffers of their own size), modes 0 (copy), 1 (every other pixel), 2
    /// (through the transparency tables), 3 (mirrored).
    #[allow(clippy::too_many_arguments)]
    pub fn copy_region_special(
        &mut self,
        page1: usize,
        w1: i32,
        h1: i32,
        x1: i32,
        y1: i32,
        page2: usize,
        w2: i32,
        h2: i32,
        x2: i32,
        y2: i32,
        w3: i32,
        h3: i32,
        mode: i32,
        tables: Option<(&[u8], &[u8])>,
    ) {
        if w3 == 0 || h3 == 0 {
            return;
        }
        let src = self.page(page1).to_vec();
        let dst = self.page_mut(page2);
        for r in 0..h3 {
            let (sy, dy) = (y1 + r, y2 + r);
            if sy < 0 || sy >= h1 || dy < 0 || dy >= h2 {
                continue;
            }
            for c in 0..w3 {
                let dx = x2 + c;
                let sx = if mode == 3 { x1 + w3 - 1 - c } else { x1 + c };
                if sx < 0 || sx >= w1 || dx < 0 || dx >= w2 {
                    continue;
                }
                let s = src[(sy * w1 + sx) as usize];
                let at = (dy * w2 + dx) as usize;
                match mode {
                    1 => {
                        if (c & 1) == (r & 1) {
                            continue;
                        }
                        dst[at] = s;
                    }
                    2 => {
                        let Some((t1, t2)) = tables else { continue };
                        let offs = t1.get(s as usize).copied().unwrap_or(0xff);
                        dst[at] = if offs & 0x80 == 0 {
                            t2.get(((offs as usize) << 8) | dst[at] as usize)
                                .copied()
                                .unwrap_or(s)
                        } else {
                            s
                        };
                    }
                    _ => dst[at] = s,
                }
            }
        }
    }

    /// `clearGuiShapeMemory`.
    pub fn clear_gui_shape_memory(&mut self, page: usize) {
        let p = self.page_mut(page);
        for i in 0..23 {
            let at = 0x79B0 + i * 320;
            p[at..at + 176].fill(0);
        }
    }

    /// `copyGuiShapeFromSceneBackupBuffer`.
    pub fn copy_gui_shape_from_scene_backup_buffer(&mut self, src_page: usize, dst_page: usize) {
        let src = self.page(src_page).to_vec();
        let dst = self.page_mut(dst_page);
        let (mut s, mut d) = (0x79C3usize, 0usize);
        for _ in 0..23 {
            let mut len: u8 = 0;
            loop {
                let v = src.get(s).copied().unwrap_or(1);
                s += 1;
                len = len.wrapping_add(1);
                if v != 0 || len >= 69 {
                    break;
                }
            }
            dst[d] = len;
            d += 1;
            let l = 69usize.saturating_sub(len as usize);
            dst[d..d + l].copy_from_slice(&src[s..s + l]);
            s += l + 251;
            d += l;
        }
    }

    /// `copyGuiShapeToSurface`: the shape kept by the function above, mirrored onto both
    /// halves of the backed-up view.
    pub fn copy_gui_shape_to_surface(&mut self, src_page: usize, dst_page: usize) {
        let src = self.page(src_page).to_vec();
        let dst = self.page_mut(dst_page);
        let (mut s, mut d) = (0isize, 0xE7C3isize);
        for _ in 0..23 {
            let v = src[s as usize] as isize;
            s += 1;
            let len = 69 - v;
            d += v;
            for k in 0..len {
                dst[(d + k) as usize] = src[(s + k) as usize];
            }
            s += len - 1;
            d += len;
            for _ in 0..len {
                dst[d as usize] = src[s as usize];
                d += 1;
                s -= 1;
            }
            s += len + 1;
            d += v + 38;
        }
    }

    /// `copyBlockAndApplyOverlayOutro`: each pixel through a 64 KB table of (source,
    /// destination) pairs.
    pub fn copy_block_and_apply_overlay_outro(
        &mut self,
        src_page: usize,
        dst_page: usize,
        ovl: &[u8],
    ) {
        let src = self.page(src_page).to_vec();
        let dst = self.page_mut(dst_page);
        for i in 0..SCREEN_W * SCREEN_H {
            let offs = ((src[i] as usize) << 8) | dst[i] as usize;
            dst[i] = ovl.get(offs).copied().unwrap_or(dst[i]);
        }
    }

    /// `wsaFrameAnimationStep`: a region scaled from `w1×h1` to `w2×h2` into a window.
    pub fn wsa_frame_animation_step(
        &mut self,
        x1: i32,
        y1: i32,
        x2: i32,
        y2: i32,
        w1: i32,
        h1: i32,
        w2: i32,
        h2: i32,
        src_page: usize,
        dst_page: usize,
        dim: usize,
    ) {
        if w1 == 0 || h1 == 0 || w2 == 0 || h2 == 0 {
            return;
        }
        let d = self.dim(dim);
        let (sx, sy) = (d.sx << 3, d.sy);
        let (mut x2, mut y2, mut w2, mut h2) = (x2, y2, w2, h2);
        let Some((na, mut nb)) = calc_bounds(d.w << 3, d.h, &mut x2, &mut y2, &mut w2, &mut h2)
        else {
            return;
        };
        let need = (w1.max(w2) as usize + na.max(0) as usize) * 2 + 1024;
        if self.wsa_buffer.len() < need {
            self.wsa_buffer.resize(need, 0);
        }
        let src = self.page(src_page).to_vec();
        let mut buf = std::mem::take(&mut self.wsa_buffer);
        let byte = |i: i64| -> u8 {
            usize::try_from(i)
                .ok()
                .and_then(|i| src.get(i).copied())
                .unwrap_or(0)
        };
        let mut row = (y2 + sy) as i64 * 320;
        let mut u = -1;
        loop {
            let t = (nb * h1) / h2;
            if t != u {
                u = t;
                let mut s = (y1 as i64 + t as i64) * 320 + x1 as i64;
                let mut dt = 0usize;
                let diff = w2 - w1;
                if diff == 0 {
                    for k in 0..w2 as usize {
                        buf[k] = byte(s + k as i64);
                    }
                } else if diff > 0 {
                    if w1 == 1 {
                        let v = byte(s);
                        buf[..w2 as usize].fill(v);
                    } else {
                        let t = (((((w2 - w1 + 1) & 0xFFFF) << 8) / w1 + 0x100) & 0xFFFF) << 8;
                        let mut bp = 0;
                        for _ in 0..w1 {
                            let mut cnt = (t >> 16) as usize;
                            bp += t & 0xFFFF;
                            if bp > 0xFFFF {
                                bp -= 0xFFFF;
                                cnt += 1;
                            }
                            let v = byte(s);
                            s += 1;
                            let end = (dt + cnt).min(buf.len());
                            buf[dt..end].fill(v);
                            dt = end;
                        }
                    }
                } else if w2 == 1 {
                    buf[0] = byte(s);
                } else {
                    let t = (((((w1 - w2) & 0xFFFF) << 8) / w2) & 0xFFFF) << 8;
                    let mut bp = 0;
                    for _ in 0..w2 {
                        buf[dt] = byte(s);
                        dt += 1;
                        s += 1;
                        bp += t & 0xFFFF;
                        if bp > 0xFFFF {
                            bp -= 0xFFFF;
                            s += 1;
                        }
                        s += (t >> 16) as i64;
                    }
                }
            }
            let at = row + (x2 + sx) as i64;
            let dst = self.page_mut(dst_page);
            for k in 0..w2 as i64 {
                if let Ok(i) = usize::try_from(at + k)
                    && let Some(p) = dst.get_mut(i)
                {
                    *p = buf.get(na as usize + k as usize).copied().unwrap_or(0);
                }
            }
            row += 320;
            nb += 1;
            if nb >= h2 {
                break;
            }
        }
        self.wsa_buffer = buf;
    }

    /// `createTransparencyTablesIntern`.
    pub fn create_transparency_tables_intern(
        ovl: &[u8],
        a: usize,
        fx_pal1: &[u8],
        fx_pal2: &[u8],
        b: i32,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut t1 = vec![0xFFu8; 256];
        let mut t2 = vec![0u8; 5120.max(a << 8)];
        let screen_pal = Pal(fx_pal2.to_vec());
        for (i, &o) in ovl.iter().enumerate().take(a) {
            t1[o as usize] = i as u8;
        }
        for i in 0..a {
            let c = ovl[i] as usize;
            if c == 0 {
                continue;
            }
            let k1 = ((b << 6) / 100) as u16;
            let k2 = 64 - k1;
            let f = [
                fx_pal1[3 * c] as u16,
                fx_pal1[3 * c + 1] as u16,
                fx_pal1[3 * c + 2] as u16,
            ];
            for ii in 0..256 {
                let s = [
                    screen_pal.0[3 * ii] as u16,
                    screen_pal.0[3 * ii + 1] as u16,
                    screen_pal.0[3 * ii + 2] as u16,
                ];
                let t: Vec<u8> = (0..3)
                    .map(|j| (((f[j] * k2) >> 6) + ((s[j] * k1) >> 6)).min(63) as u8)
                    .collect();
                t2[(i << 8) + ii] =
                    crate::lol::magic::find_least_different_color(&t, &screen_pal, 0, 255, false);
            }
        }
        (t1, t2)
    }

    /// `shakeScreen`.
    pub fn shake_screen(&mut self, times: i32) {
        const SHAKE: [(u32, i32, i32); 2] = [(32, 0, -4), (32, 0, 0)];
        for _ in 0..times {
            for (ms, x, y) in SHAKE {
                self.system.set_shake(x, y);
                self.system.delay_millis(ms);
            }
        }
    }
}

/// `Screen_v2::calcBounds`: a block `(x1, y1, w1, h1)` cut to a window `w0 × h0`; returns
/// how much was cut off on the left and at the top, `None` when nothing is left. As in the
/// original the left and top are only cut for an empty block.
fn calc_bounds(
    w0: i32,
    h0: i32,
    x1: &mut i32,
    y1: &mut i32,
    w1: &mut i32,
    h1: &mut i32,
) -> Option<(i32, i32)> {
    let (mut x2, mut y2) = (0, 0);
    let t = *x1 + *w1;
    if t < 1 {
        return None;
    }
    if t <= *x1 {
        x2 = *w1 - t;
        *w1 = t;
        *x1 = 0;
    }
    let t = w0 - *x1;
    if t < 1 {
        return None;
    }
    if t <= *w1 {
        *w1 = t;
    }
    let t = *h1 + *y1;
    if t < 1 {
        return None;
    }
    if t <= *y1 {
        y2 = *h1 - t;
        *h1 = t;
        *y1 = 0;
    }
    let t = h0 - *y1;
    if t < 1 {
        return None;
    }
    if t <= *h1 {
        *h1 = t;
    }
    Some((x2, y2))
}

/// `Screen_v2::generateOverlay`: each colour moved `weight`/255 towards `op_color`, mapped
/// to the nearest other colour up to `max_color`.
pub fn generate_overlay(pal: &Pal, op_color: usize, weight: u32, max_color: usize) -> Vec<u8> {
    let weight = (weight.min(255) >> 1) as i32;
    let p = |i: usize| pal.0.get(i).copied().unwrap_or(0) as i32;
    let (op_r, op_g, op_b) = (p(op_color * 3), p(op_color * 3 + 1), p(op_color * 3 + 2));
    let mut out = vec![0u8; 256];
    for i in 1..256 {
        let cur_r = (p(i * 3) - (((p(i * 3) - op_r) * weight) >> 7)) as u8 as i32;
        let cur_g = (p(i * 3 + 1) - (((p(i * 3 + 1) - op_g) * weight) >> 7)) as u8 as i32;
        let cur_b = (p(i * 3 + 2) - (((p(i * 3 + 2) - op_b) * weight) >> 7)) as u8 as i32;
        let mut idx_sum: u16 = 0x7FFF;
        let mut index = op_color as u8;
        for cur in 1..=max_color {
            if i == cur {
                continue;
            }
            let mut sum: u16 = 0;
            for (c, v) in [cur_r, cur_g, cur_b].into_iter().enumerate() {
                let d = p(cur * 3 + c) - v;
                sum = sum.wrapping_add((d * d) as u16);
            }
            if sum == 0 {
                index = cur as u8;
                break;
            }
            if sum <= idx_sum {
                idx_sum = sum;
                index = cur as u8;
            }
        }
        out[i] = index;
    }
    out
}
