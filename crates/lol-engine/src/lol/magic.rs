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

//! The spells (ScummVM's `lol.cpp`: `castSpell` and the `processMagic…` functions, the spell
//! book, the Bezel cup).

use lol_data::tables::{
    BUTTON_LIST8, FIREBALL_COORDS, HEAL_SHAPE_FRAMES, LIGHTNING_DEFS, SPELL_PROPERTIES,
    SPELLBOOK_ANIM, SPELLBOOK_COORDS,
};

use super::types::*;
use super::wsa::Wsa;
use super::{Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId, Pal};
use crate::shape::ShapeArgs;

/// `_mistAnimData`: the two parts of each mist and its sound.
const MIST_ANIM: [[u8; 5]; 5] = [
    [0, 7, 7, 13, 155],
    [0, 16, 16, 17, 155],
    [0, 24, 24, 24, 174],
    [0, 19, 19, 19, 174],
    [0, 16, 16, 17, 175],
];

/// What `playSpellAnimation` calls each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellCallback {
    Swarm,
    Lightning,
}

struct Fireball {
    active: bool,
    dest_x: i32,
    dest_y: i32,
    tbl_index: i32,
    progress: i32,
    step: i32,
    finalize: bool,
    fin_progress: usize,
}

impl Fireball {
    fn new(i: i32) -> Self {
        Self {
            active: true,
            dest_x: 200,
            dest_y: 60,
            tbl_index: ((i * 50) % 255) + 200,
            progress: 1000,
            step: 10,
            finalize: false,
            fin_progress: 0,
        }
    }
}

impl Lol {
    /// Opens an animation file (`WSAMovie_v2::open`).
    pub fn open_wsa(&mut self, name: &str, unk1: i32, _palette: bool) -> Option<Wsa> {
        let bytes = self.res.file_data(name)?;
        Wsa::open(&bytes, unk1)
    }

    /// `castSpell`: costs paid, the spell's function runs.
    pub fn cast_spell(&mut self, char_num: i32, spell_type: i32, spell_level: i32) -> i32 {
        if spell_type < 0 {
            return 0;
        }
        self.active_spell.char_num = char_num.max(0) as u8;
        self.active_spell.spell = spell_type as u8;
        self.active_spell.level = spell_level.unsigned_abs() as u8;
        let p = SPELL_PROPERTIES[spell_type as usize];
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction);
        if p.flags & 0x100 != 0 && self.test_wall_flag(ahead, self.current_direction as i32, 1) {
            let s = self.get_lang_string(0x4257);
            self.print_message(2, b"%s", &[Arg::Str(s)]);
            return 0;
        }
        if char_num < 0 {
            self.active_spell.char_num = (-char_num - 1) as u8;
            return self.run_spell_proc(spell_type);
        }
        let c = char_num as usize;
        let lvl = spell_level.clamp(0, 3) as usize;
        if p.mp_required[lvl] as i32 > self.characters[c].magic_points_cur as i32 {
            return 0;
        }
        if p.hp_required[lvl] as i32 >= self.characters[c].hit_points_cur as i32 {
            return 0;
        }
        self.set_character_magic_or_hit_points(c, 1, -(p.mp_required[lvl] as i32), 1);
        self.set_character_magic_or_hit_points(c, 0, -(p.hp_required[lvl] as i32), 1);
        self.gui_draw_char_portrait_with_stats(c);
        self.run_spell_proc(spell_type);
        1
    }

    fn run_spell_proc(&mut self, spell: i32) -> i32 {
        let a = self.active_spell;
        let (c, l) = (a.char_num as i32, a.level as i32);
        match spell {
            0 => self.process_magic_spark(c, l),
            1 => {
                if l < 3 {
                    self.process_magic_heal_select_target()
                } else {
                    self.process_magic_heal(-1, l)
                }
            }
            2 => self.process_magic_ice(c, l),
            3 => self.process_magic_fireball(c, l),
            4 => self.process_magic_hand_of_fate(l),
            5 => self.process_magic_mist_of_doom(c, l),
            6 => self.process_magic_lightning(c, l),
            8 => self.process_magic_fog(),
            9 => self.process_magic_swarm(c, 10),
            12 => self.process_magic_vaelans_cube(),
            16 => self.process_magic_guardian(c),
            _ => 1,
        }
    }

    pub fn cast_heal_on_single_character(&mut self) -> i32 {
        let a = self.active_spell;
        self.process_magic_heal(a.target as i32, a.level as i32);
        1
    }

    fn tables(&self) -> (Vec<u8>, Vec<u8>) {
        (
            self.transparency_table1.clone(),
            self.transparency_table2.clone(),
        )
    }

    pub fn process_magic_spark(&mut self, char_num: i32, spell_level: i32) -> i32 {
        self.screen.copy_page(0, 12);
        let Some(mut mov) = self.open_wsa("SPARK1.WSA", 0, false) else {
            return 1;
        };
        self.snd_play_sound_effect(72, -1);
        let x = self.active_chars_xpos[char_num.clamp(0, 2) as usize] as i32 - 2;
        self.play_spell_animation(Some(&mut mov), 0, 7, 4, x, 138, None, None, None, 0, false);
        self.screen.copy_page(12, 0);
        self.screen.update_screen();
        let (dist, target_block) = self.get_spell_target_block(
            self.current_block as i32,
            self.current_direction as i32,
            4,
        );
        let target =
            self.get_nearest_monster_from_character_for_block(target_block, char_num as usize);
        const DMG: [i32; 4] = [7, 15, 25, 60];
        if target != 0xFFFF {
            self.inflict_magical_damage(
                target as i32,
                char_num,
                DMG[spell_level.clamp(0, 3) as usize],
                5,
                0,
            );
            self.update_draw_page2();
            self.gui_draw_scene(0);
            self.screen.copy_page(0, 12);
        }
        let Some(mut mov) = self.open_wsa("SPARK2.WSA", 0, false) else {
            return 1;
        };
        let num_frames = mov.frames().max(1);
        let (w, h) = (mov.width, mov.height);
        let mut wx = [0i32; 6];
        let mut wy = [0i32; 6];
        let mut wf = [0i32; 6];
        for i in 0..6 {
            wx[i] = (self.rnd.get(0x7FFF) % 64) as i32 + ((176 - w) >> 1) + 80;
            wy[i] = (self.rnd.get(0x7FFF) % 32) as i32 + ((120 - h) >> 1) - 16;
            wf[i] = (i as i32) << 1;
        }
        let (t1, t2) = self.tables();
        let d = (spell_level << 1) + 12;
        for i in 0..d {
            let timer = self.millis() + 4 * TICK;
            self.screen.copy_page(12, 2);
            for ii in 0..=(spell_level.clamp(0, 5) as usize) {
                if wf[ii] >= i || wf[ii] + 13 <= i {
                    continue;
                }
                if i - wf[ii] == 1 {
                    self.snd_play_sound_effect(162, -1);
                }
                let f = ((i - wf[ii]) + (dist << 4)) % num_frames;
                mov.display_frame(
                    &mut self.screen,
                    f,
                    2,
                    wx[ii],
                    wy[ii],
                    0x5000,
                    Some(&t1),
                    Some(&t2),
                );
                self.screen
                    .copy_region(wx[ii], wy[ii], wx[ii], wy[ii], w, h, 2, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
            }
            if i < d - 1 {
                self.delay_until(timer, false);
            }
        }
        self.screen.copy_page(12, 2);
        self.update_draw_page2();
        self.scene_update_required = true;
        1
    }

    pub fn process_magic_heal_select_target(&mut self) -> i32 {
        let s = self.get_lang_string(0x4040);
        self.print_message(0, b"%s", &[Arg::Str(s)]);
        self.gui_reset_button_list();
        self.gui_set_face_frames_control_buttons(81, 0);
        self.gui_init_buttons_from_list(&BUTTON_LIST8);
        1
    }

    pub fn process_magic_heal(&mut self, char_num: i32, spell_level: i32) -> i32 {
        if self.heal_overlay.is_empty() {
            let tpal = self.screen.palette(1).clone();
            self.heal_overlay = self.generate_gray_overlay(&tpal, 52, 22, 20, 0, 256, true);
        }
        let (heal_frames, heali_frames, cure_poison, points): (usize, usize, bool, i32) =
            match spell_level {
                0 => (0, 32, false, 25),
                1 => (16, 48, false, 45),
                l if l > 3 => (16, 64, true, l),
                _ => (16, 64, true, 10000),
            };
        let (ch, n) = if char_num != -1 {
            (char_num as usize, char_num as usize + 1)
        } else {
            (0, 4)
        };
        let mut px = [0i32; 4];
        let py = 138;
        // uint16 in the original
        let mut diff = [0u16; 4];
        let mut pts = [0u16; 4];
        for c in ch..n.min(4) {
            if self.characters[c].flags & 1 == 0 {
                continue;
            }
            px[c] = self.active_chars_xpos[c.min(2)] as i32 - 6;
            self.characters[c].damage_suffered = 0;
            let dmg =
                self.characters[c].hit_points_max as i32 - self.characters[c].hit_points_cur as i32;
            diff[c] = dmg.min(points) as u16;
            self.screen
                .copy_region(px[c], py, c as i32 * 77, 32, 77, 44, 0, 2, CR_NO_P_CHECK);
        }
        let cp = self.screen.set_cur_page(2);
        self.snd_play_sound_effect(68, -1);
        let (t1, t2) = self.tables();
        for i in 0..16 {
            let timer = self.millis() + 4 * TICK;
            for c in ch..n.min(4) {
                if self.characters[c].flags & 1 == 0 {
                    continue;
                }
                self.screen
                    .copy_region(c as i32 * 77, 32, px[c], py, 77, 44, 2, 2, CR_NO_P_CHECK);
                pts[c] &= 0xFF;
                pts[c] = pts[c].wrapping_add((((diff[c] as i32) << 8) / 16) as u16);
                self.increase_character_hitpoints(c, (pts[c] / 256) as i32, true);
                self.gui_draw_char_portrait_with_stats(c);
                let hs = self
                    .heal_shapes
                    .get(HEAL_SHAPE_FRAMES[heal_frames + i] as usize)
                    .cloned()
                    .unwrap_or_default();
                self.screen.draw_shape(
                    2,
                    &hs,
                    px[c],
                    py,
                    0,
                    0x1000,
                    ShapeArgs {
                        transparency: Some((&t1, &t2)),
                        ..ShapeArgs::none()
                    },
                );
                self.screen.fill_rect(0, 0, 31, 31, 0, None);
                let his = self
                    .heali_shapes
                    .get(HEAL_SHAPE_FRAMES[heali_frames + i] as usize)
                    .cloned()
                    .unwrap_or_default();
                let page = self.screen.cur_page;
                self.screen
                    .draw_shape(page, &his, 0, 0, 0, 0, ShapeArgs::none());
                let ovl = self.heal_overlay.clone();
                self.screen.apply_overlay_special(
                    page,
                    0,
                    0,
                    2,
                    px[c] + 7,
                    py + 6,
                    32,
                    32,
                    0,
                    false,
                    &ovl,
                );
                self.screen
                    .copy_region(px[c], py, px[c], py, 77, 44, 2, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
            }
            self.delay_until(timer, false);
        }
        for c in ch..n.min(4) {
            if self.characters[c].flags & 1 == 0 {
                continue;
            }
            self.screen
                .copy_region(c as i32 * 77, 32, px[c], py, 77, 44, 2, 2, CR_NO_P_CHECK);
            if cure_poison {
                self.remove_character_effects(c, 4, 4);
            }
            self.gui_draw_char_portrait_with_stats(c);
            self.screen
                .copy_region(px[c], py, px[c], py, 77, 44, 2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
        }
        self.screen.set_cur_page(cp);
        self.update_draw_page2();
        1
    }

    /// `Screen_LoL::generateGrayOverlay`.
    pub fn generate_gray_overlay(
        &self,
        src: &Pal,
        factor: i32,
        add_r: i32,
        add_g: i32,
        add_b: i32,
        last_color: usize,
        skip_special: bool,
    ) -> Vec<u8> {
        let mut tmp = vec![0u8; last_color * 3];
        for i in 0..last_color {
            let f = |v: u8, add: i32| -> u8 {
                ((((v & 0x3F) as i32 * factor) / 0x40) + add).min(0x3F) as u8
            };
            tmp[3 * i] = f(src.0[3 * i], add_r);
            tmp[3 * i + 1] = f(src.0[3 * i + 1], add_g);
            tmp[3 * i + 2] = f(src.0[3 * i + 2], add_b);
        }
        (0..last_color)
            .map(|i| {
                find_least_different_color(&tmp[3 * i..3 * i + 3], src, 0, last_color, skip_special)
            })
            .collect::<Vec<u8>>()
            .into_iter()
            .chain(std::iter::repeat_n(0, 256usize.saturating_sub(last_color)))
            .collect()
    }

    pub fn process_magic_ice(&mut self, char_num: i32, spell_level: i32) -> i32 {
        let cp = self.screen.set_cur_page(2);
        self.disable_sys_timer(2);
        self.gui_draw_scene(0);
        self.screen.copy_page(0, 12);
        if self.current_level == 11 && self.flags_table[52] & 0x04 == 0 {
            let p0 = self.screen.palette(0).clone();
            let p2 = self.screen.palette(2).clone();
            let (mut a, mut b) = (p0.clone(), p2.clone());
            a.0[1..].copy_from_slice(&p2.0[1..]);
            b.0[1..].copy_from_slice(&p0.0[1..]);
            *self.screen.palette_mut(0) = a;
            *self.screen.palette_mut(2) = b;
            self.flags_table[52] |= 0x04;
            const FREEZE: [i32; 4] = [20, 28, 40, 60];
            self.set_character_update_event(
                char_num as usize,
                8,
                FREEZE[spell_level.clamp(0, 3) as usize],
                1,
            );
        }
        let s = self.screen.palette(1).clone();
        let mut swamp = Pal::default();
        if let Some(d) = self.res.file_data("SWAMPICE.COL") {
            swamp.load(&d);
        }
        let mut tpal = Pal::default();
        tpal.copy_from(&s, 128, 128, 128);
        swamp.copy_from(&s, 128, 128, 128);
        for i in 1..128 {
            tpal.0[i * 3] = 0;
            let v = (s.0[i * 3] as u16 + s.0[i * 3 + 1] as u16 + s.0[i * 3 + 2] as u16) / 3;
            tpal.0[i * 3 + 1] = v as u8;
            tpal.0[i * 3 + 2] = (v << 1).min(0x3F) as u8;
        }
        let (br, le) = (self.brightness, self.lamp_effect as i32);
        let mut tpal = self.generate_brightness_palette(&tpal, br, le);
        let mut swamp = self.generate_brightness_palette(&swamp, br, le);
        swamp.0[0..3].fill(0);
        tpal.0[0..3].fill(0);
        let p0 = self.screen.palette(0).clone();
        let s = self.generate_brightness_palette(&p0, br, le);
        let (mut sx, mut sy) = (112, 0);
        let mut mov = None;
        if spell_level == 0 {
            sx = 0;
        }
        if spell_level == 1 || spell_level == 2 {
            mov = self.open_wsa("SNOW.WSA", 1, false);
        }
        if spell_level == 3 {
            mov = self.open_wsa("ICE.WSA", 1, false);
            sx = 136;
            sy = 12;
        }
        self.snd_play_sound_effect(71, -1);
        self.play_spell_animation(
            None,
            0,
            0,
            2,
            0,
            0,
            None,
            Some(&s.0),
            Some(&tpal.0),
            40,
            false,
        );
        let now = self.millis();
        self.screen
            .timed_palette_fade_step(&s.0, &tpal.0, now, TICK);
        if let Some(mut m) = mov {
            let mut r = true;
            if spell_level > 2 {
                let b = self.calc_new_block_position(self.current_block, self.current_direction);
                self.level_block_properties[b as usize].flags |= 0x10;
                self.snd_play_sound_effect(165, -1);
                r = false;
            }
            let frames = m.frames();
            self.play_spell_animation(Some(&mut m), 0, frames, 2, sx, sy, None, None, None, 0, r);
        }
        const SNOW: [i32; 4] = [10, 20, 30, 55];
        const MAX: [i32; 5] = [1, 2, 15, 20, 35];
        const MIN: [i32; 5] = [10, 10, 3, 4, 4];
        const ADD: [i32; 5] = [5, 10, 30, 10, 10];
        let mut break_wall = false;
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction);
        if spell_level < 3 {
            self.inflict_magical_damage_for_block(
                ahead as i32,
                char_num,
                SNOW[spell_level.clamp(0, 3) as usize],
                3,
            );
        } else {
            let sl = spell_level as usize;
            let mut o = self.level_block_properties[ahead as usize].assigned_objects;
            while o & 0x8000 != 0 {
                let might = self.roll_dice(MIN[sl], MAX[sl], 0) + ADD[sl];
                let dmg = self.calc_inflictable_damage_per_item(char_num, 0, might, 3, 2);
                let mi = (o & 0x7FFF) as usize;
                if self.monsters[mi].hit_points as i32 <= dmg {
                    let hp = self.monsters[mi].hit_points as u32;
                    self.increase_experience(char_num as usize, 2, hp);
                    o = self.monsters[mi].obj.next_assigned_object;
                    if self.monsters[mi].flags & 0x20 != 0 {
                        self.monsters[mi].mode = 0;
                        self.monster_drop_items(mi);
                        if self.current_level != 29 {
                            self.set_monster_mode(mi, 14);
                        }
                        self.run_level_script_custom(0x404, -1, o as i32, o as i32, 0, 0);
                        let b = self.monsters[mi].obj.block;
                        self.check_scene_update_need(b as i32);
                        if self.monsters[mi].mode != 14 {
                            self.place_monster(mi, 0, 0);
                        }
                    } else {
                        self.kill_monster(mi);
                    }
                } else {
                    break_wall = true;
                    self.inflict_damage(o as i32, dmg, char_num, 2, 3);
                    self.monsters[mi].damage_received = 0;
                    o = self.monsters[mi].obj.next_assigned_object;
                }
                if self.monsters[mi].flags & 0x20 != 0 {
                    break;
                }
            }
        }
        self.update_draw_page2();
        self.gui_draw_scene(0);
        self.enable_sys_timer(2);
        if self.current_level != 11 {
            let p0 = self.screen.palette(0).clone();
            swamp = self.generate_brightness_palette(&p0, br, le);
        }
        self.play_spell_animation(
            None,
            0,
            0,
            2,
            0,
            0,
            None,
            Some(&tpal.0),
            Some(&swamp.0),
            40,
            false,
        );
        let now = self.millis();
        self.screen
            .timed_palette_fade_step(&tpal.0, &swamp.0, now, TICK);
        if break_wall {
            self.break_ice_wall(Some(&tpal.0), Some(&swamp.0));
        }
        self.screen.set_cur_page(cp);
        1
    }

    pub fn process_magic_fireball(&mut self, char_num: i32, spell_level: i32) -> i32 {
        let (fb_cnt, mut d) = match spell_level {
            0 => (4, 1),
            1 => (5, 1),
            2 => (6, 1),
            _ => (5, 0),
        };
        let (mut page1, mut page2) = (2usize, 4usize);
        let mut bl = self.current_block as i32;
        let item = self.make_item(9, 0, 0);
        let mut i = 0;
        while i < 3 {
            self.run_level_script_custom(bl, 0x200, -1, item as i32, 0, 0);
            let mut o = self.level_block_properties[bl as usize].assigned_objects;
            let w = self.level_block_properties[bl as usize].walls
                [(self.current_direction ^ 2) as usize];
            if o & 0x8000 != 0 || self.wll_wall_flags[w as usize] & 7 != 0 {
                while o & 0x8000 != 0 {
                    const DMG: [i32; 4] = [20, 40, 80, 100];
                    let dmg = self.calc_inflictable_damage_per_item(
                        char_num,
                        o as i32,
                        DMG[spell_level.clamp(0, 3) as usize],
                        4,
                        1,
                    );
                    let mi = (o & 0x7FFF) as usize;
                    o = self.monsters[mi].obj.next_assigned_object;
                    self.env_sfx_use_queue = true;
                    let id = self.monsters[mi].id as i32 | 0x8000;
                    self.inflict_damage(id, dmg, char_num, 2, 4);
                    self.env_sfx_use_queue = false;
                }
                break;
            }
            bl = self.calc_new_block_position(bl as u16, self.current_direction) as i32;
            i += 1;
        }
        d += i;
        if d > 3 {
            d = 3;
        }
        self.delete_item(item);
        self.snd_play_sound_effect(69, -1);
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 12);
        let wh = -(d << 4);
        let num = if fb_cnt > 3 { fb_cnt - 3 } else { 1 };
        let mut balls: Vec<Fireball> = (0..num).map(Fireball::new).collect();
        self.screen.copy_page(12, page1);
        let (t1, t2) = self.tables();
        let coord = |i: i32| FIREBALL_COORDS[(i & 0xFF) as usize] as i16 as i32;
        let mut done = 0;
        while done < num && !self.should_quit() {
            self.screen.set_cur_page(page1);
            let ctime = self.millis();
            for ii in 0..(fb_cnt.min(3) as usize) {
                let Some(fb) = balls.get_mut(ii) else {
                    continue;
                };
                if !fb.active {
                    continue;
                }
                const FIN1: [usize; 6] = [5, 6, 7, 7, 6, 5];
                const FIN2: [i32; 6] = [-1, 1, 2, 3, 4, -1];
                let shp = if fb.finalize {
                    self.fireball_shapes[FIN1[fb.fin_progress]].clone()
                } else {
                    self.fireball_shapes[0].clone()
                };
                let place = |shp: &[u8], fb: &Fireball| {
                    let (w, h) = (shp[3] as i32, shp[2] as i32);
                    let fx = ((fb.progress * coord(fb.tbl_index)) >> 16) + fb.dest_x
                        - ((fb.progress / 8 + w + wh) >> 1);
                    let fy = ((fb.progress * coord(fb.tbl_index + 64)) >> 16) + fb.dest_y
                        - ((fb.progress / 8 + h + wh) >> 1);
                    let sw = ((fb.progress / 8 + w + wh) << 8) / w.max(1);
                    let sh = ((fb.progress / 8 + h + wh) << 8) / h.max(1);
                    (fx, fy, sw, sh)
                };
                let (fx, fy, sw, sh) = place(&shp, fb);
                let page = self.screen.cur_page;
                self.screen.draw_shape(
                    page,
                    &shp,
                    fx,
                    fy,
                    0,
                    0x1004,
                    ShapeArgs {
                        transparency: Some((&t1, &t2)),
                        scale: (sw, sh),
                        ..ShapeArgs::none()
                    },
                );
                if fb.finalize && FIN2[fb.fin_progress] != -1 {
                    let shp2 = self.fireball_shapes[FIN2[fb.fin_progress] as usize].clone();
                    let (fx, fy, sw, sh) = place(&shp2, fb);
                    self.screen
                        .draw_shape(page, &shp2, fx, fy, 0, 4, ShapeArgs::scaled(sw, sh));
                }
                let fb = &mut balls[ii];
                if fb.finalize {
                    fb.fin_progress += 1;
                    if fb.fin_progress >= 6 {
                        fb.active = false;
                        done += 1;
                    }
                } else {
                    fb.step = (fb.step + 2).min(40);
                    if fb.progress < fb.step {
                        if ii < 1 {
                            fb.progress = 0;
                            fb.step = 0;
                            fb.fin_progress = 0;
                            fb.finalize = true;
                        } else {
                            fb.active = false;
                            done += 1;
                        }
                        const SFX: [i32; 4] = [98, 167, 167, 168];
                        self.snd_play_sound_effect(SFX[d as usize], -1);
                    } else {
                        fb.progress -= fb.step;
                    }
                }
            }
            let del = TICK as i32 - (self.millis() - ctime) as i32;
            if del > 0 {
                self.delay(del as u32, false);
            }
            self.screen.checked_page_update(page1, page2);
            self.screen.update_screen();
            std::mem::swap(&mut page1, &mut page2);
            self.screen.copy_page(12, page1);
        }
        self.screen.set_cur_page(cp);
        self.screen.copy_page(12, 0);
        self.screen.update_screen();
        self.update_draw_page2();
        self.snd_play_queued_effects();
        self.run_level_script_custom(bl, 0x20, char_num, 3, 0, 0);
        1
    }

    pub fn process_magic_hand_of_fate(&mut self, spell_level: i32) -> i32 {
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 12);
        let Some(mut mov) = self.open_wsa("HAND.WSA", 1, false) else {
            return 1;
        };
        const FRAMES: [i32; 10] = [17, 26, 11, 16, 27, 35, 27, 35, 0, 75];
        let sl = spell_level.clamp(0, 4) as usize;
        self.snd_play_sound_effect(173, -1);
        self.play_spell_animation(Some(&mut mov), 0, 10, 3, 112, 0, None, None, None, 0, false);
        self.snd_play_sound_effect(151, -1);
        self.play_spell_animation(
            Some(&mut mov),
            FRAMES[sl * 2],
            FRAMES[sl * 2 + 1],
            3,
            112,
            0,
            None,
            None,
            None,
            0,
            false,
        );
        self.snd_play_sound_effect(18, -1);
        self.play_spell_animation(Some(&mut mov), 10, 0, 3, 112, 0, None, None, None, 0, false);
        self.screen.set_cur_page(cp);
        self.screen.copy_page(12, 2);
        self.gui_draw_scene(2);
        if spell_level < 2 {
            let b1 = self.calc_new_block_position(self.current_block, self.current_direction);
            let b2 = self.calc_new_block_position(b1, self.current_direction);
            if !self.test_wall_flag(b2, 0, 4)
                && self.level_block_properties[b2 as usize].assigned_objects & 0x8000 == 0
            {
                self.check_scene_update_need(b1 as i32);
                let dir = (self.current_direction << 1) as i32;
                let mut o = self.level_block_properties[b1 as usize].assigned_objects;
                while o & 0x8000 != 0 {
                    let o2 = o;
                    let mi = (o & 0x7FFF) as usize;
                    o = self.monsters[mi].obj.next_assigned_object;
                    let (mut nx, mut ny) = self.get_next_step_coords(
                        self.monsters[mi].obj.x as i32,
                        self.monsters[mi].obj.y as i32,
                        dir,
                    );
                    for _ in 0..7 {
                        (nx, ny) = self.get_next_step_coords(nx, ny, dir);
                    }
                    self.place_monster(mi, nx as u16, ny as u16);
                    self.run_level_script_custom(b2 as i32, 0x800, -1, o2 as i32, 0, 0);
                }
            }
        } else {
            let b1 = self.calc_new_block_position(self.current_block, self.current_direction);
            self.check_scene_update_need(b1 as i32);
            const DAMAGE: [i32; 3] = [75, 125, 175];
            let mut o = self.level_block_properties[b1 as usize].assigned_objects;
            while o & 0x8000 != 0 {
                let t = o;
                o = self.obj(o).next_assigned_object;
                let dmg = self.calc_inflictable_damage_per_item(
                    -1,
                    t as i32,
                    DAMAGE[(spell_level - 2).clamp(0, 2) as usize],
                    0x80,
                    1,
                );
                self.inflict_damage(t as i32, dmg, 0xFFFF, 3, 0x80);
            }
        }
        if self.current_level == 29 {
            self.screen.copy_page(12, 2);
        }
        self.screen.copy_page(2, 0);
        self.screen.update_screen();
        self.gui_draw_scene(2);
        self.update_draw_page2();
        1
    }

    pub fn process_magic_mist_of_doom(&mut self, char_num: i32, spell_level: i32) -> i32 {
        const DMG: [i32; 4] = [30, 70, 110, 200];
        let sl = spell_level.clamp(0, 3) as usize;
        self.env_sfx_use_queue = true;
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction) as i32;
        self.inflict_magical_damage_for_block(ahead, char_num, DMG[sl], 0x80);
        self.env_sfx_use_queue = false;
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 2);
        self.gui_draw_scene(2);
        self.screen.copy_page(2, 12);
        self.snd_play_sound_effect(155, -1);
        let file = format!("MISTS{}.WSA", spell_level + 1);
        if let Some(mut mov) = self.open_wsa(&file, 1, false) {
            let m = MIST_ANIM[sl];
            self.snd_play_sound_effect(m[4] as i32, -1);
            self.play_spell_animation(
                Some(&mut mov),
                m[0] as i32,
                m[1] as i32,
                7,
                112,
                0,
                None,
                None,
                None,
                0,
                false,
            );
            self.play_spell_animation(
                Some(&mut mov),
                m[2] as i32,
                m[3] as i32,
                14,
                112,
                0,
                None,
                None,
                None,
                0,
                false,
            );
        }
        self.screen.set_cur_page(cp);
        self.screen.copy_page(12, 0);
        self.update_draw_page2();
        self.snd_play_queued_effects();
        1
    }

    pub fn process_magic_lightning(&mut self, char_num: i32, spell_level: i32) -> i32 {
        let sl = spell_level.clamp(0, 4) as usize;
        self.screen.hide_mouse();
        self.screen.copy_page(0, 2);
        self.gui_draw_scene(2);
        self.screen.copy_page(2, 12);
        let def = &LIGHTNING_DEFS[sl * 4..sl * 4 + 4];
        self.lightning_cur_sfx = u16::from_le_bytes([def[2], def[3]]) as i16;
        self.lightning_div = def[1] as i16;
        self.lightning_first_sfx = 0;
        let file = format!("LITNING{}.WSA", spell_level + 1);
        if let Some(mut mov) = self.open_wsa(&file, 1, false) {
            for _ in 0..4 {
                self.play_spell_animation(
                    Some(&mut mov),
                    0,
                    def[0] as i32,
                    3,
                    93,
                    0,
                    Some(SpellCallback::Lightning),
                    None,
                    None,
                    0,
                    false,
                );
            }
        }
        let p1 = self.screen.palette(1).clone();
        self.screen.set_screen_palette(&p1);
        self.screen.copy_page(12, 2);
        self.screen.copy_page(12, 0);
        self.update_draw_page2();
        const DMG: [i32; 4] = [18, 35, 50, 72];
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction) as i32;
        self.inflict_magical_damage_for_block(
            ahead,
            char_num,
            DMG[spell_level.clamp(0, 3) as usize],
            5,
        );
        self.scene_update_required = true;
        self.gui_draw_scene(0);
        self.screen.show_mouse();
        1
    }

    pub fn process_magic_fog(&mut self) -> i32 {
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 12);
        if let Some(mut mov) = self.open_wsa("FOG.WSA", 0, false) {
            let n = mov.frames().max(1);
            self.snd_play_sound_effect(145, -1);
            let (t1, t2) = self.tables();
            for f in 0..n {
                let timer = self.millis() + 3 * TICK;
                self.screen.copy_page(12, 2);
                mov.display_frame(
                    &mut self.screen,
                    f % n,
                    2,
                    112,
                    0,
                    0x5000,
                    Some(&t1),
                    Some(&t2),
                );
                self.screen
                    .copy_region(112, 0, 112, 0, 176, 120, 2, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
                self.delay_until(timer, false);
            }
        }
        self.screen.copy_page(12, 2);
        self.screen.set_cur_page(cp);
        self.update_draw_page2();
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction);
        let mut o = self.level_block_properties[ahead as usize].assigned_objects;
        while o & 0x8000 != 0 {
            self.inflict_magical_damage(o as i32, -1, 15, 6, 0);
            o = self.monsters[(o & 0x7FFF) as usize]
                .obj
                .next_assigned_object;
        }
        self.gui_draw_scene(0);
        1
    }

    pub fn process_magic_swarm(&mut self, char_num: i32, damage: i32) -> i32 {
        self.create_transparency_tables();
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 12);
        self.snd_play_sound_effect(74, -1);
        let mut ids = [0usize; 6];
        let mut modes = [8u8; 6];
        let mut ticks = [0i8; 6];
        let mut t = 0;
        let ahead = self.calc_new_block_position(self.current_block, self.current_direction);
        let mut o = self.level_block_properties[ahead as usize].assigned_objects;
        while o & 0x8000 != 0 && t < 6 {
            let mi = (o & 0x7FFF) as usize;
            if self.monsters[mi].mode != 13 {
                ids[t] = mi;
                t += 1;
                if self.monsters[mi].flags & 0x2000 == 0 {
                    self.env_sfx_use_queue = true;
                    self.inflict_magical_damage(mi as i32 | 0x8000, char_num, damage, 0, 0);
                    self.env_sfx_use_queue = false;
                    self.monsters[mi].flags &= 0xFFEF;
                }
            }
            o = self.monsters[mi].obj.next_assigned_object;
        }
        for i in 0..t {
            std::mem::swap(&mut modes[i], &mut self.monsters[ids[i]].mode);
            std::mem::swap(&mut ticks[i], &mut self.monsters[ids[i]].fight_cur_tick);
        }
        let page = self.screen.cur_page;
        self.gui_draw_scene(page);
        self.screen
            .copy_region(112, 0, 112, 0, 176, 120, page, 7, 0);
        for i in 0..t {
            self.monsters[ids[i]].mode = modes[i];
            self.monsters[ids[i]].fight_cur_tick = ticks[i];
        }
        if let Some(mut mov) = self.open_wsa("SWARM.WSA", 0, false) {
            self.screen.hide_mouse();
            self.play_spell_animation(Some(&mut mov), 0, 37, 2, 0, 0, None, None, None, 0, false);
            self.play_spell_animation(
                Some(&mut mov),
                38,
                41,
                8,
                0,
                0,
                Some(SpellCallback::Swarm),
                None,
                None,
                0,
                false,
            );
            self.screen.show_mouse();
        }
        self.screen.copy_page(12, 0);
        self.screen.update_screen();
        self.update_draw_page2();
        self.snd_play_queued_effects();
        self.screen.set_cur_page(cp);
        1
    }

    pub fn process_magic_vaelans_cube(&mut self) -> i32 {
        let sp1 = self.screen.palette(1).clone();
        let pal1 = sp1.0.clone();
        let mut pal2 = sp1.0.clone();
        for i in 0..128 {
            pal2[i * 3] = (sp1.0[i * 3] as u16 + 16).min(60) as u8;
            pal2[i * 3 + 1] = sp1.0[i * 3 + 1];
            pal2[i * 3 + 2] = (sp1.0[i * 3 + 2] as u16 + 19).min(60) as u8;
        }
        self.snd_play_sound_effect(146, -1);
        let start = self.millis();
        let end = start + 70 * TICK;
        while self.millis() < end && !self.should_quit() {
            let e = self.millis() - start;
            self.screen
                .timed_palette_fade_step(&pal1, &pal2, e, 70 * TICK);
            self.update_input();
            self.screen.system.delay_millis(TICK);
        }
        let bl = self.calc_new_block_position(self.current_block, self.current_direction) as usize;
        let s = self.level_block_properties[bl].walls[(self.current_direction ^ 2) as usize];
        let flg = self.wll_wall_flags[s as usize];
        let mut res = if s == 47 && (self.current_level == 17 || self.current_level == 24) {
            1
        } else {
            0
        };
        if (self.wll_vmp_map[s as usize] == 1 || self.wll_vmp_map[s as usize] == 2)
            && flg & 1 == 0
            && self.current_level != 22
        {
            self.level_block_properties[bl].walls = [0; 4];
            self.gui_draw_scene(0);
            res = 1;
        }
        let mut o = self.level_block_properties[bl].assigned_objects;
        while o & 0x8000 != 0 {
            let mi = (o & 0x7FFF) as usize;
            if self.monster_properties[self.monsters[mi].properties.min(4)].flags & 0x1000 != 0 {
                self.inflict_damage(o as i32, 100, 0xFFFF, 0, 0x80);
                res = 1;
            }
            o = self.monsters[mi].obj.next_assigned_object;
        }
        let start = self.millis();
        let end = start + 70 * TICK;
        while self.millis() < end && !self.should_quit() {
            let e = self.millis() - start;
            self.screen
                .timed_palette_fade_step(&pal2, &pal1, e, 70 * TICK);
            self.update_input();
            self.screen.system.delay_millis(TICK);
        }
        res
    }

    pub fn process_magic_guardian(&mut self, char_num: i32) -> i32 {
        let cp = self.screen.set_cur_page(2);
        self.screen.copy_page(0, 2);
        self.screen.copy_page(2, 12);
        let Some(mut mov) = self.open_wsa("GUARDIAN.WSA", 0, false) else {
            return 0;
        };
        self.snd_play_sound_effect(156, -1);
        self.play_spell_animation(Some(&mut mov), 0, 37, 2, 112, 0, None, None, None, 0, false);
        self.screen.copy_page(2, 12);
        let bl = self.calc_new_block_position(self.current_block, self.current_direction);
        let res = if self.level_block_properties[bl as usize].assigned_objects & 0x8000 != 0 {
            1
        } else {
            0
        };
        self.inflict_magical_damage_for_block(bl as i32, char_num, 200, 0x80);
        self.screen.copy_page(12, 2);
        self.update_draw_page2();
        self.gui_draw_scene(2);
        self.screen.copy_page(2, 12);
        self.snd_play_sound_effect(176, -1);
        self.play_spell_animation(
            Some(&mut mov),
            38,
            48,
            8,
            112,
            0,
            None,
            None,
            None,
            0,
            false,
        );
        self.screen.set_cur_page(cp);
        self.gui_draw_play_field();
        self.update_draw_page2();
        res
    }

    fn spell_callback(&mut self, cb: SpellCallback) {
        match cb {
            SpellCallback::Swarm => {
                if self.swarm_spell_status != 0 {
                    let page = self.screen.cur_page;
                    self.screen
                        .copy_region(112, 0, 112, 0, 176, 120, 6, page, 0);
                }
                self.swarm_spell_status ^= 1;
            }
            SpellCallback::Lightning => {
                if self.lightning_div == 2 {
                    self.shake_scene(1, 2, 3, 0);
                }
                let p1 = self.screen.palette(1).clone();
                if self.lightning_div != 0 && self.lightning_sfx_frame % self.lightning_div != 0 {
                    self.screen.set_screen_palette(&p1);
                } else {
                    let mut t = p1.clone();
                    for i in 6..384 {
                        let v = (t.0[i] as u16 * 120) / 64;
                        t.0[i] = v.min(63) as u8;
                    }
                    self.screen.set_screen_palette(&t);
                }
                if self.lightning_div == 2 {
                    if self.lightning_first_sfx == 0 {
                        let s = self.lightning_cur_sfx as i32;
                        self.snd_play_sound_effect(s, -1);
                        self.lightning_first_sfx = 1;
                    }
                } else if self.lightning_sfx_frame & 7 == 0 {
                    let s = self.lightning_cur_sfx as i32;
                    self.snd_play_sound_effect(s, -1);
                }
                self.lightning_sfx_frame += 1;
            }
        }
    }

    /// `drinkBezelCup`.
    pub fn drink_bezel_cup(&mut self, num_uses: i32, char_num: usize) {
        self.create_transparency_tables();
        let cp = self.screen.set_cur_page(2);
        self.snd_play_sound_effect(73, -1);
        let Some(mut mov) = self.open_wsa("BEZEL.WSA", 0, false) else {
            return;
        };
        let x = self.active_chars_xpos[char_num.min(2)] as i32 - 11;
        let y = 124;
        let (w, h) = (mov.width, mov.height);
        self.screen
            .copy_region(x, y, 0, 0, w, h, 0, 2, CR_NO_P_CHECK);
        const ANIM: [i32; 12] = [0, 26, 20, 27, 61, 55, 62, 92, 86, 93, 131, 125];
        let nu = num_uses.clamp(0, 3) as usize;
        let mut frm = ANIM[nu * 3];
        let hp_diff = self.characters[char_num].hit_points_max as i32
            - self.characters[char_num].hit_points_cur as i32;
        // uint16 in the original
        let mut step: u16 = 0;
        let (t1, t2) = self.tables();
        loop {
            step = ((step & 0xFF) as i32 + (hp_diff * 256) / ANIM[nu * 3 + 1].max(1)) as u16;
            self.increase_character_hitpoints(char_num, (step / 256) as i32, true);
            self.gui_draw_char_portrait_with_stats(char_num);
            let etime = self.millis() + 4 * TICK;
            self.screen
                .copy_region(0, 0, x, y, w, h, 2, 2, CR_NO_P_CHECK);
            mov.display_frame(&mut self.screen, frm, 2, x, y, 0x5000, Some(&t1), Some(&t2));
            self.screen
                .copy_region(x, y, x, y, w, h, 2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.delay_until(etime, false);
            frm += 1;
            if frm >= ANIM[nu * 3 + 1] || self.should_quit() {
                break;
            }
        }
        self.characters[char_num].hit_points_cur = self.characters[char_num].hit_points_max as i16;
        self.screen
            .copy_region(0, 0, x, y, w, h, 2, 2, CR_NO_P_CHECK);
        self.remove_character_effects(char_num, 4, 4);
        self.gui_draw_char_portrait_with_stats(char_num);
        self.screen
            .copy_region(x, y, x, y, w, h, 2, 0, CR_NO_P_CHECK);
        self.screen.update_screen();
        self.screen.set_cur_page(cp);
    }

    /// `addSpellToScroll`.
    pub fn add_spell_to_scroll(&mut self, spell: i32, char_num: i32) {
        let mut assigned = false;
        let mut slot = 0;
        for i in 0..7 {
            if !assigned && self.available_spells[i] == -1 {
                assigned = true;
                slot = i;
            }
            if self.available_spells[i] as i32 == spell {
                let s = self.get_lang_string(0x42D0);
                self.print_message(2, b"%s", &[Arg::Str(s)]);
                return;
            }
        }
        if spell > 1 {
            self.transfer_spell_to_scroll_animation(char_num, spell, slot as i32 - 1);
        }
        self.available_spells[slot] = spell as i8;
        self.gui_enable_default_playfield_buttons();
    }

    fn transfer_spell_to_scroll_animation(&mut self, char_num: i32, spell: i32, slot: i32) {
        let cx = 16 + self.active_chars_xpos[char_num.clamp(0, 2) as usize] as i32;
        if slot != 1 {
            if let Some(d) = self.res.file_data("PLAYFLD.CPS") {
                self.screen.load_bitmap(&d, 3, None);
            }
            self.screen
                .copy_region(8, 0, 216, 0, 96, 120, 3, 3, CR_NO_P_CHECK);
            self.screen.copy_page(3, 10);
            for i in 0..9 {
                let h = (slot + 1) * 9 + i + 1;
                let timer = self.millis() + TICK;
                self.screen.copy_page(10, 3);
                self.screen
                    .copy_region(216, 0, 8, 0, 96, 120, 3, 3, CR_NO_P_CHECK);
                self.screen
                    .copy_region(112, 0, 12, 0, 87, 15, 2, 2, CR_NO_P_CHECK);
                self.screen
                    .copy_region(201, 1, 17, 15, 6, h, 2, 2, CR_NO_P_CHECK);
                self.screen
                    .copy_region(208, 1, 89, 15, 6, h, 2, 2, CR_NO_P_CHECK);
                let cp = self.screen.set_cur_page(2);
                self.screen.fill_rect(21, 15, 89, h + 15, 206, None);
                self.screen
                    .copy_region(112, 16, 12, h + 15, 87, 14, 2, 2, CR_NO_P_CHECK);
                let mut y = 15;
                let of = self.screen.set_font(FontId::F9);
                for ii in 0..7 {
                    if self.available_spells[ii] == -1 {
                        continue;
                    }
                    let col = if ii as i32 == self.selected_spell {
                        132
                    } else {
                        1
                    };
                    let name = self.get_lang_string(
                        SPELL_PROPERTIES[self.available_spells[ii] as usize].spell_name_code,
                    );
                    self.screen.fprint_string(&name, 24, y, col, 0, 0);
                    y += 9;
                }
                self.screen.set_font(of);
                self.screen.set_cur_page(cp);
                self.screen
                    .copy_region(8, 0, 8, 0, 96, 120, 3, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
                self.delay_until(timer, false);
            }
        }
        self.screen.hide_mouse();
        self.screen.copy_page(0, 12);
        let sl = slot.clamp(0, 5) as usize;
        let vx = SPELLBOOK_COORDS[sl << 1] as i32 + 32;
        let vy = SPELLBOOK_COORDS[(sl << 1) + 1] as i32 + 5;
        let lang = ['e', 'f', 'g'][self.lang_index];
        let wsa_file = format!("WRITE{spell}{}.WSA", lang.to_ascii_uppercase());
        let sp = (spell.clamp(0, 6) as usize) << 2;
        self.snd_play_sound_effect(SPELLBOOK_ANIM[sp + 3] as i32, -1);
        self.snd_play_sound_effect(95, -1);
        let (t1, t2) = self.tables();
        let ax = self.active_chars_xpos[char_num.clamp(0, 2) as usize] as i32;
        if let Some(mut mov) = self.open_wsa("GETSPELL.WSA", 0, false) {
            self.snd_play_sound_effect(128, -1);
            self.play_spell_animation(Some(&mut mov), 0, 25, 5, ax, 148, None, None, None, 0, true);
            self.snd_play_sound_effect(128, -1);
            self.play_spell_animation(
                Some(&mut mov),
                26,
                52,
                5,
                ax,
                148,
                None,
                None,
                None,
                0,
                true,
            );
            for i in (1..=16).rev() {
                let timer = self.millis() + TICK;
                self.screen.copy_page(12, 2);
                let wx = vx + (((((cx - vx) << 8) / 16) * i) >> 8) - 16;
                let wy = vy + (((((160 - vy) << 8) / 16) * i) >> 8) - 16;
                mov.display_frame(
                    &mut self.screen,
                    51,
                    2,
                    wx,
                    wy,
                    0x5000,
                    Some(&t1),
                    Some(&t2),
                );
                self.screen.copy_region(
                    wx,
                    wy,
                    wx,
                    wy,
                    mov.width + 48,
                    mov.height + 48,
                    2,
                    0,
                    CR_NO_P_CHECK,
                );
                self.screen.update_screen();
                self.delay_until(timer, false);
            }
        }
        if let Some(mut mov) = self.open_wsa("SPELLEXP.WSA", 0, false) {
            self.snd_play_sound_effect(168, -1);
            self.play_spell_animation(
                Some(&mut mov),
                0,
                8,
                3,
                vx - 44,
                vy - 38,
                None,
                None,
                None,
                0,
                true,
            );
        }
        let (bx, by) = (
            SPELLBOOK_COORDS[sl << 1] as i32,
            SPELLBOOK_COORDS[(sl << 1) + 1] as i32,
        );
        if let Some(mut mov) = self.open_wsa("WRITING.WSA", 0, false) {
            self.play_spell_animation(Some(&mut mov), 0, 6, 5, bx, by, None, None, None, 0, false);
        }
        if let Some(mut mov) = self.open_wsa(&wsa_file, 0, false) {
            self.snd_play_sound_effect(SPELLBOOK_ANIM[sp + 3] as i32, -1);
            self.play_spell_animation(
                Some(&mut mov),
                SPELLBOOK_ANIM[sp + 1] as i32,
                SPELLBOOK_ANIM[sp + 2] as i32,
                SPELLBOOK_ANIM[sp] as i32,
                bx,
                by,
                None,
                None,
                None,
                0,
                false,
            );
        }
        self.gui_draw_scene(2);
        self.update_draw_page2();
        self.screen.show_mouse();
    }

    /// `playSpellAnimation`.
    pub fn play_spell_animation(
        &mut self,
        mut mov: Option<&mut Wsa>,
        first: i32,
        last: i32,
        frame_delay: i32,
        x: i32,
        y: i32,
        callback: Option<SpellCallback>,
        pal1: Option<&[u8]>,
        pal2: Option<&[u8]>,
        fade_delay: i32,
        restore_screen: bool,
    ) {
        let (w, h) = mov.as_ref().map_or((0, 0), |m| (m.width, m.height));
        let (mut w2, mut h2) = (w, h);
        let start = self.millis();
        if x < 0 {
            w2 += x;
        }
        if y < 0 {
            h2 += y;
        }
        let dir = if last >= first { 1 } else { -1 };
        let mut cur = first;
        let (t1, t2) = self.tables();
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 10_000 || self.should_quit() {
                break;
            }
            let timer = self.millis() + TICK * frame_delay.max(0) as u32;
            if mov.is_some() || callback.is_some() {
                self.screen.copy_page(12, 2);
            }
            if let Some(cb) = callback {
                self.spell_callback(cb);
            }
            if let Some(m) = mov.as_deref_mut() {
                let n = m.frames().max(1);
                m.display_frame(
                    &mut self.screen,
                    cur.rem_euclid(n),
                    2,
                    x,
                    y,
                    0x5000,
                    Some(&t1),
                    Some(&t2),
                );
            }
            if mov.is_some() || callback.is_some() {
                self.screen
                    .copy_region(x, y, x, y, w2, h2, 2, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
            }
            let tm = self.millis();
            let mut del = timer.saturating_sub(tm);
            loop {
                let step = del.min(TICK);
                if let (Some(p1), Some(p2)) = (pal1, pal2) {
                    let e = self.millis() - start;
                    if !self.screen.timed_palette_fade_step(
                        p1,
                        p2,
                        e,
                        TICK * fade_delay.max(1) as u32,
                    ) && mov.is_none()
                    {
                        return;
                    }
                }
                if del > 0 {
                    self.delay(step, false);
                    del -= step;
                } else {
                    self.update_input();
                    if pal1.is_some() && mov.is_none() {
                        self.screen.system.delay_millis(TICK);
                    }
                }
                if del == 0 {
                    break;
                }
            }
            if mov.is_none() {
                continue;
            }
            cur += dir;
            if (dir > 0 && cur >= last) || (dir < 0 && cur < last) {
                break;
            }
        }
        if restore_screen && mov.is_some() {
            self.screen.copy_page(12, 2);
            self.screen
                .copy_region(x, y, x, y, w2, h2, 2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
        }
    }

    /// `checkMagic`: 1 if the character cannot cast it.
    pub fn check_magic(&mut self, char_num: i32, spell: i32, level: i32) -> i32 {
        if spell < 0 {
            return 1;
        }
        let p = SPELL_PROPERTIES[spell as usize];
        let c = char_num as usize;
        let l = level.clamp(0, 3) as usize;
        if p.mp_required[l] as i32 > self.characters[c].magic_points_cur as i32 {
            let id = self.characters[c].id as i32;
            if self.character_says(0x4043, id, true) != 0 {
                let fmt = self.get_lang_string(0x4043);
                let name = self.characters[c].name.clone();
                self.print_message(6, &fmt, &[Arg::Str(name)]);
            }
            return 1;
        } else if p.hp_required[l] as i32 >= self.characters[c].hit_points_cur as i32 {
            let fmt = self.get_lang_string(0x4179);
            let name = self.characters[c].name.clone();
            self.print_message(2, &fmt, &[Arg::Str(name)]);
            return 1;
        }
        0
    }

    /// `getSpellTargetBlock`: the distance and the block where something stops the spell.
    pub fn get_spell_target_block(
        &self,
        mut current: i32,
        direction: i32,
        max_distance: i32,
    ) -> (i32, u16) {
        let mut c = self.calc_new_block_position(current as u16, direction as u16) as i32;
        let mut i = 0;
        while i < max_distance {
            if self.level_block_properties[current as usize].assigned_objects & 0x8000 != 0 {
                return (i, current as u16);
            }
            let w = self.level_block_properties[c as usize].walls[(direction ^ 2) as usize];
            if self.wll_wall_flags[w as usize] & 7 != 0 {
                return (i, c as u16);
            }
            current = c;
            c = self.calc_new_block_position(current as u16, direction as u16) as i32;
            i += 1;
        }
        (i, 0xFFFF)
    }

    pub fn inflict_magical_damage(
        &mut self,
        target: i32,
        attacker: i32,
        damage: i32,
        index: i32,
        hit_type: i32,
    ) {
        let hit_type = if hit_type != 0 { 1 } else { 2 };
        let damage =
            self.calc_inflictable_damage_per_item(attacker, target, damage, index, hit_type);
        self.inflict_damage(target, damage, attacker, 2, index);
    }
}

/// `Screen_v2::findLeastDifferentColor`.
pub fn find_least_different_color(
    entry: &[u8],
    pal: &Pal,
    first: usize,
    num: usize,
    skip_special: bool,
) -> u8 {
    let mut m = 0x7FFF;
    let mut r = 0x101;
    for i in 0..num {
        if skip_special && (0xC0..=0xC3).contains(&i) {
            continue;
        }
        let at = (i + first) * 3;
        let Some(rgb) = pal.0.get(at..at + 3) else {
            break;
        };
        let d = |a: u8, b: u8| (a as i32 - b as i32) * (a as i32 - b as i32);
        let c = d(entry[0], rgb[0]) + d(entry[1], rgb[1]) + d(entry[2], rgb[2]);
        if c <= m {
            m = c;
            r = i;
        }
    }
    r as u8
}
