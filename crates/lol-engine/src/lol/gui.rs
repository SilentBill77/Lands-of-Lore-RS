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

//! The play field and its controls (ScummVM's `gui_lol.cpp`, the `LoLEngine::gui_…` and
//! `clicked…` functions, and `GUI_LoL::processButton`/`processButtonList`).

use lol_data::tables::{
    BUTTON_DEFS, BUTTON_LIST1, BUTTON_LIST2, BUTTON_LIST3, BUTTON_LIST4, BUTTON_LIST5,
    BUTTON_LIST7, CHAR_INV_DEFS, CHAR_INV_INDEX, COMPASS_DEFS, EXP_REQUIREMENTS, GAME_SHAPE_MAP,
    INVENTORY_DESC, SPELL_PROPERTIES,
};

use super::types::*;
use super::{ButtonListId, Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId};
use crate::shape::ShapeArgs;

impl Lol {
    /// `gui_drawPlayField`.
    pub fn gui_draw_play_field(&mut self) {
        if let Some(d) = self.res.file_data("PLAYFLD.CPS") {
            self.screen.load_bitmap(&d, 3, None);
        }
        if self.flags_table[31] & 0x40 != 0 {
            // The compass's background.
            const CX: [i32; 3] = [112, 152, 224];
            self.screen
                .copy_region(CX[self.lang_index], 32, 288, 0, 32, 32, 2, 2, CR_NO_P_CHECK);
            self.compass_direction = -1;
        }
        if self.flags_table[31] & 0x10 != 0 {
            // The map book.
            let shp = self.game_shapes[78].clone();
            self.screen
                .draw_shape(2, &shp, 290, 32, 0, 0, ShapeArgs::none());
        }
        let cp = self.screen.set_cur_page(2);
        if self.flags_table[31] & 0x20 != 0 {
            self.gui_draw_scroll();
        } else {
            self.selected_spell = 0;
        }
        if self.flags_table[31] & 0x08 != 0 {
            self.reset_lamp_status();
        }
        self.update_draw_page2();
        self.gui_draw_scene(2);
        self.gui_draw_all_char_portraits_with_stats();
        self.gui_draw_inventory();
        let page = self.screen.cur_page;
        self.gui_draw_money_box(page);
        self.screen.set_cur_page(cp);
        self.screen.copy_page(2, 0);
        self.update_draw_page2();
    }

    /// `gui_drawScene`.
    pub fn gui_draw_scene(&mut self, page: usize) {
        if self.update_flags & 1 == 0
            && !self.weapons_disabled
            && self.party_awake
            && !self.vcn_blocks.is_empty()
        {
            self.draw_scene(page);
        }
    }

    pub fn gui_draw_inventory(&mut self) {
        if self.current_control_mode == 0 || self.need_scene_restore == 0 {
            for i in 0..9 {
                self.gui_draw_inventory_item(i);
            }
        }
    }

    pub fn gui_draw_inventory_item(&mut self, index: usize) {
        const XPOS: [i32; 9] = [0x6A, 0x7F, 0x94, 0xA9, 0xBE, 0xD3, 0xE8, 0xFD, 0x112];
        let x = XPOS[index];
        let mut item = self.inventory_cur_item + index as i32;
        if item > 47 {
            item -= 48;
        }
        let flag = if item & 1 != 0 { 0 } else { 1 };
        let page = self.screen.cur_page;
        let frame = self.game_shapes[4].clone();
        self.screen
            .draw_shape(page, &frame, x, 179, 0, flag, ShapeArgs::none());
        let it = self.inventory[item as usize];
        if it != 0 {
            let icon = self.get_item_icon_shape(it);
            self.screen
                .draw_shape(page, &icon, x + 1, 180, 0, 0, ShapeArgs::none());
        }
    }

    /// `gui_drawScroll`: the scroll of spells above the play field.
    pub fn gui_draw_scroll(&mut self) {
        self.screen
            .copy_region(112, 0, 12, 0, 87, 15, 2, 2, CR_NO_P_CHECK);
        let of = self.screen.set_font(FontId::F9);
        let mut h = 0;
        for i in 0..7 {
            if self.available_spells[i] != -1 {
                h += 9;
            }
        }
        if h == 18 {
            h = 27;
        }
        if h != 0 {
            self.screen
                .copy_region(201, 1, 17, 15, 6, h, 2, 2, CR_NO_P_CHECK);
            self.screen
                .copy_region(208, 1, 89, 15, 6, h, 2, 2, CR_NO_P_CHECK);
            self.screen.fill_rect(21, 15, 89, h + 15, 206, None);
        }
        self.screen
            .copy_region(112, 16, 12, h + 15, 87, 14, 2, 2, CR_NO_P_CHECK);
        let mut y = 15;
        for i in 0..7 {
            if self.available_spells[i] == -1 {
                continue;
            }
            let col = if i as i32 == self.selected_spell {
                132
            } else {
                1
            };
            let name = self.get_lang_string(
                SPELL_PROPERTIES[self.available_spells[i] as usize].spell_name_code,
            );
            self.screen.fprint_string(&name, 24, y, col, 0, 0);
            y += 9;
        }
        self.screen.set_font(of);
    }

    pub fn gui_highlight_selected_spell(&mut self, mode: bool) {
        let mut y = 15;
        let of = self.screen.set_font(FontId::F9);
        for i in 0..7 {
            if self.available_spells[i] == -1 {
                continue;
            }
            let col = if mode && i as i32 == self.selected_spell {
                132
            } else {
                1
            };
            let name = self.get_lang_string(
                SPELL_PROPERTIES[self.available_spells[i] as usize].spell_name_code,
            );
            self.screen.fprint_string(&name, 24, y, col, 0, 0);
            y += 9;
        }
        self.screen.set_font(of);
    }

    /// `gui_displayCharInventory`.
    pub fn gui_display_char_inventory(&mut self, char_num: usize) {
        const INVENTORY_TYPES: [u8; 10] = [0, 1, 2, 6, 3, 1, 1, 3, 5, 4];
        let cp = self.screen.set_cur_page(2);
        let id = self.characters[char_num].id.unsigned_abs() as i32;
        if id != self.last_char_inventory {
            let file = format!("INVENT{}.CPS", INVENTORY_TYPES[id as usize % 10]);
            if let Some(d) = self.res.file_data(&file) {
                self.screen.load_bitmap(&d, 3, None);
            }
            self.screen.copy_region(0, 0, 112, 0, 208, 120, 2, 6, 0);
        } else {
            self.screen.copy_region(112, 0, 0, 0, 208, 120, 6, 2, 0);
        }
        self.screen.copy_region(80, 143, 80, 143, 232, 35, 0, 2, 0);
        self.gui_draw_all_char_portraits_with_stats();
        let name = self.characters[char_num].name.clone();
        self.screen.fprint_string(&name, 157, 9, 254, 0, 5);
        self.gui_print_char_inventory_stats(char_num);
        for i in 0..11 {
            self.gui_draw_char_inventory_item(i);
        }
        let of = self.screen.set_font(FontId::F9);
        let s = self.get_lang_string(0x4033);
        self.screen.fprint_string(&s, 182, 103, 172, 0, 5);
        self.screen.set_font(of);

        const STATUS_FLAGS: [u16; 8] = [
            0x0080, 0x0000, 0x1000, 0x0002, 0x100, 0x0001, 0x0000, 0x0000,
        ];
        self.char_status_flags = [0xffff; 3];
        let mut x = 0;
        let mut c = 0;
        let flags = self.characters[char_num].flags;
        for i in 0..3 {
            if flags & STATUS_FLAGS[i << 1] == 0 {
                continue;
            }
            let shp = self.game_shapes[STATUS_FLAGS[(i << 1) + 1] as usize].clone();
            let page = self.screen.cur_page;
            self.screen
                .draw_shape(page, &shp, 108 + x, 98, 0, 0, ShapeArgs::none());
            x += shp.get(3).copied().unwrap_or(0) as i32 + 2;
            self.char_status_flags[c] = STATUS_FLAGS[(i << 1) + 1];
            c += 1;
        }
        for i in 0..3 {
            let ch = &self.characters[char_num];
            let lvl = ch.skill_levels[i] as usize;
            let req = |l: usize| EXP_REQUIREMENTS.get(l).copied().unwrap_or(u32::MAX) as i32;
            let mut b = ch.experience_pts[i] - req(lvl.saturating_sub(1));
            let mut e = req(lvl) - req(lvl.saturating_sub(1));
            while e as u32 & 0xFFFF_8000 != 0 {
                e >>= 1;
                let cc = b;
                b >>= 1;
                if cc != 0 && b == 0 {
                    b = 1;
                }
            }
            self.gui_draw_horizontal_bar_graph(154, 64 + i as i32 * 10, 34, 5, b, e, 132, 0);
        }
        self.screen.draw_clipped_line(14, 120, 194, 120, 1);
        self.screen.copy_region(0, 0, 112, 0, 208, 121, 2, 0, 0);
        self.screen.copy_region(80, 143, 80, 143, 232, 35, 2, 0, 0);
        self.screen.set_cur_page(cp);
    }

    /// `KyraRpgEngine::gui_drawHorizontalBarGraph`.
    pub fn gui_draw_horizontal_bar_graph(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        cur: i32,
        max: i32,
        col1: u8,
        col2: u8,
    ) {
        if max < 1 {
            return;
        }
        let e = cur.max(0).min(max);
        let w = w - 1;
        if w == 0 {
            return;
        }
        let h = h - 1;
        if h == 0 {
            return;
        }
        let mut t = (e * w) / max;
        if t == 0 && e != 0 {
            t += 1;
        }
        if t != 0 {
            self.screen.fill_rect(x, y, x + t - 1, y + h, col1, None);
        }
        if t < w && col2 != 0 {
            self.screen
                .fill_rect(x + t, y, x + w - 1, y + h, col2, None);
        }
    }

    pub fn gui_print_char_inventory_stats(&mut self, char_num: usize) {
        for i in 0..5 {
            let v = self.calculate_character_stats(char_num, i);
            self.gui_print_character_stats(i, true, v);
        }
        self.char_inventory_unk |= 1 << char_num;
    }

    pub fn gui_print_character_stats(&mut self, index: usize, redraw: bool, value: i32) {
        let offs = if self.screen.cur_page != 0 { 0 } else { 112 };
        let fh = 10;
        let (y, col) = if index < 2 {
            let y = index as i32 * fh + 22;
            if redraw {
                let s = self.get_lang_string(0x4014 + index as u16);
                self.screen.fprint_string(&s, offs + 108, y, 158, 0, 4);
            }
            (y, 158u8)
        } else {
            let s = index - 2;
            let y = s as i32 * fh + 62;
            let sel = self.selected_character.clamp(0, 3) as usize;
            let col = if self.characters[sel].flags & (0x200 << s) != 0 {
                254
            } else {
                180
            };
            if redraw {
                let st = self.get_lang_string(0x4014 + index as u16);
                self.screen.fprint_string(&st, offs + 108, y, col, 0, 4);
            }
            (y, col)
        };
        if offs != 0 {
            let cp = self.screen.cur_page;
            self.screen
                .copy_region(294, y, 182 + offs, y, 18, 8, 6, cp, CR_NO_P_CHECK);
        }
        let text = value.to_string().into_bytes();
        self.screen.fprint_string(&text, 200 + offs, y, col, 0, 6);
    }

    pub fn gui_change_character_stats(&mut self, char_num: usize) {
        let mut tmp = [0; 5];
        let mut inc = [0; 5];
        let mut prc = false;
        for i in 0..5 {
            tmp[i] = self.calculate_character_stats(char_num, i);
            let diff = tmp[i] - self.char_stats_temp[i];
            inc[i] = diff / 15;
            if diff != 0 {
                prc = true;
                if inc[i] == 0 {
                    inc[i] = if diff < 0 { -1 } else { 1 };
                }
            }
        }
        if !prc {
            return;
        }
        loop {
            prc = false;
            for i in 0..5 {
                if tmp[i] == self.char_stats_temp[i] {
                    continue;
                }
                self.char_stats_temp[i] += inc[i];
                if (inc[i] > 0 && tmp[i] < self.char_stats_temp[i])
                    || (inc[i] < 0 && tmp[i] > self.char_stats_temp[i])
                {
                    self.char_stats_temp[i] = tmp[i];
                }
                let v = self.char_stats_temp[i];
                self.gui_print_character_stats(i, false, v);
                prc = true;
            }
            self.delay(TICK, true);
            if !prc || self.should_quit() {
                break;
            }
        }
    }

    pub fn gui_draw_char_inventory_item(&mut self, item_index: usize) {
        const SLOT_SHAPES: [u8; 11] = [
            0x30, 0x34, 0x30, 0x34, 0x2E, 0x2F, 0x32, 0x33, 0x31, 0x35, 0x35,
        ];
        let sel = self.selected_character.clamp(0, 3) as usize;
        let rcs = self.characters[sel].race_class_sex as usize;
        let base = CHAR_INV_INDEX.get(rcs).copied().unwrap_or(0) as usize * 22 + item_index * 2;
        let mut x = CHAR_INV_DEFS[base] as i32;
        let mut y = CHAR_INV_DEFS[base + 1] as i32;
        if y == 0xff {
            return;
        }
        if self.screen.cur_page == 0 {
            x += 112;
        }
        let i = self.characters[sel].items[item_index];
        let shape_num = if i != 0 {
            if item_index < 9 { 4 } else { 5 }
        } else {
            SLOT_SHAPES[item_index] as usize
        };
        let page = self.screen.cur_page;
        let shp = self.game_shapes[shape_num].clone();
        self.screen
            .draw_shape(page, &shp, x, y, 0, 0, ShapeArgs::none());
        if item_index > 8 {
            x -= 5;
            y -= 5;
        }
        if i != 0 {
            let icon = self.get_item_icon_shape(i);
            self.screen
                .draw_shape(page, &icon, x + 1, y + 1, 0, 0, ShapeArgs::none());
        }
    }

    pub fn gui_draw_all_char_portraits_with_stats(&mut self) {
        let n = self.count_active_characters();
        for i in 0..n {
            self.gui_draw_char_portrait_with_stats(i);
        }
    }

    /// `gui_drawCharPortraitWithStats`: a character's box below the view.
    pub fn gui_draw_char_portrait_with_stats(&mut self, char_num: usize) {
        if char_num > 3 || self.characters[char_num].flags & 1 == 0 || self.update_flags & 2 != 0 {
            return;
        }
        let tmp_fid = self.screen.set_font(FontId::F6);
        let cp = self.screen.set_cur_page(6);
        self.gui_draw_box(0, 0, 66, 34, 1, 1, -1);
        let page = self.screen.cur_page;
        self.gui_draw_char_face_shape(char_num, 0, 1, page);
        let c = self.characters[char_num].clone();
        self.gui_draw_live_magic_bar(
            33,
            32,
            c.magic_points_cur as i32,
            0,
            c.magic_points_max as i32,
            5,
            32,
            162,
            1,
            false,
        );
        self.gui_draw_live_magic_bar(
            39,
            32,
            c.hit_points_cur as i32,
            0,
            c.hit_points_max as i32,
            5,
            32,
            154,
            1,
            true,
        );
        let m = self.get_lang_string(0x4253);
        let h = self.get_lang_string(0x4254);
        self.screen.print_text(&m, 33, 1, 160, 0);
        self.screen.print_text(&h, 39, 1, 152, 0);

        let mut spell_levels = 0;
        let spell = self.available_spells[self.selected_spell.clamp(0, 7) as usize];
        if spell != -1 {
            let p = &SPELL_PROPERTIES[spell as usize];
            for i in 0..4 {
                if p.mp_required[i] as i32 <= c.magic_points_cur as i32
                    && p.hp_required[i] as i32 <= c.hit_points_cur as i32
                {
                    spell_levels += 1;
                }
            }
        }
        let page = self.screen.cur_page;
        if c.flags & 0x10 != 0 {
            // The magic menu is open.
            let shp = self.game_shapes[73].clone();
            self.screen
                .draw_shape(page, &shp, 44, 0, 0, 0, ShapeArgs::none());
            if spell_levels < 4 {
                self.screen.draw_grid_box(
                    44,
                    (spell_levels << 3) + 1,
                    22,
                    32 - (spell_levels << 3),
                    1,
                );
            }
        } else {
            let mut hand_index = 0usize;
            if c.items[0] != 0 {
                let pi = self.items_in_play[c.items[0] as usize].item_property_index as usize;
                if self.item_properties.get(pi).is_some_and(|p| p.might != -1) {
                    hand_index = pi;
                }
            }
            let shp_index = self
                .item_properties
                .get(hand_index)
                .map_or(0, |p| p.shp_index) as usize;
            let mut hi = GAME_SHAPE_MAP[(shp_index << 1) + 1] as i8 as i32;
            if hi == GAME_SHAPE_MAP[1] as i8 as i32 {
                hi = (c.race_class_sex as i32 - 1).max(0) + 68;
            }
            let shp = self.game_shapes[hi.max(0) as usize].clone();
            self.screen
                .draw_shape(page, &shp, 44, 0, 0, 0, ShapeArgs::none());
            let sym = self.game_shapes[72 + c.field_41 as usize].clone();
            self.screen
                .draw_shape(page, &sym, 44, 17, 0, 0, ShapeArgs::none());
            if spell_levels == 0 {
                self.screen.draw_grid_box(44, 17, 22, 16, 1);
            }
        }
        let f = c.flags & 0x314C;
        if (f == 0 && self.weapons_disabled)
            || (f != 0 && (f != 4 || c.weapon_hit == 0 || self.weapons_disabled))
        {
            self.screen.draw_grid_box(44, 0, 22, 34, 1);
        }
        if c.weapon_hit != 0 {
            let shp = self.game_shapes[34].clone();
            self.screen
                .draw_shape(page, &shp, 44, 0, 0, 0, ShapeArgs::none());
            self.screen
                .fprint_string(c.weapon_hit.to_string().as_bytes(), 57, 7, 254, 0, 1);
        }
        if c.damage_suffered != 0 {
            self.screen
                .fprint_string(c.damage_suffered.to_string().as_bytes(), 17, 28, 254, 0, 1);
        }
        let col =
            if char_num as i32 != self.selected_character || self.count_active_characters() == 1 {
                1
            } else {
                212
            };
        self.screen.draw_box(0, 0, 65, 33, col);
        let page = self.screen.cur_page;
        self.screen.copy_region(
            0,
            0,
            self.active_chars_xpos[char_num] as i32,
            143,
            66,
            34,
            page,
            cp,
            CR_NO_P_CHECK,
        );
        self.screen.set_cur_page(cp);
        self.screen.set_font(tmp_fid);
    }

    pub fn gui_draw_char_face_shape(&mut self, char_num: usize, x: i32, y: i32, page: usize) {
        let c = &mut self.characters[char_num];
        if c.cur_face_frame < 7 && c.temp_face_frame != 0 {
            c.cur_face_frame = c.temp_face_frame;
        }
        if c.temp_face_frame == 0 && c.cur_face_frame > 1 && c.cur_face_frame < 7 {
            c.cur_face_frame = c.temp_face_frame;
        }
        let mut frm = if c.flags & 0x1108 != 0 && c.cur_face_frame < 7 {
            1
        } else {
            c.cur_face_frame as usize
        };
        if c.hit_points_cur as i32 <= (c.hit_points_max >> 1) as i32 {
            frm += 14;
        }
        let poisoned = c.flags & 0x80 != 0;
        let web = c.flags & 0x40 != 0;
        if page == 0 {
            self.screen.hide_mouse();
        }
        if let Some(shp) = self
            .character_face_shapes
            .get(frm)
            .and_then(|s| s[char_num.min(2)].clone())
        {
            let ovl = self.screen.palette_overlay2.clone();
            self.screen.draw_shape(
                page,
                &shp,
                x,
                y,
                0,
                0x100,
                ShapeArgs {
                    fade: Some((&ovl, poisoned as i32)),
                    ..ShapeArgs::none()
                },
            );
        }
        if web {
            let shp = self.game_shapes[21].clone();
            self.screen
                .draw_shape(page, &shp, x, y, 0, 0, ShapeArgs::none());
        }
        if page == 0 {
            self.screen.show_mouse();
        }
    }

    pub fn gui_highlight_portrait_frame(&mut self, char_num: usize) {
        if char_num as i32 != self.selected_character {
            let o = self.selected_character;
            self.selected_character = char_num as i32;
            if (0..4).contains(&o) {
                self.gui_draw_char_portrait_with_stats(o as usize);
            }
        }
        self.gui_draw_char_portrait_with_stats(char_num);
    }

    pub fn gui_draw_live_magic_bar(
        &mut self,
        x: i32,
        y: i32,
        cur: i32,
        unk: i32,
        max: i32,
        w: i32,
        h: i32,
        mut col1: u8,
        col2: u8,
        flag: bool,
    ) {
        let w = w - 1;
        let h = h - 1;
        if max < 1 {
            return;
        }
        let t = cur.max(0);
        let cur = max.min(t);
        let mut bar = (cur * h) / max;
        if bar < 1 && cur > 0 {
            bar = 1;
        }
        self.screen.draw_clipped_line(x - 1, y - h, x - 1, y, 1);
        if flag {
            if max >> 1 > cur {
                col1 = 144;
            }
            if max >> 2 > cur {
                col1 = 132;
            }
        }
        if bar > 0 {
            self.screen.fill_rect(x, y - bar, x + w, y, col1, None);
        }
        if bar < h {
            self.screen.fill_rect(x, y - h, x + w, y - bar, col2, None);
        }
        if unk > 0 && unk < max {
            self.screen
                .draw_box(x, y - bar, x + w, y, col1.wrapping_sub(2));
        }
    }

    pub fn calc_char_portrait_xpos(&mut self) {
        let nc = self.count_active_characters() as i32;
        if nc == 0 {
            return;
        }
        if self.current_control_mode != 0 && !self.text_enabled() {
            let t = (280 - nc * 33) / (nc + 1);
            for i in 0..nc {
                self.active_chars_xpos[i as usize] = (i * 33 + t * (i + 1) + 10) as u16;
            }
        } else {
            let t = (235 - nc * 66) / (nc + 1);
            for i in 0..nc {
                self.active_chars_xpos[i as usize] = (i * 66 + t * (i + 1) + 83) as u16;
            }
        }
    }

    pub fn gui_draw_money_box(&mut self, page: usize) {
        const MX: [i32; 5] = [0x128, 0x134, 0x12B, 0x131, 0x12E];
        const MY: [i32; 5] = [0x73, 0x73, 0x74, 0x74, 0x75];
        let backup = self.screen.set_cur_page(page);
        self.screen.fill_rect(292, 97, 316, 118, 252, Some(page));
        for i in 0..5 {
            if self.money_column_height[i] == 0 {
                continue;
            }
            let h = self.money_column_height[i] as i32 - 1;
            let cols = [0xD2, 0xD1, 0xD0, 0xD1, 0xD2];
            for (k, col) in cols.iter().enumerate() {
                self.screen.draw_clipped_line(
                    MX[i] + k as i32,
                    MY[i],
                    MX[i] + k as i32,
                    MY[i] - h,
                    *col,
                );
            }
        }
        let bf = self.screen.set_font(FontId::F6);
        self.screen
            .fprint_string(self.credits.to_string().as_bytes(), 305, 98, 254, 0, 1);
        self.screen.set_font(bf);
        self.screen.set_cur_page(backup);
        if page == 6 {
            self.screen.copy_region(292, 97, 292, 97, 25, 22, 6, 0, 0);
        }
    }

    /// `gui_drawCompass`.
    pub fn gui_draw_compass(&mut self) {
        if self.flags_table[31] & 0x40 == 0 {
            return;
        }
        if self.compass_direction == -1 {
            self.compass_direction_index = -1;
            self.compass_direction = (self.current_direction as i32) << 6;
        }
        let t = ((self.compass_direction + 4) >> 3) & 0x1F;
        if t == self.compass_direction_index {
            return;
        }
        self.compass_direction_index = t;
        let page = self.screen.cur_page;
        if page == 0 {
            self.screen.hide_mouse();
        }
        let c = COMPASS_DEFS[t as usize];
        let compass_shp = 22 + self.lang_index;
        let compass_ptr = 25;
        let base = self.game_shapes[compass_shp].clone();
        self.screen
            .draw_shape(page, &base, 294, 3, 0, 0, ShapeArgs::none());
        let ptr = self.game_shapes[compass_ptr + c.shape_index as usize].clone();
        let ovl = self.screen.palette_overlay1.clone();
        self.screen.draw_shape(
            page,
            &ptr,
            298 + c.x as i32,
            c.y as i32 + 9,
            0,
            c.flags as i32 | 0x300,
            ShapeArgs {
                fade: Some((&ovl, 1)),
                ..ShapeArgs::none()
            },
        );
        self.screen.draw_shape(
            page,
            &ptr,
            299 + c.x as i32,
            c.y as i32 + 8,
            0,
            c.flags as i32,
            ShapeArgs::none(),
        );
        if page == 0 {
            self.screen.show_mouse();
        }
    }

    pub fn gui_enable_controls(&mut self) -> i32 {
        self.floating_cursor_control = 0;
        if self.current_control_mode == 0 {
            for i in 76..85 {
                self.gui_toggle_button_display_mode(i, 2);
            }
        }
        self.gui_toggle_fight_buttons(false);
        1
    }

    pub fn gui_disable_controls(&mut self, control_mode: i32) -> i32 {
        if self.current_control_mode != 0 {
            return 0;
        }
        self.floating_cursor_control = if control_mode & 2 != 0 { 2 } else { 1 };
        self.gui_toggle_fight_buttons(true);
        for i in 76..85 {
            let mode = if control_mode & 2 != 0 && i > 78 {
                2
            } else {
                3
            };
            self.gui_toggle_button_display_mode(i, mode);
        }
        1
    }

    /// `gui_toggleButtonDisplayMode`: a button of the play field drawn pressed, released or
    /// greyed out.
    pub fn gui_toggle_button_display_mode(&mut self, mut shape_index: i32, mut mode: i32) {
        const BX: [i32; 11] = [
            0x56, 0x128, 0x0C, 0x21, 0x122, 0x0C, 0x21, 0x36, 0x0C, 0x21, 0x36,
        ];
        const BY: [i32; 11] = [
            0xB4, 0xB4, 0xB4, 0xB4, 0x20, 0x84, 0x84, 0x84, 0x96, 0x96, 0x96,
        ];
        let swtch = 78;
        let subst = 74;
        if shape_index == swtch && self.flags_table[31] & 0x10 == 0 {
            return;
        }
        if self.current_control_mode != 0 && self.need_scene_restore != 0 {
            return;
        }
        if mode == 0 {
            shape_index = self.last_button_shape;
        }
        let mut page = 0;
        let idx = (shape_index - subst).clamp(0, 10) as usize;
        let mut x1 = if shape_index != 0 { BX[idx] } else { 0 };
        let mut y1 = if shape_index != 0 { BY[idx] } else { 0 };
        let (mut x2, mut y2) = (0, 0);
        match mode {
            1 => {
                mode = 0x100;
                self.last_button_shape = shape_index;
            }
            0 | 2 => {
                if mode == 0 {
                    if self.last_button_shape == 0 {
                        return;
                    }
                    let t = self.millis();
                    if self.button_press_timer > t {
                        let d = self.button_press_timer - t;
                        self.delay(d, false);
                    }
                }
                mode = 0;
                self.last_button_shape = 0;
            }
            3 => {
                mode = 0;
                self.last_button_shape = 0;
                page = 6;
                x2 = x1;
                y2 = y1;
                x1 = 0;
                y1 = 0;
            }
            _ => {}
        }
        let Some(shp) = self.game_shapes.get(shape_index.max(0) as usize).cloned() else {
            return;
        };
        let ovl = self.screen.palette_overlay1.clone();
        self.screen.draw_shape(
            page,
            &shp,
            x1,
            y1,
            0,
            mode,
            ShapeArgs {
                fade: Some((&ovl, 1)),
                ..ShapeArgs::none()
            },
        );
        if page == 0 {
            self.screen.update_screen();
        }
        if page == 6 {
            let cp = self.screen.set_cur_page(6);
            let (w, h) = (shp[3] as i32, shp[2] as i32);
            self.screen.draw_grid_box(x1, y1, w, h, 1);
            self.screen
                .copy_region(x1, y1, x2, y2, w, h, page, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.screen.set_cur_page(cp);
        }
        self.button_press_timer = self.millis() + 6 * TICK;
    }

    pub fn gui_toggle_fight_buttons(&mut self, disable: bool) {
        for i in 0..3 {
            if self.characters[i].flags & 1 == 0 {
                continue;
            }
            if disable {
                self.characters[i].flags |= 0x2000;
            } else {
                self.characters[i].flags &= 0xDFFF;
            }
            if disable && !self.text_enabled() {
                let u = self.selected_character;
                self.selected_character = 99;
                let f = self.update_flags;
                self.update_flags &= 0xFFFD;
                self.gui_draw_char_portrait_with_stats(i);
                self.update_flags = f;
                self.selected_character = u;
            } else {
                self.gui_draw_char_portrait_with_stats(i);
            }
        }
    }

    /// `gui_updateInput`: the play field's buttons get the events.
    pub fn gui_update_input(&mut self) {
        let main = !((self.update_flags & 3) != 0 || self.weapons_disabled);
        let mut input = self.check_input(Some(ButtonListId::Active), main, 0);
        if self.preserve_events {
            self.preserve_events = false;
        } else {
            self.remove_input_top();
        }
        if input != 0 && self.active_magic_menu != -1 && input & 0x8800 == 0 {
            self.gui_enable_default_playfield_buttons();
            let m = self.active_magic_menu as usize;
            self.characters[m].flags &= 0xFFEF;
            self.gui_draw_char_portrait_with_stats(m);
            self.gui_trigger_event(input);
            self.preserve_events = false;
            self.active_magic_menu = -1;
            input = 0;
        }
        if input == Self::KEY_SPACE as i32 || input == Self::KEY_RETURN as i32 {
            self.snd_stop_speech(true);
        } else if input == Self::KEY_SLASH as i32 {
            if self.weapons_disabled || self.available_spells[1] == -1 {
                return;
            }
            self.gui_highlight_selected_spell(false);
            self.selected_spell += 1;
            if self.available_spells[self.selected_spell as usize] == -1 {
                self.selected_spell = 0;
            }
            self.gui_highlight_selected_spell(true);
            self.gui_draw_all_char_portraits_with_stats();
        }
    }

    /// `gui_triggerEvent`: an event put back for the next round.
    pub fn gui_trigger_event(&mut self, event_type: i32) {
        use crate::platform::{Event, Key};
        let (x, y) = (self.mouse_x, self.mouse_y);
        let event = if event_type == 65 || event_type == 199 {
            Event::MouseDown { button: 0, x, y }
        } else if event_type == 66 || event_type == 201 {
            Event::MouseDown { button: 1, x, y }
        } else {
            // The key map searched backwards: any key whose code this is.
            let key = KEYS
                .iter()
                .copied()
                .chain((1..=6).map(Key::F))
                .chain(KEY_CHARS.iter().map(|&c| Key::Char(c)))
                .find(|&k| Self::key_code(k) as i32 == event_type)
                .unwrap_or(Key::Other(0));
            Event::KeyDown {
                key,
                ascii: 0,
                ctrl: false,
                alt: false,
                shift: false,
            }
        };
        self.remove_input_top();
        self.event_list.push_back(super::QueuedEvent {
            event,
            caused_skip: true,
        });
        self.preserve_events = true;
    }

    pub fn gui_enable_default_playfield_buttons(&mut self) {
        self.gui_reset_button_list();
        self.gui_init_buttons_from_list(&BUTTON_LIST1);
        self.gui_set_face_frames_control_buttons(7, 44);
        self.gui_set_face_frames_control_buttons(11, 44);
        self.gui_set_face_frames_control_buttons(17, 0);
        self.gui_set_face_frames_control_buttons(29, 0);
        self.gui_set_face_frames_control_buttons(25, 33);
        if self.flags_table[31] & 0x20 != 0 {
            self.gui_init_magic_scroll_buttons();
        }
    }

    pub fn gui_enable_sequence_buttons(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        enable_flags: i32,
    ) {
        self.gui_reset_button_list();
        self.scene_window_button = (x as i16, y as i16, w as u16, h as u16);
        self.gui_init_buttons_from_list(&BUTTON_LIST3);
        if enable_flags & 1 != 0 {
            self.gui_init_buttons_from_list(&BUTTON_LIST4);
        }
        if enable_flags & 2 != 0 {
            self.gui_init_buttons_from_list(&BUTTON_LIST5);
        }
    }

    pub fn gui_special_scene_restore_buttons(&mut self) {
        if self.sps_window.2 == 0 && self.sps_window.3 == 0 {
            return;
        }
        self.gui_enable_default_playfield_buttons();
        self.sps_window = (0, 0, 0, 0);
        self.seq_trigger = 0;
    }

    pub fn gui_enable_char_inventory_buttons(&mut self, char_num: usize) {
        self.gui_reset_button_list();
        self.gui_init_buttons_from_list(&BUTTON_LIST2);
        self.gui_init_char_inventory_special_buttons(char_num);
        self.gui_set_face_frames_control_buttons(21, 0);
    }

    pub fn gui_set_face_frames_control_buttons(&mut self, index: usize, x_offs: i32) {
        let c = self.count_active_characters();
        for i in 0..c {
            let x = self.active_chars_xpos[i] as i32 + x_offs;
            self.gui_init_button(index + i, x, -1, -1);
        }
    }

    pub fn gui_init_char_inventory_special_buttons(&mut self, char_num: usize) {
        let rcs = self.characters[char_num].race_class_sex as usize;
        let base = CHAR_INV_INDEX.get(rcs).copied().unwrap_or(0) as usize * 22;
        for i in 0..11 {
            let (x, y) = (CHAR_INV_DEFS[base + i * 2], CHAR_INV_DEFS[base + i * 2 + 1]);
            if x != 0xff {
                self.gui_init_button(33 + i, x as i32, y as i32, i as i32);
            }
        }
    }

    pub fn gui_init_magic_scroll_buttons(&mut self) {
        for i in 0..7 {
            if self.available_spells[i] == -1 {
                continue;
            }
            self.gui_init_button(71 + i, -1, -1, i as i32);
        }
    }

    pub fn gui_init_magic_submenu(&mut self, char_num: usize) {
        self.gui_reset_button_list();
        self.sub_menu_index = char_num as i32;
        self.gui_init_buttons_from_list(&BUTTON_LIST7);
    }

    pub fn gui_reset_button_list(&mut self) {
        self.buttons.clear();
        self.gui_notify_button_list_changed();
    }

    pub fn gui_notify_button_list_changed(&mut self) {
        self.button_list_changed = true;
    }

    /// `gui_initButtonsFromList`: the buttons of a list of indices (`FF` ends it).
    pub fn gui_init_buttons_from_list(&mut self, list: &[u8]) {
        for &i in list {
            if i == 0xff {
                break;
            }
            self.gui_init_button(i as usize, -1, -1, -1);
        }
    }

    /// `gui_initButton`.
    pub fn gui_init_button(&mut self, index: usize, x: i32, y: i32, val: i32) {
        let d = BUTTON_DEFS[index];
        let mut b = Button::blank();
        b.index = self.buttons.len() as u16 + 1;
        b.key_code = d.key_code;
        b.key_code2 = d.key_code2;
        b.dim_table_index = d.screen_dim;
        b.flags = d.flags;
        b.arg = if val != -1 {
            (val & 0xff) as u16
        } else {
            d.index
        };
        if index == 15 {
            b.x = (self.active_chars_xpos[self.sub_menu_index.clamp(0, 2) as usize] + 44) as i16;
            b.arg = self.sub_menu_index as u16;
            b.y = d.y;
            b.width = d.w.wrapping_sub(1);
            b.height = d.h.wrapping_sub(1);
        } else if index == 64 {
            let (sx, sy, sw, sh) = self.scene_window_button;
            b.x = sx;
            b.y = sy;
            b.width = sw.wrapping_sub(1);
            b.height = sh.wrapping_sub(1);
        } else {
            b.x = if x != -1 { x as i16 } else { d.x };
            b.y = if y != -1 { y as i16 } else { d.y };
            b.width = d.w.wrapping_sub(1);
            b.height = d.h.wrapping_sub(1);
        }
        b.button_callback = Callback::Playfield(index as u8);
        self.buttons.push(b);
    }

    /// Runs a play field button's function (`_buttonCallbacks[index]`).
    pub fn run_playfield_callback(&mut self, index: u8, button: &Button) -> i32 {
        match index {
            0 | 65 => self.clicked_up_arrow(button),
            1 | 2 | 66 => self.clicked_down_arrow(button),
            3 | 67 => self.clicked_left_arrow(button),
            4 | 68 => self.clicked_right_arrow(button),
            5 | 69 => self.clicked_turn_left_arrow(button),
            6 | 70 => self.clicked_turn_right_arrow(button),
            7..=10 => self.clicked_attack_button(button),
            11..=14 => self.clicked_magic_button(button),
            15 => self.clicked_magic_submenu(button),
            16 => self.clicked_screen(button),
            17..=24 => self.clicked_portrait_left(button),
            25..=28 => self.clicked_live_magic_bars_left(button),
            29..=32 => self.clicked_portrait_etc_right(button),
            33..=43 => self.clicked_char_inventory_slot(button),
            44 => self.clicked_exit_char_inventory(button),
            45..=48 => self.clicked_scene_drop_item(button),
            49 => self.clicked_scene_pickup_item(button),
            50..=59 => self.clicked_inventory_slot(button),
            60 | 61 => self.clicked_inventory_scroll(button),
            62 | 63 => self.clicked_wall(button),
            64 => self.clicked_sequence_window(button),
            71..=80 => self.clicked_scroll(button),
            81..=84 => self.clicked_spell_target_character(button),
            85 => self.clicked_spell_target_scene(button),
            86 | 87 => self.clicked_scene_throw_item(button),
            88 => self.clicked_options(button),
            89 => self.clicked_rest_party(button),
            90 => self.clicked_money_box(button),
            91 => self.clicked_compass(button),
            92 => self.clicked_automap(button),
            93 => self.clicked_lamp(button),
            94 => self.clicked_status_icon(button),
            _ => 0,
        }
    }

    fn arrow_blocked(&self, b: &Button) -> bool {
        b.arg != 0 && !self.floating_cursors
    }

    pub fn clicked_up_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        let d = self.current_direction;
        self.move_party(d, if b.flags2 & 0x1080 == 0x1080 { 1 } else { 0 }, 0, 80);
        1
    }

    pub fn clicked_down_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        let d = self.current_direction ^ 2;
        self.move_party(d, 0, 1, 83);
        1
    }

    pub fn clicked_left_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        let d = (self.current_direction.wrapping_sub(1)) & 3;
        self.move_party(d, if b.flags2 & 0x1080 == 0x1080 { 1 } else { 0 }, 2, 82);
        1
    }

    pub fn clicked_right_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        let d = (self.current_direction + 1) & 3;
        self.move_party(d, if b.flags2 & 0x1080 == 0x1080 { 1 } else { 0 }, 3, 84);
        1
    }

    pub fn clicked_turn_left_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        self.gui_toggle_button_display_mode(79, 1);
        self.current_direction = (self.current_direction.wrapping_sub(1)) & 3;
        self.scene_default_update = 1;
        let blk = self.current_block as i32;
        self.run_level_script(blk, 0x4000);
        self.init_text_fading(2, false);
        if self.scene_default_update == 0 {
            self.gui_draw_scene(0);
        } else {
            self.move_party_smooth_scroll_turn(false, 1);
        }
        self.gui_toggle_button_display_mode(79, 0);
        let blk = self.current_block as i32;
        self.run_level_script(blk, 0x10);
        1
    }

    pub fn clicked_turn_right_arrow(&mut self, b: &Button) -> i32 {
        if self.arrow_blocked(b) {
            return 0;
        }
        self.gui_toggle_button_display_mode(81, 1);
        self.current_direction = (self.current_direction + 1) & 3;
        self.scene_default_update = 1;
        let blk = self.current_block as i32;
        self.run_level_script(blk, 0x4000);
        self.init_text_fading(2, false);
        if self.scene_default_update == 0 {
            self.gui_draw_scene(0);
        } else {
            self.move_party_smooth_scroll_turn(true, 1);
        }
        self.gui_toggle_button_display_mode(81, 0);
        let blk = self.current_block as i32;
        self.run_level_script(blk, 0x10);
        1
    }

    pub fn clicked_attack_button(&mut self, b: &Button) -> i32 {
        let c = b.arg as usize;
        if c > 3 || self.characters[c].flags & 0x314C != 0 {
            return 1;
        }
        let bl = self.calc_new_block_position(self.current_block, self.current_direction);
        if self.level_block_properties[bl as usize].flags & 0x10 != 0 {
            self.break_ice_wall(None, None);
            return 1;
        }
        let target = self.get_nearest_monster_from_character(c);
        let mut s = 0;
        for i in 0..4 {
            let it = self.characters[c].items[i];
            if it == 0 {
                continue;
            }
            self.run_item_script(c as i32, it, 0x400, target as i32, s);
            let blk = self.current_block as i32;
            self.run_level_script_custom(blk, 0x400, c as i32, it as i32, target as i32, s);
            s -= 10;
        }
        if s == 0 {
            self.run_item_script(c as i32, 0, 0x400, target as i32, s);
            let blk = self.current_block as i32;
            self.run_level_script_custom(blk, 0x400, c as i32, 0, target as i32, s);
        }
        let mut s = if self.characters[c].weapon_hit != 0 {
            4
        } else {
            self.calc_monster_skill_level(c as i32, 8) + 4
        };
        // The Zephyr ring.
        if self.item_equipped(c, 230) {
            s >>= 1;
        }
        self.characters[c].flags |= 4;
        self.gui_highlight_portrait_frame(c);
        self.set_character_update_event(c, 1, s, 1);
        1
    }

    pub fn clicked_magic_button(&mut self, b: &Button) -> i32 {
        let c = b.arg as usize;
        if c > 3 || self.characters[c].flags & 0x314C != 0 {
            return 1;
        }
        let spell = self.available_spells[self.selected_spell.clamp(0, 7) as usize] as i32;
        if self.check_magic(c as i32, spell, 0) != 0 {
            return 1;
        }
        self.characters[c].flags ^= 0x10;
        self.gui_draw_char_portrait_with_stats(c);
        self.gui_init_magic_submenu(c);
        self.active_magic_menu = c as i32;
        1
    }

    pub fn clicked_magic_submenu(&mut self, b: &Button) -> i32 {
        let spell_level = (self.mouse_y - 144) >> 3;
        let c = b.arg as usize;
        self.gui_enable_default_playfield_buttons();
        let spell = self.available_spells[self.selected_spell.clamp(0, 7) as usize] as i32;
        if self.check_magic(c as i32, spell, spell_level) != 0 {
            self.characters[c].flags &= 0xFFEF;
            self.gui_draw_char_portrait_with_stats(c);
        } else {
            self.characters[c].flags |= 4;
            self.characters[c].flags &= 0xFFEF;
            if self.cast_spell(c as i32, spell, spell_level) != 0 {
                self.set_character_update_event(c, 1, 8, 1);
                self.increase_experience(c, 2, (spell_level * spell_level) as u32);
            } else {
                self.characters[c].flags &= 0xFFFB;
                self.gui_draw_char_portrait_with_stats(c);
            }
        }
        self.active_magic_menu = -1;
        1
    }

    pub fn clicked_screen(&mut self, b: &Button) -> i32 {
        if self.active_magic_menu >= 0 {
            let m = self.active_magic_menu as usize;
            self.characters[m].flags &= 0xFFEF;
            self.gui_draw_char_portrait_with_stats(m);
        }
        self.active_magic_menu = -1;
        if b.flags2 & 0x80 == 0 {
            if b.flags2 & 0x100 != 0 {
                self.gui_trigger_event(65);
            } else {
                self.gui_trigger_event(66);
            }
        }
        self.gui_enable_default_playfield_buttons();
        1
    }

    pub fn clicked_portrait_left(&mut self, b: &Button) -> i32 {
        self.disable_sys_timer(2);
        if !self.weapons_disabled {
            self.page_buffer2 = self.screen.copy_region_to_buffer(2, 0, 0, 320, 200);
            self.screen.copy_page(0, 2);
            self.page_buffer1 = self.screen.copy_region_to_buffer(2, 0, 0, 320, 200);
            self.update_flags |= 0x0C;
            self.gui_disable_controls(1);
        }
        self.selected_character = b.arg as i32;
        self.weapons_disabled = true;
        let sel = self.selected_character as usize;
        self.gui_display_char_inventory(sel);
        self.gui_enable_char_inventory_buttons(sel);
        1
    }

    pub fn clicked_live_magic_bars_left(&mut self, b: &Button) -> i32 {
        let c = b.arg as usize;
        self.gui_highlight_portrait_frame(c);
        let ch = self.characters[c].clone();
        let fmt = self.get_lang_string(0x4047);
        self.print_message(
            0,
            &fmt,
            &[
                Arg::Str(ch.name),
                (ch.hit_points_cur as i32).into(),
                (ch.hit_points_max as i32).into(),
                (ch.magic_points_cur as i32).into(),
                (ch.magic_points_max as i32).into(),
            ],
        );
        1
    }

    pub fn clicked_portrait_etc_right(&mut self, b: &Button) -> i32 {
        if self.item_in_hand == 0 {
            return 1;
        }
        let pi = self.items_in_play[self.item_in_hand as usize].item_property_index as usize;
        let flg = self.item_properties.get(pi).map_or(0, |p| p.flags);
        let c = b.arg as usize;
        if flg & 1 != 0 {
            if self.characters[c].flags & 8 == 0 || flg & 0x20 != 0 {
                let ih = self.item_in_hand;
                self.run_item_script(c as i32, ih, 0x400, 0, 0);
                let blk = self.current_block as i32;
                self.run_level_script_custom(blk, 0x400, c as i32, ih as i32, 0, 0);
            } else {
                let fmt = self.get_lang_string(0x402C);
                let name = self.characters[c].name.clone();
                self.print_message(2, &fmt, &[Arg::Str(name)]);
            }
            return 1;
        }
        let id = if flg & 8 != 0 {
            0x4029
        } else if flg & 0x10 != 0 {
            0x402A
        } else {
            0x402B
        };
        let s = self.get_lang_string(id);
        self.print_message(2, b"%s", &[Arg::Str(s)]);
        1
    }

    pub fn clicked_char_inventory_slot(&mut self, b: &Button) -> i32 {
        let sel = self.selected_character.clamp(0, 3) as usize;
        let arg = b.arg as usize;
        if self.item_in_hand != 0 {
            let sl = 1u16 << arg;
            let pi = self.items_in_play[self.item_in_hand as usize].item_property_index as usize;
            let ty = self.item_properties.get(pi).map_or(0, |p| p.type_);
            if sl & ty == 0 {
                let mut f = false;
                for i in 0..11 {
                    if ty & (1 << i) == 0 {
                        continue;
                    }
                    let fmt = self.get_lang_string(if i > 3 { 0x418A } else { 0x418B });
                    let name = self.get_lang_string(self.item_properties[pi].name_string_id);
                    let slot = self.get_lang_string(INVENTORY_DESC[i]);
                    self.print_message(0, &fmt, &[Arg::Str(name), Arg::Str(slot)]);
                    f = true;
                }
                if !f {
                    let s = self.get_lang_string(0x418C);
                    self.print_message(if pi == 231 { 2 } else { 0 }, b"%s", &[Arg::Str(s)]);
                }
                return 1;
            }
        } else if self.characters[sel].items[arg] == 0 {
            let s = self.get_lang_string(INVENTORY_DESC[arg] + 8);
            self.print_message(0, b"%s", &[Arg::Str(s)]);
            return 1;
        }
        let ih = self.item_in_hand;
        let it = self.characters[sel].items[arg];
        self.set_hand_item(it);
        self.characters[sel].items[arg] = ih;
        self.gui_draw_char_inventory_item(arg);
        self.recalc_character_stats(sel);
        if self.item_in_hand != 0 {
            let h = self.item_in_hand;
            self.run_item_script(sel as i32, h, 0x100, 0, 0);
        }
        if ih != 0 {
            self.run_item_script(sel as i32, ih, 0x80, 0, 0);
        }
        self.gui_draw_char_inventory_item(arg);
        self.gui_draw_char_portrait_with_stats(sel);
        self.gui_change_character_stats(sel);
        1
    }

    pub fn clicked_exit_char_inventory(&mut self, _b: &Button) -> i32 {
        self.update_flags &= 0xFFF3;
        self.gui_enable_default_playfield_buttons();
        self.weapons_disabled = false;
        for i in 0..4 {
            if self.char_inventory_unk & (1 << i) != 0 {
                self.characters[i].flags &= 0xF1FF;
            }
        }
        let pb1 = self.page_buffer1.clone();
        self.screen.copy_block_to_page(2, 0, 0, 320, 200, &pb1);
        let cp = self.screen.set_cur_page(2);
        self.gui_draw_all_char_portraits_with_stats();
        self.gui_draw_inventory();
        self.screen.set_cur_page(cp);
        self.screen.copy_page(2, 0);
        self.screen.update_screen();
        self.gui_enable_controls();
        let pb2 = self.page_buffer2.clone();
        self.screen.copy_block_to_page(2, 0, 0, 320, 200, &pb2);
        self.last_char_inventory = -1;
        self.update_draw_page2();
        self.enable_sys_timer(2);
        1
    }

    pub fn clicked_scene_drop_item(&mut self, b: &Button) -> i32 {
        const OFFS_X: [u16; 4] = [0x40, 0xC0, 0x40, 0xC0];
        const OFFS_Y: [u16; 4] = [0x40, 0x40, 0xC0, 0xC0];
        const DROP_ITEM_DIR_INDEX: [u8; 16] = [0, 1, 2, 3, 1, 3, 0, 2, 3, 2, 1, 0, 2, 0, 3, 1];
        if self.update_flags & 1 != 0 || self.item_in_hand == 0 {
            return 0;
        }
        let mut block = self.current_block;
        if b.arg > 1 {
            block = self.calc_new_block_position(self.current_block, self.current_direction);
            let w = self.level_block_properties[block as usize].walls
                [(self.current_direction ^ 2) as usize];
            let f = self.wll_wall_flags[w as usize];
            if f & 0x80 == 0 || f & 2 != 0 {
                return 1;
            }
        }
        let i =
            DROP_ITEM_DIR_INDEX[((self.current_direction as usize) << 2) + b.arg as usize] as usize;
        let (x, y) = self.calc_coordinates(block, OFFS_X[i], OFFS_Y[i]);
        let ih = self.item_in_hand;
        self.set_item_position(ih, x, y, 0, 1);
        self.set_hand_item(0);
        1
    }

    pub fn clicked_scene_pickup_item(&mut self, b: &Button) -> i32 {
        const CX: [i32; 21] = [
            0, 0, 1, 0, -1, -1, 1, 1, -1, 0, 2, 0, -2, -1, 1, 2, 2, 1, -1, -2, -2,
        ];
        const CY: [i32; 21] = [
            0, -1, 0, 1, 0, -1, -1, 1, 1, -2, 0, 2, 0, -2, -2, -1, 1, 2, 2, 1, -1,
        ];
        if self.update_flags & 1 != 0 || self.item_in_hand != 0 {
            return 0;
        }
        let page = self.scene_draw_page1;
        let cp = self.screen.set_cur_page(page);
        self.redraw_scene_item();
        let dim = self.screen.dim(b.dim_table_index as usize);
        let cl = (dim.sx << 3) + b.x as i32;
        let ct = dim.sy + b.y as i32;
        let cr = (dim.sx << 3) + b.x as i32 + b.width as i32 - 1;
        let cb = dim.sy + b.y as i32 + b.height as i32 - 1;
        let mut p = 0;
        for i in 0..21 {
            p = self.screen.get_page_pixel(
                page,
                (self.mouse_x + CX[i]).clamp(cl, cr),
                (self.mouse_y + CY[i]).clamp(ct, cb),
            ) as i32;
            if p != 0 {
                break;
            }
        }
        self.screen.set_cur_page(cp);
        if p == 0 {
            return 0;
        }
        let block = if p <= 128 {
            self.calc_new_block_position(self.current_block, self.current_direction)
        } else {
            self.current_block
        };
        let found = self.check_scene_for_items(block as usize, p & 0x7F);
        if found != -1 {
            self.remove_level_item(found as u16, block as i32);
            self.set_hand_item(found as u16);
        }
        self.scene_update_required = true;
        1
    }

    pub fn clicked_inventory_slot(&mut self, b: &Button) -> i32 {
        let mut slot = self.inventory_cur_item + b.arg as i32;
        if slot > 47 {
            slot -= 48;
        }
        let slot = slot as usize;
        let slot_item = self.inventory[slot];
        let h_item = self.item_in_hand;
        let pi = |s: &Self, it: u16| s.items_in_play[it as usize].item_property_index;
        if (pi(self, h_item) == 281 || pi(self, slot_item) == 281)
            && (pi(self, h_item) == 220 || pi(self, slot_item) == 220)
        {
            // The ruby of truth made whole.
            self.screen.hide_mouse();
            self.inventory[slot] = 0;
            self.gui_draw_inventory_item(b.arg as usize);
            self.screen.copy_region(
                b.x as i32,
                b.y as i32 - 3,
                b.x as i32,
                b.y as i32 - 3,
                25,
                27,
                0,
                2,
                0,
            );
            // `KyraEngine_v1::snd_playSoundEffect(99)`: the driver's track itself, not
            // through the game's list of sounds.
            self.screen.system.mixer().play_adlib_effect(99, 0xFF);
            if let Some(mut wsa) = self.open_wsa("TRUTH.WSA", 0, false) {
                for i in 0..25 {
                    let timer = self.millis() + 7 * TICK;
                    self.screen
                        .copy_region(b.x as i32, b.y as i32 - 3, 0, 0, 25, 27, 2, 2, 0);
                    wsa.display_frame(&mut self.screen, i, 2, 0, 0, 0x4000, None, None);
                    self.screen
                        .copy_region(0, 0, b.x as i32, b.y as i32 - 3, 25, 27, 2, 0, 0);
                    self.screen.update_screen();
                    self.delay_until(timer, false);
                }
            }
            self.screen.show_mouse();
            self.delete_item(slot_item);
            self.delete_item(h_item);
            self.set_hand_item(0);
            self.inventory[slot] = self.make_item(280, 0, 0);
        } else {
            self.set_hand_item(slot_item);
            self.inventory[slot] = h_item;
        }
        self.gui_draw_inventory_item(b.arg as usize);
        1
    }

    pub fn clicked_inventory_scroll(&mut self, b: &Button) -> i32 {
        let mut inc = b.arg as u8 as i8 as i32;
        let shp = if inc == 1 { 75 } else { 74 };
        if b.flags2 & 0x1000 != 0 {
            inc *= 9;
        }
        self.inventory_cur_item += inc;
        self.gui_toggle_button_display_mode(shp, 1);
        if self.inventory_cur_item < 0 {
            self.inventory_cur_item += 48;
        }
        if self.inventory_cur_item > 47 {
            self.inventory_cur_item -= 48;
        }
        self.gui_draw_inventory();
        self.gui_toggle_button_display_mode(shp, 0);
        1
    }

    pub fn clicked_wall(&mut self, _b: &Button) -> i32 {
        let block = self.calc_new_block_position(self.current_block, self.current_direction);
        let dir = (self.current_direction ^ 2) as usize;
        let w = self.level_block_properties[block as usize].walls[dir];
        match self.special_wall_types[w as usize] {
            1 => self.clicked_wall_shape(block, dir as u16),
            2 => self.clicked_lever_on(block, dir as u16),
            3 => self.clicked_lever_off(block, dir as u16),
            4 => self.clicked_wall_only_script(block),
            5 => self.clicked_door_switch(block, dir as u16),
            6 => self.clicked_niche(block, dir as u16),
            _ => 0,
        }
    }

    pub fn clicked_sequence_window(&mut self, _b: &Button) -> i32 {
        let blk = self.calc_new_block_position(self.current_block, self.current_direction) as i32;
        self.run_level_script(blk, 0x40);
        let (x1, y1, x2, y2) = self.seq_window;
        if self.seq_trigger == 0
            || !Self::pos_within_rect(self.mouse_x, self.mouse_y, x1, y1, x2, y2)
        {
            self.seq_trigger = 0;
            self.remove_input_top();
        }
        1
    }

    pub fn clicked_scroll(&mut self, b: &Button) -> i32 {
        if self.selected_spell == b.arg as i32 {
            return 1;
        }
        self.gui_highlight_selected_spell(false);
        self.selected_spell = b.arg as i32;
        self.gui_highlight_selected_spell(true);
        self.gui_draw_all_char_portraits_with_stats();
        1
    }

    pub fn clicked_spell_target_character(&mut self, b: &Button) -> i32 {
        let t = b.arg as usize;
        let name = self.characters[t].name.clone();
        self.print_message(0, b"%s.\r", &[Arg::Str(name)]);
        if SPELL_PROPERTIES[self.active_spell.spell as usize].flags & 0xFF == 1 {
            self.active_spell.target = t as u8;
            self.cast_heal_on_single_character();
        }
        self.gui_enable_default_playfield_buttons();
        1
    }

    pub fn clicked_spell_target_scene(&mut self, _b: &Button) -> i32 {
        let s = self.get_lang_string(0x4041);
        self.print_message(0, b"%s", &[Arg::Str(s)]);
        let a = self.active_spell;
        let p = SPELL_PROPERTIES[a.spell as usize];
        let c = &mut self.characters[a.char_num as usize];
        c.magic_points_cur = (c.magic_points_cur as i32 + p.mp_required[a.level as usize] as i32)
            .min(c.magic_points_max as i32) as i16;
        c.hit_points_cur = (c.hit_points_cur as i32 + p.hp_required[a.level as usize] as i32)
            .min(c.hit_points_max as i32) as i16;
        self.gui_draw_char_portrait_with_stats(a.char_num as usize);
        self.gui_enable_default_playfield_buttons();
        1
    }

    pub fn clicked_scene_throw_item(&mut self, _b: &Button) -> i32 {
        if self.update_flags & 1 != 0 {
            return 0;
        }
        let block = self.calc_new_block_position(self.current_block, self.current_direction);
        let w = self.level_block_properties[block as usize].walls
            [(self.current_direction ^ 2) as usize];
        if self.wll_wall_flags[w as usize] & 2 != 0 || self.item_in_hand == 0 {
            return 0;
        }
        let (x, y) = self.calc_coordinates(self.current_block, 0x80, 0x80);
        let ih = self.item_in_hand;
        let dir = (self.current_direction << 1) as i32;
        let sc = self.selected_character;
        if self.launch_object(0, ih, x as i32, y as i32, 12, dir, 6, sc, 0x3F) {
            self.snd_play_sound_effect(18, -1);
            self.set_hand_item(0);
        }
        self.scene_update_required = true;
        1
    }

    pub fn clicked_options(&mut self, _b: &Button) -> i32 {
        self.remove_input_top();
        self.gui_toggle_button_display_mode(76, 1);
        self.update_flags |= 4;
        let b = Button::blank();
        if self.weapons_disabled {
            self.clicked_exit_char_inventory(&b);
        }
        self.init_text_fading(0, true);
        self.stop_portrait_speech_anim();
        self.set_lamp_mode(true);
        self.set_mouse_cursor_to_icon(0);
        self.disable_sys_timer(2);
        self.gui_toggle_button_display_mode(76, 0);
        let speech_was_enabled = self.speech_enabled();
        if self.get_volume(2) == 2 {
            self.config_voice |= if self.text_enabled() { 2 } else { 1 };
        }
        self.run_menu(MenuCb::MainMenu);
        self.update_flags &= 0xFFFB;
        self.set_mouse_cursor_to_item_in_hand();
        self.reset_lamp_status();
        self.gui_enable_default_playfield_buttons();
        self.enable_sys_timer(2);
        self.update_draw_page2();
        self.gui_draw_play_field();
        if self.get_volume(2) == 2 {
            self.config_voice &= if self.text_enabled() { !2 } else { !1 };
        }
        if speech_was_enabled && !self.text_enabled() && !self.speech_enabled() {
            self.config_voice = 0;
        }
        self.write_settings();
        1
    }

    pub fn clicked_rest_party(&mut self, _b: &Button) -> i32 {
        self.gui_toggle_button_display_mode(77, 1);
        let b = Button::blank();
        if self.weapons_disabled {
            self.clicked_exit_char_inventory(&b);
        }
        let (mut t_hp, mut t_mp, mut t_ha) = (-1i32, -1i32, -1i32);
        let (mut need_poison, mut need_heal, mut need_magic) = (0, 0, 0);
        for i in 0..4 {
            let c = &mut self.characters[i];
            if c.flags & 1 == 0 || c.flags & 8 != 0 {
                continue;
            }
            t_hp = t_hp.max(c.hit_points_max as i32);
            t_mp = t_mp.max(c.magic_points_max as i32);
            if c.flags & 0x80 != 0 {
                need_poison |= 1 << i;
                t_ha = t_ha.max(c.hit_points_cur as i32);
            } else if (c.hit_points_cur as i32) < c.hit_points_max as i32 {
                need_heal |= 1 << i;
            }
            if (c.magic_points_cur as i32) < c.magic_points_max as i32 {
                need_magic |= 1 << i;
            }
            c.flags |= 0x1000;
        }
        self.remove_input_top();
        if need_heal != 0 || need_magic != 0 {
            self.screen.fill_rect(112, 0, 288, 120, 1, None);
            self.gui_draw_all_char_portraits_with_stats();
            let s = self.get_lang_string(0x4057);
            self.print_message(0x8000, b"%s", &[Arg::Str(s)]);
            self.gui_toggle_button_display_mode(77, 0);
            let h = (600 / t_hp.max(1)).min(30) as u32;
            let m = (600 / t_mp.max(1)).min(30) as u32;
            let a = (600 / t_ha.max(1)).min(15) as u32;
            let mut d1 = self.millis() + h * TICK;
            let mut d2 = self.millis() + m * TICK;
            let mut d3 = self.millis() + a * TICK;
            self.party_awake = false;
            self.update_flags |= 1;
            let im = if self.smooth_scroll_mode_normal != 0 {
                32
            } else {
                16
            };
            for _ in 0..im {
                self.timer_process_monsters(0);
                self.timer_process_monsters(1);
                self.timer_process_doors();
                self.timer_process_flying_objects();
                if self.party_awake {
                    break;
                }
            }
            self.reset_block_properties();
            loop {
                let im = if self.smooth_scroll_mode_normal != 0 {
                    8
                } else {
                    4
                };
                for _ in 0..im {
                    self.timer_process_monsters(0);
                    self.timer_process_monsters(1);
                    self.timer_process_doors();
                    self.timer_process_flying_objects();
                    if self.party_awake {
                        break;
                    }
                }
                let f = self.check_input(None, false, 0);
                self.remove_input_top();
                if f & 0x800 != 0 {
                    self.gui_notify_button_list_changed();
                } else if f != 0 {
                    self.gui_trigger_event(f);
                    break;
                }
                if !self.party_awake {
                    if self.millis() > d3 {
                        for i in 0..4 {
                            if need_poison & (1 << i) == 0 {
                                continue;
                            }
                            self.inflict_damage(i, 1, 0x8000, 1, 0x80);
                            if self.characters[i as usize].flags & 8 != 0 {
                                need_poison &= !(1 << i);
                            }
                        }
                        d3 = self.millis() + a * TICK;
                    }
                    if self.millis() > d1 {
                        for i in 0..4 {
                            if need_heal & (1 << i) == 0 {
                                continue;
                            }
                            self.increase_character_hitpoints(i, 1, false);
                            self.gui_draw_char_portrait_with_stats(i);
                            if self.characters[i].hit_points_cur as i32
                                == self.characters[i].hit_points_max as i32
                            {
                                need_heal &= !(1 << i);
                            }
                        }
                        d1 = self.millis() + h * TICK;
                    }
                    if self.millis() > d2 {
                        for i in 0..4 {
                            if need_magic & (1 << i) == 0 {
                                continue;
                            }
                            self.characters[i].magic_points_cur += 1;
                            self.gui_draw_char_portrait_with_stats(i);
                            if self.characters[i].magic_points_cur as i32
                                == self.characters[i].magic_points_max as i32
                            {
                                need_magic &= !(1 << i);
                            }
                        }
                        d2 = self.millis() + m * TICK;
                    }
                    self.screen.update_screen();
                }
                if self.party_awake || (need_heal == 0 && need_magic == 0) || self.should_quit() {
                    break;
                }
                self.delay(TICK, false);
            }
            for i in 0..4 {
                let (mut frm, mut upd, mut setframe) = (0, 0, true);
                if self.characters[i].flags & 0x1000 != 0 {
                    self.characters[i].flags &= 0xEFFF;
                    if self.party_awake {
                        if self.characters[i].damage_suffered != 0 {
                            frm = 5;
                            let s = self.characters[i].scream_sfx as i32;
                            self.snd_play_sound_effect(s, -1);
                        } else {
                            frm = 4;
                        }
                        upd = 6;
                    }
                } else if self.characters[i].damage_suffered != 0 {
                    setframe = false;
                } else {
                    frm = 4;
                }
                if setframe {
                    self.set_temporary_face_frame(i, frm, upd, true);
                }
            }
            self.update_flags &= 0xFFFE;
            self.party_awake = true;
            self.update_draw_page2();
            self.gui_draw_scene(0);
            let s = self.get_lang_string(0x4059);
            self.print_message(0x8000, b"%s", &[Arg::Str(s)]);
            self.screen.fade_to_palette1(40);
        } else {
            for i in 0..4 {
                self.characters[i].flags &= 0xEFFF;
            }
            if need_poison != 0 {
                self.set_temporary_face_frame_for_all_characters(0, 0, false);
                for i in 0..4 {
                    if need_poison & (1 << i) != 0 {
                        self.set_temporary_face_frame(i, 3, 8, false);
                    }
                }
                let s = self.get_lang_string(0x405A);
                self.print_message(0x8000, b"%s", &[Arg::Str(s)]);
                self.gui_draw_all_char_portraits_with_stats();
            } else {
                self.set_temporary_face_frame_for_all_characters(2, 4, true);
                let s = self.get_lang_string(0x4058);
                self.print_message(0x8000, b"%s", &[Arg::Str(s)]);
            }
            self.gui_toggle_button_display_mode(77, 0);
        }
        1
    }

    pub fn clicked_money_box(&mut self, _b: &Button) -> i32 {
        let fmt = self.get_lang_string(if self.credits == 1 { 0x402D } else { 0x402E });
        let c = self.credits as i32;
        self.print_message(0, &fmt, &[c.into()]);
        1
    }

    pub fn clicked_compass(&mut self, _b: &Button) -> i32 {
        if self.flags_table[31] & 0x40 == 0 {
            return 0;
        }
        if self.compass_broken != 0 {
            if self.character_says(0x425B, -1, true) != 0 {
                let s = self.get_lang_string(0x425B);
                self.print_message(4, b"%s", &[Arg::Str(s)]);
            }
        } else {
            let s = self.get_lang_string(0x402F + self.current_direction);
            self.print_message(0, b"%s", &[Arg::Str(s)]);
        }
        1
    }

    pub fn clicked_automap(&mut self, _b: &Button) -> i32 {
        if self.flags_table[31] & 0x10 == 0 {
            return 0;
        }
        self.remove_input_top();
        self.display_automap();
        self.gui_draw_play_field();
        let p = self.screen.palette(0).clone();
        let (br, le) = (self.brightness, self.lamp_effect as i32);
        self.set_palette_brightness(&p, br, le);
        1
    }

    pub fn clicked_lamp(&mut self, _b: &Button) -> i32 {
        if self.flags_table[31] & 0x08 == 0 {
            return 0;
        }
        if self.items_in_play[self.item_in_hand as usize].item_property_index == 248 {
            if self.lamp_oil_status >= 100 {
                let s = self.get_lang_string(0x4061);
                self.print_message(0, b"%s", &[Arg::Str(s)]);
                return 1;
            }
            let s = self.get_lang_string(0x4062);
            self.print_message(0, b"%s", &[Arg::Str(s)]);
            let ih = self.item_in_hand;
            self.delete_item(ih);
            self.snd_play_sound_effect(181, -1);
            self.set_hand_item(0);
            self.lamp_oil_status += 100;
        } else {
            let s = if self.lamp_oil_status >= 100 {
                0x4060
            } else if self.lamp_oil_status == 0 {
                0x405C
            } else {
                (self.lamp_oil_status / 33) as u16 + 0x405D
            };
            let fmt = self.get_lang_string(0x405B);
            let st = self.get_lang_string(s);
            self.print_message(0, &fmt, &[Arg::Str(st)]);
        }
        if self.brightness != 0 {
            let p = self.screen.palette(0).clone();
            let (br, le) = (self.brightness, self.lamp_effect as i32);
            self.set_palette_brightness(&p, br, le);
        }
        1
    }

    pub fn clicked_status_icon(&mut self, _b: &Button) -> i32 {
        let t = ((self.mouse_x - 220).max(0) / 14).min(2) as usize;
        let s = self.char_status_flags[t].wrapping_add(1);
        if s == 0 || s > 3 {
            return 1;
        }
        let st = self.get_lang_string(match s {
            1 => 0x424C,
            2 => 0x424E,
            _ => 0x424D,
        });
        self.print_message(0x8002, b"%s", &[Arg::Str(st)]);
        1
    }

    /// `KyraRpgEngine::gui_drawBox`.
    pub fn gui_draw_box(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        frame1: u8,
        frame2: u8,
        fill: i32,
    ) {
        let w = w - 1;
        let h = h - 1;
        if fill != -1 {
            self.screen
                .fill_rect(x + 1, y + 1, x + w - 1, y + h - 1, fill as u8, None);
        }
        self.screen.draw_clipped_line(x + 1, y, x + w, y, frame2);
        self.screen
            .draw_clipped_line(x + w, y, x + w, y + h - 1, frame2);
        self.screen.draw_clipped_line(x, y, x, y + h, frame1);
        self.screen
            .draw_clipped_line(x, y + h, x + w, y + h, frame1);
    }

    // ---- buttons (GUI_LoL)

    fn button_list(&self, id: ButtonListId) -> &Vec<Button> {
        match id {
            ButtonListId::Active => &self.buttons,
            ButtonListId::Menu => &self.menu.buttons,
        }
    }

    fn button_list_mut(&mut self, id: ButtonListId) -> &mut Vec<Button> {
        match id {
            ButtonListId::Active => &mut self.buttons,
            ButtonListId::Menu => &mut self.menu.buttons,
        }
    }

    /// `GUI_LoL::processButton`: a button drawn as it should look now.
    pub fn process_button(&mut self, b: &Button) {
        if b.flags & 8 != 0 {
            return;
        }
        let entry = b.flags2 & 5;
        let (val1, shape, callback, val2, val3) = if entry == 1 {
            (
                b.data1_val1,
                b.data1_shape.clone(),
                b.data1_callback,
                b.data1_val2,
                b.data1_val3,
            )
        } else if entry == 4 || entry == 5 {
            (
                b.data2_val1,
                b.data2_shape.clone(),
                b.data2_callback,
                b.arg as u8,
                b.data2_val3,
            )
        } else {
            (
                b.data0_val1,
                b.data0_shape.clone(),
                b.data0_callback,
                b.data0_val2,
                b.data0_val3,
            )
        };
        let dim = self.screen.dim(b.dim_table_index as usize);
        let mut x = b.x as i32;
        if x < 0 {
            x += dim.w << 3;
        }
        x += dim.sx << 3;
        let x2 = x + b.width as i32 - 1;
        let mut y = b.y as i32;
        if y < 0 {
            y += dim.h << 3;
        }
        y += dim.sy << 3;
        let y2 = y + b.height as i32 - 1;
        match val1 as i32 - 1 {
            0 => {
                if let Some(s) = shape {
                    let page = self.screen.cur_page;
                    self.screen.draw_shape(
                        page,
                        &s,
                        x,
                        y,
                        b.dim_table_index as usize,
                        0x10,
                        ShapeArgs::none(),
                    );
                }
            }
            1 => {
                if let Some(s) = shape {
                    self.screen.print_text(&s, x, y, val2, val3);
                }
            }
            3 => {
                self.run_button_callback(callback, b);
            }
            4 => self.screen.draw_box(x, y, x2, y2, val2),
            5 => self.screen.fill_rect_xor(x, y, x2, y2, val2, None),
            _ => {}
        }
    }

    fn run_button_callback(&mut self, cb: Callback, b: &Button) -> i32 {
        match cb {
            Callback::None => 0,
            Callback::Playfield(i) => self.run_playfield_callback(i, b),
            Callback::Menu(m) => self.run_menu_callback(m, b),
            Callback::ScrollUp => self.menu_scroll_up(b),
            Callback::ScrollDown => self.menu_scroll_down(b),
            Callback::RedrawButton => self.menu_redraw_button(b, false),
            Callback::RedrawShadedButton => self.menu_redraw_button(b, true),
        }
    }

    /// `GUI_LoL::processButtonList`.
    pub fn process_button_list(
        &mut self,
        id: ButtonListId,
        mut input_flag: i32,
        mouse_wheel: i8,
    ) -> i32 {
        if self.button_list(id).is_empty() {
            return input_flag & 0x7FFF;
        }
        if self.gui_state.backup_list != Some(id) || self.button_list_changed {
            self.gui_state.special_process_button = None;
            self.gui_state.flags_modifier = 0;
            if self.mouse_click == 1 {
                self.gui_state.flags_modifier |= 0x200;
            }
            if self.mouse_click == 2 {
                self.gui_state.flags_modifier |= 0x2000;
            }
            self.mouse_click = 0;
            self.gui_state.backup_list = Some(id);
            self.button_list_changed = false;
            let list = self.button_list(id).clone();
            for b in &list {
                self.process_button(b);
            }
        }
        let (mouse_x, mouse_y) = (self.mouse_x, self.mouse_y);
        let mut flags: u16 = 0;
        {
            let in_flags = (input_flag & 0xFF) as u16;
            let mut temp: u16 = 0;
            if in_flags == 199 || in_flags == 200 {
                temp = 0x100;
            }
            if in_flags == 201 || in_flags == 202 {
                temp = 0x1000;
            }
            if input_flag & 0x800 != 0 {
                temp <<= 2;
            }
            flags |= temp;
            let fm = &mut self.gui_state.flags_modifier;
            *fm &= !((temp & 0x4400) >> 1);
            *fm |= (temp & 0x1100) * 2;
            flags |= *fm;
            flags |= (*fm << 2) ^ 0x8800;
        }
        let mut start = 0usize;
        if let Some(sp) = self.gui_state.special_process_button {
            start = sp;
            if self
                .button_list(id)
                .get(sp)
                .is_some_and(|b| b.flags & 8 != 0)
            {
                self.gui_state.special_process_button = None;
            }
        }
        let mut return_value = 0;
        let mut i = start;
        while i < self.button_list(id).len() {
            if self.button_list(id)[i].flags & 8 != 0 {
                i += 1;
                continue;
            }
            let dim = self
                .screen
                .dim(self.button_list(id)[i].dim_table_index as usize);
            let special = self.gui_state.special_process_button;
            let b = &mut self.button_list_mut(id)[i];
            b.flags2 &= !0x18;
            b.flags2 |= (b.flags2 & 3) << 3;
            let mut x = b.x as i32;
            if x < 0 {
                x += dim.w << 3;
            }
            x += dim.sx << 3;
            let mut y = b.y as i32;
            if y < 0 {
                y += dim.h;
            }
            y += dim.sy;
            let mut progress = mouse_x >= x
                && mouse_y >= y
                && mouse_x <= x + b.width as i32
                && mouse_y <= y + b.height as i32;
            b.flags2 &= !0x80;
            let in_flags = (input_flag & 0x7FFF) as u16;
            let mut new_special = special;
            if in_flags != 0 {
                if b.key_code == in_flags {
                    progress = true;
                    flags = b.flags & 0x0F00;
                    b.flags2 |= 0x80;
                    input_flag = 0;
                    new_special = Some(i);
                } else if b.key_code2 == in_flags {
                    flags = b.flags & 0xF000;
                    if flags == 0 {
                        flags = b.flags & 0x0F00;
                    }
                    progress = true;
                    b.flags2 |= 0x80;
                    input_flag = 0;
                    new_special = Some(i);
                }
            }
            let mut unk1 = false;
            if mouse_wheel != 0 && b.mouse_wheel == mouse_wheel {
                progress = true;
                unk1 = true;
            }
            if !progress {
                b.flags2 &= !6;
            }
            if flags & 0x3300 != 0
                && b.flags & 4 != 0
                && progress
                && (new_special == Some(i) || new_special.is_none())
            {
                b.flags |= 6;
                if new_special.is_none() {
                    new_special = Some(i);
                }
            } else if flags & 0x8800 != 0 && b.flags & 4 == 0 && progress {
                b.flags2 |= 6;
            } else {
                b.flags2 &= !6;
            }
            let progress_switch = match new_special {
                None => progress,
                Some(sp) => {
                    if sp == i {
                        if b.flags & 0x40 != 0 { true } else { progress }
                    } else {
                        // The special button's flag 0x40 decides; looked up after this borrow.
                        progress
                    }
                }
            };
            let _ = progress_switch;
            self.gui_state.special_process_button = new_special;
            let sp_flags40 = new_special
                .and_then(|sp| self.button_list(id).get(sp))
                .is_some_and(|sb| sb.flags & 0x40 != 0);
            let progress_switch = match new_special {
                None => progress,
                Some(sp) => {
                    if sp_flags40 {
                        sp == i
                    } else {
                        progress
                    }
                }
            };
            let mut special = self.gui_state.special_process_button;
            let b = &mut self.button_list_mut(id)[i];
            if progress_switch {
                if flags & 0x1100 != 0 && progress && special.is_none() {
                    input_flag = 0;
                    special = Some(i);
                }
                if b.flags & flags != 0 && (progress || b.flags & 1 == 0) {
                    let mut combined = b.flags & flags;
                    combined = ((combined & 0xF000) >> 4) | (combined & 0x0F00);
                    combined >>= 8;
                    const FLAG_TABLE: [u16; 16] = [
                        0x000, 0x100, 0x200, 0x100, 0x400, 0x100, 0x400, 0x100, 0x800, 0x100,
                        0x200, 0x100, 0x400, 0x100, 0x400, 0x100,
                    ];
                    match FLAG_TABLE[(combined & 15) as usize] {
                        0x400 => {
                            if b.flags & 1 == 0 || (b.flags & 1 != 0 && special == Some(i)) {
                                b.flags2 ^= 1;
                                return_value = b.index as i32 | 0x8000;
                                unk1 = true;
                            }
                            if b.flags & 4 == 0 {
                                b.flags2 &= !4;
                                b.flags2 &= !2;
                            }
                        }
                        0x800 => {
                            if b.flags & 4 == 0 {
                                b.flags2 |= 4;
                                b.flags2 |= 2;
                            }
                            if b.flags & 1 == 0 {
                                unk1 = true;
                            }
                        }
                        0x200 => {
                            if b.flags & 4 != 0 {
                                b.flags2 |= 4;
                                b.flags2 |= 2;
                            }
                            if b.flags & 1 == 0 {
                                unk1 = true;
                            }
                        }
                        _ => {
                            b.flags2 ^= 1;
                            return_value = b.index as i32 | 0x8000;
                            unk1 = true;
                            if b.flags & 4 != 0 {
                                b.flags2 |= 4;
                                b.flags2 |= 2;
                            }
                            special = Some(i);
                        }
                    }
                }
            }
            let mut unk2 = false;
            if flags & 0x2200 != 0 && progress {
                b.flags2 |= 6;
                if b.flags & 4 == 0 && b.flags2 & 1 == 0 {
                    unk2 = true;
                    b.flags2 |= 1;
                }
            }
            if flags & 0x8800 == 0x8800 {
                special = None;
                if !progress || b.flags & 4 != 0 {
                    b.flags2 &= !6;
                }
            }
            if !progress && special == Some(i) && b.flags & 0x40 == 0 {
                special = None;
            }
            self.gui_state.special_process_button = special;
            let snapshot = self.button_list(id)[i].clone();
            if (snapshot.flags2 & 0x18) != ((snapshot.flags2 & 3) << 3) {
                self.process_button(&snapshot);
            }
            if unk2 {
                self.button_list_mut(id)[i].flags2 &= !1;
            }
            if unk1 {
                {
                    let b = &mut self.button_list_mut(id)[i];
                    b.flags2 &= 0xFF;
                    b.flags2 |= flags;
                }
                let b = self.button_list(id)[i].clone();
                if b.button_callback != Callback::None {
                    let changed_before = self.gui_state.list_serial;
                    let r = self.run_button_callback(b.button_callback, &b);
                    if r != 0 {
                        break;
                    }
                    if self.gui_state.list_serial != changed_before
                        || i >= self.button_list(id).len()
                    {
                        break;
                    }
                }
                if b.flags & 0x20 != 0 {
                    break;
                }
            }
            if self.gui_state.special_process_button == Some(i)
                && self
                    .button_list(id)
                    .get(i)
                    .is_some_and(|b| b.flags & 0x40 != 0)
            {
                break;
            }
            i += 1;
        }
        if return_value == 0 {
            return_value = input_flag & 0x7FFF;
        }
        return_value
    }
}

/// Keys of the key map, for putting events back (`gui_triggerEvent`; their codes come from
/// `Lol::key_code`).
const KEYS: [crate::platform::Key; 13] = {
    use crate::platform::Key;
    [
        Key::Space,
        Key::Enter,
        Key::Up,
        Key::Right,
        Key::Down,
        Key::Left,
        Key::Home,
        Key::PageUp,
        Key::End,
        Key::PageDown,
        Key::Escape,
        Key::Pad(5),
        Key::Tab,
    ]
};

/// The character keys of the key map.
const KEY_CHARS: &[u8] = b"abcdefikmnoprswyz1234567/-+,.";

/// The state `GUI_LoL` keeps between calls.
#[derive(Debug, Clone, Default)]
pub struct GuiState {
    pub backup_list: Option<ButtonListId>,
    pub special_process_button: Option<usize>,
    pub flags_modifier: u16,
    /// Changes whenever a button list is rebuilt (the original's lists are reused memory).
    pub list_serial: u32,
}
