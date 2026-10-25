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

//! The magic atlas (ScummVM's `lol.cpp`: `displayAutomap` and its helpers).

use lol_data::tables::{LEGEND_DATA, MAP_CURSOR_OVL, MAP_STRING_ID};

use super::types::*;
use super::{Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId, Pal};
use crate::shape::{self, ShapeArgs};

const MAP_COORDS: [[i8; 4]; 12] = [
    [0, 7, 0, -5],
    [-5, 0, 6, 0],
    [7, 5, 7, 1],
    [5, 6, 4, 6],
    [0, 7, 0, -1],
    [-3, 0, 6, 0],
    [6, 7, 6, -3],
    [-3, 5, 6, 5],
    [1, 5, 1, 1],
    [3, 1, 3, 1],
    [-1, 6, -1, -8],
    [-7, -1, 5, -1],
];

impl Lol {
    /// `_defaultLegendData` from the program's table (four bytes an entry: shape, enable,
    /// string id; the last entry 5 pixels higher).
    pub fn init_legend_data(&mut self) {
        let mut v = Vec::new();
        for (i, e) in LEGEND_DATA.as_chunks::<5>().0.iter().enumerate().take(11) {
            v.push(MapLegendData {
                shape_index: e[0],
                enable: e[1] != 0,
                y: e[2] as i8,
                string_id: u16::from_le_bytes([e[3], e[4]]),
            });
            let _ = i;
        }
        self.default_legend_data = v;
        self.map_cursor_overlay = MAP_CURSOR_OVL.to_vec();
    }

    pub fn display_automap(&mut self) {
        self.snd_play_sound_effect(105, -1);
        self.gui_toggle_button_display_mode(78, 1);
        self.current_map_level = self.current_level as u8;
        let tmp_wll = self.wll_automap_data[..80].to_vec();
        let mut pal3 = Pal::default();
        if let Some(d) = self.res.file_data("PARCH.CPS") {
            self.screen.load_bitmap(&d, 2, Some(&mut pal3));
            *self.screen.palette_mut(3) = pal3.clone();
        }
        self.automap_shapes.clear();
        if let Some(d) = self.res.file_data("AUTOBUT.SHP") {
            self.screen.load_bitmap(&d, 5, None);
            let page = self.screen.page(5).to_vec();
            for i in 0..109 {
                self.automap_shapes
                    .push(shape::shape_copy(&page, i + 11).unwrap_or_default());
            }
        }
        let p3 = self.screen.palette(3).clone();
        self.map_overlay = self.generate_gray_overlay(&p3, 52, 0, 0, 0, 256, false);
        if let Some(d) = self.res.file_data("FONT9PN.FNT") {
            self.screen.load_font(FontId::F9, &d);
        }
        if let Some(d) = self.res.file_data("FONT6PN.FNT") {
            self.screen.load_font(FontId::F6, &d);
        }
        for l in self.default_legend_data.iter_mut() {
            l.enable = false;
        }
        self.disable_sys_timer(2);
        self.generate_temp_data();
        self.reset_items(1);
        self.disable_monsters();
        let mut exit = false;
        self.map_update_needed = false;
        let lvl = self.current_map_level as i32;
        self.restore_block_temp_data(lvl);
        self.load_map_legend_data(lvl);
        self.screen.fade_to_black(10);
        self.draw_map_page(2);
        self.screen.copy_page(2, 0);
        self.screen.update_screen();
        let p3 = self.screen.palette(3).clone();
        self.screen.fade_palette(&p3, 10);
        let mut delay_timer = self.millis() + 8 * TICK;
        while !exit && !self.should_quit() {
            if self.map_update_needed {
                self.draw_map_page(2);
                self.screen.copy_page(2, 0);
                self.screen.update_screen();
                self.map_update_needed = false;
            }
            if self.millis() >= delay_timer {
                self.redraw_map_cursor();
                delay_timer = self.millis() + 8 * TICK;
            }
            let f = self.check_input(None, true, 0) & 0xFF;
            self.remove_input_top();
            if f != 0 {
                exit = self.automap_process_buttons(f);
                self.gui_notify_button_list_changed();
            }
            if f == Self::key_code(crate::platform::Key::Char(b'c')) as i32 {
                for b in self.level_block_properties.iter_mut().take(1024) {
                    b.flags |= 7;
                }
                self.map_update_needed = true;
            } else if f == Self::KEY_ESCAPE as i32 {
                exit = true;
            }
            self.delay(TICK, false);
        }
        if let Some(d) = self.res.file_data("FONT9P.FNT") {
            self.screen.load_font(FontId::F9, &d);
        }
        if let Some(d) = self.res.file_data("FONT6P.FNT") {
            self.screen.load_font(FontId::F6, &d);
        }
        self.screen.fade_to_black(10);
        let cl = self.current_level;
        self.load_level_wall_data(cl, false);
        self.wll_automap_data[..80].copy_from_slice(&tmp_wll);
        self.restore_block_temp_data(cl);
        self.add_level_items();
        self.gui_notify_button_list_changed();
        self.enable_sys_timer(2);
    }

    pub fn update_auto_map(&mut self, block: u16) {
        if self.flags_table[31] & 0x10 == 0 {
            return;
        }
        self.level_block_properties[block as usize].flags |= 7;
        let x = block & 0x1F;
        let y = block >> 5;
        for (xo, yo) in [
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
            (0, -1),
            (0, 1),
            (-1, 0),
            (1, 0),
        ] {
            self.update_auto_map_intern(block, x, y, xo, yo);
        }
    }

    fn update_auto_map_intern(
        &mut self,
        block: u16,
        x: u16,
        y: u16,
        x_offs: i16,
        y_offs: i16,
    ) -> bool {
        const T: [i16; 12] = [1, -1, 3, 2, -1, 0, -1, 0, 1, -32, 0, 32];
        let x = x.wrapping_add(x_offs as u16);
        let y = y.wrapping_add(y_offs as u16);
        if x & 0xFFE0 != 0 || y & 0xFFE0 != 0 {
            return false;
        }
        let xo = (x_offs + 1) as usize;
        let yo = (y_offs + 1) as usize;
        let blk = |o: i16| (block as i32 + o as i32).clamp(0, 1023) as usize;
        let fx = T[xo];
        let b = blk(T[6 + xo]);
        if fx != -1
            && self.wll_automap_data[self.level_block_properties[b].walls[fx as usize] as usize]
                & 0xC0
                != 0
        {
            return false;
        }
        let fy = T[3 + yo];
        let b = blk(T[9 + yo]);
        if fy != -1
            && self.wll_automap_data[self.level_block_properties[b].walls[fy as usize] as usize]
                & 0xC0
                != 0
        {
            return false;
        }
        let b = blk(T[6 + xo] + T[9 + yo]);
        if fx != -1
            && fy != -1
            && self.wll_automap_data[self.level_block_properties[b].walls[fx as usize] as usize]
                & 0xC0
                != 0
            && self.wll_automap_data[self.level_block_properties[b].walls[fy as usize] as usize]
                & 0xC0
                != 0
        {
            return false;
        }
        self.level_block_properties[b].flags |= 7;
        true
    }

    fn legend(&self) -> Vec<[u16; 6]> {
        (0..32)
            .map(|i| {
                let at = i * 12;
                let w = |o: usize| {
                    u16::from_le_bytes([
                        self.temp_buffer5120[at + o],
                        self.temp_buffer5120[at + o + 1],
                    ])
                };
                [w(0), w(2), w(4), w(6), w(8), w(10)]
            })
            .collect()
    }

    pub fn load_map_legend_data(&mut self, level: i32) {
        let mut legend = vec![[0u16; 6]; 32];
        for l in legend.iter_mut() {
            l[0] = 0xFFFF;
            l[5] = 0xFFFF;
        }
        let file = format!("LEVEL{level}.XXX");
        let data = self.res.file_data(&file).unwrap_or_default();
        let n = (data.len() / 12).min(32);
        for (i, l) in legend.iter_mut().enumerate().take(n) {
            let w = |o: usize| u16::from_le_bytes([data[i * 12 + o], data[i * 12 + o + 1]]);
            l[3] = w(0);
            l[4] = w(2);
            l[5] = w(4);
            l[0] = w(6);
            l[1] = w(8);
            l[2] = w(10);
        }
        for (i, l) in legend.iter().enumerate() {
            for (j, v) in l.iter().enumerate() {
                let at = i * 12 + j * 2;
                self.temp_buffer5120[at..at + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
    }

    pub fn draw_map_page(&mut self, page_num: usize) {
        let x_offset: i32 = if self.lang_index == 1 { -2 } else { 0 };
        for _ in 0..2 {
            let mut pal3 = self.screen.palette(3).clone();
            if let Some(d) = self.res.file_data("PARCH.CPS") {
                self.screen.load_bitmap(&d, page_num, Some(&mut pal3));
            }
            *self.screen.palette_mut(3) = pal3;
            if self.lang_index == 1 {
                self.screen.copy_region(
                    236,
                    16,
                    236 + x_offset,
                    16,
                    -x_offset,
                    1,
                    page_num,
                    page_num,
                    CR_NO_P_CHECK,
                );
            }
            let cp = self.screen.set_cur_page(page_num);
            let of = self.screen.set_font(FontId::F9);
            let title = MAP_STRING_ID
                .get(self.current_map_level as usize)
                .copied()
                .unwrap_or(0xFFFF);
            let s = self.get_lang_string(title);
            self.screen.print_text(&s, 236 + x_offset, 8, 1, 0);
            let bl_x = self.map_get_start_pos_x();
            let mut bl = (self.map_get_start_pos_y() << 5) + bl_x;
            let mut sx = self.automap_top_left_x as i32;
            let mut sy = self.automap_top_left_y as i32;
            let ovl = self.map_overlay.clone();
            while bl < 1024 {
                let w = self.level_block_properties[bl as usize].walls;
                let ad = |i: usize| self.wll_automap_data[w[i] as usize] & 0xC0 == 0;
                if self.level_block_properties[bl as usize].flags & 7 == 7
                    && ad(0)
                    && ad(2)
                    && ad(1)
                    && ad(3)
                {
                    let b0 = self.calc_new_block_position(bl as u16, 0);
                    let b2 = self.calc_new_block_position(bl as u16, 2);
                    let b1 = self.calc_new_block_position(bl as u16, 1);
                    let b3 = self.calc_new_block_position(bl as u16, 3);
                    let w02 = self.level_block_properties[b0 as usize].walls[2];
                    let w20 = self.level_block_properties[b2 as usize].walls[0];
                    let w13 = self.level_block_properties[b1 as usize].walls[3];
                    let w31 = self.level_block_properties[b3 as usize].walls[1];
                    let p = self.screen.cur_page;
                    self.screen
                        .copy_block_and_apply_overlay(p, sx, sy, p, sx, sy, 7, 6, 0, &ovl);
                    self.draw_map_block_wall(b3, w31, sx, sy, 3);
                    self.draw_map_shape(w31, sx, sy, 3);
                    if self.wll_automap_data[w31 as usize] & 0xC0 != 0 {
                        self.screen
                            .copy_block_and_apply_overlay(p, sx, sy, p, sx, sy, 1, 6, 0, &ovl);
                    }
                    self.draw_map_block_wall(b1, w13, sx, sy, 1);
                    self.draw_map_shape(w13, sx, sy, 1);
                    if self.wll_automap_data[w13 as usize] & 0xC0 != 0 {
                        self.screen.copy_block_and_apply_overlay(
                            p,
                            sx + 6,
                            sy,
                            p,
                            sx + 6,
                            sy,
                            1,
                            6,
                            0,
                            &ovl,
                        );
                    }
                    self.draw_map_block_wall(b0, w02, sx, sy, 0);
                    self.draw_map_shape(w02, sx, sy, 0);
                    if self.wll_automap_data[w02 as usize] & 0xC0 != 0 {
                        self.screen
                            .copy_block_and_apply_overlay(p, sx, sy, p, sx, sy, 7, 1, 0, &ovl);
                    }
                    self.draw_map_block_wall(b2, w20, sx, sy, 2);
                    self.draw_map_shape(w20, sx, sy, 2);
                    if self.wll_automap_data[w20 as usize] & 0xC0 != 0 {
                        self.screen.copy_block_and_apply_overlay(
                            p,
                            sx,
                            sy + 5,
                            p,
                            sx,
                            sy + 5,
                            7,
                            1,
                            0,
                            &ovl,
                        );
                    }
                }
                sx += 7;
                if bl % 32 == 31 {
                    sx = self.automap_top_left_x as i32;
                    sy += 6;
                    bl += bl_x;
                }
                bl += 1;
            }
            self.screen.set_font(of);
            self.screen.set_cur_page(cp);
            let of = self.screen.set_font(FontId::F6);
            let mut ty = 0;
            let sx0 = self.map_get_start_pos_x();
            let sy0 = self.map_get_start_pos_y();
            let legend = self.legend();
            for l in legend.iter() {
                if l[0] == 0xFFFF {
                    break;
                }
                let cbl = l[0].wrapping_add(l[1] << 5) as usize;
                if self.level_block_properties.get(cbl).map_or(0, |b| b.flags) & 7 != 7 {
                    continue;
                }
                if l[2] == 0xFFFF {
                    continue;
                }
                self.print_map_text(l[2], 244 + x_offset, (ty << 3) + 22);
                if l[5] == 0xFFFF {
                    ty += 1;
                    continue;
                }
                let cbl2 = l[3].wrapping_add(l[4] << 5) as usize;
                if let Some(b) = self.level_block_properties.get_mut(cbl2) {
                    b.flags |= 7;
                }
                let shp = self
                    .automap_shapes
                    .get((l[5] as usize) << 2)
                    .cloned()
                    .unwrap_or_default();
                let tlx = self.automap_top_left_x as i32;
                let tly = self.automap_top_left_y as i32;
                self.screen.draw_shape(
                    2,
                    &shp,
                    (l[3] as i32 - sx0) * 7 + tlx - 3,
                    (l[4] as i32 - sy0) * 6 + tly - 3,
                    0,
                    0,
                    ShapeArgs::none(),
                );
                self.screen.draw_shape(
                    2,
                    &shp,
                    231 + x_offset,
                    (ty << 3) + 19,
                    0,
                    0,
                    ShapeArgs::none(),
                );
                ty += 1;
            }
            let cp = self.screen.set_cur_page(page_num);
            for ii in 0..self.default_legend_data.len() {
                let l = self.default_legend_data[ii];
                if !l.enable {
                    continue;
                }
                let p = self.screen.cur_page;
                self.screen.copy_block_and_apply_overlay(
                    p,
                    235,
                    (ty << 3) + 21,
                    p,
                    235 + x_offset,
                    (ty << 3) + 21,
                    7,
                    6,
                    0,
                    &ovl,
                );
                let shp = self
                    .automap_shapes
                    .get((l.shape_index as usize) << 2)
                    .cloned()
                    .unwrap_or_default();
                self.screen.draw_shape(
                    p,
                    &shp,
                    232 + x_offset,
                    (ty << 3) + 18 + l.y as i32,
                    0,
                    0,
                    ShapeArgs::none(),
                );
                self.print_map_text(l.string_id, 244 + x_offset, (ty << 3) + 22);
                ty += 1;
            }
            self.screen.set_font(of);
            self.screen.set_cur_page(cp);
        }
        self.print_map_exit_button_text();
    }

    fn automap_process_buttons(&mut self, mut input_flag: i32) -> bool {
        let r;
        if input_flag == Self::KEY_RIGHT as i32 {
            r = 0;
        } else if input_flag == Self::KEY_LEFT as i32 {
            r = 1;
        } else if input_flag == 199 {
            let (mx, my) = (self.mouse_x, self.mouse_y);
            r = if Self::pos_within_rect(mx, my, 252, 175, 273, 200) {
                0
            } else if Self::pos_within_rect(mx, my, 231, 175, 252, 200) {
                1
            } else if Self::pos_within_rect(mx, my, 275, 175, 315, 197) {
                2
            } else {
                -1
            };
            self.print_map_exit_button_text();
            while (input_flag == 199 || input_flag == 200) && !self.should_quit() {
                input_flag = self.check_input(None, false, 0);
                self.remove_input_top();
                self.delay(TICK, false);
            }
        } else {
            return false;
        }
        if r == 0 {
            self.automap_forward_button();
            self.print_map_exit_button_text();
        } else if r == 1 {
            self.automap_back_button();
            self.print_map_exit_button_text();
        }
        r == 2
    }

    fn automap_switch_level(&mut self, i: i32) {
        for l in self.default_legend_data.iter_mut() {
            l.enable = false;
        }
        self.current_map_level = i as u8;
        self.load_level_wall_data(i, false);
        self.restore_block_temp_data(i);
        self.load_map_legend_data(i);
        self.map_update_needed = true;
    }

    fn automap_forward_button(&mut self) {
        let cur = self.current_map_level as i32;
        let mut i = cur + 1;
        let mut guard = 0;
        while self.has_temp_data_flags & (1u32 << ((i - 1) & 31)) == 0 && guard < 64 {
            i = (i + 1) & 0x1F;
            guard += 1;
        }
        if i == cur || guard >= 64 {
            return;
        }
        self.automap_switch_level(i);
    }

    fn automap_back_button(&mut self) {
        let cur = self.current_map_level as i32;
        let mut i = cur - 1;
        let mut guard = 0;
        while self.has_temp_data_flags & (1u32 << ((i - 1) & 31)) == 0 && guard < 64 {
            i = (i - 1) & 0x1F;
            guard += 1;
        }
        if i == cur || guard >= 64 {
            return;
        }
        self.automap_switch_level(i);
    }

    fn redraw_map_cursor(&mut self) {
        let sx = self.map_get_start_pos_x();
        let sy = self.map_get_start_pos_y();
        if self.current_level != self.current_map_level as i32 {
            return;
        }
        let cb = self.current_block as i32;
        let cx = self.automap_top_left_x as i32 + (((cb - sx) % 32) * 7);
        let cy = self.automap_top_left_y as i32 + (((cb - (sy << 5)) / 32) * 6);
        self.screen.fill_rect(0, 0, 16, 16, 0, Some(2));
        let shp = self
            .automap_shapes
            .get(48 + self.current_direction as usize)
            .cloned()
            .unwrap_or_default();
        self.screen
            .draw_shape(2, &shp, 0, 0, 0, 0, ShapeArgs::none());
        self.screen.copy_region(cx, cy, cx, cy, 16, 16, 2, 0, 0);
        let ovl = self.map_cursor_overlay.clone();
        self.screen
            .copy_block_and_apply_overlay(2, 0, 0, 0, cx - 3, cy - 2, 16, 16, 0, &ovl);
        self.map_cursor_overlay[24] = self.map_cursor_overlay[1];
        for i in 1..24 {
            self.map_cursor_overlay[i] = self.map_cursor_overlay[i + 1];
        }
        self.screen.update_screen();
    }

    fn draw_map_block_wall(&mut self, block: u16, wall: u8, x: i32, y: i32, direction: usize) {
        if (1 << direction) & self.level_block_properties[block as usize].flags != 0
            || (self.wll_automap_data[wall as usize] & 0x1F) != 13
        {
            return;
        }
        let cp = self.screen.cur_page;
        let c = |i: usize| MAP_COORDS[i][direction] as i32;
        let ovl = self.map_overlay.clone();
        self.screen.copy_block_and_apply_overlay(
            cp,
            x + c(0),
            y + c(1),
            cp,
            x + c(0),
            y + c(1),
            c(2),
            c(3),
            0,
            &ovl,
        );
        self.screen.copy_block_and_apply_overlay(
            cp,
            x + c(4),
            y + c(5),
            cp,
            x + c(4),
            y + c(5),
            c(8),
            c(9),
            0,
            &ovl,
        );
        self.screen.copy_block_and_apply_overlay(
            cp,
            x + c(6),
            y + c(7),
            cp,
            x + c(6),
            y + c(7),
            c(8),
            c(9),
            0,
            &ovl,
        );
    }

    fn draw_map_shape(&mut self, wall: u8, x: i32, y: i32, direction: usize) {
        let l = (self.wll_automap_data[wall as usize] & 0x1F) as usize;
        if l == 0x1F {
            return;
        }
        let shp = self
            .automap_shapes
            .get((l << 2) + direction)
            .cloned()
            .unwrap_or_default();
        let p = self.screen.cur_page;
        self.screen.draw_shape(
            p,
            &shp,
            x + MAP_COORDS[10][direction] as i32 - 2,
            y + MAP_COORDS[11][direction] as i32 - 2,
            0,
            0,
            ShapeArgs::none(),
        );
        self.map_include_legend_data(l as i32);
    }

    fn map_get_start_pos_x(&mut self) -> i32 {
        let used = |s: &Self, col: i32| {
            (0..32).any(|a| s.level_block_properties[((a << 5) + col) as usize].flags != 0)
        };
        let mut c = 0;
        while c < 32 && !used(self, c) {
            c += 1;
        }
        let mut d = 31;
        while d > 0 && !used(self, d) {
            d -= 1;
        }
        self.automap_top_left_x = if d > c {
            (((32 - (d - c)) >> 1) * 7 + 5) as u8
        } else {
            5
        };
        if d > c { c } else { 0 }
    }

    fn map_get_start_pos_y(&mut self) -> i32 {
        let used = |s: &Self, row: i32| {
            (0..32).any(|a| s.level_block_properties[((row << 5) + a) as usize].flags != 0)
        };
        let mut c = 0;
        while c < 32 && !used(self, c) {
            c += 1;
        }
        let mut d = 31;
        while d > 0 && !used(self, d) {
            d -= 1;
        }
        self.automap_top_left_y = if d > c {
            (((32 - (d - c)) >> 1) * 6 + 4) as u8
        } else {
            4
        };
        if d > c { c } else { 0 }
    }

    fn map_include_legend_data(&mut self, t: i32) {
        let t = t & 0x7F;
        if let Some(l) = self
            .default_legend_data
            .iter_mut()
            .find(|l| l.shape_index as i32 == t)
        {
            l.enable = true;
        }
    }

    fn print_map_text(&mut self, string_id: u16, x: i32, y: i32) {
        let cp = self.screen.set_cur_page(2);
        let s = self.get_lang_string(string_id);
        self.screen.print_text(&s, x, y, 239, 0);
        self.screen.set_cur_page(cp);
    }

    fn print_map_exit_button_text(&mut self) {
        let cp = self.screen.set_cur_page(2);
        let of = self.screen.set_font(FontId::F9);
        let s = self.get_lang_string(0x4033);
        self.screen.fprint_string(&s, 295, 182, 172, 0, 5);
        self.screen.set_font(of);
        self.screen.set_cur_page(cp);
    }
}
