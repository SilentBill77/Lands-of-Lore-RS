//! Drawing a figure as the original does, from its stored bytes (ScummVM's
//! `Screen::drawShape` with its margin, skip, line and plot functions): scaled in 256ths,
//! mirrored either way, centred, relative to a screen window or not, cut to the window, each
//! pixel through one of the plot functions (colour table, fading table, transparency tables,
//! background fading).
//!
//! A figure: flags (u16: bit 0 a colour table follows, bit 1 not packed, bit 2 the table's
//! length is given), height (u8), width (u16), the height again (u8), its size (u16), the size
//! of its lines unpacked (u16), [table length], [table], lines (LCW unless bit 1). In the lines a
//! pixel is a byte, `00 n` a transparent run of `n`.

use lol_data::compression::lcw;

pub const XFLIP: i32 = 0x01;
pub const YFLIP: i32 = 0x02;
pub const SCALE: i32 = 0x04;
pub const WINREL: i32 = 0x10;
pub const CENTER: i32 = 0x20;
pub const FADE: i32 = 0x100;
pub const PREDATOR: i32 = 0x200;
pub const COMPACT: i32 = 0x400;
pub const PRIORITY: i32 = 0x800;
pub const TRANSPARENT: i32 = 0x1000;
pub const BCKGRNDFADE: i32 = 0x2000;
pub const MORPH: i32 = 0x4000;
pub const COLOR: i32 = 0x8000;

/// What `drawShape` takes after its flags, as its variable arguments (each used only when
/// its flag is set).
#[derive(Default, Clone, Copy)]
pub struct ShapeArgs<'a> {
    pub color: Option<&'a [u8]>,
    pub fade: Option<(&'a [u8], i32)>,
    pub transparency: Option<(&'a [u8], &'a [u8])>,
    pub morph: i32,
    pub layer: i32,
    pub scale: (i32, i32),
    pub background_fade: Option<&'a [u8]>,
}

impl<'a> ShapeArgs<'a> {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn scaled(w: i32, h: i32) -> Self {
        Self {
            scale: (w, h),
            ..Self::default()
        }
    }
}

/// The window a figure is drawn into (`ScreenDim`: x and width in columns of 8 pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dim {
    pub sx: i32,
    pub sy: i32,
    pub w: i32,
    pub h: i32,
    pub col1: i32,
    pub col2: i32,
    pub line: i32,
    pub column: i32,
}

pub fn height(shape: &[u8]) -> i32 {
    shape.get(2).copied().unwrap_or(0) as i32
}

pub fn width(shape: &[u8]) -> i32 {
    shape
        .get(3..5)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32)
}

/// The size stored in the figure (`getShapeSize`).
pub fn size(shape: &[u8]) -> usize {
    shape
        .get(6..8)
        .map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize)
}

pub fn scaled_width(shape: &[u8], scale: i32) -> i32 {
    (width(shape) * scale) >> 8
}

pub fn scaled_height(shape: &[u8], scale: i32) -> i32 {
    (height(shape) * scale) >> 8
}

/// Figure `index` of a file of figures (`getPtrToShape`), as a copy of its bytes
/// (`makeShapeCopy`).
pub fn shape_copy(file: &[u8], index: usize) -> Option<Vec<u8>> {
    let count = u16::from_le_bytes([*file.first()?, *file.get(1)?]) as usize;
    if index >= count {
        return None;
    }
    let at = 2 + index * 4;
    let offset = u32::from_le_bytes(file.get(at..at + 4)?.try_into().ok()?) as usize + 2;
    let start = file.get(offset..)?;
    let len = size(start).min(start.len());
    (len > 0).then(|| start[..len].to_vec())
}

struct State<'a> {
    flags: i32,
    plot: i32,
    color: Option<&'a [u8]>,
    fade: Option<&'a [u8]>,
    fade_level: i32,
    trans1: Option<&'a [u8]>,
    trans2: Option<&'a [u8]>,
    background_fade: Option<&'a [u8]>,
    scale_w: i32,
    scale_h: i32,
    offscreen_left: i32,
    offscreen_right: i32,
    offscreen_scale_val1: i32,
    offscreen_scale_val2: i32,
    tmp_width: i32,
    src: Vec<u8>,
}

impl State<'_> {
    fn byte(&self, i: usize) -> u8 {
        self.src.get(i).copied().unwrap_or(0)
    }

    fn put(&self, page: &mut [u8], d: isize, cmd: u8) {
        let Ok(at) = usize::try_from(d) else { return };
        let Some(&old) = page.get(at) else { return };
        let fade = |mut c: u8| -> u8 {
            if let Some(table) = self.fade {
                for _ in 0..self.fade_level {
                    c = table.get(c as usize).copied().unwrap_or(c);
                }
            }
            c
        };
        let ct = |c: u8| -> u8 {
            self.color
                .and_then(|t| t.get(c as usize).copied())
                .unwrap_or(c)
        };
        let blend = |c: u8, old: u8| -> u8 {
            match (self.trans1, self.trans2) {
                (Some(t1), Some(t2)) => {
                    let offs = t1.get(c as usize).copied().unwrap_or(0xff);
                    if offs & 0x80 == 0 {
                        t2.get(((offs as usize) << 8) | old as usize)
                            .copied()
                            .unwrap_or(c)
                    } else {
                        c
                    }
                }
                _ => c,
            }
        };
        let value = match self.plot {
            0 => Some(cmd),
            1 => Some(fade(cmd)).filter(|&c| c != 0),
            3 | 7 => Some(fade(old)).filter(|&c| c != 0),
            4 => Some(ct(cmd)),
            5 => Some(fade(ct(cmd))).filter(|&c| c != 0),
            16 | 48 => Some(blend(cmd, old)),
            20 | 52 => Some(blend(ct(cmd), old)),
            21 => Some(fade(blend(ct(cmd), old))).filter(|&c| c != 0),
            33 => {
                if cmd == 255 {
                    Some(
                        self.background_fade
                            .and_then(|t| t.get(old as usize).copied())
                            .unwrap_or(old),
                    )
                } else {
                    Some(fade(cmd)).filter(|&c| c != 0)
                }
            }
            37 => {
                let c = ct(cmd);
                let c = if c == 255 {
                    self.background_fade
                        .and_then(|t| t.get(old as usize).copied())
                        .unwrap_or(old)
                } else {
                    fade(c)
                };
                Some(c).filter(|&c| c != 0)
            }
            // The priority and invisibility kinds belong to the Kyrandia games; for the kinds
            // without a function the original draws nothing.
            _ => None,
        };
        if let Some(v) = value {
            page[at] = v;
        }
    }

    fn downwind(&self) -> bool {
        self.flags & XFLIP != 0
    }

    fn scaled(&self) -> bool {
        self.flags & SCALE != 0
    }

    /// `drawShapeMargin…`: skips the pixels left of the window.
    fn margin(&mut self, d: &mut isize, s: &mut usize, cnt: &mut i32) -> i32 {
        if !self.scaled() {
            while {
                let go = *cnt > 0;
                *cnt -= 1;
                go
            } {
                let c = self.byte(*s);
                *s += 1;
                if c != 0 {
                    continue;
                }
                *cnt = *cnt + 1 - self.byte(*s) as i32;
                *s += 1;
            }
            *cnt += 1;
            if self.downwind() {
                *d += *cnt as isize;
            } else {
                *d -= *cnt as isize;
            }
            return 0;
        }
        self.tmp_width -= *cnt;
        while *cnt > 0 {
            *cnt -= 1;
            let c = self.byte(*s);
            *s += 1;
            if c != 0 {
                continue;
            }
            *cnt = *cnt + 1 - self.byte(*s) as i32;
            *s += 1;
        }
        if *cnt == 0 {
            return self.offscreen_scale_val1;
        }
        self.tmp_width += *cnt;
        let mut i = (self.offscreen_left - *cnt) * self.scale_w;
        let res = i & 0xff;
        i >>= 8;
        i -= self.offscreen_scale_val2;
        if self.downwind() {
            *d -= i as isize;
        } else {
            *d += i as isize;
        }
        *cnt = -i;
        res
    }

    /// `drawShapeSkip…`: skips the rest of a line.
    fn skip(&mut self, d: &mut isize, s: &mut usize, cnt: &mut i32) -> i32 {
        if !self.scaled() {
            // Without scaling the skip is the margin function.
            return self.margin(d, s, cnt);
        }
        *cnt = self.tmp_width;
        if self.downwind() {
            if *cnt == 0 {
                return 0;
            }
            let mut found = false;
            loop {
                *cnt -= 1;
                let c = self.byte(*s);
                *s += 1;
                if c == 0 {
                    found = true;
                    *cnt = *cnt + 1 - self.byte(*s) as i32;
                    *s += 1;
                }
                if *cnt <= 0 {
                    break;
                }
            }
            return if found { 0 } else { self.offscreen_scale_val1 };
        }
        if *cnt <= 0 {
            return 0;
        }
        loop {
            *cnt -= 1;
            let c = self.byte(*s);
            *s += 1;
            if c == 0 {
                *cnt = *cnt + 1 - self.byte(*s) as i32;
                *s += 1;
            }
            if *cnt <= 0 {
                break;
            }
        }
        0
    }

    /// `drawShapeProcessLine…`: the visible pixels of a line.
    fn line(
        &mut self,
        page: &mut [u8],
        d: &mut isize,
        s: &mut usize,
        cnt: &mut i32,
        scale_state: i32,
    ) {
        let step: isize = if self.downwind() { -1 } else { 1 };
        if !self.scaled() {
            loop {
                let c = self.byte(*s);
                *s += 1;
                if c != 0 {
                    self.put(page, *d, c);
                    *d += step;
                    *cnt -= 1;
                } else {
                    let run = self.byte(*s) as i32;
                    *s += 1;
                    *d += step * run as isize;
                    *cnt -= run;
                }
                if *cnt <= 0 {
                    break;
                }
            }
            return;
        }
        let mut state = scale_state as i16;
        let mut c: u8 = 0;
        loop {
            let st = state as u16;
            if st & 0x8000 != 0 || st & 0xff00 == 0 {
                c = self.byte(*s);
                *s += 1;
                self.tmp_width -= 1;
                if c != 0 {
                    state = (state as i32 + self.scale_w) as i16;
                } else {
                    self.tmp_width += 1;
                    let run = self.byte(*s) as i32;
                    *s += 1;
                    self.tmp_width -= run;
                    let r = run * self.scale_w + state as i32;
                    *d += step * (r >> 8) as isize;
                    *cnt -= r >> 8;
                    state = (r & 0xff) as i16;
                }
            } else if !self.downwind() && state == 0 {
                // (`else if (scaleState)` in the upwind version: nothing to draw.)
            } else {
                self.put(page, *d, c);
                *d += step;
                state = (state as i32 - 0x100) as i16;
                *cnt -= 1;
            }
            if *cnt <= 0 {
                break;
            }
        }
        *cnt = -1;
    }
}

/// `Screen::drawShape` into `page` (320 pixels wide), the window `dim` (the screen dimension
/// `sd` already looked up).
pub fn draw_shape(
    page: &mut [u8],
    shape: &[u8],
    mut x: i32,
    mut y: i32,
    dim: Dim,
    mut flags: i32,
    args: ShapeArgs,
) {
    if shape.len() < 10 {
        return;
    }
    if shape[0] & 1 != 0 {
        flags |= COMPACT;
    }
    let (scale_w, scale_h) = if flags & SCALE != 0 {
        args.scale
    } else {
        (0x100, 0x100)
    };
    let mut st = State {
        flags,
        plot: (flags >> 8) & 0x3f,
        color: if flags & COLOR != 0 { args.color } else { None },
        fade: None,
        fade_level: 0,
        trans1: None,
        trans2: None,
        background_fade: if flags & BCKGRNDFADE != 0 {
            args.background_fade
        } else {
            None
        },
        scale_w,
        scale_h,
        offscreen_left: 0,
        offscreen_right: 0,
        offscreen_scale_val1: 0,
        offscreen_scale_val2: 0,
        tmp_width: 0,
        src: Vec::new(),
    };
    if flags & FADE != 0
        && let Some((table, level)) = args.fade
    {
        st.fade = Some(table);
        st.fade_level = level;
        if level == 0 {
            st.flags &= !FADE;
            st.plot = (st.flags >> 8) & 0x3f;
        }
    }
    if flags & TRANSPARENT != 0
        && let Some((t1, t2)) = args.transparency
    {
        st.trans1 = Some(t1);
        st.trans2 = Some(t2);
    }

    let mut scale_counter_v: i32 = 0;
    let base: isize = (dim.sx << 3) as isize;
    if flags & WINREL == 0 {
        x -= dim.sx << 3;
    }
    let x2 = dim.w << 3;
    let y1 = dim.sy;
    if flags & WINREL != 0 {
        y += y1;
    }
    let y2 = y1 + dim.h;

    let word = |i: usize| u16::from_le_bytes([shape[i], shape[i + 1]]) as i32;
    let shape_flags = word(0);
    let mut shape_height = shape[2] as i32;
    let shape_width = word(3);
    let mut w_scaled1 = shape_width;
    let mut w_scaled2 = shape_width;
    if flags & SCALE != 0 {
        shape_height = (shape_height * scale_h) >> 8;
        w_scaled1 = (shape_width * scale_w) >> 8;
        w_scaled2 = w_scaled1;
        if shape_height == 0 || w_scaled1 == 0 {
            return;
        }
    }
    if flags & CENTER != 0 {
        x -= w_scaled1 >> 1;
        y -= shape_height >> 1;
    }
    let frame_size = word(8) as usize;
    let mut pos = 10;
    let table_colors = if shape_flags & 4 != 0 {
        pos += 1;
        shape[10] as usize
    } else {
        16
    };
    if flags & COLOR == 0 && shape_flags & 1 != 0 {
        // A pointer into the figure: a colour past the table reads the bytes that follow.
        st.color = shape.get(pos..);
    }
    if st.flags & COMPACT != 0 {
        pos += table_colors;
    }
    let body = shape.get(pos..).unwrap_or(&[]);
    st.src = if shape_flags & 2 == 0 {
        lcw("Figur", body, frame_size).unwrap_or_default()
    } else {
        body.to_vec()
    };
    let mut s: usize = 0;
    let mut dst: isize = base;

    let mut t = if flags & YFLIP != 0 {
        y2 - y - shape_height
    } else {
        y - y1
    };
    if t < 0 {
        shape_height += t;
        if shape_height <= 0 {
            return;
        }
        t = -t;
        let mut backup;
        loop {
            st.offscreen_scale_val1 = 0;
            backup = s;
            st.tmp_width = shape_width;
            let mut cnt = shape_width;
            st.skip(&mut dst, &mut s, &mut cnt);
            scale_counter_v += st.scale_h;
            if scale_counter_v & 0xff00 != 0 {
                let r = scale_counter_v >> 8;
                scale_counter_v &= 0xff;
                t -= r;
            }
            if !(scale_counter_v & 0xff00 == 0 && t > 0) {
                break;
            }
        }
        if t < 0 {
            s = backup;
            scale_counter_v += (-t) << 8;
        }
        if flags & YFLIP == 0 {
            y = y1;
        }
    }
    t = if flags & YFLIP != 0 {
        y + shape_height - y1
    } else {
        y2 - y
    };
    if t <= 0 {
        return;
    }
    if t < shape_height {
        shape_height = t;
        if flags & YFLIP != 0 {
            y = y1;
        }
    }
    st.offscreen_left = 0;
    if x < 0 {
        w_scaled1 += x;
        st.offscreen_left = -x;
        if st.offscreen_left >= w_scaled2 {
            return;
        }
        x = 0;
    }
    st.offscreen_right = 0;
    t = x2 - x;
    if t <= 0 {
        return;
    }
    if t < w_scaled1 {
        w_scaled1 = t;
        st.offscreen_right = w_scaled2 - st.offscreen_left - w_scaled1;
    }
    let mut pitch: isize = 320;
    let mut ty = y;
    if flags & YFLIP != 0 {
        pitch = -320;
        ty = ty - 1 + shape_height;
    }
    if flags & XFLIP != 0 {
        std::mem::swap(&mut st.offscreen_left, &mut st.offscreen_right);
        dst += (w_scaled1 - 1) as isize;
    }
    dst += (320 * ty + x) as isize;
    if flags & SCALE != 0 {
        st.offscreen_right = 0;
        st.offscreen_scale_val2 = st.offscreen_left;
        st.offscreen_left <<= 8;
        st.offscreen_scale_val1 = -(st.offscreen_left % st.scale_w);
        st.offscreen_left /= st.scale_w;
    }
    if shape_height <= 0 || w_scaled1 <= 0 {
        return;
    }
    let mut d = dst;
    let mut guard = 0;
    loop {
        while scale_counter_v & 0xff00 == 0 {
            scale_counter_v += st.scale_h;
            if scale_counter_v & 0xff00 == 0 {
                st.tmp_width = shape_width;
                let mut cnt = shape_width;
                st.skip(&mut d, &mut s, &mut cnt);
            }
            guard += 1;
            if guard > 100_000 {
                return;
            }
        }
        let b_src = s;
        loop {
            s = b_src;
            st.tmp_width = shape_width;
            let mut cnt = st.offscreen_left;
            let scale_state = st.margin(&mut d, &mut s, &mut cnt);
            if st.tmp_width != 0 {
                cnt += w_scaled1;
                if cnt > 0 {
                    st.line(page, &mut d, &mut s, &mut cnt, scale_state);
                }
                cnt += st.offscreen_right;
                if cnt != 0 {
                    st.skip(&mut d, &mut s, &mut cnt);
                }
            }
            dst += pitch;
            d = dst;
            shape_height -= 1;
            if shape_height == 0 {
                return;
            }
            scale_counter_v -= 0x100;
            if scale_counter_v & 0xff00 == 0 {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3×2 figure, unpacked: `5 _ 6` / `_ _ 7`.
    fn figure() -> Vec<u8> {
        let lines = [5u8, 0, 1, 6, 0, 2, 7];
        let mut shape = vec![2, 0, 2, 3, 0, 2];
        let size = (10 + lines.len()) as u16;
        shape.extend_from_slice(&size.to_le_bytes());
        shape.extend_from_slice(&(lines.len() as u16).to_le_bytes());
        shape.extend_from_slice(&lines);
        shape
    }

    const FULL: Dim = Dim {
        sx: 0,
        sy: 0,
        w: 40,
        h: 200,
        col1: 0,
        col2: 0,
        line: 0,
        column: 0,
    };

    #[test]
    fn draws_plain_and_mirrored() {
        let mut page = vec![0u8; 320 * 200];
        draw_shape(&mut page, &figure(), 10, 5, FULL, 0, ShapeArgs::none());
        assert_eq!(&page[5 * 320 + 10..5 * 320 + 13], &[5, 0, 6]);
        assert_eq!(&page[6 * 320 + 10..6 * 320 + 13], &[0, 0, 7]);
        let mut page = vec![0u8; 320 * 200];
        draw_shape(&mut page, &figure(), 10, 5, FULL, XFLIP, ShapeArgs::none());
        assert_eq!(&page[5 * 320 + 10..5 * 320 + 13], &[6, 0, 5]);
        assert_eq!(&page[6 * 320 + 10..6 * 320 + 13], &[7, 0, 0]);
    }

    #[test]
    fn cuts_to_the_window_and_scales() {
        let mut page = vec![0u8; 320 * 200];
        draw_shape(&mut page, &figure(), -1, -1, FULL, 0, ShapeArgs::none());
        assert_eq!(&page[0..2], &[0, 7]);
        let mut page = vec![0u8; 320 * 200];
        draw_shape(
            &mut page,
            &figure(),
            0,
            0,
            FULL,
            SCALE,
            ShapeArgs::scaled(0x200, 0x200),
        );
        assert_eq!(&page[0..6], &[5, 5, 0, 0, 6, 6]);
        assert_eq!(&page[320..326], &[5, 5, 0, 0, 6, 6]);
        assert_eq!(&page[640..646], &[0, 0, 0, 0, 7, 7]);
    }

    #[test]
    fn copies_a_figure_out_of_its_file() {
        let fig = figure();
        let mut file = vec![1, 0];
        file.extend_from_slice(&4u32.to_le_bytes());
        file.extend_from_slice(&fig);
        assert_eq!(shape_copy(&file, 0).unwrap(), fig);
        assert!(shape_copy(&file, 1).is_none());
    }
}
