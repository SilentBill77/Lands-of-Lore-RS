//! The fight (ScummVM's `lol.cpp`: hits, damage, the monsters' attack and defence skills,
//! poison and paralysis, the nearest monster or character).

use lol_data::tables::MONSTER_MODIFIERS4;

use super::types::*;
use super::wsa::Wsa;
use super::{Lol, TICK};
use crate::screen::CR_NO_P_CHECK;

impl Lol {
    pub fn inflict_magical_damage_for_block(
        &mut self,
        block: i32,
        attacker: i32,
        damage: i32,
        index: i32,
    ) {
        let mut o = self.level_block_properties[block as usize].assigned_objects;
        while o & 0x8000 != 0 {
            let d = self.calc_inflictable_damage_per_item(attacker, o as i32, damage, index, 2);
            self.inflict_damage(o as i32, d, attacker, 2, index);
            let mi = (o & 0x7FFF) as usize;
            if self.monsters[mi].flags & 0x20 != 0 && self.current_level != 22 {
                break;
            }
            o = self.monsters[mi].obj.next_assigned_object;
        }
    }

    /// `battleHitSkillTest`: 0 missed, 1 hit, 2 a critical hit.
    pub fn battle_hit_skill_test(&mut self, attacker: i16, target: i16, skill: usize) -> i32 {
        if target == -1 {
            return 0;
        }
        if attacker == -1 {
            return 1;
        }
        let target = target as u16;
        let attacker = attacker as u16;
        if target & 0x8000 != 0 && self.monsters[(target & 0x7FFF) as usize].mode >= 13 {
            return 0;
        }
        let hit_mod: u16;
        let sk: i32;
        if attacker & 0x8000 != 0 {
            let p =
                &self.monster_properties[self.monsters[(attacker & 0x7FFF) as usize].properties];
            hit_mod = p.fighting_stats[0];
            sk = 100 - p.skill_level as i32;
        } else {
            let a = attacker as usize;
            hit_mod = self.default_modifiers(a)[0];
            // int8 in the original: the tripled modifier wraps
            let mut m = self.characters[a].skill_modifiers[skill];
            if skill == 1 {
                m = m.wrapping_mul(3);
            }
            sk = 100 - (self.characters[a].skill_levels[skill] as i32 + m as i32);
        }
        let evade: u16 = if target & 0x8000 != 0 {
            let mi = (target & 0x7FFF) as usize;
            let mut e = self.monster_properties[self.monsters[mi].properties].fighting_stats[3];
            e = ((e as u32
                * MONSTER_MODIFIERS4[self.monster_difficulty.clamp(0, 2) as usize] as u32)
                >> 8) as u16;
            self.monsters[mi].flags |= 0x10;
            e
        } else {
            self.default_modifiers(target as usize)[3]
        };
        let r = self.roll_dice(1, 100, 0);
        if r >= sk {
            return 2;
        }
        let v = (((evade as u32) << 8) / (hit_mod.max(1) as u32)) as u16;
        if r < v as i32 {
            return 0;
        }
        1
    }

    pub fn calc_inflictable_damage(&mut self, attacker: i16, target: i16, hit_type: i32) -> i32 {
        let s = self.get_character_or_monster_items_might(attacker as i32);
        let mut res = 0;
        for (i, &m) in s.iter().enumerate() {
            res += self.calc_inflictable_damage_per_item(
                attacker as i32,
                target as i32,
                m as i32,
                i as i32,
                hit_type,
            );
        }
        res
    }

    /// `inflictDamage`: the damage done.
    pub fn inflict_damage(
        &mut self,
        target: i32,
        damage: i32,
        attacker: i32,
        skill: i32,
        flags: i32,
    ) -> i32 {
        let target = target as u16;
        let attacker = attacker as u16;
        if target & 0x8000 != 0 {
            let mi = (target & 0x7FFF) as usize;
            if self.monsters[mi].mode >= 13 {
                return 0;
            }
            let max_hp = self.monster_properties[self.monsters[mi].properties].hit_points as i32;
            if damage > 0 {
                let ox = self.roll_dice(1, 24, 0) - 12;
                let oy = self.roll_dice(1, 24, 0) - 12;
                let m = &mut self.monsters[mi];
                // int16 arithmetic as in the original, then clipped
                m.hit_points =
                    (m.hit_points.wrapping_sub(damage as i16) as i32).clamp(0, max_hp) as i16;
                m.damage_received = 0x8000 | damage as u16;
                m.flags |= 0x10;
                m.hit_offs_x = ox as i8;
                m.hit_offs_y = oy as i8;
                if attacker & 0x8000 == 0 {
                    self.apply_monster_defense_skill(mi, attacker as i16, flags, skill, damage);
                }
                let sfx = self.monster_properties[self.monsters[mi].properties].sounds[2] as i32;
                let block = self.monsters[mi].obj.block;
                self.snd_queue_environmental_sound_effect(sfx, block as i32);
                self.check_scene_update_need(block as i32);
                if self.monsters[mi].hit_points <= 0 {
                    self.monsters[mi].hit_points = 0;
                    if attacker & 0x8000 == 0 {
                        self.increase_experience(attacker as usize, skill as usize, max_hp as u32);
                    }
                    self.set_monster_mode(mi, 13);
                }
            } else {
                let m = &mut self.monsters[mi];
                m.hit_points = (m.hit_points.wrapping_sub(damage as i16) as i32)
                    .clamp(1, max_hp.max(1)) as i16;
            }
        } else {
            let mut t = target as usize;
            if t > 3 {
                match (0..4).find(|&i| self.characters[i].id as i32 == target as i32) {
                    Some(i) => t = i,
                    None => return 0,
                }
            }
            let c = &self.characters[t];
            if c.flags & 1 == 0 || c.flags & 8 != 0 {
                return 0;
            }
            if c.flags & 0x1000 == 0 {
                let s = c.scream_sfx as i32;
                self.snd_play_sound_effect(s, -1);
            }
            self.set_temporary_face_frame(t, 6, 4, false);
            let mut damage = damage;
            if flags == 4 && self.item_equipped(t, 229) {
                damage >>= 2;
            }
            self.set_character_magic_or_hit_points(t, 0, -damage, 1);
            if self.characters[t].hit_points_cur <= 0 {
                self.character_hitpoints_zero(t, flags);
            } else {
                self.characters[t].damage_suffered = damage as u16;
                self.set_character_update_event(t, 2, 4, 1);
            }
            self.gui_draw_char_portrait_with_stats(t);
            if attacker & 0x8000 == 0 {
                let a = attacker as usize;
                if skill == 0 {
                    self.characters[a].weapon_hit = damage as u16;
                }
                // uint32 in the original: negative damage lowers the experience
                self.increase_experience(a, skill as usize, damage as u32);
            }
            return damage;
        }
        if attacker & 0x8000 == 0 {
            let a = attacker as usize;
            if skill == 0 {
                self.characters[a].weapon_hit = damage as u16;
            }
            self.increase_experience(a, skill as usize, damage as u32);
        }
        damage
    }

    pub fn character_hitpoints_zero(&mut self, char_num: usize, flags: i32) {
        self.characters[char_num].hit_points_cur = 0;
        self.characters[char_num].flags |= 8;
        self.remove_character_effects(char_num, 1, 5);
        self.party_damage_flags = flags;
    }

    pub fn remove_character_effects(&mut self, char_num: usize, first: i32, last: i32) {
        let c = &mut self.characters[char_num];
        for i in first..=last {
            match i - 1 {
                0 => {
                    c.flags &= 0xFFFB;
                    c.weapon_hit = 0;
                }
                1 => c.damage_suffered = 0,
                2 => c.flags &= 0xFFBF,
                3 => c.flags &= 0xFF7F,
                4 => c.flags &= 0xFEFF,
                6 => c.flags &= 0xEFFF,
                _ => {}
            }
            for ii in 0..5 {
                if i != c.character_update_events[ii] as i32 {
                    continue;
                }
                c.character_update_events[ii] = 0;
                c.character_update_delay[ii] = 0;
            }
        }
        self.timers.enable(3);
    }

    pub fn calc_inflictable_damage_per_item(
        &mut self,
        attacker: i32,
        target: i32,
        item_might: i32,
        index: i32,
        hit_type: i32,
    ) -> i32 {
        let attacker16 = attacker as i16;
        let mut dmg: i32 = if attacker16 == -1 {
            0x100
        } else {
            self.get_character_or_monster_stats(attacker16 as i32)[1] as i32
        };
        let st_t = self.get_character_or_monster_protection_against_items(target as i16 as i32);
        dmg = (dmg * (item_might & 0xFFFF)) >> 8;
        if dmg == 0 {
            return 0;
        }
        if (attacker16 as u16) & 0x8000 == 0 {
            dmg = (dmg * self.characters[attacker16 as usize & 3].total_might_modifier as i32) >> 8;
            if dmg == 0 {
                return 0;
            }
        }
        let d = if index & 0x80 != 0 {
            st_t[7] as i16 as i32
        } else {
            st_t[(index & 7) as usize] as i16 as i32
        };
        let r = (dmg * d.abs()) >> 8;
        dmg = if d < 0 { -r } else { r };
        if hit_type == 2 || dmg == 0 {
            return if dmg == 1 { 2 } else { dmg };
        }
        let prot = self.calculate_protection(target as i16 as i32);
        dmg = (dmg * (256 - ((prot << 7) / dmg).min(217))) >> 8;
        if dmg < 2 { 2 } else { dmg }
    }

    pub fn check_for_party_death(&mut self) {
        for i in 0..4 {
            if self.characters[i].flags & 1 == 0 || self.characters[i].hit_points_cur <= 0 {
                continue;
            }
            return;
        }
        if self.weapons_disabled {
            let b = Button::blank();
            self.clicked_exit_char_inventory(&b);
        }
        self.gui_draw_all_char_portraits_with_stats();
        if self.party_damage_flags & 0x40 != 0 {
            self.screen.fade_to_black(40);
            for i in 0..4 {
                if self.characters[i].flags & 1 != 0 {
                    self.increase_character_hitpoints(i, 1, true);
                }
            }
            self.gui_draw_all_char_portraits_with_stats();
            self.screen.fade_to_palette1(40);
        } else {
            self.screen.fade_clear_scene_window(10);
            self.restore_after_special_scene(0, 1, 1, 0);
            self.snd_play_track(325);
            self.stop_portrait_speech_anim();
            self.init_text_fading(0, true);
            self.set_mouse_cursor_to_icon(0);
            self.update_flags |= 4;
            self.set_lamp_mode(true);
            self.disable_sys_timer(2);
            self.run_menu(MenuCb::DeathMenu);
            self.set_mouse_cursor_to_item_in_hand();
            self.update_flags &= 0xFFFB;
            self.reset_lamp_status();
            self.gui_enable_default_playfield_buttons();
            self.enable_sys_timer(2);
            self.update_draw_page2();
        }
    }

    pub fn apply_monster_attack_skill(&mut self, monster: usize, target: i16, damage: i16) {
        let p = self.monster_properties[self.monsters[monster].properties];
        if self.roll_dice(1, 100, 0) > p.attack_skill_chance as i32 {
            return;
        }
        let t = target as usize;
        match p.attack_skill_type as i32 - 1 {
            0 => {
                let it = self.remove_character_item(t, 0x7FF);
                if it != 0 {
                    self.give_item_to_monster(monster, it);
                    let id = self.characters[t].id as i32;
                    if self.character_says(0x4019, id, true) != 0 {
                        let s = self.get_lang_string(0x4019);
                        self.print_message(6, b"%s", &[Arg::Str(s)]);
                    }
                }
            }
            1 => {
                self.paralyze_poison_character(t, 0x80, 0x88, 100, true);
            }
            2 => {
                let it = self.remove_character_item(t, 0x20);
                if it != 0 {
                    self.delete_item(it);
                    let id = self.characters[t].id as i32;
                    if self.character_says(0x401B, id, true) != 0 {
                        let s = self.get_lang_string(0x401B);
                        self.print_message(6, b"%s", &[Arg::Str(s)]);
                    }
                }
            }
            3 => {
                let it = self.remove_character_item(t, 0x0F);
                if it != 0 {
                    let id = self.characters[t].id as i32;
                    if self.character_says(0x401E, id, true) != 0 {
                        let f = self.get_lang_string(0x401E);
                        let n = self.characters[t].name.clone();
                        self.print_message(6, &f, &[Arg::Str(n)]);
                    }
                    let (x, y) = (self.monsters[monster].obj.x, self.monsters[monster].obj.y);
                    self.set_item_position(it, x, y, 0, 1);
                }
            }
            5 => {
                if self.characters[t].magic_points_cur <= 0 {
                    return;
                }
                let mp = self.characters[t].magic_points_cur;
                self.monsters[monster].hit_points =
                    self.monsters[monster].hit_points.wrapping_add(mp);
                self.characters[t].magic_points_cur = 0;
                self.gui_draw_char_portrait_with_stats(t);
                let id = self.characters[t].id as i32;
                if self.character_says(0x4020, id, true) != 0 {
                    let f = self.get_lang_string(0x4020);
                    let n = self.characters[t].name.clone();
                    self.print_message(6, &f, &[Arg::Str(n)]);
                }
            }
            7 => self.stun_character(t),
            8 => {
                let m = &mut self.monsters[monster];
                m.hit_points = m.hit_points.wrapping_add(damage);
                if m.hit_points as i32 > p.hit_points as i32 {
                    m.hit_points = p.hit_points as i16;
                }
            }
            9 => self.paralyze_poison_all_characters(0x40, 0x48, 100),
            _ => {}
        }
    }

    pub fn apply_monster_defense_skill(
        &mut self,
        monster: usize,
        attacker: i16,
        flags: i32,
        skill: i32,
        damage: i32,
    ) {
        let p = self.monster_properties[self.monsters[monster].properties];
        if self.roll_dice(1, 100, 0) > p.defense_skill_chance as i32 {
            return;
        }
        let a = attacker as usize;
        match p.defense_skill_type as i32 - 1 {
            0 | 1 => {
                if (flags & 0x3F) == 2 || skill != 0 {
                    return;
                }
                for i in 0..3 {
                    let itm = self.characters[a].items[i];
                    if itm == 0 {
                        continue;
                    }
                    let pi = self.items_in_play[itm as usize].item_property_index as usize;
                    if (self.item_properties.get(pi).map_or(0, |p| p.protection) & 0x3F) as i32
                        != flags
                    {
                        continue;
                    }
                    self.remove_character_item(a, 0x7FFF);
                    let id = self.characters[a].id as i32;
                    if p.defense_skill_type == 1 {
                        self.give_item_to_monster(monster, itm);
                        if self.character_says(0x401C, id, true) != 0 {
                            let s = self.get_lang_string(0x401C);
                            self.print_message(6, b"%s", &[Arg::Str(s)]);
                        }
                    } else {
                        self.delete_item(itm);
                        if self.character_says(0x401D, id, true) != 0 {
                            let s = self.get_lang_string(0x401D);
                            self.print_message(6, b"%s", &[Arg::Str(s)]);
                        }
                    }
                }
            }
            2 => {
                if flags & 0x80 == 0 {
                    return;
                }
                self.monsters[monster].flags |= 8;
                let (x, y) = (self.monsters[monster].obj.x, self.monsters[monster].obj.y);
                let (px, py) = (self.party_pos_x, self.party_pos_y);
                self.monsters[monster].direction =
                    (self.calc_monster_direction(x, y, px, py) ^ 4) as u8;
                self.set_monster_mode(monster, 9);
                self.monsters[monster].fight_cur_tick = 30;
            }
            3 => {
                if flags != 3 {
                    return;
                }
                let m = &mut self.monsters[monster];
                m.hit_points = m.hit_points.wrapping_add(damage as i16);
                if m.hit_points as i32 > p.hit_points as i32 {
                    m.hit_points = p.hit_points as i16;
                }
            }
            4 => {
                if flags & 0x80 == 0 {
                    return;
                }
                let m = &mut self.monsters[monster];
                m.hit_points = m.hit_points.wrapping_add(damage as i16);
                if m.hit_points as i32 > p.hit_points as i32 {
                    m.hit_points = p.hit_points as i16;
                }
            }
            5 if (flags & 0x84) == 0x84 => {
                self.monsters[monster].num_dist_attacks =
                    self.monsters[monster].num_dist_attacks.wrapping_add(1);
            }
            _ => {}
        }
    }

    pub fn remove_character_item(&mut self, char_num: usize, item_flags: i32) -> Item {
        for i in 0..11 {
            let s = self.characters[char_num].items[i];
            if (1 << i) & item_flags == 0 || s == 0 {
                continue;
            }
            self.characters[char_num].items[i] = 0;
            self.run_item_script(char_num as i32, s, 0x100, 0, 0);
            return s;
        }
        0
    }

    pub fn paralyze_poison_character(
        &mut self,
        char_num: usize,
        type_flag: u16,
        immunity_flags: u16,
        hit_chance: i32,
        redraw: bool,
    ) -> bool {
        if self.characters[char_num].flags & 1 == 0
            || self.characters[char_num].flags & immunity_flags != 0
        {
            return false;
        }
        if self.roll_dice(1, 100, 0) > hit_chance {
            return false;
        }
        let mut r = false;
        if type_flag == 0x40 {
            self.characters[char_num].flags |= 0x40;
            self.set_character_update_event(char_num, 3, 3600, 1);
            r = true;
        } else if type_flag == 0x80 && !self.item_equipped(char_num, 225) {
            self.characters[char_num].flags |= 0x80;
            self.set_character_update_event(char_num, 4, 10, 1);
            let id = self.characters[char_num].id as i32;
            if self.character_says(0x4021, id, true) != 0 {
                let f = self.get_lang_string(0x4021);
                let n = self.characters[char_num].name.clone();
                self.print_message(6, &f, &[Arg::Str(n)]);
            }
            r = true;
        } else if type_flag == 0x1000 {
            self.characters[char_num].flags |= 0x1000;
            self.set_character_update_event(char_num, 7, 120, 1);
            r = true;
        }
        if r && redraw {
            self.gui_draw_char_portrait_with_stats(char_num);
        }
        r
    }

    pub fn paralyze_poison_all_characters(
        &mut self,
        type_flag: u16,
        immunity_flags: u16,
        hit_chance: i32,
    ) {
        let mut r = false;
        for i in 0..4 {
            if self.paralyze_poison_character(i, type_flag, immunity_flags, hit_chance, false) {
                r = true;
            }
        }
        if r {
            self.gui_draw_all_char_portraits_with_stats();
        }
    }

    pub fn stun_character(&mut self, char_num: usize) {
        if self.characters[char_num].flags & 1 == 0 || self.characters[char_num].flags & 0x108 != 0
        {
            return;
        }
        self.characters[char_num].flags |= 0x100;
        self.set_character_update_event(char_num, 5, 20, 1);
        self.gui_draw_char_portrait_with_stats(char_num);
        let f = self.get_lang_string(0x4026);
        let n = self.characters[char_num].name.clone();
        self.print_message(6, &f, &[Arg::Str(n)]);
    }

    pub fn restore_swamp_palette(&mut self) {
        self.flags_table[52] &= 0xFB;
        if self.current_level != 11 {
            return;
        }
        let mut s = self.screen.palette(2).clone();
        let mut d = self.screen.palette(0).clone();
        for i in 1..d.0.len().min(s.0.len()) {
            std::mem::swap(&mut s.0[i], &mut d.0[i]);
        }
        *self.screen.palette_mut(2) = s.clone();
        *self.screen.palette_mut(0) = d.clone();
        let (br, le) = (self.brightness, self.lamp_effect as i32);
        let mut p1 = self.generate_brightness_palette(&d, br, le);
        let mut p2 = s.clone();
        self.screen.load_special_colors(&mut p2);
        self.screen.load_special_colors(&mut p1);
        *self.screen.palette_mut(2) = p2.clone();
        *self.screen.palette_mut(1) = p1.clone();
        self.play_spell_animation(
            None,
            0,
            0,
            2,
            0,
            0,
            None,
            Some(&p2.0),
            Some(&p1.0),
            40,
            false,
        );
    }

    pub fn launch_magic_viper(&mut self) {
        self.party_awake = true;
        let mut d = 0;
        let mut b = self.current_block;
        while d < 3 {
            let o = self.level_block_properties[b as usize].assigned_objects;
            if o & 0x8000 != 0 {
                break;
            }
            b = self.calc_new_block_position(b, self.current_direction);
            let w = self.level_block_properties[b as usize].walls
                [(self.current_direction ^ 2) as usize];
            if self.wll_wall_flags[w as usize] & 7 != 0 {
                break;
            }
            d += 1;
        }
        self.screen.copy_page(0, 12);
        self.snd_play_sound_effect(148, -1);
        let Some(mut mov) = self.open_wsa("VIPER.WSA", 1, false) else {
            return;
        };
        let num_frames = mov.frames().max(1);
        const VIPER: [i32; 12] = [15, 25, 20, 10, 25, 20, 5, 25, 20, 0, 25, 20];
        let v = &VIPER[(d.min(3) as usize) * 3..];
        let mut frm = v[0];
        let (t1, t2) = (
            self.transparency_table1.clone(),
            self.transparency_table2.clone(),
        );
        loop {
            let etime = self.millis() + 5 * TICK;
            self.screen.copy_page(12, 2);
            if frm == v[2] {
                self.snd_play_sound_effect(172, -1);
            }
            mov.display_frame(
                &mut self.screen,
                frm % num_frames,
                2,
                112,
                0,
                0x5000,
                Some(&t1),
                Some(&t2),
            );
            frm += 1;
            self.screen
                .copy_region(112, 0, 112, 0, 176, 120, 2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.delay_until(etime, false);
            if frm > v[1] || self.should_quit() {
                break;
            }
        }
        self.screen.copy_page(12, 0);
        self.screen.copy_page(12, 2);
        let mut t = self.roll_dice(1, 4, 0);
        for i in 0..4 {
            if self.characters[i].flags & 1 == 0 {
                t %= 4;
                continue;
            }
            let lvl = self.current_level + 10;
            self.inflict_damage(t, lvl, 0x8000, 2, 0x86);
        }
    }

    pub fn break_ice_wall(&mut self, pal1: Option<&[u8]>, pal2: Option<&[u8]>) {
        self.screen.hide_mouse();
        let bl = self.calc_new_block_position(self.current_block, self.current_direction);
        self.level_block_properties[bl as usize].flags &= 0xEF;
        self.screen.copy_page(0, 2);
        self.gui_draw_scene(2);
        self.screen.copy_page(2, 10);
        if let Some(mut mov) = self.open_wsa("SHATTER.WSA", 1, false) {
            self.snd_play_sound_effect(166, -1);
            let n = mov.frames();
            self.play_spell_animation(Some(&mut mov), 0, n, 1, 58, 0, None, pal1, pal2, 20, true);
        }
        self.screen.copy_page(10, 0);
        self.update_draw_page2();
        self.gui_draw_scene(0);
        self.screen.show_mouse();
    }

    pub fn get_nearest_monster_from_character(&mut self, char_num: usize) -> u16 {
        let b = self.calc_new_block_position(self.current_block, self.current_direction);
        self.get_nearest_monster_from_character_for_block(b, char_num)
    }

    pub fn get_nearest_monster_from_character_for_block(
        &mut self,
        block: u16,
        char_num: usize,
    ) -> u16 {
        let mut id = 0xFFFF;
        let mut min_dist = 0x7FFF;
        if block == 0xFFFF {
            return id;
        }
        let (cx, cy) = self.calc_coordinates_for_single_character(char_num);
        let mut o = self.level_block_properties[block as usize].assigned_objects;
        while o & 0x8000 != 0 {
            let m = &self.monsters[(o & 0x7FFF) as usize];
            if m.mode >= 13 {
                o = m.obj.next_assigned_object;
                continue;
            }
            let d = (cx as i32 - m.obj.x as i32).abs() + (cy as i32 - m.obj.y as i32).abs();
            if d < min_dist {
                min_dist = d;
                id = o;
            }
            o = m.obj.next_assigned_object;
        }
        id
    }

    pub fn get_nearest_monster_from_pos(&self, x: i32, y: i32) -> u16 {
        let mut id = 0xFFFF;
        let mut min_dist = 0x7FFF;
        for (i, m) in self.monsters.iter().enumerate() {
            if m.mode > 13 {
                continue;
            }
            let d = (x - m.obj.x as i32).abs() + (y - m.obj.y as i32).abs();
            if d < min_dist {
                min_dist = d;
                id = 0x8000 | i as u16;
            }
        }
        id
    }

    pub fn get_nearest_party_member_from_pos(&mut self, x: i32, y: i32) -> u16 {
        let mut id = 0xFFFF;
        let mut min_dist = 0x7FFF;
        for i in 0..4 {
            if self.characters[i].flags & 1 == 0 || self.characters[i].hit_points_cur <= 0 {
                continue;
            }
            let (cx, cy) = self.calc_coordinates_for_single_character(i);
            let d = (x - cx as i32).abs() + (y - cy as i32).abs();
            if d < min_dist {
                min_dist = d;
                id = i as u16;
            }
        }
        id
    }

    /// Opens an animation that must exist (the original stops with an error otherwise).
    pub fn open_wsa_required(&mut self, name: &str, unk1: i32) -> Option<Wsa> {
        let w = self.open_wsa(name, unk1, false);
        if w.is_none() {
            eprintln!("{name}: animation missing");
        }
        w
    }
}
