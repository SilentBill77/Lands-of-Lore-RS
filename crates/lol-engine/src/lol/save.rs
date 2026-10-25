//! Saving and loading (ScummVM's `saveload_lol.cpp` and `saveload_rpg.cpp`): the state of
//! the party and the items, and of each level visited (its walls as changed, the monsters,
//! the thrown objects). The body is ScummVM's layout (big-endian); the header is our own.

use std::path::PathBuf;

use lol_data::tables::MONSTER_MODIFIERS1;

use super::types::*;
use super::{LevelTempData, Lol};

const MAGIC: &[u8; 8] = b"LOLRSSAV";
const VERSION: u32 = 1;

/// A save's slot and description, as the menus list them.
#[derive(Debug, Clone)]
pub struct SaveSlot {
    pub slot: i32,
    pub description: Vec<u8>,
}

struct Out(Vec<u8>);

impl Out {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
}

struct In<'a> {
    d: &'a [u8],
    p: usize,
}

impl In<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.d.get(self.p).copied().unwrap_or(0);
        self.p += 1;
        v
    }
    fn u16(&mut self) -> u16 {
        u16::from_be_bytes([self.u8(), self.u8()])
    }
    fn u32(&mut self) -> u32 {
        u32::from_be_bytes([self.u8(), self.u8(), self.u8(), self.u8()])
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.u8()).collect()
    }
}

impl Lol {
    // ---- the levels' temporary data

    pub fn generate_temp_data(&mut self) {
        let l = (self.current_level - 1).clamp(0, 28) as usize;
        let lvl = self.current_level;
        let p = self.get_block_file_data(lvl);
        let len = u16::from_le_bytes([p[4], p[5]]) as usize;
        let mut walls_xor = vec![0u8; 4096];
        let mut flags = vec![0u16; 1024];
        for i in 0..1024 {
            for ii in 0..4 {
                walls_xor[i * 4 + ii] = p.get(6 + i * len + ii).copied().unwrap_or(0)
                    ^ self.level_block_properties[i].walls[ii];
            }
            flags[i] = self.level_block_properties[i].flags;
        }
        self.level_temp_data[l] = Some(LevelTempData {
            walls_xor,
            flags,
            monsters: self.monsters.clone(),
            flying_objects: self.flying_objects.to_vec(),
            monster_difficulty: self.monster_difficulty as u8,
        });
        self.has_temp_data_flags |= 1 << l;
    }

    pub fn restore_block_temp_data(&mut self, level_index: i32) {
        self.temp_buffer5120.fill(0);
        let l = (level_index - 1).clamp(0, 28) as usize;
        let p = self.get_block_file_data(level_index);
        let len = u16::from_le_bytes([p[4], p[5]]) as usize;
        let Some(t) = self.level_temp_data[l].clone() else {
            return;
        };
        for i in 0..1024 {
            self.level_block_properties[i] = LevelBlockProperty::default();
            for ii in 0..4 {
                self.level_block_properties[i].walls[ii] =
                    p.get(6 + i * len + ii).copied().unwrap_or(0) ^ t.walls_xor[i * 4 + ii];
            }
            self.level_block_properties[i].flags = t.flags[i];
        }
        // restoreMonsterTempData
        self.monsters = t.monsters.clone();
        for i in 0..30 {
            if self.monsters[i].obj.block != 0 {
                self.monsters[i].obj.block = 0;
                self.monsters[i].properties = self.monsters[i].type_ as usize;
                let (x, y) = (self.monsters[i].obj.x, self.monsters[i].obj.y);
                self.place_monster(i, x, y);
            }
        }
        for (i, f) in t.flying_objects.iter().enumerate().take(8) {
            self.flying_objects[i] = *f;
        }
        self.restore_temp_data_adjust_monster_strength(l);
    }

    fn restore_temp_data_adjust_monster_strength(&mut self, index: usize) {
        let Some(t) = &self.level_temp_data[index] else {
            return;
        };
        let md = t.monster_difficulty as usize;
        let cur = self.monster_difficulty.clamp(0, 2) as usize;
        if md == cur {
            return;
        }
        let d = ((MONSTER_MODIFIERS1[md.min(2)] as u32) << 8) / MONSTER_MODIFIERS1[cur] as u32;
        for m in self.monsters.iter_mut() {
            if m.mode >= 14 || m.obj.block == 0 || m.hit_points <= 0 {
                continue;
            }
            m.hit_points = ((d * m.hit_points as u32) >> 8) as i16;
            if cur < md {
                m.hit_points += 1;
            }
            if m.hit_points == 0 {
                m.hit_points = 1;
            }
        }
    }

    // ---- files

    pub fn savegame_filename(&self, slot: i32) -> Option<PathBuf> {
        Some(self.save_dir.as_ref()?.join(format!("lol.{slot:03}")))
    }

    /// The saves there are, the newest first (`updateSaveSlotsList`, `sortSaveSlots`).
    pub fn savegame_list(&self) -> Vec<SaveSlot> {
        let mut v = Vec::new();
        for slot in 0..990 {
            let Some(f) = self.savegame_filename(slot) else {
                break;
            };
            let Ok(d) = std::fs::read(&f) else { continue };
            if d.len() < 16 || &d[..8] != MAGIC {
                continue;
            }
            let mut i = In { d: &d, p: 12 };
            let n = i.u16() as usize;
            let description = i.bytes(n);
            v.push(SaveSlot { slot, description });
        }
        v.sort_by_key(|s| std::cmp::Reverse(s.slot));
        v
    }

    pub fn next_savegame_slot(&self) -> i32 {
        (0..990)
            .find(|&s| self.savegame_filename(s).is_some_and(|f| !f.exists()))
            .unwrap_or(0)
    }

    pub fn delete_savegame(&mut self, slot: i32) {
        if let Some(f) = self.savegame_filename(slot) {
            let _ = std::fs::remove_file(f);
        }
        // The slots after the deleted one move down (`clickedChoiceMenu`).
        let mut slots: Vec<i32> = self.savegame_list().into_iter().map(|s| s.slot).collect();
        slots.sort();
        for s in slots {
            if s > slot
                && s < 990
                && let (Some(a), Some(b)) =
                    (self.savegame_filename(s), self.savegame_filename(s - 1))
            {
                let _ = std::fs::rename(a, b);
            }
        }
    }

    /// `saveGameStateIntern`.
    pub fn save_game_state(&mut self, slot: i32, description: &[u8]) -> bool {
        let Some(file) = self.savegame_filename(slot) else {
            return false;
        };
        self.complete_door_operations();
        self.generate_temp_data();
        let mut o = Out(Vec::new());
        o.bytes(MAGIC);
        o.u32(VERSION);
        o.u16(description.len() as u16);
        o.bytes(description);
        self.update_play_timer();
        o.u32(self.play_time_total / 1000);
        for c in &self.characters {
            o.u16(c.flags);
            let mut name = c.name.clone();
            name.resize(11, 0);
            o.bytes(&name);
            o.u8(c.race_class_sex);
            o.u16(c.id as u16);
            o.u8(c.cur_face_frame);
            o.u8(c.temp_face_frame);
            o.u8(c.scream_sfx);
            for v in c.items_might {
                o.u16(v);
            }
            for v in c.protection_against_items {
                o.u16(v);
            }
            o.u16(c.item_protection);
            o.u16(c.hit_points_cur as u16);
            o.u16(c.hit_points_max);
            o.u16(c.magic_points_cur as u16);
            o.u16(c.magic_points_max);
            o.u8(c.field_41);
            o.u16(c.damage_suffered);
            o.u16(c.weapon_hit);
            o.u16(c.total_might_modifier);
            o.u16(c.total_protection_modifier);
            o.u16(c.might);
            o.u16(c.protection);
            o.u16(c.next_anim_update_countdown as u16);
            for v in c.items {
                o.u16(v);
            }
            for v in c.skill_levels {
                o.u8(v);
            }
            for v in c.skill_modifiers {
                o.u8(v as u8);
            }
            for v in c.experience_pts {
                o.u32(v as u32);
            }
            o.bytes(&c.character_update_events);
            o.bytes(&c.character_update_delay);
        }
        o.u16(self.current_block);
        o.u16(self.party_pos_x);
        o.u16(self.party_pos_y);
        o.u16(self.update_flags as u16);
        o.u8(self.script_direction as u8);
        o.u8(self.selected_spell as u8);
        o.u8(self.scene_default_update as u8);
        o.u8(self.compass_broken);
        o.u8(self.drain_magic);
        o.u16(self.current_direction);
        o.u16(self.compass_direction as u16);
        o.u8(self.selected_character as u8);
        o.u8(self.current_level as u8);
        for v in self.inventory {
            o.u16(v);
        }
        o.u16(self.inventory_cur_item as u16);
        o.u16(self.item_in_hand);
        o.u16(self.last_mouse_region as u16);
        o.u32(self.flags_table.len() as u32);
        o.bytes(&self.flags_table.clone());
        for v in self.global_script_vars {
            o.u16(v as u16);
        }
        o.u8(self.brightness as u8);
        o.u8(self.lamp_oil_status as u8);
        o.u8(self.lamp_effect as u8);
        o.u16(self.credits);
        for v in self.global_script_vars2 {
            o.u16(v);
        }
        for v in &self.available_spells[..7] {
            o.u8(*v as u8);
        }
        o.u32(self.has_temp_data_flags);
        self.reset_items(0);
        for t in &self.items_in_play {
            o.u16(t.obj.next_assigned_object);
            o.u16(t.obj.next_draw_object);
            o.u8(t.obj.flying_height);
            o.u16(t.obj.block);
            o.u16(t.obj.x);
            o.u16(t.obj.y);
            o.u8(t.level as u8);
            o.u16(t.item_property_index);
            o.u16(t.shp_cur_frame_flg);
        }
        self.add_level_items();
        for i in 0..29 {
            let Some(l) = &self.level_temp_data[i] else {
                continue;
            };
            if self.has_temp_data_flags & (1 << i) == 0 {
                continue;
            }
            o.bytes(&l.walls_xor);
            for f in &l.flags {
                o.u8(*f as u8);
            }
            for m in &l.monsters {
                o.u16(m.obj.next_assigned_object);
                o.u16(m.obj.next_draw_object);
                o.u8(m.obj.flying_height);
                o.u16(m.obj.block);
                o.u16(m.obj.x);
                o.u16(m.obj.y);
                o.u8(m.shift_step as u8);
                o.u16(m.dest_x);
                o.u16(m.dest_y);
                o.u8(m.dest_direction);
                o.u8(m.hit_offs_x as u8);
                o.u8(m.hit_offs_y as u8);
                o.u8(m.current_sub_frame);
                o.u8(m.mode);
                o.u8(m.fight_cur_tick as u8);
                o.u8(m.id);
                o.u8(m.direction);
                o.u8(m.facing);
                o.u16(m.flags);
                o.u16(m.damage_received);
                o.u16(m.hit_points as u16);
                o.u8(m.speed_tick);
                o.u8(m.type_);
                o.u8(m.num_dist_attacks);
                o.u8(m.cur_dist_weapon);
                o.u8(m.dist_attack_tick as u8);
                o.u16(m.assigned_items);
                o.bytes(&m.equipment_shapes);
            }
            for f in &l.flying_objects {
                o.u8(f.enable);
                o.u8(f.object_type);
                o.u16(f.attacker_id);
                o.u16(f.item);
                o.u16(f.x);
                o.u16(f.y);
                o.u8(f.flying_height);
                o.u8(f.direction);
                o.u8(f.distance);
                o.u8(f.field_d as u8);
                o.u8(f.c);
                o.u8(f.flags);
                o.u8(f.wall_flags);
            }
            o.u8(l.monster_difficulty);
        }
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&file, &o.0).is_ok()
    }

    /// `loadGameState`.
    pub fn load_game_state(&mut self, slot: i32) -> bool {
        let data = self
            .savegame_filename(slot)
            .and_then(|f| std::fs::read(f).ok());
        let Some(data) = data.filter(|d| d.len() > 16 && &d[..8] == MAGIC) else {
            let s = self.get_lang_string(0x425D);
            self.print_message(2, b"%s", &[Arg::Str(s)]);
            return false;
        };
        self.screen.fade_clear_scene_window(10);
        self.complete_door_operations();
        self.screen.fill_rect(112, 0, 287, 119, 0, Some(0));
        self.screen.update_screen();
        let mut i = In { d: &data, p: 12 };
        let n = i.u16() as usize;
        i.bytes(n);
        let play_secs = i.u32();
        for ci in 0..4 {
            let flags = i.u16();
            let mut name = i.bytes(11);
            if let Some(e) = name.iter().position(|&b| b == 0) {
                name.truncate(e);
            }
            let c = &mut self.characters[ci];
            c.flags = flags;
            c.name = name;
            c.race_class_sex = i.u8();
            c.id = i.u16() as i16;
            c.cur_face_frame = i.u8();
            c.temp_face_frame = i.u8();
            c.scream_sfx = i.u8();
            for v in c.items_might.iter_mut() {
                *v = i.u16();
            }
            for v in c.protection_against_items.iter_mut() {
                *v = i.u16();
            }
            c.item_protection = i.u16();
            c.hit_points_cur = i.u16() as i16;
            c.hit_points_max = i.u16();
            c.magic_points_cur = i.u16() as i16;
            c.magic_points_max = i.u16();
            c.field_41 = i.u8();
            c.damage_suffered = i.u16();
            c.weapon_hit = i.u16();
            c.total_might_modifier = i.u16();
            c.total_protection_modifier = i.u16();
            c.might = i.u16();
            c.protection = i.u16();
            c.next_anim_update_countdown = i.u16() as i16;
            for v in c.items.iter_mut() {
                *v = i.u16();
            }
            for v in c.skill_levels.iter_mut() {
                *v = i.u8();
            }
            for v in c.skill_modifiers.iter_mut() {
                *v = i.u8() as i8;
            }
            for v in c.experience_pts.iter_mut() {
                *v = i.u32() as i32;
            }
            for v in c.character_update_events.iter_mut() {
                *v = i.u8();
            }
            for v in c.character_update_delay.iter_mut() {
                *v = i.u8();
            }
            if c.flags & 1 != 0 {
                let id = c.id as i32;
                c.default_modifiers = match c.race_class_sex {
                    1 => 1,
                    2 => 2,
                    4 => 3,
                    _ => 0,
                };
                self.load_char_face_shapes(ci, id);
            }
        }
        self.current_block = i.u16();
        self.party_pos_x = i.u16();
        self.party_pos_y = i.u16();
        self.update_flags = i.u16() as i32;
        self.script_direction = i.u8() as u16;
        self.selected_spell = i.u8() as i32;
        self.scene_default_update = i.u8() as i32;
        self.compass_broken = i.u8();
        self.drain_magic = i.u8();
        self.current_direction = i.u16();
        self.compass_direction = i.u16() as i16 as i32;
        self.selected_character = i.u8() as i8 as i32;
        self.current_level = i.u8() as i32;
        for v in self.inventory.iter_mut() {
            *v = i.u16();
        }
        self.inventory_cur_item = i.u16() as i16 as i32;
        self.item_in_hand = i.u16();
        self.last_mouse_region = i.u16() as i16 as i32;
        let fs = i.u32() as usize;
        let f = i.bytes(fs);
        self.flags_table = [0; 100];
        let n = fs.min(100);
        self.flags_table[..n].copy_from_slice(&f[..n]);
        for v in self.global_script_vars.iter_mut() {
            *v = i.u16() as i16;
        }
        self.brightness = i.u8() as i32;
        self.lamp_oil_status = i.u8() as i32;
        self.lamp_effect = i.u8() as i8;
        self.credits = i.u16();
        for v in self.global_script_vars2.iter_mut() {
            *v = i.u16();
        }
        for k in 0..7 {
            self.available_spells[k] = i.u8() as i8;
        }
        self.has_temp_data_flags = i.u32();
        for t in self.items_in_play.iter_mut() {
            t.obj.next_assigned_object = i.u16();
            t.obj.next_draw_object = i.u16();
            t.obj.flying_height = i.u8();
            t.obj.block = i.u16();
            t.obj.x = i.u16();
            t.obj.y = i.u16();
            t.level = i.u8() as i8;
            t.item_property_index = i.u16();
            t.shp_cur_frame_flg = i.u16();
        }
        for l in self.level_block_properties.iter_mut().take(1024) {
            l.assigned_objects = 0;
            l.draw_objects = 0;
            l.direction = 5;
        }
        for k in 0..29 {
            if self.has_temp_data_flags & (1 << k) == 0 {
                continue;
            }
            let walls_xor = i.bytes(4096);
            let flags: Vec<u16> = (0..1024).map(|_| i.u8() as u16).collect();
            let mut monsters = vec![Monster::default(); 30];
            for m in monsters.iter_mut() {
                m.obj.next_assigned_object = i.u16();
                m.obj.next_draw_object = i.u16();
                m.obj.flying_height = i.u8();
                m.obj.block = i.u16();
                m.obj.x = i.u16();
                m.obj.y = i.u16();
                m.shift_step = i.u8() as i8;
                m.dest_x = i.u16();
                m.dest_y = i.u16();
                m.dest_direction = i.u8();
                m.hit_offs_x = i.u8() as i8;
                m.hit_offs_y = i.u8() as i8;
                m.current_sub_frame = i.u8();
                m.mode = i.u8();
                m.fight_cur_tick = i.u8() as i8;
                m.id = i.u8();
                m.direction = i.u8();
                m.facing = i.u8();
                m.flags = i.u16();
                m.damage_received = i.u16();
                m.hit_points = i.u16() as i16;
                m.speed_tick = i.u8();
                m.type_ = i.u8();
                m.num_dist_attacks = i.u8();
                m.cur_dist_weapon = i.u8();
                m.dist_attack_tick = i.u8() as i8;
                m.assigned_items = i.u16();
                m.properties = (m.type_ as usize).min(4);
                let e = i.bytes(4);
                m.equipment_shapes.copy_from_slice(&e);
            }
            let mut fos = vec![FlyingObject::default(); 8];
            for f in fos.iter_mut() {
                f.enable = i.u8();
                f.object_type = i.u8();
                f.attacker_id = i.u16();
                f.item = i.u16();
                f.x = i.u16();
                f.y = i.u16();
                f.flying_height = i.u8();
                f.direction = i.u8();
                f.distance = i.u8();
                f.field_d = i.u8() as i8;
                f.c = i.u8();
                f.flags = i.u8();
                f.wall_flags = i.u8();
            }
            let md = i.u8();
            self.level_temp_data[k] = Some(LevelTempData {
                walls_xor,
                flags,
                monsters,
                flying_objects: fos,
                monster_difficulty: md,
            });
        }
        self.calc_char_portrait_xpos();
        self.money_column_height = [0; 5];
        let t = self.credits as i32;
        self.credits = 0;
        self.give_credits(t, false);
        let ih = self.item_in_hand;
        self.set_hand_item(ih);
        let lvl = self.current_level;
        self.load_level(lvl);
        self.gui_draw_play_field();
        self.restart_play_timer_at(play_secs);
        self.timer_special_character_update();
        self.flags_table[73] |= 0x08;
        while !self.screen.is_mouse_visible() {
            self.screen.show_mouse();
        }
        true
    }

    pub fn update_play_timer(&mut self) {
        let now = self.millis();
        self.play_time_total = self
            .play_time_total
            .wrapping_add(now.wrapping_sub(self.play_time_start));
        self.play_time_start = now;
    }

    pub fn restart_play_timer_at(&mut self, secs: u32) {
        self.play_time_total = secs * 1000;
        self.play_time_start = self.millis();
    }
}
