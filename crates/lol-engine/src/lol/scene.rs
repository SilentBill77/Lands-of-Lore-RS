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

//! The level and the view (ScummVM's `scene_lol.cpp` and `scene_rpg.cpp`): loading a level,
//! the walls and decorations drawn block by block, moving and turning with smooth scrolling,
//! the lamp and the compass, doors, levers and niches.

use lol_data::tables::{
    BASE_DSC_Y, DSC_BLOCK_INDEX, DSC_BLOCK_MAP, DSC_DIM_DATA1, DSC_DIM_DATA2, DSC_DIM_MAP,
    DSC_DOOR_FRAME_Y1, DSC_DOOR_FRAME_Y2, DSC_DOOR_SCALE_OFFS, DSC_DOOR_SHAPE_INDEX, DSC_OVL_INDEX,
    DSC_OVL_MAP, DSC_SCALE_HEIGHT_DATA, DSC_SCALE_WIDTH_DATA, DSC_SHAPE_INDEX, DSC_TILE_INDEX,
    DSC_WALLS, DSC_X, LEVEL_DAT_LIST, LEVEL_SHP_LIST, SCROLL_X_BOTTOM, SCROLL_X_TOP,
    SCROLL_Y_BOTTOM, SCROLL_Y_TOP,
};

use super::sprites::SCENE_SHP_DIM;
use super::types::*;
use super::{LevelDecoration, Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, Pal, generate_overlay};
use crate::shape::{self, ShapeArgs};

/// `_vmpOffsetsDefault`.
const VMP_VIS_OFFS: [i32; 9] = [102, 97, 129, 117, 81, 159, 45, 239, 0];
/// `_sceneXoffset`.
const SCENE_X_OFFSET: i32 = 112;
/// `_clickedSpecialFlag`.
const CLICKED_SPECIAL_FLAG: i32 = 0x40;

impl Lol {
    // ---- loading a level

    pub fn load_level(&mut self, index: i32) {
        self.flags_table[73] |= 0x08;
        self.set_mouse_cursor_to_icon(0x85);
        self.next_script_func = 0;
        self.snd_stop_music();
        self.stop_portrait_speech_anim();
        for s in self.level_decoration_shapes.iter_mut() {
            *s = None;
        }
        self.script_data = None;
        self.reset_items(1);
        self.disable_monsters();
        self.reset_block_properties();
        self.release_monster_shapes(0);
        self.release_monster_shapes(1);
        for i in 0x50..0x53 {
            self.timers.disable(i);
        }
        self.current_level = index;
        self.update_flags = 0;
        self.set_default_button_state();
        self.load_talk_file(index);
        self.load_level_wall_data(index, true);
        self.load_level_flag = 1;
        let f = self.has_temp_data_flags & (1 << (index - 1));
        self.run_init_script(&format!("LEVEL{index}.INI"), if f != 0 { 0 } else { 1 });
        if f != 0 {
            self.restore_block_temp_data(index);
        }
        self.run_inf_script(&format!("LEVEL{index}.INF"));
        if std::env::var_os("LOL_DEBUG").is_some() {
            eprintln!(
                "Ebene {index}: Block {} Richtung {} ({}, {})",
                self.current_block, self.current_direction, self.party_pos_x, self.party_pos_y
            );
        }
        self.add_level_items();
        let cb = self.current_block as usize;
        self.delete_monsters_from_block(cb);
        let p0 = self.screen.palette(0).clone();
        self.screen.gray_overlay = self.generate_gray_overlay(&p0, 32, 16, 0, 0, 128, true);
        self.scene_default_update = 0;
        if self.screen.fade_flag == 3 {
            self.screen.fade_to_black(10);
        }
        self.gui_draw_play_field();
        let p0 = self.screen.palette(0).clone();
        let (br, le) = (self.brightness, self.lamp_effect as i32);
        self.set_palette_brightness(&p0, br, le);
        self.set_mouse_cursor_to_item_in_hand();
        let t = self.cur_music_theme;
        self.snd_play_track(t);
    }

    pub fn add_level_items(&mut self) {
        for i in 0..400 {
            if self.items_in_play[i].level as i32 != self.current_level {
                continue;
            }
            let b = self.items_in_play[i].obj.block as usize;
            self.assign_block_item(b, i as u16);
            self.level_block_properties[b].direction = 5;
            self.items_in_play[i].obj.next_draw_object = 0;
        }
    }

    pub fn load_level_wall_data(&mut self, index: i32, map_shapes: bool) {
        let file = self
            .res
            .file_data(&format!("LEVEL{index}.WLL"))
            .unwrap_or_default();
        if file.len() < 2 {
            return;
        }
        let c = u16::from_le_bytes([file[0], file[1]]) as usize;
        self.load_level_shp_dat(LEVEL_SHP_LIST[c.min(11)], LEVEL_DAT_LIST[c.min(11)], false);
        let n = (file.len() - 2) / 12;
        for i in 0..n {
            let d = &file[2 + i * 12..2 + i * 12 + 12];
            let c = u16::from_le_bytes([d[0], d[1]]) as usize & 0xFF;
            self.wll_vmp_map[c] = d[2];
            if map_shapes {
                let sh = i16::from_le_bytes([d[4], d[5]]);
                self.wll_shape_map[c] = if sh > 0 {
                    self.assign_level_decoration_shapes(sh as usize) as i8
                } else {
                    d[4] as i8
                };
            }
            self.special_wall_types[c] = d[6];
            self.wll_wall_flags[c] = d[8];
            self.wll_automap_data[c] = d[10];
        }
        self.lvl_shp_file = None;
    }

    fn temp_u16(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.temp_buffer5120[at], self.temp_buffer5120[at + 1]])
    }

    fn set_temp_u16(&mut self, at: usize, v: u16) {
        self.temp_buffer5120[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    pub fn assign_level_decoration_shapes(&mut self, index: usize) -> u16 {
        // p1: shapes already loaded (at 0), p2: decorations already mapped (at 4000).
        let r = self.temp_u16(4000 + index * 2);
        if r != 0 {
            return r;
        }
        let o = self.mapped_decorations_count as usize;
        self.mapped_decorations_count += 1;
        if o >= self.level_decoration_properties.len() {
            return 0;
        }
        self.level_decoration_properties[o] = self
            .level_decoration_data
            .get(index)
            .copied()
            .unwrap_or_default();
        for i in 0..10 {
            let t = self.level_decoration_properties[o].shape_index[i];
            if t == 0xFFFF {
                continue;
            }
            let pv = self.temp_u16(t as usize * 2);
            if pv != 0 {
                self.level_decoration_properties[o].shape_index[i] = pv;
            } else {
                let li = self.lvl_shape_index as usize;
                self.release_decorations(li, 1);
                if li < self.level_decoration_shapes.len() {
                    self.level_decoration_shapes[li] = self.get_level_decoration_shapes(t as usize);
                }
                self.set_temp_u16(t as usize * 2, li as u16);
                self.level_decoration_properties[o].shape_index[i] = li as u16;
                self.lvl_shape_index += 1;
            }
        }
        self.set_temp_u16(4000 + index * 2, o as u16);
        let next = self.level_decoration_properties[o].next;
        if next != 0 {
            self.level_decoration_properties[o].next =
                self.assign_level_decoration_shapes(next as u8 as usize) as i8;
        }
        o as u16
    }

    pub fn get_level_decoration_shapes(&self, shape_index: usize) -> Option<Vec<u8>> {
        if self.decoration_count as usize <= shape_index {
            return None;
        }
        let f = self.lvl_shp_file.as_ref()?;
        let at = shape_index * 4 + 2;
        let offs = u32::from_le_bytes(f.get(at..at + 4)?.try_into().ok()?) as usize + 2;
        let start = f.get(offs..)?;
        let size = shape::size(start).min(start.len());
        Some(start[..size].to_vec())
    }

    pub fn release_decorations(&mut self, first: usize, num: usize) {
        for i in first..(first + num).min(self.level_decoration_shapes.len()) {
            self.level_decoration_shapes[i] = None;
        }
    }

    pub fn load_block_properties(&mut self, cmz_file: &str) {
        for b in self.level_block_properties.iter_mut().take(1024) {
            *b = LevelBlockProperty::default();
        }
        let Some(d) = self.res.file_data(cmz_file) else {
            return;
        };
        self.screen.load_bitmap(&d, 2, None);
        let h = self.screen.page(2).to_vec();
        let len = u16::from_le_bytes([h[4], h[5]]) as usize;
        for i in 0..1024 {
            for ii in 0..4 {
                self.level_block_properties[i].walls[ii] =
                    h.get(6 + i * len + ii).copied().unwrap_or(0);
            }
            self.level_block_properties[i].direction = 5;
            if self.wll_automap_data[self.level_block_properties[i].walls[0] as usize] == 17 {
                self.level_block_properties[i].flags &= 0xEF;
                self.level_block_properties[i].flags |= 0x20;
            }
        }
    }

    /// `getBlockFileData`: the level's block file unpacked (into pages 14/15).
    pub fn get_block_file_data(&mut self, level: i32) -> Vec<u8> {
        if let Some(d) = self.res.file_data(&format!("LEVEL{level}.CMZ")) {
            self.screen.load_bitmap(&d, 15, None);
        }
        self.screen.page(14).to_vec()
    }

    pub fn load_level_shp_dat(&mut self, shp_file: &str, dat_file: &str, flag: bool) {
        self.temp_buffer5120.fill(0);
        self.lvl_shp_file = self.res.file_data(shp_file);
        self.decoration_count = self
            .lvl_shp_file
            .as_ref()
            .map_or(0, |f| u16::from_le_bytes([f[0], f[1]]));
        let s = self.res.file_data(dat_file).unwrap_or_default();
        let n = if s.len() >= 2 {
            u16::from_le_bytes([s[0], s[1]]) as usize
        } else {
            0
        };
        self.level_decoration_data = (0..n)
            .map(|i| {
                let e = s.get(2 + i * 72..2 + i * 72 + 72).unwrap_or(&[0u8; 72]);
                let w = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]);
                LevelDecoration {
                    shape_index: std::array::from_fn(|k| w(k * 2)),
                    scale_flag: std::array::from_fn(|k| e[20 + k]),
                    shape_x: std::array::from_fn(|k| w(30 + k * 2) as i16),
                    shape_y: std::array::from_fn(|k| w(50 + k * 2) as i16),
                    next: e[70] as i8,
                    flags: e[71],
                }
            })
            .collect();
        if !flag {
            self.mapped_decorations_count = 1;
            self.lvl_shape_index = 1;
        }
    }

    pub fn load_level_graphics(
        &mut self,
        file: Option<&str>,
        special_color: i32,
        weight: i32,
        vcn_len: i32,
        vmp_len: i32,
        pal_file: Option<&str>,
    ) {
        if let Some(f) = file {
            self.last_special_color = special_color;
            self.last_special_color_weight = weight;
            self.last_block_data_file = f.to_string();
            self.last_override_pal_file = pal_file.map(|s| s.to_string()).unwrap_or_default();
        }
        let base = self.last_block_data_file.clone();
        let vcn = self
            .res
            .file_data(&format!("{base}.VCN"))
            .unwrap_or_default();
        self.screen.load_bitmap(&vcn, 3, None);
        let v = self.screen.page(2).to_vec();
        let tlen = u16::from_le_bytes([v[0], v[1]]) as usize;
        let mut p = 2;
        let vcn_len = if vcn_len == -1 {
            tlen << 5
        } else {
            vcn_len as usize
        };
        self.vcn_shift = v[p..p + tlen].to_vec();
        p += tlen;
        self.vcn_col_table.copy_from_slice(&v[p..p + 128]);
        p += 128;
        if !self.last_override_pal_file.is_empty() {
            if let Some(d) = self.res.file_data(&self.last_override_pal_file.clone()) {
                let n = d.len().min(384);
                self.screen.palette_mut(0).0[..n].copy_from_slice(&d[..n]);
            }
        } else {
            let pal = v[p..p + 384].to_vec();
            self.screen.palette_mut(0).0[..384].copy_from_slice(&pal);
        }
        p += 384;
        if self.current_level == 11 {
            let mut p2 = Pal::default();
            if let Some(d) = self.res.file_data("SWAMPICE.COL") {
                p2.load(&d);
            }
            let p0 = self.screen.palette(0).clone();
            p2.copy_from(&p0, 128, 128, 128);
            *self.screen.palette_mut(2) = p2;
            if self.flags_table[52] & 0x04 != 0 {
                let mut a = self.screen.palette(0).clone();
                let mut b = self.screen.palette(2).clone();
                for i in 1..a.0.len().min(b.0.len()) {
                    std::mem::swap(&mut a.0[i], &mut b.0[i]);
                }
                *self.screen.palette_mut(0) = a;
                *self.screen.palette_mut(2) = b;
            }
        }
        self.vcn_blocks = v
            .get(p..p + vcn_len)
            .map(|s| s.to_vec())
            .unwrap_or_else(|| v[p..].to_vec());
        let vmp = self
            .res
            .file_data(&format!("{base}.VMP"))
            .unwrap_or_default();
        self.screen.load_bitmap(&vmp, 3, None);
        let v = self.screen.page(2).to_vec();
        let vmp_len = if vmp_len == -1 {
            u16::from_le_bytes([v[0], v[1]]) as usize
        } else {
            vmp_len as usize
        };
        self.vmp = (0..vmp_len)
            .map(|i| u16::from_le_bytes([v[2 + i * 2], v[3 + i * 2]]))
            .collect();
        let tpal = self.screen.palette(0).clone();
        for i in 0..7 {
            let w = 100 - (i as i32 * self.last_special_color_weight);
            let w = if w > 0 { (w * 255) / 100 } else { 0 };
            let mut ovl = generate_overlay(&tpal, self.last_special_color as usize, w as u32, 127);
            for o in ovl.iter_mut().take(128) {
                if *o == 255 {
                    *o = 0;
                }
            }
            for (ii, o) in ovl.iter_mut().enumerate().skip(128) {
                *o = ii as u8;
            }
            self.screen.level_overlays[i] = ovl;
        }
        self.screen.level_overlays[7] = (0..=255u8).collect();
        self.load_supp_files_flag = 0;
        let p0 = self.screen.palette(0).clone();
        let (br, le) = (self.brightness, self.lamp_effect as i32);
        let p1 = self.generate_brightness_palette(&p0, br, le);
        *self.screen.palette_mut(1) = p1;
        if let Some(d) = self
            .res
            .file_data(&format!("LEVEL{:02}.TLC", self.current_level))
            && d.len() >= 256 + 5120
        {
            self.transparency_table1 = d[..256].to_vec();
            self.transparency_table2 = d[256..256 + 5120].to_vec();
        }
        self.load_supp_files_flag = 1;
    }

    pub fn reset_items(&mut self, flag: i32) {
        for i in 0..1024 {
            self.level_block_properties[i].direction = 5;
            let mut id = self.level_block_properties[i].assigned_objects;
            let mut r: Option<u16> = None;
            let mut guard = 0;
            while id & 0x8000 != 0 && guard < 100 {
                guard += 1;
                r = Some(id);
                id = self.obj(id).next_assigned_object;
            }
            if id == 0 {
                continue;
            }
            let it = &mut self.items_in_play[id as usize];
            it.level = self.current_level as i8;
            it.obj.block = i as u16;
            if let Some(r) = r {
                self.link_set_next_assigned(r, 0);
            }
        }
        if flag != 0 {
            self.flying_objects = [FlyingObject::default(); 8];
        }
    }

    fn link_set_next_assigned(&mut self, id: u16, v: u16) {
        if let Some(o) = self.obj_mut(id) {
            o.next_assigned_object = v;
        }
    }

    pub fn disable_monsters(&mut self) {
        for m in self.monsters.iter_mut() {
            *m = Monster::default();
            m.mode = 0x10;
        }
    }

    pub fn reset_block_properties(&mut self) {
        for i in 0..1024 {
            let f = self.level_block_properties[i].flags;
            if f & 0x10 != 0 {
                self.level_block_properties[i].flags &= 0xEF;
                if self.test_wall_invisibility(i, 0) && self.test_wall_invisibility(i, 1) {
                    self.level_block_properties[i].flags |= 0x40;
                }
            } else if f & 0x40 != 0 {
                self.level_block_properties[i].flags &= 0xBF;
            } else if f & 0x80 != 0 {
                self.level_block_properties[i].flags &= 0x7F;
            }
        }
    }

    pub fn test_wall_flag(&self, block: u16, direction: i32, flag: i32) -> bool {
        let b = &self.level_block_properties[block as usize & 0x3FF];
        if b.flags & 0x10 != 0 {
            return true;
        }
        if direction != -1 {
            return self.wll_wall_flags[b.walls[((direction ^ 2) & 3) as usize] as usize] as i32
                & flag
                != 0;
        }
        b.walls
            .iter()
            .any(|&w| self.wll_wall_flags[w as usize] as i32 & flag != 0)
    }

    pub fn test_wall_invisibility(&self, block: usize, direction: usize) -> bool {
        let w = self.level_block_properties[block].walls[direction] as usize;
        !(self.wll_vmp_map[w] != 0
            || self.wll_shape_map[w] != 0
            || self.level_block_properties[block].flags & 0x80 != 0)
    }

    // ---- lamp and compass

    pub fn reset_lamp_status(&mut self) {
        self.flags_table[31] |= 0x04;
        self.lamp_effect = -1;
        self.update_lamp_status();
    }

    pub fn set_lamp_mode(&mut self, lamp_on: bool) {
        self.flags_table[31] &= 0xFB;
        if self.flags_table[31] & 0x08 == 0 || !lamp_on {
            return;
        }
        let s = self.game_shapes.get(43).cloned().unwrap_or_default();
        self.screen
            .draw_shape(0, &s, 291, 56, 0, 0, ShapeArgs::none());
        self.lamp_effect = 8;
    }

    pub fn update_lamp_status(&mut self) {
        let mut new_lamp: i8;
        if self.update_flags & 4 != 0 || self.flags_table[31] & 0x08 == 0 {
            return;
        }
        if self.brightness == 0 || self.lamp_oil_status == 0 {
            new_lamp = 8;
            if new_lamp != self.lamp_effect && self.screen.fade_flag == 0 {
                let p0 = self.screen.palette(0).clone();
                self.set_palette_brightness(&p0, self.brightness, new_lamp as i32);
            }
        } else {
            let tmp = self.lamp_oil_status.min(100);
            new_lamp = ((3 - ((tmp - 1) / 25)) << 1) as i8;
            if self.lamp_effect == -1 {
                if self.screen.fade_flag == 0 {
                    let p0 = self.screen.palette(0).clone();
                    self.set_palette_brightness(&p0, self.brightness, new_lamp as i32);
                }
                let r = self.roll_dice(1, 30, 0);
                self.lamp_status_timer = self.millis() + (10 + r as u32) * TICK;
            } else if (self.lamp_effect & !1) == (new_lamp & !1) {
                if self.millis() <= self.lamp_status_timer {
                    new_lamp = self.lamp_effect;
                } else {
                    new_lamp = self.lamp_effect ^ 1;
                    let r = self.roll_dice(1, 30, 0);
                    self.lamp_status_timer = self.millis() + (10 + r as u32) * TICK;
                }
            } else if self.screen.fade_flag == 0 {
                let p0 = self.screen.palette(0).clone();
                self.set_palette_brightness(&p0, self.brightness, new_lamp as i32);
            }
        }
        if new_lamp == self.lamp_effect {
            return;
        }
        self.screen.hide_mouse();
        let s = self
            .game_shapes
            .get((35 + new_lamp as i32).max(0) as usize)
            .cloned()
            .unwrap_or_default();
        let p = self.screen.cur_page;
        self.screen
            .draw_shape(p, &s, 291, 56, 0, 0, ShapeArgs::none());
        self.screen.show_mouse();
        self.lamp_effect = new_lamp;
    }

    pub fn update_compass(&mut self) {
        if self.flags_table[31] & 0x40 == 0 || self.update_flags & 4 != 0 {
            return;
        }
        if self.compass_direction == -1 {
            self.compass_step = 0;
            self.gui_draw_compass();
            return;
        }
        if self.compass_timer >= self.millis() {
            return;
        }
        if ((self.current_direction as i32) << 6) == self.compass_direction
            && self.compass_step == 0
        {
            return;
        }
        self.compass_timer = self.millis() + 3 * TICK;
        let dir = if self.compass_step >= 0 { 1 } else { -1 };
        if self.compass_step != 0 {
            self.compass_step -= ((self.compass_step.abs() >> 4) + 2) * dir;
        }
        let mut diff: i16 = if self.compass_broken != 0 {
            (self.rnd.get(255) as u8 as i8 as i16) - self.compass_direction as i16
        } else {
            ((self.current_direction as i16) << 6) - self.compass_direction as i16
        };
        if diff <= -128 {
            diff += 256;
        }
        if diff >= 128 {
            diff -= 256;
        }
        diff >>= 2;
        self.compass_step += diff as i32;
        self.compass_step = self.compass_step.clamp(-24, 24);
        self.compass_direction += self.compass_step;
        if self.compass_direction < 0 {
            self.compass_direction += 256;
        }
        if self.compass_direction > 255 {
            self.compass_direction -= 256;
        }
        if (((self.compass_direction + 3) & 0xFD) >> 6) == self.current_direction as i32
            && self.compass_step < 2
            && diff.abs() < 4
        {
            self.compass_direction = (self.current_direction as i32) << 6;
            self.compass_step = 0;
        }
        self.gui_draw_compass();
    }

    // ---- moving

    pub fn move_party(&mut self, direction: u16, _unk1: i32, unk2: i32, button_shape: i32) {
        if button_shape != 0 {
            self.gui_toggle_button_display_mode(button_shape, 1);
        }
        let opos = self.current_block;
        let npos = self.calc_new_block_position(self.current_block, direction);
        if !self.check_block_passability(npos, direction) {
            self.notify_block_not_passable(if unk2 != 0 { 0 } else { 1 });
            self.gui_toggle_button_display_mode(button_shape, 0);
            return;
        }
        self.script_direction = direction;
        self.current_block = npos;
        self.scene_default_update = 1;
        let (x, y) = self.calc_coordinates(self.current_block, 0x80, 0x80);
        self.party_pos_x = x;
        self.party_pos_y = y;
        self.flags_table[73] &= 0xFD;
        self.run_level_script(opos as i32, 4);
        self.run_level_script(npos as i32, 1);
        if self.flags_table[73] & 0x02 == 0 {
            self.init_text_fading(2, false);
            if self.scene_default_update != 0 {
                match unk2 {
                    0 => self.move_party_smooth_scroll_up(2),
                    1 => self.move_party_smooth_scroll_down(2),
                    2 => self.move_party_smooth_scroll_left(1),
                    3 => self.move_party_smooth_scroll_right(1),
                    _ => {}
                }
            } else {
                self.gui_draw_scene(0);
            }
            self.gui_toggle_button_display_mode(button_shape, 0);
            if npos == self.current_block {
                self.run_level_script(opos as i32, 8);
                self.run_level_script(npos as i32, 2);
                if self.level_block_properties[npos as usize].walls[0] == 0x1A {
                    self.level_block_properties[npos as usize].walls = [0; 4];
                }
            }
        }
        let cb = self.current_block;
        self.update_auto_map(cb);
    }

    pub fn calc_block_index(x: u16, y: u16) -> u16 {
        (((y & 0xFF00) >> 3) | (x >> 8)) & 0x3FF
    }

    pub fn calc_coordinates(&self, block: u16, x_offs: u16, y_offs: u16) -> (u16, u16) {
        (
            ((block & 0x1F) << 8) | x_offs,
            ((block & 0xFFE0) << 3) | y_offs,
        )
    }

    pub fn calc_coordinates_for_single_character(&self, char_num: usize) -> (u16, u16) {
        const XO: [u16; 9] = [0x80, 0x00, 0x00, 0x40, 0xC0, 0x00, 0x40, 0x80, 0xC0];
        let c = self.count_active_characters();
        if c == 0 {
            return (0, 0);
        }
        let c = ((c - 1) * 3 + char_num).min(8);
        let (x, y) =
            Self::calc_coordinates_add_direction_offset(XO[c], 0x80, self.current_direction as i32);
        (
            x | (self.party_pos_x & 0xFF00),
            y | (self.party_pos_y & 0xFF00),
        )
    }

    pub fn calc_coordinates_add_direction_offset(x: u16, y: u16, direction: i32) -> (u16, u16) {
        if direction == 0 {
            return (x, y);
        }
        let (mut tx, mut ty) = (x as i32, y as i32);
        if direction & 1 != 0 {
            std::mem::swap(&mut tx, &mut ty);
        }
        if direction != 1 {
            ty = -(ty - 256);
        }
        if direction != 3 {
            tx = -(tx - 256);
        }
        (tx as u16, ty as u16)
    }

    pub fn check_block_passability(&self, block: u16, direction: u16) -> bool {
        if self.test_wall_flag(block, direction as i32, 1) {
            return false;
        }
        let mut d = self.level_block_properties[block as usize].assigned_objects;
        let mut guard = 0;
        while d != 0 && guard < 1000 {
            guard += 1;
            if d & 0x8000 != 0 {
                return false;
            }
            d = self.obj(d).next_assigned_object;
        }
        true
    }

    pub fn notify_block_not_passable(&mut self, scroll_flag: i32) {
        if scroll_flag != 0 {
            self.move_party_smooth_scroll_blocked(2);
        }
        self.snd_stop_speech(true);
        let s = self.get_lang_string(0x403F);
        self.print_message(0x8002, b"%s", &[Arg::Str(s)]);
        self.snd_play_sound_effect(19, -1);
    }

    pub fn clicked_door_switch(&mut self, block: u16, direction: u16) -> i32 {
        let v = self.wll_shape_map
            [self.level_block_properties[block as usize].walls[direction as usize & 3] as usize];
        if !self.clicked_shape(v as i32) {
            return 0;
        }
        self.snd_play_sound_effect(78, -1);
        self.block_door = 0;
        self.run_level_script(block as i32, 0x40);
        if self.block_door == 0 {
            self.delay(15 * TICK, false);
            self.process_door_switch(block, 0);
        }
        1
    }

    pub fn clicked_niche(&mut self, block: u16, direction: u16) -> i32 {
        let v = self.wll_shape_map
            [self.level_block_properties[block as usize].walls[direction as usize & 3] as usize];
        if !self.clicked_shape(v as i32) || self.item_in_hand == 0 {
            return 0;
        }
        let (x, y) =
            Self::calc_coordinates_add_direction_offset(0x80, 0xFF, self.current_direction as i32);
        let (x, y) = self.calc_coordinates(block, x, y);
        let ih = self.item_in_hand;
        self.set_item_position(ih, x, y, 8, 1);
        self.set_hand_item(0);
        1
    }

    fn smooth_scroll_zoom(&mut self, i: usize, src: usize, dst: usize) {
        self.screen.smooth_scroll_zoom_step(
            src,
            dst,
            SCROLL_X_TOP[i] as i32,
            SCROLL_Y_TOP[i] as i32,
            true,
        );
        self.screen.smooth_scroll_zoom_step(
            src,
            dst,
            SCROLL_X_BOTTOM[i] as i32,
            SCROLL_Y_BOTTOM[i] as i32,
            false,
        );
    }

    pub fn move_party_smooth_scroll_blocked(&mut self, speed: i32) {
        if !self.smooth_scrolling || self.need_scene_restore != 0 {
            return;
        }
        let src = if self.scene_draw_page2 == 2 { 2 } else { 6 };
        self.screen.backup_scene_window(src, 6);
        let mut timer = self.millis();
        let mut i = 0;
        while i < 2 {
            timer += speed as u32 * TICK;
            self.smooth_scroll_zoom(i, 6, 2);
            self.screen.restore_scene_window(2, 0);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
            if self.smooth_scroll_mode_normal == 0 {
                i += 1;
            }
            i += 1;
        }
        let mut timer = self.millis();
        let mut i = 2;
        while i > 0 {
            timer += speed as u32 * TICK;
            self.smooth_scroll_zoom(i, 6, 2);
            self.screen.restore_scene_window(2, 0);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
            i -= 1;
        }
        if self.scene_default_update != 2 {
            self.screen.restore_scene_window(6, 0);
            self.screen.update_screen();
        }
        self.update_draw_page2();
    }

    pub fn move_party_smooth_scroll_up(&mut self, speed: i32) {
        if !self.smooth_scrolling || self.need_scene_restore != 0 {
            return;
        }
        let d;
        if self.scene_draw_page2 == 2 {
            d = self.smooth_scroll_draw_special_gui_shape(6);
            self.gui_draw_scene(6);
            self.screen.backup_scene_window(6, 12);
            self.screen.backup_scene_window(2, 6);
        } else {
            d = self.smooth_scroll_draw_special_gui_shape(2);
            self.gui_draw_scene(2);
            self.screen.backup_scene_window(2, 12);
            self.screen.backup_scene_window(6, 6);
        }
        let mut timer = self.millis();
        let mut i = 0;
        while i < 5 {
            timer += speed as u32 * TICK;
            self.smooth_scroll_zoom(i, 6, 2);
            if d {
                self.screen.copy_gui_shape_to_surface(14, 2);
            }
            self.screen.restore_scene_window(2, 0);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
            if self.smooth_scroll_mode_normal == 0 {
                i += 1;
            }
            i += 1;
        }
        if d {
            self.screen.copy_gui_shape_to_surface(14, 12);
        }
        if self.scene_default_update != 2 {
            self.screen.restore_scene_window(12, 0);
            self.screen.update_screen();
        }
        self.update_draw_page2();
    }

    pub fn move_party_smooth_scroll_down(&mut self, speed: i32) {
        if !self.smooth_scrolling {
            return;
        }
        let d = self.smooth_scroll_draw_special_gui_shape(2);
        self.gui_draw_scene(2);
        self.screen.backup_scene_window(2, 6);
        let mut timer = self.millis();
        let mut i: i32 = 4;
        while i >= 0 {
            timer += speed as u32 * TICK;
            self.smooth_scroll_zoom(i as usize, 6, 2);
            if d {
                self.screen.copy_gui_shape_to_surface(14, 2);
            }
            self.screen.restore_scene_window(2, 0);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
            i -= 1;
        }
        if d {
            self.screen.copy_gui_shape_to_surface(14, 12);
        }
        if self.scene_default_update != 2 {
            self.screen.restore_scene_window(6, 0);
            self.screen.update_screen();
        }
        self.update_draw_page2();
    }

    pub fn move_party_smooth_scroll_left(&mut self, speed: i32) {
        if !self.smooth_scrolling {
            return;
        }
        let speed = speed << 1;
        let (p1, p2) = (self.scene_draw_page1, self.scene_draw_page2);
        self.gui_draw_scene(p1);
        let mut timer = self.millis();
        let (mut i, mut d) = (88, 88);
        while i > 22 {
            timer += speed as u32 * TICK;
            self.screen.smooth_scroll_horizontal_step(p2, 66, d, i);
            self.screen
                .copy_region(112 + i, 0, 112, 0, d, 120, p1, p2, CR_NO_P_CHECK);
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
            i -= 22;
            d += 22;
        }
        if self.scene_default_update != 2 {
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p1, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
        }
        std::mem::swap(&mut self.scene_draw_page1, &mut self.scene_draw_page2);
    }

    pub fn move_party_smooth_scroll_right(&mut self, speed: i32) {
        if !self.smooth_scrolling {
            return;
        }
        let speed = speed << 1;
        let (p1, p2) = (self.scene_draw_page1, self.scene_draw_page2);
        self.gui_draw_scene(p1);
        let mut timer = self.millis() + speed as u32 * TICK;
        self.screen
            .copy_region(112, 0, 222, 0, 66, 120, p1, p2, CR_NO_P_CHECK);
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, p2, 0, CR_NO_P_CHECK);
        self.screen.update_screen();
        self.fade_text_step();
        self.delay_until(timer, false);
        timer += speed as u32 * TICK;
        self.screen.smooth_scroll_horizontal_step(p2, 22, 0, 66);
        self.screen
            .copy_region(112, 0, 200, 0, 88, 120, p1, p2, CR_NO_P_CHECK);
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, p2, 0, CR_NO_P_CHECK);
        self.screen.update_screen();
        self.fade_text_step();
        self.delay_until(timer, false);
        timer += speed as u32 * TICK;
        self.screen.smooth_scroll_horizontal_step(p2, 44, 0, 22);
        self.screen
            .copy_region(112, 0, 178, 0, 110, 120, p1, p2, CR_NO_P_CHECK);
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, p2, 0, CR_NO_P_CHECK);
        self.screen.update_screen();
        self.fade_text_step();
        self.delay_until(timer, false);
        if self.scene_default_update != 2 {
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p1, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
        }
        std::mem::swap(&mut self.scene_draw_page1, &mut self.scene_draw_page2);
    }

    fn smooth_scroll_turn(&mut self, speed: i32, steps: [(i32, bool); 3]) {
        if !self.smooth_scrolling {
            return;
        }
        let speed = speed << 1;
        let p1 = self.scene_draw_page1;
        let d = self.smooth_scroll_draw_special_gui_shape(p1);
        self.gui_draw_scene(p1);
        let (p1, p2) = (self.scene_draw_page1, self.scene_draw_page2);
        let dp = if p2 == 2 { p2 } else { p1 };
        let mut timer = self.millis();
        for (step, swapped) in steps {
            timer += speed as u32 * TICK;
            let (a, b) = if swapped { (p2, p1) } else { (p1, p2) };
            self.screen.smooth_scroll_turn_step(step, a, b, dp);
            if d {
                self.screen.copy_gui_shape_to_surface(14, dp);
            }
            self.screen.restore_scene_window(dp, 0);
            self.screen.update_screen();
            self.fade_text_step();
            self.delay_until(timer, false);
        }
        if self.scene_default_update != 2 {
            self.draw_special_gui_shape(p1);
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p1, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
        }
    }

    pub fn move_party_smooth_scroll_turn_left(&mut self, speed: i32) {
        self.smooth_scroll_turn(speed, [(1, false), (2, false), (3, false)]);
    }

    pub fn move_party_smooth_scroll_turn_right(&mut self, speed: i32) {
        self.smooth_scroll_turn(speed, [(3, true), (2, true), (1, true)]);
    }

    /// `movePartySmoothScrollTurnLeft/Right`.
    pub fn move_party_smooth_scroll_turn(&mut self, right: bool, speed: i32) {
        if right {
            self.move_party_smooth_scroll_turn_right(speed);
        } else {
            self.move_party_smooth_scroll_turn_left(speed);
        }
    }

    /// `clickedShape`: whether the pointer is on the decoration (with its chain).
    pub fn clicked_shape(&self, shape_index: i32) -> bool {
        let mut si = shape_index;
        let mut guard = 0;
        while si > 0 && guard < 64 {
            guard += 1;
            let p = self.level_decoration_properties[si as usize];
            let s = p.shape_index[1];
            if s != 0xFFFF {
                let shp = self
                    .level_decoration_shapes
                    .get(s as usize)
                    .cloned()
                    .flatten()
                    .unwrap_or_default();
                let mut w = shp.get(3).copied().unwrap_or(0) as i32;
                let h = shp.get(2).copied().unwrap_or(0) as i32;
                let x = p.shape_x[1] as i32 + 136;
                let y = p.shape_y[1] as i32 + 8;
                if p.flags & 1 != 0 {
                    w <<= 1;
                }
                if Self::pos_within_rect(
                    self.mouse_x,
                    self.mouse_y,
                    x - 4,
                    y - 4,
                    x + w + 8,
                    y + h + 8,
                ) {
                    return true;
                }
            }
            si = p.next as i32;
        }
        false
    }

    pub fn pit_drop_scroll(&mut self, num_steps: i32) {
        self.screen
            .copy_region_special(0, 320, 200, 112, 0, 6, 176, 120, 0, 0, 176, 120, 0, None);
        let mut etime = self.millis();
        for i in 0..num_steps {
            etime += TICK;
            let ys = ((30720 / num_steps.max(1)) * i) >> 8;
            self.screen.copy_region_special(
                6,
                176,
                120,
                0,
                ys,
                0,
                320,
                200,
                112,
                0,
                176,
                120 - ys,
                0,
                None,
            );
            self.screen.copy_region_special(
                2,
                320,
                200,
                112,
                0,
                0,
                320,
                200,
                112,
                120 - ys,
                176,
                ys,
                0,
                None,
            );
            self.screen.update_screen();
            self.delay_until(etime, false);
        }
        etime += TICK;
        self.screen
            .copy_region_special(2, 320, 200, 112, 0, 0, 320, 200, 112, 0, 176, 120, 0, None);
        self.screen.update_screen();
        self.delay_until(etime, false);
        self.update_draw_page2();
    }

    pub fn shake_scene(&mut self, duration: i32, width: i32, height: i32, restore: i32) {
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, 0, 6, CR_NO_P_CHECK);
        let end = self.millis() + duration.max(0) as u32 * TICK;
        let mut timer = self.millis();
        while end > self.millis() && !self.should_quit() {
            timer += 2 * TICK;
            let s1 = if width != 0 {
                (self.rnd.get(255) as i32 % (width << 1)) - width
            } else {
                0
            };
            let s2 = if height != 0 {
                (self.rnd.get(255) as i32 % (height << 1)) - height
            } else {
                0
            };
            let (x1, x2, w) = if s1 >= 0 {
                (112, 112 + s1, 176 - s1)
            } else {
                (112 - s1, 112, 176 + s1)
            };
            let (y1, y2, h) = if s2 >= 0 {
                (0, s2, 120 - s2)
            } else {
                (-s2, 0, 120 + s2)
            };
            self.screen
                .copy_region(x1, y1, x2, y2, w, h, 6, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.delay_until(timer, false);
        }
        if restore != 0 {
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, 6, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.update_draw_page2();
        }
    }

    pub fn process_gas_explosion(&mut self, sound_id: i32) {
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 12);
        const SOUNDS: [i32; 4] = [0x62, 0xA7, 0xA7, 0xA8];
        self.snd_play_sound_effect(SOUNDS[sound_id.clamp(0, 3) as usize], -1);
        let (dist, _) = self.get_spell_target_block(
            self.current_block as i32,
            self.current_direction as i32,
            3,
        );
        if dist != 0 {
            if let Some(mut mov) = self.open_wsa(&format!("GASEXP{dist}.WSA"), 1, false) {
                let (w, h) = (mov.width, mov.height);
                self.play_spell_animation(
                    Some(&mut mov),
                    0,
                    6,
                    1,
                    (176 - w) / 2 + 112,
                    (120 - h) / 2,
                    None,
                    None,
                    None,
                    0,
                    false,
                );
            }
        } else {
            let p1 = self.screen.palette(1).0.clone();
            let mut p2 = p1.clone();
            for i in 1..128 {
                p2[i * 3] = 0x3F;
            }
            *self.screen.palette_mut(3) = Pal(p2.clone());
            let p0 = self.screen.palette(0).0.clone();
            let t = self.millis();
            while self
                .screen
                .timed_palette_fade_step(&p0, &p2, self.millis() - t, 10 * TICK)
                && !self.should_quit()
            {
                self.update_input();
                self.screen.system.delay_millis(TICK);
            }
            let t = self.millis();
            while self
                .screen
                .timed_palette_fade_step(&p2, &p0, self.millis() - t, 50 * TICK)
                && !self.should_quit()
            {
                self.update_input();
                self.screen.system.delay_millis(TICK);
            }
        }
        self.screen.copy_page(12, 2);
        self.screen.set_cur_page(cp);
        self.update_draw_page2();
        self.scene_update_required = true;
        self.gui_draw_scene(0);
    }

    pub fn smooth_scroll_draw_special_gui_shape(&mut self, page: usize) -> bool {
        let Some(s) = self.special_gui_shape.clone() else {
            return false;
        };
        self.screen.clear_gui_shape_memory(page);
        let (x, y) = (
            self.special_gui_shape_x as i32,
            self.special_gui_shape_y as i32,
        );
        self.screen
            .draw_shape(page, &s, x, y, 2, 0, ShapeArgs::none());
        self.screen
            .copy_gui_shape_from_scene_backup_buffer(page, 14);
        true
    }

    pub fn draw_scene(&mut self, page_num: usize) {
        if page_num != 0 && page_num != self.scene_draw_page1 {
            std::mem::swap(&mut self.scene_draw_page1, &mut self.scene_draw_page2);
            self.update_draw_page2();
        }
        if page_num != 0 && page_num != self.scene_draw_page1 {
            std::mem::swap(&mut self.scene_draw_page1, &mut self.scene_draw_page2);
            self.update_draw_page2();
        }
        self.generate_block_drawing_buffer();
        self.draw_vcn_blocks();
        self.draw_scene_shapes();
        if page_num == 0 {
            let (p1, p2) = (self.scene_draw_page1, self.scene_draw_page2);
            self.draw_special_gui_shape(p1);
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p1, p2, CR_NO_P_CHECK);
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, p1, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            std::mem::swap(&mut self.scene_draw_page1, &mut self.scene_draw_page2);
        }
        self.snd_update_environmental_sfx(0);
        self.gui_draw_compass();
        self.scene_update_required = false;
    }

    pub fn set_wall_type(&mut self, block: i32, wall: i32, val: i32) {
        let b = block as usize & 0x3FF;
        if wall == -1 {
            self.level_block_properties[b].walls = [val as u8; 4];
            if self.wll_automap_data[val as usize & 0xFF] == 17 {
                self.level_block_properties[b].flags &= 0xEF;
                self.level_block_properties[b].flags |= 0x20;
            } else {
                self.level_block_properties[b].flags &= 0xDF;
            }
        } else {
            self.level_block_properties[b].walls[wall as usize & 3] = val as u8;
        }
        self.check_scene_update_need(block);
    }

    pub fn update_draw_page2(&mut self) {
        let p2 = self.scene_draw_page2;
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, 0, p2, CR_NO_P_CHECK);
    }

    pub fn prepare_special_scene(
        &mut self,
        field_type: i32,
        has_dialogue: i32,
        suspend_gui: i32,
        allow_scene_update: i32,
        control_mode: i32,
        fade_flag: i32,
    ) {
        self.reset_portraits_and_disable_sys_timer();
        if field_type != 0 {
            if suspend_gui != 0 {
                self.gui_special_scene_suspend_controls(1);
            }
            if allow_scene_update == 0 {
                self.scene_default_update = 0;
            }
            if has_dialogue != 0 {
                self.init_dialogue_sequence(field_type, 0);
            }
            if fade_flag != 0 {
                let p3 = self.screen.palette(3).clone();
                self.screen.fade_palette(&p3, 10);
                self.screen.fade_flag = 0;
            }
            self.set_special_scene_buttons(0, 0, 320, 130, control_mode);
        } else {
            if suspend_gui != 0 {
                self.gui_special_scene_suspend_controls(0);
            }
            if allow_scene_update == 0 {
                self.scene_default_update = 0;
            }
            self.gui_disable_controls(control_mode);
            if fade_flag != 0 {
                let p0 = self.screen.palette(0).clone();
                let mut p3 = self.screen.palette(3).clone();
                p3.copy_from(&p0, 128, 128, 128);
                self.screen.load_special_colors(&mut p3);
                *self.screen.palette_mut(3) = p3.clone();
                self.screen.fade_palette(&p3, 10);
                self.screen.fade_flag = 0;
            }
            if has_dialogue != 0 {
                self.init_dialogue_sequence(field_type, 0);
            }
            self.set_special_scene_buttons(112, 0, 176, 120, control_mode);
        }
    }

    pub fn restore_after_special_scene(
        &mut self,
        fade_flag: i32,
        redraw_play_field: i32,
        release_tim_scripts: i32,
        scene_update_mode: i32,
    ) -> i32 {
        if self.need_scene_restore == 0 {
            return 0;
        }
        self.need_scene_restore = 0;
        self.enable_sys_timer(2);
        if self.dialogue_field {
            let m = self.current_control_mode;
            self.restore_after_dialogue_sequence(m);
        }
        if self.special_scene_flag != 0 {
            let m = self.current_control_mode;
            self.gui_special_scene_restore_controls(m);
        }
        let l = self.current_control_mode;
        self.current_control_mode = 0;
        self.gui_special_scene_restore_buttons();
        self.calc_char_portrait_xpos();
        self.current_control_mode = l;
        if release_tim_scripts != 0 {
            for i in 0..crate::lol::tim::WSA_SLOTS {
                self.tim_free_anim_struct(i);
            }
            for i in 0..10 {
                self.active_tim[i] = None;
            }
        }
        self.gui_enable_controls();
        if fade_flag != 0 {
            let ff = self.screen.fade_flag;
            if (ff != 1 && ff != 2) || (ff == 1 && self.current_control_mode != 0) {
                if self.current_control_mode != 0 {
                    self.screen.fade_to_black(10);
                } else {
                    self.screen.fade_clear_scene_window(10);
                }
            }
            self.current_control_mode = 0;
            self.calc_char_portrait_xpos();
            if redraw_play_field != 0 {
                self.gui_draw_play_field();
            }
            let p0 = self.screen.palette(0).clone();
            let (br, le) = (self.brightness, self.lamp_effect as i32);
            self.set_palette_brightness(&p0, br, le);
        } else {
            self.current_control_mode = 0;
            self.calc_char_portrait_xpos();
            if redraw_play_field != 0 {
                self.gui_draw_play_field();
            }
        }
        self.scene_default_update = scene_update_mode;
        1
    }

    pub fn set_sequence_buttons(&mut self, x: i32, y: i32, w: i32, h: i32, enable_flags: i32) {
        self.gui_enable_sequence_buttons(x, y, w, h, enable_flags);
        self.seq_window = (x, y, x + w, y + h);
        let offs = if self.item_in_hand != 0 { 10 } else { 0 };
        let shp = self.get_item_icon_shape(self.item_in_hand);
        self.screen.set_mouse_cursor(offs, offs, &shp);
        self.current_floating_cursor = -1;
        if w == 320 {
            self.set_lamp_mode(false);
            self.lamp_status_suspended = true;
        }
    }

    pub fn set_special_scene_buttons(&mut self, x: i32, y: i32, w: i32, h: i32, enable_flags: i32) {
        self.gui_enable_sequence_buttons(x, y, w, h, enable_flags);
        self.sps_window = (x, y, w, h);
    }

    pub fn set_default_button_state(&mut self) {
        self.gui_enable_default_playfield_buttons();
        self.seq_window = (0, 0, 0, 0);
        self.seq_trigger = 0;
        if self.lamp_status_suspended {
            self.reset_lamp_status();
        }
        self.lamp_status_suspended = false;
    }

    // ---- drawing the view

    fn vblock(&self, i: usize) -> &LevelBlockProperty {
        &self.level_block_properties[self.visible_blocks[i]]
    }

    pub fn draw_scene_shapes(&mut self) {
        for i in 0..18 {
            let t = DSC_TILE_INDEX[i] as usize;
            let s = self.vblock(t).walls[self.scene_draw_var_down];
            let (x1, x2) = self.set_level_shapes_dim(t, SCENE_SHP_DIM);
            self.shp_dm_x1 = x1;
            self.shp_dm_x2 = x2;
            if x2 <= x1 {
                continue;
            }
            self.draw_decorations(t);
            let mut w = self.wll_wall_flags[s as usize] as u16;
            if t == 16 {
                w |= 0x80;
            }
            self.draw_block_effects(t, 0);
            if self.vblock(t).assigned_objects != 0 && w & 0x80 != 0 {
                self.draw_block_objects(t);
            }
            self.draw_block_effects(t, 1);
            if w & 8 == 0 {
                continue;
            }
            let si = s as usize;
            let v = (20
                * (s as i32
                    - if si < 23 {
                        DSC_DOOR_SCALE_OFFS[si] as i32
                    } else {
                        0
                    }))
            .min(80);
            self.set_door_shape_dim(t, SCENE_SHP_DIM as i32);
            let which = if si < 23 {
                DSC_DOOR_SHAPE_INDEX[si] as usize
            } else {
                0
            };
            let door = self.door_shapes.get(which).cloned().flatten();
            self.draw_door(door.as_deref(), None, t, 10, 0, -v, 2);
            self.set_level_shapes_dim(t, SCENE_SHP_DIM);
        }
    }

    pub fn draw_decorations(&mut self, index: usize) {
        for i in [1usize, 0] {
            let s = index * 2 + i;
            let mut scale_w = DSC_SCALE_WIDTH_DATA[s] as i32;
            let mut scale_h = DSC_SCALE_HEIGHT_DATA[s] as i32;
            let mut ix = DSC_SHAPE_INDEX[s] as i8 as i32;
            let mut shp_ix = ix.unsigned_abs() as usize;
            let ovl_index =
                (DSC_OVL_INDEX[4 + DSC_DIM_MAP[index] as usize * 5] as usize + 2).min(7);
            if scale_w == 0 || scale_h == 0 {
                continue;
            }
            let d = ((self.current_direction as i32 + DSC_WALLS[s] as i8 as i32) & 3) as usize;
            let mut l = self.wll_shape_map[self.vblock(index).walls[d] as usize] as i32;
            let mut guard = 0;
            while l > 0 && guard < 64 {
                guard += 1;
                let prop = self.level_decoration_properties[l as usize];
                if prop.flags & 8 != 0 && index != 3 && index != 9 && index != 13 {
                    l = prop.next as i32;
                    continue;
                }
                if DSC_OVL_MAP.get(shp_ix) == Some(&1)
                    && (prop.flags & 2 != 0 || (prop.flags & 4 != 0 && self.wll_process_flag))
                {
                    ix = -ix;
                }
                let (mut x_offs, mut y_offs) = (0, 0);
                let mut ovl: Option<Vec<u8>> = None;
                if prop.scale_flag.get(shp_ix).is_some_and(|f| f & 1 != 0) {
                    x_offs = prop.shape_x[shp_ix] as i32;
                    y_offs = prop.shape_y[shp_ix] as i32;
                    shp_ix = DSC_OVL_MAP.get(shp_ix).copied().unwrap_or(0) as usize;
                    ovl = Some(self.screen.level_overlays[ovl_index].clone());
                } else if prop.shape_index.get(shp_ix).is_some_and(|&v| v != 0xFFFF) {
                    scale_w = 0x100;
                    scale_h = 0x100;
                    ovl = Some(self.screen.level_overlays[7].clone());
                }
                let si = prop.shape_index.get(shp_ix).copied().unwrap_or(0xFFFF);
                if si != 0xFFFF
                    && let Some(shape_data) = self
                        .level_decoration_shapes
                        .get(si as usize)
                        .cloned()
                        .flatten()
                {
                    let sx = prop.shape_x[shp_ix] as i32;
                    let base_x = DSC_X[s] as i16 as i32;
                    let mut x;
                    let mut flags;
                    if ix < 0 {
                        x = base_x + x_offs + ((sx * scale_w) >> 8);
                        if ix == DSC_SHAPE_INDEX[s] as i8 as i32 {
                            x = base_x
                                - ((sx * scale_w) >> 8)
                                - shape::scaled_width(&shape_data, scale_w)
                                - x_offs;
                        }
                        flags = 0x105;
                    } else {
                        x = base_x + x_offs + ((sx * scale_w) >> 8);
                        flags = 0x104;
                    }
                    let y = BASE_DSC_Y[s] as i8 as i32
                        + y_offs
                        + ((prop.shape_y[shp_ix] as i32 * scale_h) >> 8);
                    let page = self.scene_draw_page1;
                    let o = ovl.clone().unwrap_or_else(|| (0..=255u8).collect());
                    self.screen.draw_shape(
                        page,
                        &shape_data,
                        x + 112,
                        y,
                        SCENE_SHP_DIM,
                        flags,
                        ShapeArgs {
                            fade: Some((&o, 1)),
                            scale: (scale_w, scale_h),
                            ..ShapeArgs::none()
                        },
                    );
                    if prop.flags & 1 != 0 && shp_ix < 4 {
                        x += shape::scaled_width(&shape_data, scale_w);
                        flags ^= 1;
                        self.screen.draw_shape(
                            page,
                            &shape_data,
                            x + 112,
                            y,
                            SCENE_SHP_DIM,
                            flags,
                            ShapeArgs {
                                fade: Some((&o, 1)),
                                scale: (scale_w, scale_h),
                                ..ShapeArgs::none()
                            },
                        );
                    }
                }
                l = prop.next as i32;
                shp_ix = (DSC_SHAPE_INDEX[s] as i8 as i32).unsigned_abs() as usize;
            }
        }
    }

    pub fn draw_block_effects(&mut self, index: usize, type_: i32) {
        const Y_OFFS: [u16; 4] = [0xFF, 0xFF, 0x80, 0x80];
        let flg = self.vblock(index).flags;
        if flg & 0xF0 == 0 {
            return;
        }
        let mut t = if type_ == 0 { 2 } else { 0 };
        for _ in 0..2 {
            if (0x10 << t) & flg != 0 {
                let draw_flag = if t == 3 { 0x80 } else { 0x20 };
                let ovl = if t == 3 {
                    Some(self.screen.gray_overlay.clone())
                } else {
                    None
                };
                let (x, y) = Self::calc_coordinates_add_direction_offset(
                    0x80,
                    Y_OFFS[t],
                    self.current_direction as i32,
                );
                let vbi = self.visible_blocks[index] as u16;
                let x = x | ((vbi & 0x1F) << 8);
                let y = y | ((vbi & 0xFFE0) << 3);
                let shp = self.effect_shapes.get(t).cloned().unwrap_or_default();
                self.draw_item_or_monster(
                    &shp,
                    ovl.as_deref(),
                    x as i32,
                    y as i32,
                    0,
                    if t == 1 { -20 } else { 0 },
                    draw_flag,
                    -1,
                    false,
                );
            }
            t += 1;
        }
    }

    pub fn draw_special_gui_shape(&mut self, page: usize) {
        let Some(s) = self.special_gui_shape.clone() else {
            return;
        };
        let (x, y) = (
            self.special_gui_shape_x as i32,
            self.special_gui_shape_y as i32,
        );
        self.screen
            .draw_shape(page, &s, x, y, 2, 0, ShapeArgs::none());
        if self.special_gui_shape_mirror_flag & 1 != 0 {
            let w = s.get(3).copied().unwrap_or(0) as i32;
            self.screen
                .draw_shape(page, &s, x + w, y, 2, 1, ShapeArgs::none());
        }
    }

    // ---- scene_rpg

    /// `setLevelShapesDim`: the columns in which block `index` is seen; the window `dim` set
    /// to them.
    pub fn set_level_shapes_dim(&mut self, index: usize, dim: usize) -> (i16, i16) {
        let (x1, x2);
        if self.lvl_shape_left_right[index << 1] == -1 {
            let (mut a1, mut a2) = (0i16, 22i16);
            let (mut y1, mut y2) = (0i16, 120i16);
            let m = index * 18;
            for i in 0..18 {
                let d = self.vblock(i).walls[self.scene_draw_var_down] as usize;
                let a = self.wll_wall_flags[d];
                if a & 8 != 0 {
                    let t = DSC_DIM_DATA2[(m + i) << 1] as i8 as i16;
                    if t > a1 {
                        a1 = t;
                        if a & 0x10 == 0 {
                            (y1, y2) = self.door_shape_dim_y(index, -1);
                        }
                    }
                    let t = DSC_DIM_DATA2[((m + i) << 1) + 1] as i8 as i16;
                    if t < a2 {
                        a2 = t;
                        if a & 0x10 == 0 {
                            (y1, y2) = self.door_shape_dim_y(index, -1);
                        }
                    }
                } else {
                    let t = DSC_DIM_DATA1[m + i] as i8 as i16;
                    if self.wll_vmp_map[d] == 0 || t == -40 {
                        continue;
                    }
                    if t == -41 {
                        a1 = 22;
                        a2 = 0;
                        break;
                    }
                    if t > 0 && a2 > t {
                        a2 = t;
                    }
                    if t < 0 && a1 < -t {
                        a1 = -t;
                    }
                }
                if a2 < a1 {
                    break;
                }
            }
            a1 += (SCENE_X_OFFSET >> 3) as i16;
            a2 += (SCENE_X_OFFSET >> 3) as i16;
            self.lvl_shape_top[index] = y1;
            self.lvl_shape_bottom[index] = y2;
            self.lvl_shape_left_right[index << 1] = a1;
            self.lvl_shape_left_right[(index << 1) + 1] = a2;
            x1 = a1;
            x2 = a2;
        } else {
            x1 = self.lvl_shape_left_right[index << 1];
            x2 = self.lvl_shape_left_right[(index << 1) + 1];
        }
        self.draw_level_modify_screen_dim(dim, x1, 0, x2, 15);
        (x1, x2)
    }

    fn door_shape_dim_y(&self, index: usize, dim: i32) -> (i16, i16) {
        let mut a = DSC_DIM_MAP[index] as usize;
        if dim == -1 && a != 3 {
            a += 1;
        }
        (DSC_DOOR_FRAME_Y1[a] as i16, DSC_DOOR_FRAME_Y2[a] as i16)
    }

    /// `setDoorShapeDim`.
    pub fn set_door_shape_dim(&mut self, index: usize, dim: i32) -> (i16, i16) {
        let (y1, y2) = self.door_shape_dim_y(index, dim);
        if dim == -1 {
            return (y1, y2);
        }
        let c = self.screen.dim(dim as usize);
        self.screen
            .modify_screen_dim(dim as usize, c.sx, y1 as i32, c.w, (y2 - y1) as i32);
        (y1, y2)
    }

    pub fn draw_level_modify_screen_dim(&mut self, dim: usize, x1: i16, y1: i16, x2: i16, y2: i16) {
        self.screen.modify_screen_dim(
            dim,
            x1 as i32,
            (y1 as i32) << 3,
            (x2 - x1) as i32,
            ((y2 - y1) as i32) << 3,
        );
    }

    pub fn generate_block_drawing_buffer(&mut self) {
        let cd = self.current_direction as usize;
        self.scene_draw_var_down = DSC_BLOCK_MAP[cd] as usize;
        self.scene_draw_var_right = DSC_BLOCK_MAP[cd + 4] as usize;
        self.scene_draw_var_left = DSC_BLOCK_MAP[cd + 8] as usize;
        self.block_drawing_buffer[..660].fill(0);
        let cb = self.current_block as usize;
        self.wll_process_flag = ((cb >> 5) + (cb & 0x1F) + cd) & 1 != 0;
        if self.wll_process_flag {
            self.generate_vmp_tile_data_flipped(0, 15, 1, -330, 22, 15);
        } else {
            self.generate_vmp_tile_data(0, 15, 1, -330, 22, 15);
        }
        self.assign_visible_blocks(cb as i32, cd as i32);
        let (dn, rt, lt) = (
            self.scene_draw_var_down,
            self.scene_draw_var_right,
            self.scene_draw_var_left,
        );
        let w = |s: &Self, i: usize, side: usize| s.vblock(i).walls[side];
        let o = |i: usize| VMP_VIS_OFFS[i];
        let t = w(self, 0, rt);
        if t != 0 {
            self.generate_vmp_tile_data(-2, 3, t, o(0), 3, 5);
        }
        let t = w(self, 6, lt);
        if t != 0 {
            self.generate_vmp_tile_data_flipped(21, 3, t, o(0), 3, 5);
        }
        let t = w(self, 1, rt);
        let t2 = w(self, 2, dn);
        if self.has_wall(t) && self.wll_wall_flags[t2 as usize] & 8 == 0 {
            self.generate_vmp_tile_data(2, 3, t, o(0), 3, 5);
        } else if t != 0 && self.wll_wall_flags[t2 as usize] & 8 != 0 {
            self.generate_vmp_tile_data(2, 3, t2, o(0), 3, 5);
        }
        let t = w(self, 5, lt);
        let t2 = w(self, 4, dn);
        if self.has_wall(t) && self.wll_wall_flags[t2 as usize] & 8 == 0 {
            self.generate_vmp_tile_data_flipped(17, 3, t, o(0), 3, 5);
        } else if t != 0 && self.wll_wall_flags[t2 as usize] & 8 != 0 {
            self.generate_vmp_tile_data_flipped(17, 3, t2, o(0), 3, 5);
        }
        let t = w(self, 2, rt);
        if t != 0 {
            self.generate_vmp_tile_data(8, 3, t, o(1), 1, 5);
        }
        let t = w(self, 4, lt);
        if t != 0 {
            self.generate_vmp_tile_data_flipped(13, 3, t, o(1), 1, 5);
        }
        let t = w(self, 1, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(-4, 3, t, o(2), 6, 5);
        }
        let t = w(self, 5, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(20, 3, t, o(2), 6, 5);
        }
        let t = w(self, 2, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(2, 3, t, o(2), 6, 5);
        }
        let t = w(self, 4, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(14, 3, t, o(2), 6, 5);
        }
        let t = w(self, 3, dn);
        if t != 0 {
            self.generate_vmp_tile_data(8, 3, t, o(2), 6, 5);
        }
        let t = w(self, 7, rt);
        if t != 0 {
            self.generate_vmp_tile_data(0, 3, t, o(3), 2, 6);
        }
        let t = w(self, 11, lt);
        if t != 0 {
            self.generate_vmp_tile_data_flipped(20, 3, t, o(3), 2, 6);
        }
        let t = w(self, 8, rt);
        if t != 0 {
            self.generate_vmp_tile_data(6, 2, t, o(4), 2, 8);
        }
        let t = w(self, 10, lt);
        if t != 0 {
            self.generate_vmp_tile_data_flipped(14, 2, t, o(4), 2, 8);
        }
        let t = w(self, 8, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(-4, 2, t, o(5), 10, 8);
        }
        let t = w(self, 10, dn);
        if self.has_wall(t) {
            self.generate_vmp_tile_data(16, 2, t, o(5), 10, 8);
        }
        let t = w(self, 9, dn);
        if t != 0 {
            self.generate_vmp_tile_data(6, 2, t, o(5), 10, 8);
        }
        let t = w(self, 12, rt);
        if t != 0 {
            self.generate_vmp_tile_data(3, 1, t, o(6), 3, 12);
        }
        let t = w(self, 14, lt);
        if t != 0 {
            self.generate_vmp_tile_data_flipped(16, 1, t, o(6), 3, 12);
        }
        let t = w(self, 12, dn);
        if self.wll_wall_flags[t as usize] & 8 == 0 {
            self.generate_vmp_tile_data(-13, 1, t, o(7), 16, 12);
        }
        let t = w(self, 14, dn);
        if self.wll_wall_flags[t as usize] & 8 == 0 {
            self.generate_vmp_tile_data(19, 1, t, o(7), 16, 12);
        }
        let t = w(self, 13, dn);
        if t != 0 {
            self.generate_vmp_tile_data(3, 1, t, o(7), 16, 12);
        }
        let t = w(self, 15, rt);
        let t2 = w(self, 17, lt);
        if t != 0 {
            self.generate_vmp_tile_data(0, 0, t, o(8), 3, 15);
        }
        if t2 != 0 {
            self.generate_vmp_tile_data_flipped(19, 0, t2, o(8), 3, 15);
        }
    }

    fn vmp_at(&self, i: isize) -> u16 {
        if i < 0 {
            0
        } else {
            self.vmp.get(i as usize).copied().unwrap_or(0)
        }
    }

    pub fn generate_vmp_tile_data(
        &mut self,
        start_x: i32,
        start_y: i32,
        vmp_map_index: u8,
        vmp_offset: i32,
        num_x: i32,
        num_y: i32,
    ) {
        let m = self.wll_vmp_map[vmp_map_index as usize] as isize;
        if m == 0 {
            return;
        }
        let mut vmp = (m - 1) * 431 + vmp_offset as isize + 330;
        for i in 0..num_y {
            let mut bl = (start_y + i) * 22 + start_x;
            for ii in 0..num_x {
                let v = self.vmp_at(vmp);
                if start_x + ii >= 0 && start_x + ii < 22 && v != 0 && (0..1320).contains(&bl) {
                    self.block_drawing_buffer[bl as usize] = v;
                }
                bl += 1;
                vmp += 1;
            }
        }
    }

    pub fn generate_vmp_tile_data_flipped(
        &mut self,
        start_x: i32,
        start_y: i32,
        vmp_map_index: u8,
        vmp_offset: i32,
        num_x: i32,
        num_y: i32,
    ) {
        let m = self.wll_vmp_map[vmp_map_index as usize] as isize;
        if m == 0 {
            return;
        }
        let mut vmp = (m - 1) * 431 + vmp_offset as isize + 330 + (num_x as isize - 1);
        let mut bl = start_y * 22 + start_x;
        for _ in 0..num_y {
            let (bl2, vmpb) = (bl, vmp);
            for ii in 0..num_x {
                let v = self.vmp_at(vmp);
                if start_x + ii >= 0 && start_x + ii < 22 && v != 0 && (0..1320).contains(&bl) {
                    self.block_drawing_buffer[bl as usize] = if v & 0x4000 != 0 {
                        v - 0x4000
                    } else {
                        v | 0x4000
                    };
                }
                bl += 1;
                vmp -= 1;
            }
            bl = bl2 + 22;
            vmp = vmpb + num_x as isize;
        }
    }

    pub fn has_wall(&self, index: u8) -> bool {
        !(index == 0 || self.wll_wall_flags[index as usize] & 8 != 0)
    }

    pub fn assign_visible_blocks(&mut self, block: i32, direction: i32) {
        for i in 0..18 {
            let t = ((block + DSC_BLOCK_INDEX[(direction as usize & 3) * 18 + i] as i8 as i32)
                & 0x3FF) as usize;
            self.visible_blocks[i] = t;
            self.visible_block_index[i] = t as u16;
            self.lvl_shape_left_right[i] = -1;
            self.lvl_shape_left_right[18 + i] = -1;
        }
    }

    pub fn check_scene_update_need(&mut self, block: i32) -> bool {
        if self.scene_update_required {
            return true;
        }
        if self.visible_block_index[..15]
            .iter()
            .any(|&b| b as i32 == block)
            || self.current_block as i32 == block
        {
            self.scene_update_required = true;
            return true;
        }
        false
    }

    pub fn calc_new_block_position(&self, cur_block: u16, direction: u16) -> u16 {
        const T: [i32; 4] = [-32, 1, 32, -1];
        ((cur_block as i32 + T[(direction & 3) as usize]) & 0x3FF) as u16
    }

    pub fn draw_vcn_blocks(&mut self) {
        let mut d = 0usize;
        let mut bdb = 0usize;
        let col = self.vcn_col_table;
        let blocks = &self.vcn_blocks;
        let shift = &self.vcn_shift;
        let buf = &mut self.scene_window_buffer;
        let draw =
            |buf: &mut [u8], d: usize, src: usize, flip: bool, trans: bool, shift_val: u8| {
                for row in 0..8 {
                    let line = d + row * 176;
                    for bx in 0..4 {
                        let b = blocks
                            .get(src + row * 4 + if flip { 3 - bx } else { bx })
                            .copied()
                            .unwrap_or(0);
                        let (h, l) = if flip {
                            (b & 0x0F, b >> 4)
                        } else {
                            (b >> 4, b & 0x0F)
                        };
                        let ch = col[((h as usize) | shift_val as usize) & 127];
                        let cl = col[((l as usize) | shift_val as usize) & 127];
                        let at = line + bx * 2;
                        if !trans || ch != 0 {
                            buf[at] = ch;
                        }
                        if !trans || cl != 0 {
                            buf[at + 1] = cl;
                        }
                    }
                }
            };
        for _y in 0..15 {
            for _x in 0..22 {
                let mut flip = false;
                let mut vcn_offset = self.block_drawing_buffer[bdb];
                bdb += 1;
                let mut extra = 0u16;
                if vcn_offset & 0x8000 != 0 {
                    extra = vcn_offset - 0x8000;
                    vcn_offset = 0;
                }
                if vcn_offset & 0x4000 != 0 {
                    flip = true;
                    vcn_offset &= 0x3FFF;
                }
                if vcn_offset == 0 {
                    vcn_offset = self
                        .block_drawing_buffer
                        .get(bdb + 329)
                        .copied()
                        .unwrap_or(0);
                    if vcn_offset & 0x4000 != 0 {
                        flip = true;
                        vcn_offset &= 0x3FFF;
                    }
                }
                let sv = shift.get(vcn_offset as usize).copied().unwrap_or(0);
                draw(buf, d, vcn_offset as usize * 32, flip, false, sv);
                if extra != 0 {
                    let mut f = false;
                    if extra & 0x4000 != 0 {
                        extra &= 0x3FFF;
                        f = true;
                    }
                    let sv = shift.get(extra as usize).copied().unwrap_or(0);
                    draw(buf, d, extra as usize * 32, f, true, sv);
                }
                d += 8;
            }
            d += 7 * 176;
        }
        let page = self.scene_draw_page1;
        let b = self.scene_window_buffer.clone();
        self.screen
            .copy_block_to_page(page, SCENE_X_OFFSET, 0, 176, 120, &b);
    }

    pub fn clicked_wall_shape(&mut self, block: u16, direction: u16) -> i32 {
        let v = self.wll_shape_map
            [self.level_block_properties[block as usize].walls[direction as usize & 3] as usize];
        if !self.clicked_shape(v as i32) {
            return 0;
        }
        self.snd_stop_speech(true);
        self.run_level_script(block as i32, 0x40);
        1
    }

    pub fn clicked_lever_on(&mut self, block: u16, direction: u16) -> i32 {
        let w = &mut self.level_block_properties[block as usize].walls[direction as usize & 3];
        let v = self.wll_shape_map[*w as usize];
        if !self.clicked_shape(v as i32) {
            return 0;
        }
        let w = &mut self.level_block_properties[block as usize].walls[direction as usize & 3];
        *w = w.wrapping_add(1);
        self.scene_update_required = true;
        self.snd_play_sound_effect(30, -1);
        self.run_level_script(block as i32, CLICKED_SPECIAL_FLAG);
        1
    }

    pub fn clicked_lever_off(&mut self, block: u16, direction: u16) -> i32 {
        let v = self.wll_shape_map
            [self.level_block_properties[block as usize].walls[direction as usize & 3] as usize];
        if !self.clicked_shape(v as i32) {
            return 0;
        }
        let w = &mut self.level_block_properties[block as usize].walls[direction as usize & 3];
        *w = w.wrapping_sub(1);
        self.scene_update_required = true;
        self.snd_play_sound_effect(29, -1);
        self.run_level_script(block as i32, CLICKED_SPECIAL_FLAG);
        1
    }

    pub fn clicked_wall_only_script(&mut self, block: u16) -> i32 {
        self.run_level_script(block as i32, CLICKED_SPECIAL_FLAG);
        1
    }

    pub fn process_door_switch(&mut self, block: u16, mut open_close: i32) {
        if block == self.current_block
            || self.level_block_properties[block as usize].assigned_objects & 0x8000 != 0
        {
            return;
        }
        if open_close == 0
            && let Some(s) = self.open_door_state.iter().find(|s| s.block == block)
        {
            open_close = -(s.state as i32);
        }
        if open_close == 0 {
            let b = &self.level_block_properties[block as usize];
            let c = if self.wll_wall_flags[b.walls[0] as usize] & 8 != 0 {
                0
            } else {
                1
            };
            open_close = if self.wll_wall_flags[b.walls[c] as usize] & 1 != 0 {
                1
            } else {
                -1
            };
        }
        self.open_close_door(block as i32, open_close);
    }

    pub fn open_close_door(&mut self, block: i32, open_close: i32) {
        let b = block as usize & 0x3FF;
        let mut s1: i32 = -1;
        let mut s2: i32 = -1;
        let c = if self.wll_wall_flags[self.level_block_properties[b].walls[0] as usize] & 8 != 0 {
            0
        } else {
            1
        };
        let mut v = self.level_block_properties[b].walls[c] as i32;
        let flg = if open_close == 1 {
            0x10
        } else if open_close == -1 {
            0x20
        } else {
            0
        };
        if self.wll_wall_flags[v as usize & 0xFF] as i32 & flg != 0 {
            return;
        }
        for i in 0..3 {
            if self.open_door_state[i].block as usize == b {
                s1 = i as i32;
                break;
            } else if self.open_door_state[i].block == 0 && s2 == -1 {
                s2 = i as i32;
            }
        }
        if s1 != -1 || s2 != -1 {
            if s1 == -1 {
                s1 = s2;
            }
            let st = &mut self.open_door_state[s1 as usize];
            st.block = b as u16;
            st.state = open_close as i8;
            st.wall = c as i8;
            let flg = if -open_close == 1 {
                0x10
            } else if -open_close == -1 {
                0x20
            } else {
                0
            };
            if self.wll_wall_flags[v as usize & 0xFF] as i32 & flg != 0 {
                let w = &mut self.level_block_properties[b].walls;
                w[c] = (w[c] as i32 + open_close) as u8;
                w[c ^ 2] = (w[c ^ 2] as i32 + open_close) as u8;
                let snd = if open_close == -1 { 4 } else { 3 };
                let cb = self.current_block as i32;
                self.snd_process_environmental_sound_effect(snd + 28, cb);
                if !self.check_scene_update_need(b as i32) {
                    self.snd_update_environmental_sfx(0);
                }
            }
            self.enable_timer(0);
        } else {
            let mut guard = 0;
            while flg & self.wll_wall_flags[v as usize & 0xFF] as i32 == 0 && guard < 256 {
                guard += 1;
                v += open_close;
            }
            self.level_block_properties[b].walls[c] = v as u8;
            self.level_block_properties[b].walls[c ^ 2] = v as u8;
            self.check_scene_update_need(b as i32);
        }
    }

    pub fn complete_door_operations(&mut self) {
        for i in 0..3 {
            let st = self.open_door_state[i];
            if st.block == 0 {
                continue;
            }
            let b = st.block as usize;
            let mut guard = 0;
            loop {
                guard += 1;
                let w = &mut self.level_block_properties[b].walls;
                let c = st.wall as usize & 3;
                w[c] = (w[c] as i32 + st.state as i32) as u8;
                w[c ^ 2] = (w[c ^ 2] as i32 + st.state as i32) as u8;
                if self.wll_wall_flags[w[c] as usize] & 0x30 != 0 || guard > 256 {
                    break;
                }
            }
            self.open_door_state[i].block = 0;
        }
    }

    pub fn enable_timer(&mut self, id: u8) {
        self.timers.enable(id);
        let d = self.timers.get_delay(id);
        let now = self.millis();
        self.timers.set_countdown(id, d, now, TICK);
    }
}
