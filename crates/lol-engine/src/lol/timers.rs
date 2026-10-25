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

//! The game's timers (ScummVM's `timer_lol.cpp`, `timer_rpg.cpp` and `TimerManager::update`).

use super::types::Arg;
use super::{Lol, TICK};

/// `_clock2Timers`: paused while the game shows something else (`disableSysTimer(2)`).
const CLOCK2_TIMERS: [u8; 11] = [
    0x00, 0x10, 0x11, 0x03, 0x04, 0x50, 0x51, 0x52, 0x08, 0x09, 0x0A,
];

impl Lol {
    pub fn setup_timers(&mut self) {
        let now = self.millis();
        self.timers.add(0, 15, true);
        self.timers.add(0x10, 6, true);
        self.timers.add(0x11, 6, true);
        self.timers.set_next_run(0x11, now + 3 * TICK, now);
        self.timers.add(3, 15, true);
        self.timers.add(4, 1, true);
        self.timers.add(0x50, 0, false);
        self.timers.add(0x51, 0, false);
        self.timers.add(0x52, 0, false);
        self.timers.add(8, 1200, true);
        self.timers.add(9, 10, true);
        self.timers.add(10, 360, true);
        self.timers.add(11, 360, false);
        self.timers.reset_next_run();
    }

    /// `TimerManager::update`: each enabled timer that is due runs its function.
    pub fn timer_update(&mut self) {
        let now = self.millis();
        if now < self.timers.next_run {
            return;
        }
        self.timers.next_run = self.timers.next_run.wrapping_add(99_999);
        let mut i = 0;
        while i < self.timers.list.len() {
            let t = self.timers.list[i];
            if t.enabled == 1 && t.countdown >= 0 {
                if t.next_run <= self.millis() {
                    self.run_timer(t.id);
                    let cur = self.millis();
                    if let Some(e) = self.timers.list.get_mut(i) {
                        e.last_update = cur as i64;
                        e.next_run = cur + e.countdown.max(0) as u32 * TICK;
                    }
                }
                if let Some(e) = self.timers.list.get(i) {
                    self.timers.next_run = self.timers.next_run.min(e.next_run);
                }
            }
            i += 1;
        }
    }

    fn run_timer(&mut self, id: u8) {
        match id {
            0 => self.timer_process_doors(),
            0x10 | 0x11 => self.timer_process_monsters(id as i32),
            3 => self.timer_special_character_update(),
            4 => self.timer_process_flying_objects(),
            0x50..=0x52 => self.run_level_script(0x401 + (id as i32 & 0x0F), -1),
            8 => self.timer_regenerate_points(),
            9 => self.timer_update_portrait_animations(0),
            10 => self.timer_update_lamp_state(),
            11 => {
                self.timers.disable(11);
                self.init_text_fading(0, false);
            }
            _ => {}
        }
    }

    pub fn enable_sys_timer(&mut self, sys_timer: i32) {
        if sys_timer != 2 {
            return;
        }
        let now = self.millis();
        for t in CLOCK2_TIMERS {
            self.timers.pause_single(t, false, now);
        }
    }

    pub fn disable_sys_timer(&mut self, sys_timer: i32) {
        if sys_timer != 2 {
            return;
        }
        let now = self.millis();
        for t in CLOCK2_TIMERS {
            self.timers.pause_single(t, true, now);
        }
    }

    /// `timerProcessDoors`.
    pub fn timer_process_doors(&mut self) {
        for i in 0..3 {
            let st = self.open_door_state[i];
            let b = st.block as usize;
            if b == 0 {
                continue;
            }
            let v = st.state as i32;
            let c = st.wall as usize & 3;
            let w = &mut self.level_block_properties[b].walls;
            w[c] = (w[c] as i32 + v) as u8;
            w[c ^ 2] = (w[c ^ 2] as i32 + v) as u8;
            let flg = self.wll_wall_flags[w[c] as usize];
            let snd = if flg & 0x20 != 0 {
                5
            } else if v == -1 {
                4
            } else {
                3
            };
            if self.update_flags & 1 == 0 {
                self.snd_process_environmental_sound_effect(snd + 28, b as i32);
                if !self.check_scene_update_need(b as i32) {
                    self.snd_update_environmental_sfx(0);
                }
            }
            if flg & 0x30 != 0 {
                self.open_door_state[i].block = 0;
            }
        }
    }

    pub fn timer_process_monsters(&mut self, timer_num: i32) {
        let mut i = (timer_num & 0x0F) as usize;
        while i < 30 {
            self.update_monster(i);
            i += 2;
        }
    }

    pub fn timer_special_character_update(&mut self) {
        let mut events_left: i32 = 0;
        for i in 0..4 {
            if self.characters[i].flags & 1 == 0 {
                continue;
            }
            for ii in 0..5 {
                if self.characters[i].character_update_events[ii] == 0 {
                    continue;
                }
                let d = self.characters[i].character_update_delay[ii].wrapping_sub(1);
                self.characters[i].character_update_delay[ii] = d;
                if d > 0 {
                    if d as i32 > events_left {
                        events_left = d as i32;
                    }
                    continue;
                }
                match self.characters[i].character_update_events[ii] as i32 - 1 {
                    0 => {
                        if self.characters[i].weapon_hit != 0 {
                            self.characters[i].weapon_hit = 0;
                            // Stored as uint8 before the comparison.
                            let v = self.calc_monster_skill_level(i as i32, 6) as u8 as i32;
                            self.characters[i].character_update_delay[ii] = v as u8;
                            if v > events_left {
                                events_left = v;
                            }
                        } else {
                            self.characters[i].flags &= 0xFFFB;
                        }
                        self.gui_draw_char_portrait_with_stats(i);
                    }
                    1 => {
                        self.characters[i].damage_suffered = 0;
                        self.gui_draw_char_portrait_with_stats(i);
                    }
                    2 => {
                        self.characters[i].flags &= 0xFFBF;
                        self.gui_draw_char_portrait_with_stats(i);
                    }
                    3 => {
                        events_left = self.roll_dice(1, 2, 0);
                        if self.inflict_damage(i as i32, events_left, 0x8000, 0, 0x80) != 0 {
                            let f = self.get_lang_string(0x4022);
                            let n = self.characters[i].name.clone();
                            self.print_message(2, &f, &[Arg::Str(n)]);
                            self.characters[i].character_update_delay[ii] = 10;
                            if 10 > events_left {
                                events_left = 10;
                            }
                        }
                    }
                    4 => {
                        self.characters[i].flags &= 0xFEFF;
                        let f = self.get_lang_string(0x4027);
                        let n = self.characters[i].name.clone();
                        self.print_message(0, &f, &[Arg::Str(n)]);
                        self.gui_draw_char_portrait_with_stats(i);
                    }
                    5 => self.set_temporary_face_frame(i, 0, 0, true),
                    6 => {
                        self.characters[i].flags &= 0xEFFF;
                        self.gui_draw_char_portrait_with_stats(i);
                    }
                    7 => self.restore_swamp_palette(),
                    _ => {}
                }
                if self.characters[i].character_update_delay[ii] == 0 {
                    self.characters[i].character_update_events[ii] = 0;
                }
            }
        }
        if events_left != 0 {
            self.timers.enable(3);
        } else {
            self.timers.disable(3);
        }
    }

    pub fn timer_process_flying_objects(&mut self) {
        for i in 0..8 {
            if self.flying_objects[i].enable == 0 {
                continue;
            }
            self.update_flying_object(i);
        }
    }

    pub fn timer_regenerate_points(&mut self) {
        for i in 0..4 {
            if self.characters[i].flags & 1 == 0 {
                continue;
            }
            let dead = self.characters[i].flags & 8 != 0;
            let h_inc = if dead {
                0
            } else if self.item_equipped(i, 228) {
                4
            } else {
                1
            };
            let mmax = self.characters[i].magic_points_max as i32;
            let m_inc = if self.drain_magic != 0 {
                -(mmax >> 5)
            } else if dead {
                0
            } else if self.item_equipped(i, 227) {
                mmax / 10
            } else {
                1
            };
            let c = &mut self.characters[i];
            c.magic_points_cur = (c.magic_points_cur as i32 + m_inc).clamp(0, mmax) as i16;
            if c.flags & 0x80 == 0 {
                self.increase_character_hitpoints(i, h_inc, false);
            }
            self.gui_draw_char_portrait_with_stats(i);
        }
    }

    pub fn timer_update_portrait_animations(&mut self, skip_update: i32) {
        let skip = skip_update == 1;
        for i in 0..4 {
            let c = &self.characters[i];
            if c.flags & 1 == 0 || c.flags & 8 != 0 || c.cur_face_frame > 1 {
                continue;
            }
            if c.cur_face_frame != 1 {
                self.characters[i].next_anim_update_countdown -= 1;
                if self.characters[i].next_anim_update_countdown <= 0 && !skip {
                    self.characters[i].cur_face_frame = 1;
                    self.gui_draw_char_portrait_with_stats(i);
                    let now = self.millis();
                    self.timers.set_countdown(9, 10, now, TICK);
                }
            } else {
                self.characters[i].cur_face_frame = 0;
                self.gui_draw_char_portrait_with_stats(i);
                self.characters[i].next_anim_update_countdown =
                    (self.roll_dice(1, 12, 0) + 6) as i16;
            }
        }
    }

    pub fn timer_update_lamp_state(&mut self) {
        if self.flags_table[31] & 0x08 != 0
            && self.flags_table[31] & 0x04 != 0
            && self.brightness != 0
            && self.lamp_oil_status != 0
        {
            self.lamp_oil_status -= 1;
        }
    }
}
