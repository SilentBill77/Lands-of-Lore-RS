//! The animation player (ScummVM's `WSAMovie_v2` with `WSAMovie_v1::displayFrame`): frames
//! as LCW-packed XOR deltas, decoded into a buffer of their own (or straight onto a page) and
//! copied onto a page through one of `copyWsaRect`'s plot functions (0 copy, 1 through the
//! transparency tables, 4 leaving colour 0 out, 5 both).

use lol_data::compression::lcw;

use crate::screen::{Pal, SCREEN_W, Screen};

const WF_OFFSCREEN_DECODE: u16 = 0x10;
const WF_NO_LAST_FRAME: u16 = 0x20;
const WF_NO_FIRST_FRAME: u16 = 0x40;
const WF_HAS_PALETTE: u16 = 0x100;
const WF_XOR: u16 = 0x200;

pub struct Wsa {
    pub num_frames: i32,
    pub x_add: i32,
    pub y_add: i32,
    pub width: i32,
    pub height: i32,
    flags: u16,
    delta: Vec<u8>,
    offscreen: Vec<u8>,
    frame_offsets: Vec<usize>,
    frame_data: Vec<u8>,
    current_frame: i32,
    pub palette: Option<Pal>,
}

/// `decodeFrameDelta` / `decodeFrameDeltaPage`: an XOR delta onto a block of `width` pixels a
/// line, its lines `stride` apart, from `start`.
fn decode_delta(
    dst: &mut [u8],
    start: usize,
    width: usize,
    stride: usize,
    src: &[u8],
    no_xor: bool,
) {
    let mut s = 0usize;
    let mut count = 0usize;
    let mut line = start;
    let mut d = start;
    let byte = |i: usize| src.get(i).copied().unwrap_or(0);
    let put = |dst: &mut [u8], d: &mut usize, count: &mut usize, line: &mut usize, v: u8| {
        if let Some(p) = dst.get_mut(*d) {
            if no_xor {
                *p = v;
            } else {
                *p ^= v;
            }
        }
        *d += 1;
        *count += 1;
        if *count == width {
            *count = 0;
            *line += stride;
            *d = *line;
        }
    };
    let skip = |d: &mut usize, count: &mut usize, line: &mut usize, n: usize| {
        *d += n;
        *count += n;
        while *count >= width {
            *count -= width;
            *line += stride;
            *d = *line + *count;
        }
    };
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 2_000_000 || s >= src.len() {
            break;
        }
        let code = byte(s);
        s += 1;
        if code == 0 {
            let len = byte(s) as usize;
            let v = byte(s + 1);
            s += 2;
            for _ in 0..len {
                put(dst, &mut d, &mut count, &mut line, v);
            }
        } else if code & 0x80 != 0 {
            let c = (code - 0x80) as usize;
            if c != 0 {
                skip(&mut d, &mut count, &mut line, c);
            } else {
                let sub = byte(s) as usize | (byte(s + 1) as usize) << 8;
                s += 2;
                if sub == 0 {
                    break;
                } else if sub & 0x8000 != 0 {
                    let sub = sub - 0x8000;
                    if sub & 0x4000 != 0 {
                        let len = sub - 0x4000;
                        let v = byte(s);
                        s += 1;
                        for _ in 0..len {
                            put(dst, &mut d, &mut count, &mut line, v);
                        }
                    } else {
                        for _ in 0..sub {
                            let v = byte(s);
                            s += 1;
                            put(dst, &mut d, &mut count, &mut line, v);
                        }
                    }
                } else {
                    skip(&mut d, &mut count, &mut line, sub);
                }
            }
        } else {
            for _ in 0..code {
                let v = byte(s);
                s += 1;
                put(dst, &mut d, &mut count, &mut line, v);
            }
        }
    }
}

impl Wsa {
    /// `WSAMovie_v2::open`: `offscreen` unless `unk1 & 2` (the animation decoded straight
    /// onto the page).
    pub fn open(bytes: &[u8], unk1: i32) -> Option<Self> {
        if bytes.len() < 14 {
            return None;
        }
        let w = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let mut num_frames = w(0) as i32;
        let x_add = w(2) as i16 as i32;
        let y_add = w(4) as i16 as i32;
        let width = w(6) as i32;
        let height = w(8) as i32;
        let delta_size = w(10) as usize;
        let file_flags = w(12);
        let mut pos = 14;
        let mut flags = 0u16;
        let mut offs_pal = 0;
        let mut palette = None;
        if file_flags & 1 != 0 {
            offs_pal = 0x300;
            flags |= WF_HAS_PALETTE;
            let at = pos + 8 + ((num_frames << 2) & 0xFFFF) as usize;
            if let Some(p) = bytes.get(at..at + 0x300) {
                palette = Some(Pal::from_vga(p));
            }
        }
        if file_flags & 2 != 0 {
            flags |= WF_XOR;
        }
        if unk1 & 2 == 0 {
            flags |= WF_OFFSCREEN_DECODE;
        }
        if num_frames & 0x8000 != 0 {
            flags |= 0x80;
            num_frames &= 0x7FFF;
        }
        let rd = |i: usize| -> usize {
            bytes
                .get(i..i + 4)
                .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        };
        let mut offsets = vec![0usize; num_frames as usize + 2];
        let mut frame_data_offs = rd(pos);
        pos += 4;
        let mut first_frame = true;
        if frame_data_offs == 0 {
            first_frame = false;
            frame_data_offs = rd(pos);
            flags |= WF_NO_FIRST_FRAME;
        }
        for o in offsets.iter_mut().skip(1) {
            let v = rd(pos);
            *o = if v != 0 {
                v.wrapping_sub(frame_data_offs)
            } else {
                0
            };
            pos += 4;
        }
        if offsets[num_frames as usize + 1] == 0 {
            flags |= WF_NO_LAST_FRAME;
        }
        pos += offs_pal;
        let frame_data = bytes.get(pos..).unwrap_or(&[]).to_vec();
        let mut wsa = Self {
            num_frames,
            x_add,
            y_add,
            width,
            height,
            flags,
            delta: vec![0; delta_size.max(1)],
            offscreen: if flags & WF_OFFSCREEN_DECODE != 0 {
                vec![0; (width * height).max(0) as usize]
            } else {
                Vec::new()
            },
            frame_offsets: offsets,
            frame_data,
            current_frame: num_frames,
            palette,
        };
        if first_frame {
            wsa.delta =
                lcw("WSA", &wsa.frame_data, delta_size).unwrap_or_else(|_| vec![0; delta_size]);
        }
        Some(wsa)
    }

    pub fn frames(&self) -> i32 {
        self.num_frames
    }

    fn process_frame(&mut self, frame: i32, screen: &mut Screen, page: usize, x: i32, y: i32) {
        let off = self.frame_offsets.get(frame as usize).copied().unwrap_or(0);
        let src = self.frame_data.get(off..).unwrap_or(&[]);
        let size = self.delta.len();
        self.delta = lcw("WSA", src, size).unwrap_or_else(|_| vec![0; size]);
        if self.flags & WF_OFFSCREEN_DECODE != 0 {
            let w = self.width.max(1) as usize;
            decode_delta(&mut self.offscreen, 0, w, w, &self.delta, false);
        } else {
            let start = (y.max(0) as usize) * SCREEN_W + x.max(0) as usize;
            decode_delta(
                screen.page_mut(page),
                start,
                self.width.max(1) as usize,
                SCREEN_W,
                &self.delta,
                false,
            );
        }
    }

    /// `displayFrame` (with `WSAMovie_v2`'s offset): the frame onto page `page` at `(x, y)`;
    /// `flags >> 12` the plot function.
    pub fn display_frame(
        &mut self,
        screen: &mut Screen,
        frame: i32,
        page: usize,
        x: i32,
        y: i32,
        flags: u16,
        table1: Option<&[u8]>,
        table2: Option<&[u8]>,
    ) {
        let (x, y) = (x + self.x_add, y + self.y_add);
        if frame >= self.num_frames || frame < 0 {
            return;
        }
        if self.current_frame == self.num_frames {
            if self.flags & WF_NO_FIRST_FRAME == 0 {
                if self.flags & WF_OFFSCREEN_DECODE != 0 {
                    let w = self.width.max(1) as usize;
                    decode_delta(&mut self.offscreen, 0, w, w, &self.delta, false);
                } else {
                    let start = (y.max(0) as usize) * SCREEN_W + x.max(0) as usize;
                    let no_xor = self.flags & WF_XOR == 0;
                    decode_delta(
                        screen.page_mut(page),
                        start,
                        self.width.max(1) as usize,
                        SCREEN_W,
                        &self.delta,
                        no_xor,
                    );
                }
            }
            self.current_frame = 0;
        }
        let diff = (self.current_frame - frame).abs();
        let mut step = 1;
        let mut count;
        if self.current_frame < frame {
            count = self.num_frames - frame + self.current_frame;
            if diff > count && self.flags & WF_NO_LAST_FRAME == 0 {
                step = -1;
            } else {
                count = diff;
            }
        } else {
            count = self.num_frames - self.current_frame + frame;
            if count >= diff || self.flags & WF_NO_LAST_FRAME != 0 {
                step = -1;
                count = diff;
            }
        }
        let mut cf = self.current_frame;
        if step > 0 {
            for _ in 0..count {
                cf += step;
                self.process_frame(cf, screen, page, x, y);
                if cf == self.num_frames {
                    cf = 0;
                }
            }
        } else {
            for _ in 0..count {
                if cf == 0 {
                    cf = self.num_frames;
                }
                self.process_frame(cf, screen, page, x, y);
                cf += step;
            }
        }
        self.current_frame = frame;
        if self.flags & WF_OFFSCREEN_DECODE != 0 {
            let plot = ((flags & 0xFF00) >> 12) as i32;
            self.copy_wsa_rect(screen, page, x, y, plot, table1, table2);
        }
    }

    /// `copyWsaRect` into window 0 (the whole screen).
    fn copy_wsa_rect(
        &self,
        screen: &mut Screen,
        page: usize,
        x: i32,
        y: i32,
        plot: i32,
        t1: Option<&[u8]>,
        t2: Option<&[u8]>,
    ) {
        let (w, h) = (self.width, self.height);
        for r in 0..h {
            let py = y + r;
            if !(0..200).contains(&py) {
                continue;
            }
            for c in 0..w {
                let px = x + c;
                if !(0..320).contains(&px) {
                    continue;
                }
                let d = self.offscreen[(r * w + c) as usize];
                let at = py as usize * SCREEN_W + px as usize;
                let dst = screen.page(page)[at];
                let v = match plot {
                    0 => Some(d),
                    1 => Some(blend(d, dst, t1, t2)),
                    4 => (d != 0).then_some(d),
                    5 => (d != 0).then(|| blend(d, dst, t1, t2)),
                    // The layer kinds (8, 9, 12, 13) belong to the Kyrandia games; any other
                    // kind draws nothing (`default: break`).
                    _ => None,
                };
                if let Some(v) = v {
                    screen.page_mut(page)[at] = v;
                }
            }
        }
    }
}

fn blend(d: u8, dst: u8, t1: Option<&[u8]>, t2: Option<&[u8]>) -> u8 {
    match (t1, t2) {
        (Some(t1), Some(t2)) => {
            let t = t1.get(d as usize).copied().unwrap_or(0xFF);
            if t != 0xFF {
                t2.get(dst as usize + ((t as usize) << 8))
                    .copied()
                    .unwrap_or(d)
            } else {
                d
            }
        }
        _ => d,
    }
}
