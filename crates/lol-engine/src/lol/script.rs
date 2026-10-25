//! The levels' scripts (ScummVM's `script_lol.cpp`): running `LEVELn.INI`/`.INF` and
//! `ITEM.INF`, and the 192 functions of the game the scripts call (`olol_…`).

use std::rc::Rc;

use lol_data::emc::{Args, Host, STACK_SIZE, Script, State};
use lol_data::tables::{EXP_REQUIREMENTS, INGAME_SFX_INDEX, ITEM_PRICES, MONSTER_MODIFIERS1};

use super::tim::{TimOpcodes, TimRef};
use super::types::*;
use super::{Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId, Pal};
use crate::shape;

impl Lol {
    /// Loads a script of the game's (`EMCInterpreter::load`).
    pub fn load_script(&self, name: &str) -> Option<Rc<Script>> {
        let bytes = self.res.file_data(name)?;
        Script::parse(name, &bytes).ok().map(Rc::new)
    }

    /// Runs a started script to its end (`while (isValid) run`).
    pub fn run_script_to_end(&mut self, script: &Script, st: &mut State) {
        let mut steps = 0u64;
        while st.ip.is_some() && !self.should_quit() {
            script.step(st, self);
            if self.emc_stop {
                self.emc_stop = false;
                st.ip = None;
            }
            // Scripts may wait for the player as long as they like (dialogue loops), so
            // there is no limit, only a note for test runs.
            steps += 1;
            if steps == 20_000_000 && std::env::var_os("LOL_DEBUG").is_some() {
                eprintln!("{}: Skript läuft sehr lange (ip {:?})", script.name, st.ip);
            }
        }
    }

    pub fn run_init_script(&mut self, filename: &str, optional_func: i32) {
        self.suspend_script = true;
        self.script_data = None;
        let loaded = self.load_script(filename);
        if std::env::var_os("LOL_DEBUG").is_some() {
            eprintln!(
                "{filename}: {}",
                if loaded.is_some() { "geladen" } else { "fehlt" }
            );
        }
        if let Some(script) = loaded {
            let mut st = State::default();
            if script.start(&mut st, 0) {
                self.run_script_to_end(&script, &mut st);
            }
            if optional_func != 0 {
                // `EMCInterpreter::init` resets only ip, sp, bp and the stack's last entry: the
                // registers, the return value and the rest of the stack stay from function 0.
                let prev = st.clone();
                let ok = script.start(&mut st, optional_func as usize);
                st.regs = prev.regs;
                st.ret = prev.ret;
                st.stack[..STACK_SIZE - 1].copy_from_slice(&prev.stack[..STACK_SIZE - 1]);
                if std::env::var_os("LOL_DEBUG").is_some() {
                    eprintln!(
                        "{filename}: Funktion {optional_func} {}",
                        if ok { "startet" } else { "fehlt" }
                    );
                }
                if ok {
                    self.run_script_to_end(&script, &mut st);
                }
            }
        }
        self.suspend_script = false;
    }

    pub fn run_inf_script(&mut self, filename: &str) {
        self.script_data = self.load_script(filename);
        self.run_level_script(0x400, -1);
    }

    pub fn run_level_script(&mut self, block: i32, flags: i32) {
        self.run_level_script_custom(block, flags, -1, 0, 0, 0);
    }

    pub fn run_level_script_custom(
        &mut self,
        block: i32,
        flags: i32,
        char_num: i32,
        item: i32,
        reg3: i32,
        reg4: i32,
    ) {
        if !self.suspend_script
            && let Some(script) = self.script_data.clone()
        {
            let mut st = State::default();
            if script.start(&mut st, block as u16 as usize) {
                st.regs[0] = flags as i16;
                st.regs[1] = char_num as i16;
                st.regs[2] = item as i16;
                st.regs[3] = reg3 as i16;
                st.regs[4] = reg4 as i16;
                st.regs[5] = block as i16;
                st.regs[6] = self.script_direction as i16;
                if (script.entry_flags(&st) as i32) & flags != 0 {
                    self.run_script_to_end(&script, &mut st);
                }
            }
        }
        self.check_scene_update_need(block);
    }

    fn tim_slot(&self, i: i32) -> usize {
        (i.max(0) as usize).min(self.active_tim.len() - 1)
    }

    /// One of the game's functions (`_opcodes[id]`); `None` where the table has none.
    fn olol(&mut self, id: u8, a: &Args) -> Option<i32> {
        let sp = |i: usize| a.pos(i) as i32;
        let safe = |i: usize| {
            if a.sp() + i < STACK_SIZE {
                a.pos(i) as i32
            } else {
                0
            }
        };
        let s = |i: usize| String::from_utf8_lossy(a.bytes(i)).to_string();
        let stack: Vec<i32> = (0..40).map(&safe).collect();
        let r = match id {
            0x00 => {
                if sp(2) != -1 && self.wll_wall_flags[sp(2) as usize & 0xFF] & 4 != 0 {
                    self.delete_monsters_from_block(sp(0) as usize & 0x3FF);
                }
                self.set_wall_type(sp(0), sp(1), sp(2));
                1
            }
            0x01 => {
                self.level_block_properties[sp(0) as usize & 0x3FF].walls[sp(1) as usize & 3] as i8
                    as i32
            }
            0x02 => {
                self.draw_scene(sp(0) as usize);
                1
            }
            0x03 => self.roll_dice(sp(0), sp(1), 0),
            0x04 => {
                let mut mode = sp(0);
                if mode > 5 && mode < 10 {
                    mode = (mode - 6 - self.current_direction as i32) & 3;
                }
                let b = Button::blank();
                match mode {
                    0 => {
                        self.clicked_up_arrow(&b);
                    }
                    1 => {
                        self.clicked_right_arrow(&b);
                    }
                    2 => {
                        self.clicked_down_arrow(&b);
                    }
                    3 => {
                        self.clicked_left_arrow(&b);
                    }
                    4 => {
                        self.clicked_turn_left_arrow(&b);
                    }
                    5 => {
                        self.clicked_turn_right_arrow(&b);
                    }
                    10..=13 => {
                        let mut m = (mode - 10 - self.current_direction as i32).abs();
                        if m > 2 {
                            m = -(m ^ 2);
                        }
                        while m != 0 && !self.should_quit() {
                            if m > 0 {
                                self.clicked_turn_right_arrow(&b);
                                m -= 1;
                            } else {
                                self.clicked_turn_left_arrow(&b);
                                m += 1;
                            }
                        }
                    }
                    _ => {}
                }
                1
            }
            0x06 => {
                self.delay(sp(0).max(0) as u32 * TICK, true);
                1
            }
            0x07 => {
                if sp(1) != 0 {
                    self.set_game_flag(sp(0));
                } else {
                    self.reset_game_flag(sp(0));
                }
                1
            }
            0x08 => {
                if sp(0) < 0 {
                    0
                } else {
                    self.query_game_flag(sp(0))
                }
            }
            0x09 => {
                let vcn = if sp(3) == -1 { -1 } else { sp(3) as u16 as i32 };
                let vmp = if sp(4) == -1 { -1 } else { sp(4) as u16 as i32 };
                let pal = if sp(5) == -1 { None } else { Some(s(5)) };
                let f = s(0);
                self.load_level_graphics(Some(&f), sp(1), sp(2), vcn, vmp, pal.as_deref());
                1
            }
            0x0A => {
                self.load_block_properties(&s(0));
                1
            }
            0x0B => {
                self.load_monster_shapes(&s(0), sp(1) as usize % 3, sp(2));
                1
            }
            0x0C => {
                let r = self.item_in_hand as i32;
                let ih = self.item_in_hand;
                self.delete_item(ih);
                self.set_hand_item(0);
                r
            }
            0x0D => {
                self.item_properties = vec![ItemProperty::default(); sp(0).max(0) as usize];
                1
            }
            0x0E => {
                let i = sp(0) as usize;
                if i >= self.item_properties.len() {
                    self.item_properties.resize(i + 1, ItemProperty::default());
                }
                let t = &mut self.item_properties[i];
                t.name_string_id = sp(1) as u16;
                t.shp_index = sp(2) as u8;
                t.type_ = sp(3) as u16;
                if i == 264 && t.type_ == 5 {
                    t.type_ = 0;
                }
                t.item_script_func = sp(4) as u8;
                t.might = sp(5) as i8;
                t.skill = sp(6) as u8;
                t.protection = sp(7) as u8;
                t.flags = sp(8) as u16;
                t.unk_b = sp(9) as u16;
                1
            }
            0x0F => self.make_item(sp(0), sp(1), sp(2)) as i32,
            0x10 => {
                self.place_move_level_item(
                    sp(0) as u16,
                    sp(1),
                    sp(2),
                    sp(3) & 0xFF,
                    sp(4) & 0xFF,
                    sp(5),
                );
                1
            }
            0x11 => {
                let item = self.make_item(sp(0), sp(1), sp(2));
                self.place_move_level_item(item, sp(3), sp(4), sp(5), sp(6), sp(7));
                item as i32
            }
            0x12 => {
                if sp(0) == 0 {
                    return Some(0);
                }
                let i = self.items_in_play[sp(0) as usize % 400];
                let p = self
                    .item_properties
                    .get(i.item_property_index as usize)
                    .copied()
                    .unwrap_or_default();
                match sp(1) {
                    0 => i.obj.block as i32,
                    1 => i.obj.x as i32,
                    2 => i.obj.y as i32,
                    3 => i.level as i32,
                    4 => i.item_property_index as i32,
                    5 => i.shp_cur_frame_flg as i16 as i32,
                    6 => p.name_string_id as i32,
                    8 => p.shp_index as i32,
                    9 => p.type_ as i32,
                    10 => p.item_script_func as i32,
                    11 => p.might as i32,
                    12 => p.skill as i32,
                    13 => p.protection as i32,
                    14 => p.unk_b as i32,
                    15 => (i.shp_cur_frame_flg & 0x1FFF) as i32,
                    16 => p.flags as i16 as i32,
                    17 => ((p.skill as i32) << 8) | (p.might as u8 as i32),
                    _ => -1,
                }
            }
            0x13 => {
                let c = &self.characters[sp(0) as usize & 3];
                let d = sp(2) as usize;
                match sp(1) {
                    0 => c.flags as i16 as i32,
                    1 => c.race_class_sex as i32,
                    5 => c.hit_points_cur as i32,
                    6 => c.hit_points_max as i32,
                    7 => c.magic_points_cur as i32,
                    8 => c.magic_points_max as i32,
                    9 => c.item_protection as i32,
                    10 => c.items[d % 11] as i32,
                    11 => c.skill_levels[d % 3] as i32 + c.skill_modifiers[d % 3] as i32,
                    12 => c.protection_against_items[d % 8] as i32,
                    13 => {
                        if d & 0x80 != 0 {
                            c.items_might[7] as i32
                        } else {
                            c.items_might[d % 8] as i32
                        }
                    }
                    14 => c.skill_modifiers[d % 3] as i32,
                    15 => c.id as i32,
                    _ => 0,
                }
            }
            0x14 => {
                let ci = sp(0) as usize & 3;
                let d = sp(2) as usize;
                let e = sp(3);
                match sp(1) {
                    0 => self.characters[ci].flags = e as u16,
                    1 => self.characters[ci].race_class_sex = (e & 0x0F) as u8,
                    5 => self.set_character_magic_or_hit_points(ci, 0, e, 0),
                    6 => self.characters[ci].hit_points_max = e as u16,
                    7 => self.set_character_magic_or_hit_points(ci, 1, e, 0),
                    8 => self.characters[ci].magic_points_max = e as u16,
                    9 => self.characters[ci].item_protection = e as u16,
                    10 => self.characters[ci].items[d % 11] = 0,
                    11 => self.characters[ci].skill_levels[d % 3] = e as u8,
                    12 => self.characters[ci].protection_against_items[d % 8] = e as u16,
                    13 => {
                        if d & 0x80 != 0 {
                            self.characters[ci].items_might[7] = e as u16;
                        } else {
                            self.characters[ci].items_might[d % 8] = e as u16;
                        }
                    }
                    14 => self.characters[ci].skill_modifiers[d % 3] = e as i8,
                    _ => {}
                }
                0
            }
            0x15 => {
                let (a0, a1) = (s(0), s(1));
                self.load_level_shp_dat(&a0, &a1, true);
                1
            }
            0x16 => {
                self.lvl_shp_file = None;
                1
            }
            0x18 => {
                if let Some(d) = self.res.file_data(&s(0)) {
                    self.screen.load_bitmap(&d, 3, None);
                    let p = self.screen.page(2).to_vec();
                    self.door_shapes[0] = shape::shape_copy(&p, sp(1).max(0) as usize);
                    self.door_shapes[1] = shape::shape_copy(&p, sp(2).max(0) as usize);
                }
                for i in 0..20 {
                    self.wll_wall_flags[i + 3] |= 7;
                    let t = i % 5;
                    if t == 4 {
                        self.wll_wall_flags[i + 3] &= 0xF8;
                    }
                    if t == 3 {
                        self.wll_wall_flags[i + 3] &= 0xFD;
                    }
                }
                if sp(3) != 0 {
                    for i in 3..13 {
                        self.wll_wall_flags[i] &= 0xFD;
                    }
                }
                if sp(4) != 0 {
                    for i in 13..23 {
                        self.wll_wall_flags[i] &= 0xFD;
                    }
                }
                1
            }
            0x19 => {
                let f = s(0);
                if self.tim_init_anim_struct(
                    sp(1) as usize % 6,
                    &f,
                    sp(2),
                    sp(3),
                    sp(4),
                    0,
                    sp(5) as u16,
                ) != 0
                {
                    1
                } else {
                    0
                }
            }
            0x1A => {
                self.anim_play_part(sp(0) as usize, sp(1), sp(2), sp(3));
                1
            }
            0x1B => self.tim_free_anim_struct(sp(0) as usize % 6),
            0x1C => self.current_direction as i32,
            0x1D => {
                for i in 0..4 {
                    if self.characters[i].flags & 1 == 0 || self.characters[i].id >= 0 {
                        continue;
                    }
                    let s = -(self.characters[i].id as i32);
                    let sfx = match s {
                        1 => 136,
                        5 => 50,
                        8 => 49,
                        9 => 48,
                        _ => 0,
                    };
                    if sfx != 0 {
                        self.snd_play_sound_effect(sfx, -1);
                    }
                    return Some(1);
                }
                1
            }
            0x1E => {
                self.cur_music_theme = sp(0);
                1
            }
            0x1F => {
                self.set_sequence_buttons(sp(0), sp(1), sp(2), sp(3), sp(4));
                1
            }
            0x20 => {
                self.set_default_button_state();
                1
            }
            0x21 => {
                Self::pos_within_rect(self.mouse_x, self.mouse_y, sp(0), sp(1), sp(2), sp(3)) as i32
            }
            0x22 => {
                if self.current_control_mode != 0 && !self.text_enabled() {
                    return Some(1);
                }
                self.screen.set_screen_dim(5);
                // (dimension 5 is in pixels: 85, 123, 233, 54)
                let d = self.screen.dim(5);
                self.screen.fill_rect(
                    d.sx,
                    d.sy,
                    d.sx + d.w - 2,
                    d.sy + d.h - 2,
                    d.col2 as u8,
                    None,
                );
                self.txt_clear_dim(4);
                self.txt_reset_dim_text_positions(4);
                1
            }
            0x23 => {
                self.anim_setup_part(
                    sp(0) as usize,
                    sp(1) as usize,
                    sp(2),
                    sp(3),
                    sp(4),
                    sp(5),
                    sp(6),
                    sp(7),
                    sp(8),
                    sp(9),
                );
                0
            }
            0x24 => {
                self.anim_start(sp(0) as usize, sp(1) as usize);
                1
            }
            0x25 => {
                self.screen.hide_mouse();
                1
            }
            0x26 => {
                self.screen.show_mouse();
                1
            }
            0x27 => {
                self.screen.fade_to_black(10);
                1
            }
            0x28 => {
                let p3 = self.screen.palette(3).clone();
                self.screen.fade_palette(&p3, 10);
                self.screen.fade_flag = 0;
                1
            }
            0x29 => {
                if let Some(d) = self.res.file_data(&s(0)) {
                    let mut p3 = self.screen.palette(3).clone();
                    self.screen.load_bitmap(&d, 3, Some(&mut p3));
                    *self.screen.palette_mut(3) = p3;
                }
                if sp(1) != 2 {
                    self.screen.copy_page(3, sp(1) as usize);
                }
                1
            }
            0x2A => {
                self.anim_stop(sp(0) as usize);
                1
            }
            0x2D => self.global_script_vars[sp(0) as usize % 24] as i32,
            0x2E => {
                self.global_script_vars[sp(0) as usize % 24] = sp(1) as i16;
                1
            }
            0x2F => match sp(0) {
                0 => self.current_block as i32,
                1 => self.current_direction as i32,
                2 => self.current_level,
                3 => self.item_in_hand as i32,
                4 => self.brightness,
                5 => self.credits as i32,
                6 => self.global_script_vars2[sp(1) as usize % 8] as i32,
                8 => self.update_flags,
                9 => self.lamp_oil_status,
                10 => self.scene_default_update,
                11 => self.compass_broken as i32,
                12 => self.drain_magic as i32,
                13 => self.speech_volume_level() - 2,
                14 => self.tim.abort_flag,
                _ => 0,
            },
            0x30 => {
                let a2 = sp(1) as u16;
                let b = sp(2) as u16;
                match sp(0) {
                    0 => {
                        self.current_block = b;
                        let (x, y) = self.calc_coordinates(b, 0x80, 0x80);
                        self.party_pos_x = x;
                        self.party_pos_y = y;
                        self.update_auto_map(b);
                    }
                    1 => self.current_direction = b,
                    2 => self.current_level = (b & 0xFF) as i32,
                    3 => self.set_hand_item(b),
                    4 => self.brightness = (b & 0xFF) as i32,
                    5 => self.credits = b,
                    6 => self.global_script_vars2[a2 as usize % 8] = b,
                    8 => {
                        self.update_flags = b as i32;
                        if b == 1 {
                            if !self.text_enabled() || self.current_control_mode & 2 == 0 {
                                self.timer_update_portrait_animations(1);
                            }
                            self.disable_sys_timer(2);
                        } else {
                            self.enable_sys_timer(2);
                        }
                    }
                    9 => self.lamp_oil_status = (b & 0xFF) as i32,
                    10 => {
                        self.scene_default_update = (b & 0xFF) as i32;
                        self.gui_toggle_button_display_mode(0, 0);
                    }
                    11 => self.compass_broken = (a2 & 0xFF) as u8,
                    12 => self.drain_magic = (a2 & 0xFF) as u8,
                    _ => {}
                }
                1
            }
            0x31 => {
                self.process_door_switch(sp(0) as u16, sp(1));
                1
            }
            0x32 => {
                for i in 0..4 {
                    if self.characters[i].flags & 1 == 0 {
                        continue;
                    }
                    for ii in 0..4 {
                        let it = self.characters[i].items[ii];
                        let pi = self.items_in_play[it as usize].item_property_index as usize;
                        let f = self
                            .item_properties
                            .get(pi)
                            .map_or(0xFF, |p| p.item_script_func);
                        if f == 0 || f == 2 {
                            return Some(1);
                        }
                    }
                }
                0
            }
            0x33 => {
                let b = sp(0) as usize & 0x3FF;
                if sp(1) != 0 {
                    self.level_block_properties[b].flags =
                        (self.level_block_properties[b].flags & 0xEF) | 0x20;
                } else {
                    self.level_block_properties[b].flags &= 0xDF;
                }
                1
            }
            0x34 => {
                let block = sp(0);
                let wall = sp(1);
                let cur = self.level_block_properties[block as usize & 0x3FF].walls
                    [if wall == -1 { 0 } else { wall as usize & 3 }]
                    as i32;
                let v = if cur == sp(2) { sp(3) } else { sp(2) };
                self.set_wall_type(block, wall, v);
                0
            }
            0x35 => self.assign_level_decoration_shapes(sp(0).max(0) as usize) as i32,
            0x36 => {
                let v = (sp(0) & 0xFF) as i8;
                for i in 3..8 {
                    self.wll_shape_map[i] = v;
                }
                for i in 13..18 {
                    self.wll_shape_map[i] = v;
                }
                1
            }
            0x37 => {
                self.screen.copy_region(
                    sp(0),
                    sp(1),
                    sp(2),
                    sp(3),
                    sp(4),
                    sp(5),
                    sp(6) as usize,
                    sp(7) as usize,
                    CR_NO_P_CHECK,
                );
                if sp(7) == 0 {
                    self.screen.update_screen();
                }
                1
            }
            0x38 => {
                let (x, y) = self.calc_coordinates(sp(0) as u16, sp(1) as u16, sp(2) as u16);
                let t = sp(4).clamp(0, 4) as usize;
                let w = self.monster_properties[t].max_width as u16;
                if self.check_block_before_object_placement(x, y, w, 7, 7) != 0 {
                    return Some(-1);
                }
                for i in 0..30 {
                    let l = self.monsters[i];
                    if l.hit_points != 0 || l.mode == 13 {
                        continue;
                    }
                    let mut m = Monster {
                        id: i as u8,
                        ..Monster::default()
                    };
                    m.obj.x = x;
                    m.obj.y = y;
                    m.facing = sp(3) as u8;
                    m.type_ = sp(4) as u8;
                    m.properties = t;
                    m.direction = m.facing << 1;
                    let p = self.monster_properties[t];
                    m.hit_points = ((p.hit_points as i32
                        * MONSTER_MODIFIERS1[self.monster_difficulty.clamp(0, 2) as usize] as i32)
                        >> 8) as i16;
                    self.monsters[i] = m;
                    if self.current_level != 12 || m.type_ != 2 {
                        let r = self.roll_dice(1, 128, 0) + 192;
                        self.monsters[i].hit_points =
                            ((self.monsters[i].hit_points as i32 * r) >> 8) as i16;
                    }
                    self.monsters[i].num_dist_attacks = p.num_dist_attacks as u8;
                    let lvl = self.calc_monster_skill_level(i as i32 | 0x8000, 8);
                    self.monsters[i].dist_attack_tick = (self.roll_dice(1, lvl, 0) - 1) as i8;
                    self.monsters[i].obj.flying_height = 2;
                    self.monsters[i].flags = sp(5) as u16;
                    self.monsters[i].assigned_items = 0;
                    self.set_monster_mode(i, sp(6));
                    self.place_monster(i, x, y);
                    let mo = &mut self.monsters[i];
                    mo.dest_x = mo.obj.x;
                    mo.dest_y = mo.obj.y;
                    mo.dest_direction = mo.direction;
                    for ii in 0..4 {
                        mo.equipment_shapes[ii] = (stack[7 + ii] & 0xFF) as u8;
                    }
                    let b = mo.obj.block;
                    self.check_scene_update_need(b as i32);
                    return Some(i as i32);
                }
                -1
            }
            0x39 => {
                self.screen.fade_clear_scene_window(10);
                1
            }
            0x3A => {
                let p0 = self.screen.palette(0).clone();
                let mut p3 = self.screen.palette(3).clone();
                p3.copy_from(&p0, 128, 128, 128);
                self.screen.load_special_colors(&mut p3);
                *self.screen.palette_mut(3) = p3.clone();
                self.screen.fade_palette(&p3, 10);
                self.screen.fade_flag = 0;
                1
            }
            0x3B => {
                if self.screen.fade_flag != 2 {
                    self.screen.fade_clear_scene_window(10);
                }
                self.gui_draw_play_field();
                let p0 = self.screen.palette(0).clone();
                let (br, le) = (self.brightness, self.lamp_effect as i32);
                self.set_palette_brightness(&p0, br, le);
                self.screen.fade_flag = 0;
                1
            }
            0x3C => {
                self.screen.fade_clear_scene_window(10);
                self.screen.fill_rect(112, 0, 288, 120, 0, None);
                self.disable_sys_timer(2);
                for i in 0..8 {
                    let f = self.flying_objects[i];
                    if f.enable == 0 || f.object_type != 0 {
                        continue;
                    }
                    self.end_object_flight(i, f.x as i32, f.y as i32, 1);
                }
                self.complete_door_operations();
                self.generate_temp_data();
                self.current_block = sp(1) as u16;
                self.current_direction = sp(2) as u16;
                let (x, y) = self.calc_coordinates(self.current_block, 0x80, 0x80);
                self.party_pos_x = x;
                self.party_pos_y = y;
                self.load_level(sp(0));
                self.enable_sys_timer(2);
                self.emc_stop = true;
                1
            }
            0x3D => self.get_nearest_monster_from_character(sp(0) as usize & 3) as i16 as i32,
            0x3E | 0xBF => 0,
            0x3F => {
                let li = sp(0).clamp(0, 4) as usize;
                let shape_index = (sp(1) & 0xFF) as u8;
                let max_w = (0..16)
                    .filter_map(|i| {
                        self.monster_shapes
                            .get(((shape_index as usize) << 4) + i)
                            .cloned()
                            .flatten()
                    })
                    .map(|s| s.get(3).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                let l = &mut self.monster_properties[li];
                l.shape_index = shape_index;
                l.max_width = max_w;
                l.fighting_stats[0] = ((sp(2) << 8) / 100) as u16;
                l.fighting_stats[1] = 256;
                l.fighting_stats[2] = ((sp(3) << 8) / 100) as u16;
                l.fighting_stats[3] = sp(4) as u16;
                l.fighting_stats[4] = ((sp(5) << 8) / 100) as u16;
                l.fighting_stats[5] = ((sp(6) << 8) / 100) as u16;
                l.fighting_stats[6] = ((sp(7) << 8) / 100) as u16;
                l.fighting_stats[7] = ((sp(8) << 8) / 100) as u16;
                l.fighting_stats[8] = 0;
                for i in 0..8 {
                    l.items_might[i] = sp(9 + i) as u16;
                    l.protection_against_items[i] = ((sp(17 + i) << 8) / 100) as u16;
                }
                l.item_protection = sp(25) as u16;
                l.hit_points = sp(26) as u16;
                l.speed_total_wait_ticks = 1;
                l.flags = sp(27) as u16;
                l.unk5 = sp(29) as u16;
                l.num_dist_attacks = sp(30) as u16;
                l.num_dist_weapons = sp(31) as u16;
                for i in 0..3 {
                    l.dist_weapons[i] = sp(32 + i) as u16;
                }
                l.attack_skill_chance = sp(35) as u8;
                l.attack_skill_type = sp(36) as u8;
                l.defense_skill_chance = sp(37) as u8;
                l.defense_skill_type = sp(38) as u8;
                for i in 0..3 {
                    l.sounds[i] = sp(39 + i) as u8;
                }
                1
            }
            0x40 => self.battle_hit_skill_test(sp(0) as i16, sp(1) as i16, sp(2) as usize % 3),
            0x41 => {
                if sp(0) == -1 {
                    for i in 0..4 {
                        self.inflict_damage(i, sp(1), sp(2), sp(3), sp(4));
                    }
                } else {
                    self.inflict_damage(sp(0), sp(1), sp(2), sp(3), sp(4));
                }
                1
            }
            0x44 => {
                let m = sp(0) as usize % 30;
                if self.monsters[m].mode == 1 || self.monsters[m].mode == 2 {
                    let (dx, dy) = self.calc_coordinates(sp(1) as u16, sp(2) as u16, sp(3) as u16);
                    let mo = &mut self.monsters[m];
                    mo.dest_x = dx;
                    mo.dest_y = dy;
                    mo.dest_direction = (sp(4) << 1) as u8;
                    if mo.obj.x != dx || mo.obj.y != dy {
                        let (x, y) = (mo.obj.x, mo.obj.y);
                        let d = self.calc_monster_direction(x, y, dx, dy);
                        self.set_monster_direction(m, d);
                    }
                }
                1
            }
            0x45 => {
                let (a1, a2, a3) = (
                    self.get_lang_string(sp(1) as u16),
                    self.get_lang_string(sp(2) as u16),
                    self.get_lang_string(sp(3) as u16),
                );
                self.setup_dialogue_buttons(sp(0), a1, a2, a3);
                1
            }
            0x46 => {
                let c = sp(0);
                if c >= 0 {
                    self.give_credits(c, true);
                } else {
                    self.take_credits(-c, true);
                }
                1
            }
            0x47 => (sp(0) <= self.credits as i32) as i32,
            0x48 => {
                let id = 0x50 + sp(0) as u8;
                if sp(1) != 0 {
                    self.timers.enable(id);
                    let now = self.millis();
                    self.timers.set_countdown(id, sp(1), now, TICK);
                } else {
                    self.timers.disable(id);
                }
                1
            }
            0x49 => {
                if self.item_in_hand != 0 {
                    return Some(0);
                }
                let it = self.make_item(sp(0), sp(1), sp(2));
                self.set_hand_item(it);
                1
            }
            0x4A => {
                const SOUNDS: [i32; 3] = [12, 62, 63];
                let d = sp(0);
                if !(70..=74).contains(&d)
                    && !(81..=89).contains(&d)
                    && !(93..=97).contains(&d)
                    && !(102..=106).contains(&d)
                {
                    let skill = self
                        .item_properties
                        .get(d.max(0) as usize)
                        .map_or(0, |p| p.skill);
                    self.snd_play_sound_effect(SOUNDS[(skill & 3).min(2) as usize], -1);
                } else {
                    self.snd_play_sound_effect(12, -1);
                }
                1
            }
            0x4B => {
                let mut id = sp(0) as i16;
                if id < 0 {
                    id = -id;
                    for i in 0..4 {
                        if self.characters[i].flags & 1 == 0 || self.characters[i].id != id {
                            continue;
                        }
                        self.characters[i].flags &= !1;
                        self.calc_char_portrait_xpos();
                        if self.selected_character == i as i32 {
                            self.selected_character = 0;
                        }
                        break;
                    }
                } else {
                    self.add_character(id as i32);
                }
                if self.update_flags == 0 {
                    self.gui_enable_default_playfield_buttons();
                    self.gui_draw_play_field();
                }
                1
            }
            0x4C => {
                let item = self.make_item(sp(0), sp(1), sp(2));
                if self.add_item_to_inventory(item) {
                    1
                } else {
                    self.delete_item(item);
                    0
                }
            }
            0x4E => {
                let slot = self.tim_slot(sp(0));
                if self.active_tim[slot].is_none() {
                    let file = format!("{}.TIM", s(1));
                    self.active_tim[slot] = self.tim_load(&file, TimOpcodes::Ingame);
                }
                1
            }
            0x4F => {
                let slot = self.tim_slot(sp(0));
                self.tim_exec(TimRef::Active(slot), sp(1) != 0)
            }
            0x50 => {
                let slot = self.tim_slot(sp(0));
                self.active_tim[slot] = None;
                1
            }
            0x51 => {
                self.init_scene_window_dialogue(sp(0));
                1
            }
            0x52 => {
                self.restore_after_scene_window_dialogue(sp(0) != 0);
                1
            }
            0x53 => self.item_in_hand as i32,
            0x54 => self.check_magic(sp(0), sp(1), sp(2)),
            0x55 => {
                if sp(0) == -1 {
                    return Some(0);
                }
                self.give_item_to_monster(sp(0) as usize % 30, sp(1) as u16);
                1
            }
            0x56 => {
                let f = format!("{}.{}", s(0), self.language_ext());
                self.level_lang_file = self.res.file_data(&f).unwrap_or_default();
                1
            }
            0x57 => {
                self.snd_play_sound_effect(sp(0), -1);
                1
            }
            0x58 => self.process_dialogue(),
            0x59 => {
                let slot = self.tim_slot(sp(0));
                self.tim_stop_all_funcs(TimRef::Active(slot));
                1
            }
            0x5A => {
                self.wll_wall_flags[self.level_block_properties[sp(0) as usize & 0x3FF].walls
                    [sp(1) as usize & 3] as usize] as i32
            }
            0x5B => {
                if sp(0) == -1 {
                    return Some(1);
                }
                let m = (sp(0) & 0x7FFF) as usize % 30;
                let d = sp(2) as i16;
                match sp(1) {
                    0 => self.set_monster_mode(m, d as i32),
                    1 => self.monsters[m].hit_points = d,
                    2 => {
                        let (mx, my) =
                            (self.monsters[m].obj.x & 0xFF, self.monsters[m].obj.y & 0xFF);
                        let (x, y) = self.calc_coordinates(d as u16, mx, my);
                        if self.walk_monster_check_dest(x as i32, y as i32, m, 7) == 0 {
                            self.place_monster(m, x, y);
                        }
                    }
                    3 => self.set_monster_direction(m, (d as i32) << 1),
                    6 => self.monsters[m].flags |= d as u16,
                    _ => {}
                }
                1
            }
            0x5C => {
                if sp(0) == -1 {
                    return Some(0);
                }
                let m = self.monsters[(sp(0) & 0x7FFF) as usize % 30];
                let p = self.monster_properties[m.properties.min(4)];
                match sp(1) {
                    0 => m.mode as i32,
                    1 => m.hit_points as i32,
                    2 => m.obj.block as i32,
                    3 => m.facing as i32,
                    4 => m.type_ as i32,
                    5 => p.hit_points as i32,
                    6 => m.flags as i16 as i32,
                    7 => p.flags as i16 as i32,
                    8 => self
                        .monster_anim_type
                        .get(p.shape_index as usize)
                        .copied()
                        .unwrap_or(0) as i32,
                    _ => 0,
                }
            }
            0x5D => {
                for i in 0..3 {
                    self.release_monster_shapes(i);
                }
                0
            }
            0x5E => {
                self.snd_stop_speech(true);
                self.stop_portrait_speech_anim();
                let st = self.get_lang_string(sp(2) as u16);
                self.play_character_script_chat(sp(0), sp(1), 1, &st, &stack, 3)
            }
            0x5F => {
                self.update();
                1
            }
            0x60 => {
                let block = if sp(1) == -1 {
                    self.current_block as i32
                } else {
                    sp(1)
                };
                self.snd_process_environmental_sound_effect(sp(0), block);
                1
            }
            0x61 => {
                if sp(3) != 0 {
                    self.process_magic_heal(sp(0), sp(1));
                } else {
                    self.increase_character_hitpoints(sp(0) as usize & 3, sp(1), true);
                    if sp(2) != 0 {
                        self.gui_draw_char_portrait_with_stats(sp(0) as usize & 3);
                    }
                }
                1
            }
            0x62 => {
                const PARA: [i32; 6] = [0x90, 0x78, 0x0C, 0x9F, 0x80, 0x1E];
                let cp = self.screen.set_cur_page(0);
                let cf = self.screen.set_font(FontId::F6);
                let k = (sp(0) as usize).min(1);
                let x = PARA[3 * k] << 1;
                let y = PARA[3 * k + 1];
                let offs = PARA[3 * k + 2];
                let st = self.get_lang_string(0x4033);
                let w = self.screen.text_width(&st);
                let hb = self.screen.font_height() + 3;
                self.gui_draw_box(x - offs - w, y - hb, w + offs, hb, 136, 251, 252);
                self.screen
                    .print_text(&st, x - (offs >> 1) - w, y - hb + 2, 144, 0);
                if sp(1) != 0 {
                    self.screen.draw_grid_box(
                        x - offs - w + 1,
                        y - hb + 1,
                        w + offs - 2,
                        hb - 2,
                        1,
                    );
                }
                self.screen.set_font(cf);
                self.screen.set_cur_page(cp);
                1
            }
            0x63 => {
                self.snd_load_sound_file(sp(0));
                1
            }
            0x64 => self.snd_play_track(sp(0)),
            0x65 => {
                self.delete_monsters_from_block(sp(0) as usize & 0x3FF);
                1
            }
            0x66 => {
                let mut o = self.level_block_properties[sp(0) as usize & 0x3FF].assigned_objects;
                let mut res = 0;
                let mut guard = 0;
                while o != 0 && guard < 1000 {
                    guard += 1;
                    if o & 0x8000 == 0 {
                        res += 1;
                    }
                    o = self.obj(o).next_assigned_object;
                }
                res
            }
            0x67 => {
                let skill = sp(0) as usize % 3;
                let n = self.count_active_characters();
                let (mut m, mut c) = (0, 0);
                for i in 0..n {
                    let v = self.characters[i].skill_modifiers[skill] as i32
                        + self.characters[i].skill_levels[skill] as i32
                        + 25;
                    if v > m {
                        m = v;
                        c = i as i32;
                    }
                }
                if self.roll_dice(1, 100, 0) > m { -1 } else { c }
            }
            0x68 => self
                .monsters
                .iter()
                .filter(|m| m.hit_points > 0 && m.mode != 13)
                .count() as i32,
            0x69 => {
                let c = match self.characters[0].id {
                    -9 => 1,
                    -5 => 3,
                    -1 => 2,
                    _ => 0,
                };
                while self.snd_update_character_speech() != 0 && !self.should_quit() {
                    self.delay(TICK, false);
                }
                self.event_list.clear();
                self.screen.hide_mouse();
                *self.screen.palette_mut(1) = Pal::default();
                let hard = self.monster_difficulty == 2;
                self.show_outro(c, hard);
                if !self.should_quit() {
                    self.quit_game();
                }
                0
            }
            0x6A => {
                self.snd_stop_speech(true);
                self.stop_portrait_speech_anim();
                1
            }
            0x6B => {
                let old = self.brightness;
                self.brightness = sp(0);
                if sp(1) == 1 {
                    let p0 = self.screen.palette(0).clone();
                    let le = self.lamp_effect as i32;
                    self.set_palette_brightness(&p0, sp(0), le);
                }
                old
            }
            0x6C => self.calc_inflictable_damage(sp(0) as i16, sp(1) as i16, sp(2)),
            0x6D => self.roll_dice(2, sp(0), 0),
            0x6E => {
                for i in 0..4 {
                    if self.characters[i].flags & 9 != 0 && self.characters[i].id as i32 == sp(0) {
                        return Some(1);
                    }
                }
                0
            }
            0x6F => {
                let snd = safe(2);
                let f = self.get_lang_string(safe(1) as u16);
                let args: Vec<Arg> = (3..10).map(|i| Arg::Int(safe(i))).collect();
                self.print_message(safe(0) as u16, &f, &args);
                if snd >= 0 {
                    self.snd_play_sound_effect(snd, -1);
                }
                1
            }
            0x70 => {
                let it = sp(0) as usize % 400;
                let b = self.items_in_play[it].obj.block;
                if b != 0 {
                    self.remove_level_item(it as u16, b as i32);
                }
                self.delete_item(it as u16);
                1
            }
            0x71 => self.calc_inflictable_damage_per_item(sp(0), sp(1), sp(2), sp(3), sp(4)),
            0x72 => {
                let (mut fx, mut fy) = (sp(3) as u16, sp(4) as u16);
                if sp(8) & 0x8000 == 0 {
                    fx = 0x80;
                    fy = 0x80;
                }
                let (x, y) = self.calc_coordinates(sp(2) as u16, fx, fy);
                if self.launch_object(
                    sp(0),
                    sp(1) as u16,
                    x as i32,
                    y as i32,
                    sp(5),
                    sp(6) << 1,
                    sp(7),
                    sp(8),
                    0x3F,
                ) {
                    1
                } else {
                    self.delete_item(sp(1) as u16);
                    0
                }
            }
            0x73 => {
                self.remove_character_effects(sp(0) as usize & 3, sp(1), sp(2));
                1
            }
            0x74 => self.inventory.iter().all(|&i| i == 0) as i32,
            0x75 => {
                let mut o = self.level_block_properties[sp(0) as usize & 0x3FF].assigned_objects;
                let mut res = 0;
                let mut level = sp(2);
                let mut dest = sp(1);
                let run_script = sp(4);
                let monsters = sp(3);
                let items = sp(5);
                if self.current_level == 21 && level == 21 && dest == 0x3E0 {
                    level = 20;
                    dest = 0x0247;
                }
                let mut guard = 0;
                while o != 0 && guard < 1000 {
                    guard += 1;
                    let l = o;
                    o = self.obj(o).next_assigned_object;
                    if l & 0x8000 != 0 {
                        if monsters == 0 {
                            continue;
                        }
                        let m = (l & 0x7FFF) as usize;
                        self.set_monster_mode(m, 14);
                        let b = self.monsters[m].obj.block;
                        self.check_scene_update_need(b as i32);
                        self.place_monster(m, 0, 0);
                        res = 1;
                    } else {
                        let it = self.items_in_play[l as usize];
                        if it.shp_cur_frame_flg & 0x4000 == 0 || items == 0 {
                            continue;
                        }
                        self.place_move_level_item(
                            l,
                            level,
                            dest,
                            (it.obj.x & 0xFF) as i32,
                            (it.obj.y & 0xFF) as i32,
                            it.obj.flying_height as i32,
                        );
                        res = 1;
                        if run_script == 0 || level != self.current_level {
                            continue;
                        }
                        self.run_level_script_custom(dest, 0x80, -1, l as i32, 0, 0);
                    }
                }
                res
            }
            0x78 => {
                self.add_spell_to_scroll(sp(0), sp(1));
                1
            }
            0x79 => {
                let st = self.get_lang_string(sp(0) as u16);
                self.txt_print_dialogue_text2(3, &st, &stack, 1);
                1
            }
            0x7A => {
                let track = sp(0);
                if !self.snd_play_character_speech(track, 0, false) || self.text_enabled() {
                    let st = self.get_lang_string(track as u16);
                    self.txt_print_dialogue_text2(4, &st, &stack, 1);
                }
                1
            }
            0x7B => {
                for i in 0..30 {
                    if sp(0) != self.monsters[i].type_ as i32 && sp(0) != -1 {
                        continue;
                    }
                    return Some(if self.monsters[i].mode == 1 { 0 } else { 1 });
                }
                1
            }
            0x7C => {
                self.next_script_func = sp(0);
                1
            }
            0x7D | 0xB2 => 1,
            0x7F => {
                let m = (sp(0) & 0x7FFF) as usize % 30;
                self.set_monster_mode(m, 14);
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
                self.place_monster(m, 0, 0);
                1
            }
            0x80 => {
                self.txt.script_text_parameter = sp(0);
                1
            }
            0x81 => {
                self.gui_notify_button_list_changed();
                self.snd_update_character_speech();
                let f = self.check_input(None, true, 0);
                self.remove_input_top();
                if f == 0 || f & 0x800 != 0 {
                    return Some(0);
                }
                let evt = sp(0);
                if evt != 0 {
                    self.gui_trigger_event(evt);
                    self.seq_trigger = 1;
                } else {
                    self.remove_input_top();
                }
                1
            }
            0x82 => {
                let dim = sp(0);
                let flg = sp(1);
                self.screen.set_screen_dim(dim as usize);
                if flg & 1 != 0 {
                    self.txt_clear_cur_dim();
                }
                if flg & 3 != 0 {
                    self.txt_reset_dim_text_positions(dim);
                }
                let st = self.get_lang_string(sp(2) as u16);
                self.txt_print_dialogue_text2(dim, &st, &stack, 3);
                1
            }
            0x83 => {
                let mut types: u32 = 0;
                let mut cnt = 0;
                while cnt < 30 && sp(cnt) != -1 {
                    types |= 1 << (sp(cnt) & 31);
                    cnt += 1;
                }
                self.monsters
                    .iter()
                    .filter(|m| (1u32 << (m.type_ & 31)) & types != 0 && m.mode < 14)
                    .count() as i32
            }
            0x84 => {
                let num_frames = sp(3).max(1);
                let cur = sp(2) % num_frames;
                self.set_wall_type(sp(0), sp(1), sp(4 + cur.max(0) as usize));
                0
            }
            0x85 => {
                let p = sp(1) as u16;
                if sp(2) == 0 {
                    for i in 0..48 {
                        let it = self.inventory[i];
                        if it != 0 && self.items_in_play[it as usize].item_property_index == p {
                            return Some(1);
                        }
                    }
                    if self.items_in_play[self.item_in_hand as usize].item_property_index == p {
                        return Some(1);
                    }
                }
                let (first, last) = if sp(0) == -1 { (0, 3) } else { (sp(0), sp(0)) };
                for i in first..=last {
                    if self.item_equipped(i as usize, p) {
                        return Some(1);
                    }
                }
                0
            }
            0x86 => {
                self.block_door = sp(0);
                self.block_door
            }
            0x87 => {
                let slot = self.tim_slot(sp(0));
                self.tim_reset_dialogue_state(TimRef::Active(slot));
                1
            }
            0x88 => {
                let mut px = sp(1);
                if px != -1 {
                    px &= 0xFF;
                }
                let mut py = sp(2);
                if py != -1 {
                    py &= 0xFF;
                }
                let mut o = if sp(3) != 0 || self.emc_last_item == -1 {
                    sp(0)
                } else {
                    self.emc_last_item
                };
                self.emc_last_item =
                    self.level_block_properties[o as usize & 0x3FF].assigned_objects as i32;
                let mut guard = 0;
                while self.emc_last_item != 0 && guard < 1000 {
                    guard += 1;
                    let e = self.emc_last_item as u16;
                    if e & 0x8000 != 0 {
                        o = (e & 0x7FFF) as i32;
                        self.emc_last_item =
                            self.level_block_properties[o as usize & 0x3FF].assigned_objects as i32;
                        continue;
                    }
                    let it = self.items_in_play[e as usize];
                    if (px != -1 && (it.obj.x & 0xFF) as i32 != px)
                        || (py != -1 && (it.obj.y & 0xFF) as i32 != py)
                    {
                        o = (e & 0x7FFF) as i32;
                        self.emc_last_item =
                            self.level_block_properties[o as usize & 0x3FF].assigned_objects as i32;
                        continue;
                    }
                    return Some(self.emc_last_item);
                }
                0
            }
            0x89 => {
                self.remove_level_item(sp(0) as u16, sp(1));
                1
            }
            0x8A => 1,
            0x8B => {
                for i in 0..6 {
                    self.tim_free_anim_struct(i);
                }
                1
            }
            0x8C => {
                self.init_dialogue_sequence(sp(0), sp(1) as usize);
                1
            }
            0x8D => {
                self.restore_after_dialogue_sequence(sp(0));
                1
            }
            0x8E => {
                self.set_special_scene_buttons(sp(0), sp(1), sp(2), sp(3), sp(4));
                1
            }
            0x8F => {
                self.gui_special_scene_restore_buttons();
                1
            }
            0x92 => {
                self.prepare_special_scene(sp(0), sp(1), sp(2), sp(3), sp(4), sp(5));
                1
            }
            0x93 => self.restore_after_special_scene(sp(0), sp(1), sp(2), sp(3)),
            0x94 => {
                let c = s(0);
                let i = sp(1);
                if c.is_empty() || i > 250 {
                    return Some(0);
                }
                let t = INGAME_SFX_INDEX[(i.max(0) as usize) << 1];
                if t == 0xFFFF {
                    return Some(0);
                }
                if let Some(e) = self.ingame_sound_list.get_mut(t as usize) {
                    *e = c;
                }
                0
            }
            0x96 => {
                let mut o = if sp(1) == -1 {
                    self.level_block_properties[sp(0) as usize & 0x3FF].assigned_objects
                } else {
                    self.obj(sp(1) as u16).next_assigned_object
                };
                let mut guard = 0;
                while o != 0 && guard < 1000 {
                    guard += 1;
                    if o & 0x8000 != 0 {
                        return Some((o & 0x7FFF) as i32);
                    }
                    o = self.obj(o).next_assigned_object;
                }
                -1
            }
            0x97 => {
                let block = sp(0) as u16;
                let id = (sp(1) as u16) | 0x8000;
                let mut o = self.level_block_properties[block as usize & 0x3FF].assigned_objects;
                let mut guard = 0;
                while o & 0x8000 != 0 && guard < 100 {
                    guard += 1;
                    if id == 0xFFFF || id == o {
                        return Some((o & 0x7FFF) as i32);
                    }
                    o = self.obj(o).next_assigned_object;
                }
                -1
            }
            0x98 => {
                self.screen.cross_fade_region(
                    sp(0),
                    sp(1),
                    sp(2),
                    sp(3),
                    sp(4),
                    sp(5),
                    sp(6) as usize,
                    sp(7) as usize,
                    &mut self.rnd,
                );
                1
            }
            0x99 => {
                let (x, y) =
                    Self::calc_coordinates_add_direction_offset(sp(0) as u16, sp(1) as u16, sp(2));
                if sp(3) != 0 { x as i32 } else { y as i32 }
            }
            0x9A => {
                self.reset_portraits_and_disable_sys_timer();
                1
            }
            0x9B => {
                self.need_scene_restore = 0;
                self.enable_sys_timer(2);
                1
            }
            0x9C => self.need_scene_restore,
            0x9D => {
                if sp(0) != 0 {
                    self.script_character_cycle = 0;
                } else {
                    self.script_character_cycle += 1;
                }
                while self.script_character_cycle < 4 {
                    if self.characters[self.script_character_cycle as usize].flags & 1 != 0 {
                        return Some(self.script_character_cycle);
                    }
                    self.script_character_cycle += 1;
                }
                -1
            }
            0x9E => self.paralyze_poison_character(
                sp(0) as usize & 3,
                sp(1) as u16,
                sp(2) as u16,
                sp(3),
                sp(4) != 0,
            ) as i32,
            0x9F => {
                if sp(0) == -1 {
                    self.gui_draw_all_char_portraits_with_stats();
                } else {
                    self.gui_draw_char_portrait_with_stats(sp(0) as usize & 3);
                }
                1
            }
            0xA0 => {
                let t = sp(0) as u16;
                for i in 0..48 {
                    let it = self.inventory[i];
                    if it == 0 || self.items_in_play[it as usize].item_property_index != t {
                        continue;
                    }
                    self.inventory[i] = 0;
                    self.gui_draw_inventory();
                    return Some(1);
                }
                for i in 0..4 {
                    if self.characters[i].flags & 1 == 0 {
                        continue;
                    }
                    for ii in 0..11 {
                        let it = self.characters[i].items[ii];
                        if it == 0 || self.items_in_play[it as usize].item_property_index != t {
                            continue;
                        }
                        self.characters[i].items[ii] = 0;
                        return Some(1);
                    }
                }
                0
            }
            0xA3 => self.anim_reset_last_part(sp(0) as usize),
            0xA4 => {
                if sp(0) != 0 {
                    let d = self.wll_shape_map[sp(0) as usize & 0xFF] as u8 as usize;
                    let si = self
                        .level_decoration_properties
                        .get(d)
                        .map_or(0, |p| p.shape_index[sp(1) as usize % 10])
                        as usize;
                    self.special_gui_shape =
                        self.level_decoration_shapes.get(si).cloned().flatten();
                    self.special_gui_shape_x = sp(2) as u16;
                    self.special_gui_shape_y = sp(3) as u16;
                    self.special_gui_shape_mirror_flag = sp(4) as u16;
                } else {
                    self.special_gui_shape = None;
                    self.special_gui_shape_x = 0;
                    self.special_gui_shape_y = 0;
                    self.special_gui_shape_mirror_flag = 0;
                }
                1
            }
            0xA5 => {
                let t = sp(2) as u16;
                if sp(0) == 0 {
                    for i in 0..48 {
                        let it = self.inventory[i];
                        if it != 0 && self.items_in_play[it as usize].item_property_index == t {
                            return Some(0);
                        }
                    }
                }
                let (mut cur, last) = if sp(1) == -1 { (0, 4) } else { (sp(1), sp(1)) };
                while cur < last {
                    let c = cur as usize & 3;
                    if self.characters[c].flags & 1 != 0 {
                        for i in 0..11 {
                            let it = self.characters[c].items[i];
                            if it != 0 && self.items_in_play[it as usize].item_property_index == t {
                                return Some(cur);
                            }
                        }
                    }
                    cur += 1;
                }
                -1
            }
            0xA6 => {
                let p1 = self.screen.palette(1).clone();
                let mut p0 = self.screen.palette(0).clone();
                p0.copy_from(&p1, 0, 128, 0);
                *self.screen.palette_mut(0) = p0.clone();
                self.screen.fade_palette(&p0, 10);
                self.screen.fade_flag = 0;
                1
            }
            0xA7 | 0xB7 => self.calc_new_block_position(sp(0) as u16, sp(1) as u16) as i32,
            0xA8 => self.selected_character,
            0xA9 => {
                self.set_hand_item(sp(0) as u16);
                1
            }
            0xAA => {
                self.drink_bezel_cup(3 - sp(0), sp(1) as usize & 3);
                1
            }
            0xAB => {
                if sp(0) < 1 {
                    return Some(0);
                }
                let it = &mut self.items_in_play[sp(0) as usize % 400];
                let mut val = sp(2) as i16;
                if sp(1) == 4 {
                    it.item_property_index = val as u16;
                } else if sp(1) == 15 {
                    it.shp_cur_frame_flg = (it.shp_cur_frame_flg & 0xE000) | (val as u16 & 0x1FFF);
                } else {
                    val = -1;
                }
                val as i32
            }
            0xAC => {
                let t = sp(0) as u16;
                let Some(i) = (0..48).find(|&i| {
                    self.inventory[i] != 0
                        && self.items_in_play[self.inventory[i] as usize].item_property_index == t
                }) else {
                    return Some(-1);
                };
                self.inventory_cur_item = i as i32;
                let r = self.item_in_hand;
                let inv = self.inventory[i];
                self.set_hand_item(inv);
                self.inventory[i] = r;
                if sp(1) != 0 {
                    self.gui_draw_inventory();
                }
                r as i32
            }
            0xAD => self.cast_spell(sp(0), sp(1), sp(2)),
            0xAE => {
                let m = sp(0);
                self.screen.update_screen();
                if m != 0 {
                    self.gui_draw_scene(2);
                    self.pit_drop_scroll(9);
                    self.snd_play_sound_effect(-1, -1);
                    self.shake_scene(30, 4, 0, 1);
                } else {
                    let mut t = -1;
                    for i in 0..4 {
                        if self.characters[i].flags & 1 == 0 || self.characters[i].id >= 0 {
                            continue;
                        }
                        t = match self.characters[i].id {
                            -1 => 54,
                            -5 => 53,
                            -8 => 52,
                            -9 => 51,
                            _ => t,
                        };
                    }
                    self.screen.fill_rect(112, 0, 288, 120, 0, Some(2));
                    self.snd_play_sound_effect(t, -1);
                    self.pit_drop_scroll(12);
                }
                1
            }
            0xAF => {
                let c = sp(0) as usize & 3;
                let sk = sp(1) as usize % 3;
                let l = self.characters[c].skill_levels[sk];
                let need = EXP_REQUIREMENTS[(l as usize).min(10)] as i32
                    - self.characters[c].experience_pts[sk];
                self.increase_experience(c, sk, need.max(0) as u32);
                self.characters[c].skill_levels[sk] as i32 - l as i32
            }
            0xB0 => {
                let p1 = self.screen.palette(1).clone();
                let mut p2 = self.generate_flash_palette(&p1, sp(0));
                let mut p1b = p1.clone();
                self.screen.load_special_colors(&mut p1b);
                self.screen.load_special_colors(&mut p2);
                *self.screen.palette_mut(1) = p1b.clone();
                *self.screen.palette_mut(3) = p2.clone();
                if self.smooth_scroll_mode_normal != 0 {
                    let mut ovl: Vec<u8> = (0..=255u8).collect();
                    ovl[1] = 6;
                    self.screen.copy_region(112, 0, 112, 0, 176, 120, 0, 2, 0);
                    self.screen.apply_overlay(112, 0, 176, 120, 0, &ovl);
                }
                self.screen.set_screen_palette(&p2);
                self.screen.update_screen();
                self.delay(2 * TICK, false);
                self.screen.set_screen_palette(&p1b);
                if self.smooth_scroll_mode_normal != 0 {
                    self.screen.copy_region(112, 0, 112, 0, 176, 120, 2, 0, 0);
                }
                self.screen.update_screen();
                0
            }
            0xB1 => {
                self.restore_magic_shroud();
                1
            }
            0xB3 => self.gui_disable_controls(sp(0)),
            0xB4 => self.gui_enable_controls(),
            0xB5 => {
                self.shake_scene(sp(0), sp(1), sp(2), 1);
                1
            }
            0xB6 => {
                self.process_gas_explosion(sp(0));
                1
            }
            0xB8 => {
                self.gui_draw_scene(2);
                self.screen
                    .cross_fade_region(112, 0, 112, 0, 176, 120, 2, 0, &mut self.rnd);
                self.update_draw_page2();
                1
            }
            0xB9 => {
                self.update_draw_page2();
                1
            }
            0xBA => {
                if sp(0) == 1 {
                    self.set_mouse_cursor_to_icon(133);
                } else {
                    self.set_mouse_cursor_to_item_in_hand();
                }
                1
            }
            0xBB => {
                if sp(0) == -1 {
                    self.snd_stop_speech(true);
                    1
                } else if sp(0) != -2 {
                    self.character_says(sp(0), sp(1), sp(2) != 0)
                } else {
                    self.snd_update_character_speech()
                }
            }
            0xBC => {
                if sp(0) != 0 && sp(1) != 0 {
                    self.next_speech_id = sp(0) + 1000;
                    self.next_speaker = sp(1);
                }
                1
            }
            0xBD => {
                let c = sp(0);
                if c < 0 {
                    let c = -c;
                    if c < 50 {
                        return Some(50);
                    }
                    ((c + 99) / 100) * 100
                } else {
                    ITEM_PRICES
                        .iter()
                        .find(|&&p| p as i32 >= c)
                        .map_or(0, |&p| p as i32)
                }
            }
            0xBE => self.lang_index as i32,
            _ => return None,
        };
        Some(r)
    }

    /// `olol_restoreMagicShroud`.
    pub fn restore_magic_shroud(&mut self) {
        let Some(mut mov) = self.open_wsa("DARKLITE.WSA", 2, false) else {
            return;
        };
        self.screen.hide_mouse();
        let load = |s: &Self, name: &str| -> Pal {
            let mut p = Pal::default();
            if let Some(d) = s.res.file_data(name) {
                p.load(&d);
            }
            p
        };
        let tpal1 = load(self, "LITEPAL1.COL");
        let fade1 = self.screen.generate_fade_table(None, &tpal1, 21);
        let tpal2 = load(self, "LITEPAL2.COL");
        let tpal3 = load(self, "LITEPAL3.COL");
        let fade2 = self.screen.generate_fade_table(Some(&tpal2), &tpal3, 4);
        let mut tables: Vec<Pal> = fade1;
        tables.push(tpal2);
        tables.extend(fade2);
        let mut ti = 0;
        for i in 0..21 {
            let etime = self.millis() + 20 * TICK;
            mov.display_frame(&mut self.screen, i, 0, 0, 0, 0, None, None);
            if let Some(p) = tables.get(ti) {
                self.screen.set_screen_palette(p);
            }
            ti += 1;
            self.screen.update_screen();
            if [2, 5, 8, 11, 13, 15, 17, 19].contains(&i) {
                self.snd_play_sound_effect(95, -1);
            }
            self.delay_until(etime, false);
        }
        self.snd_play_sound_effect(91, -1);
        if let Some(p) = tables.get(ti).cloned() {
            self.screen.fade_palette(&p, 300);
        }
        ti += 1;
        for i in 22..38 {
            let etime = self.millis() + 12 * TICK;
            mov.display_frame(&mut self.screen, i, 0, 0, 0, 0, None, None);
            if [22, 24, 28, 32].contains(&i) {
                self.snd_play_sound_effect(131, -1);
                if let Some(p) = tables.get(ti) {
                    self.screen.set_screen_palette(p);
                }
                ti += 1;
            }
            self.screen.update_screen();
            self.delay_until(etime, false);
        }
        self.screen.show_mouse();
    }
}

impl Host for Lol {
    fn sys_call(&mut self, id: u8, args: &Args) -> i16 {
        if self.trace_calls {
            let a: Vec<i16> = (0..6).map(|i| args.pos(i)).collect();
            eprintln!(
                "  sysCall {:#04x} {} {:?}",
                id,
                lol_data::tables::SCRIPT_FUNCTIONS
                    .get(id as usize)
                    .unwrap_or(&"?"),
                a
            );
        }
        match self.olol(id, args) {
            Some(v) => v as i16,
            None => 0,
        }
    }
}
