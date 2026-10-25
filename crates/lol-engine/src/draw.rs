// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
// Ported from ScummVM's Kyra engine, Copyright (C) The ScummVM Team (www.scummvm.org).
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing figures as the original does (ScummVM's `Screen::drawShape`): scaled by a factor
//! in 256ths, mirrored, through a table of colours and a table that darkens them, cut to a
//! rectangle of the screen. And the tables that darken colours with distance
//! (`Screen_v2::generateOverlay`).

use lol_data::palette::Palette;
use lol_data::shp::Shape;

use crate::game::WIDTH;

/// A rectangle of the screen, the end exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

/// How to draw a figure.
#[derive(Debug, Clone, Copy, Default)]
pub struct Draw<'a> {
    pub flip: bool,
    /// Width and height factors, 256 the figure's own size.
    pub scale: Option<(u16, u16)>,
    /// A table of colours in place of the figure's own.
    pub table: Option<&'a [u8]>,
    /// Applied to each colour (once); colour 0 after it leaves the pixel as it is.
    pub fade: Option<&'a [u8; 256]>,
}

/// The figure's width at a scale, as `getShapeScaledWidth`.
pub fn scaled_width(shape: &Shape, scale: u16) -> i32 {
    (shape.width as i32 * scale as i32) >> 8
}

pub fn scaled_height(shape: &Shape, scale: u16) -> i32 {
    (shape.height as i32 * scale as i32) >> 8
}

/// Draws a figure with its top left corner at `(x, y)` (the scaled figure's).
pub fn draw_shape(screen: &mut [u8], shape: &Shape, x: i32, y: i32, clip: Clip, how: Draw) {
    let (sw, sh) = how.scale.unwrap_or((256, 256));
    let (sw, sh) = (sw as i32, sh as i32);
    let out_w = (shape.width as i32 * sw) >> 8;
    let out_h = (shape.height as i32 * sh) >> 8;
    if out_w <= 0 || out_h <= 0 || shape.width == 0 {
        return;
    }
    let table = how.table.or(shape.table.as_deref());
    let height = (screen.len() / WIDTH) as i32;
    let plot = |screen: &mut [u8], px: i32, py: i32, raw: u8| {
        if px < clip.x1 || px >= clip.x2 || py < clip.y1 || py >= clip.y2 {
            return;
        }
        if px < 0 || px >= WIDTH as i32 || py < 0 || py >= height {
            return;
        }
        let mut c = match table {
            Some(t) => t.get(raw as usize).copied().unwrap_or(raw),
            None => raw,
        };
        if let Some(fade) = how.fade {
            c = fade[c as usize];
            if c == 0 {
                return;
            }
        }
        screen[py as usize * WIDTH + px as usize] = c;
    };
    // Rows: the counter gains the height factor per row of the figure; each 256 is a row
    // drawn, the rows between skipped.
    let mut counter_v = 0;
    let mut out_row = 0;
    for row in shape.pixels.chunks(shape.width) {
        counter_v += sh;
        while counter_v >= 256 {
            counter_v -= 256;
            // Columns alike.
            let mut state = 0;
            let mut out_x = 0;
            for &raw in row {
                state += sw;
                while state >= 256 {
                    state -= 256;
                    if raw != 0 && out_x < out_w {
                        let px = if how.flip {
                            x + out_w - 1 - out_x
                        } else {
                            x + out_x
                        };
                        plot(screen, px, y + out_row, raw);
                    }
                    out_x += 1;
                }
            }
            out_row += 1;
            if out_row >= out_h {
                return;
            }
        }
    }
}

/// A table turning each colour towards `color` by `weight` (0–255), as `generateOverlay`:
/// the nearest of colours 1–127 to the mixed one (Lands of Lore leaves the upper half, the
/// play field's, alone: the caller sets it).
pub fn overlay(palette: &Palette, color: u8, weight: u32) -> [u8; 256] {
    let pal = &palette.vga;
    let weight = (weight.min(255) >> 1) as i32;
    let [or, og, ob] = pal[color as usize].map(|v| v as i32);
    let mut table = [0u8; 256];
    for (i, entry) in table.iter_mut().enumerate().skip(1) {
        let [r, g, b] = pal[i].map(|v| v as i32);
        let cur_r = (r - (((r - or) * weight) >> 7)) as u8 as i32;
        let cur_g = (g - (((g - og) * weight) >> 7)) as u8 as i32;
        let cur_b = (b - (((b - ob) * weight) >> 7)) as u8 as i32;
        let mut best = 0x7fff_u16;
        let mut index = color;
        for (cur, rgb) in pal.iter().enumerate().take(128).skip(1) {
            if cur == i {
                continue;
            }
            let d = |a: u8, b: i32| {
                let v = a as i32 - b;
                (v * v) as u16
            };
            let sum = d(rgb[0], cur_r)
                .wrapping_add(d(rgb[1], cur_g))
                .wrapping_add(d(rgb[2], cur_b));
            if sum == 0 {
                index = cur as u8;
                break;
            }
            if sum <= best {
                best = sum;
                index = cur as u8;
            }
        }
        *entry = index;
    }
    table
}

/// The level's eight tables of darkening (`loadLevelGraphics`): table `i` turns the walls'
/// colours `100 − i · weight` percent towards `color`, the play field's colours stay; table 7
/// changes nothing.
pub fn level_overlays(palette: &Palette, color: u8, weight: i32) -> [[u8; 256]; 8] {
    let mut overlays = [[0u8; 256]; 8];
    for (i, table) in overlays.iter_mut().enumerate().take(7) {
        let w = 100 - i as i32 * weight;
        let w = if w > 0 { (w * 255) / 100 } else { 0 };
        *table = overlay(palette, color, w as u32);
        for entry in table.iter_mut().take(128) {
            if *entry == 255 {
                *entry = 0;
            }
        }
        for (k, entry) in table.iter_mut().enumerate().skip(128) {
            *entry = k as u8;
        }
    }
    for (k, entry) in overlays[7].iter_mut().enumerate() {
        *entry = k as u8;
    }
    overlays
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape() -> Shape {
        // 2×2: 1 2 / 3 4.
        Shape {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 4],
            table: None,
        }
    }

    const ALL: Clip = Clip {
        x1: 0,
        y1: 0,
        x2: 320,
        y2: 200,
    };

    #[test]
    fn draws_flipped_and_cut() {
        let mut screen = vec![0u8; 320 * 200];
        let how = Draw {
            flip: true,
            ..Draw::default()
        };
        draw_shape(&mut screen, &shape(), 10, 5, ALL, how);
        assert_eq!(&screen[5 * 320 + 10..5 * 320 + 12], &[2, 1]);
        let clip = Clip { x2: 11, ..ALL };
        let mut screen = vec![0u8; 320 * 200];
        draw_shape(&mut screen, &shape(), 10, 5, clip, Draw::default());
        assert_eq!(&screen[6 * 320 + 10..6 * 320 + 12], &[3, 0]);
    }

    #[test]
    fn scales_up_and_down() {
        let mut screen = vec![0u8; 320 * 200];
        let how = Draw {
            scale: Some((512, 512)),
            ..Draw::default()
        };
        draw_shape(&mut screen, &shape(), 0, 0, ALL, how);
        assert_eq!(&screen[0..4], &[1, 1, 2, 2]);
        assert_eq!(&screen[320..324], &[1, 1, 2, 2]);
        assert_eq!(&screen[640..644], &[3, 3, 4, 4]);
        let mut screen = vec![0u8; 320 * 200];
        let how = Draw {
            scale: Some((128, 128)),
            ..Draw::default()
        };
        draw_shape(&mut screen, &shape(), 0, 0, ALL, how);
        // Half size: the second row and column of each pair.
        assert_eq!(&screen[0..2], &[4, 0]);
    }

    #[test]
    fn the_last_table_changes_nothing() {
        let palette = Palette::from_vga(&[30; 768]);
        let overlays = level_overlays(&palette, 1, 10);
        assert!(overlays[7].iter().enumerate().all(|(i, &v)| v == i as u8));
        assert!(
            overlays[0][200..]
                .iter()
                .enumerate()
                .all(|(i, &v)| v == 200 + i as u8)
        );
    }
}
