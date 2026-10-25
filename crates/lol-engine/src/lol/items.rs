//! The items (ScummVM's `items_lol.cpp`): money, making and deleting items, placing them in
//! the level, the item in hand, the item scripts, thrown objects.

use lol_data::tables::STASH_SETUP;

use super::Lol;
use super::TICK;
use super::types::*;

impl Lol {
    pub fn give_credits(&mut self, credits: i32, redraw: bool) {
        if redraw {
            self.snd_play_sound_effect(101, -1);
        }
        let mut credits = credits;
        let mut t = credits / 30;
        if t == 0 {
            t = 1;
        }
        let mut guard = 0;
        while credits != 0 && guard < 10_000 {
            guard += 1;
            if t > credits {
                t = credits;
            }
            if self.credits < 60 && t > 0 {
                let mut cnt = 0;
                loop {
                    if self.credits < 60 {
                        let mut d = STASH_SETUP[(self.credits % 12) as usize] as i32
                            - self.credits as i32 / 12;
                        if d < 0 {
                            d += 5;
                        }
                        self.money_column_height[d as usize % 5] =
                            self.money_column_height[d as usize % 5].wrapping_add(1);
                    }
                    self.credits += 1;
                    cnt += 1;
                    if cnt >= t {
                        break;
                    }
                }
            } else if self.credits >= 60 {
                self.credits = (self.credits as i32 + t) as u16;
            }
            if redraw {
                self.gui_draw_money_box(6);
                self.delay(TICK, true);
            }
            credits -= t;
        }
    }

    pub fn take_credits(&mut self, credits: i32, redraw: bool) {
        if redraw {
            self.snd_play_sound_effect(101, -1);
        }
        let mut credits = credits.min(self.credits as i32);
        let mut t = credits / 30;
        if t == 0 {
            t = 1;
        }
        let mut guard = 0;
        while credits != 0 && self.credits > 0 && guard < 10_000 {
            guard += 1;
            if t > credits {
                t = credits;
            }
            if (self.credits as i32) - t < 60 && t > 0 {
                let mut cnt = 0;
                loop {
                    self.credits = self.credits.wrapping_sub(1);
                    if self.credits < 60 {
                        let mut d = STASH_SETUP[(self.credits % 12) as usize] as i32
                            - self.credits as i32 / 12;
                        if d < 0 {
                            d += 5;
                        }
                        self.money_column_height[d as usize % 5] =
                            self.money_column_height[d as usize % 5].wrapping_sub(1);
                    }
                    cnt += 1;
                    if cnt >= t {
                        break;
                    }
                }
            } else if (self.credits as i32) - t >= 60 {
                self.credits = (self.credits as i32 - t) as u16;
            }
            if redraw {
                self.gui_draw_money_box(6);
                if credits != 0 {
                    self.delay(TICK, true);
                }
            }
            credits -= t;
        }
    }

    pub fn make_item(&mut self, item_type: i32, cur_frame: i32, flags: i32) -> Item {
        let mut cnt = 0;
        let mut r: usize = 0;
        let mut i: usize = 1;
        while i < 400 {
            if self.items_in_play[i].shp_cur_frame_flg & 0x8000 != 0 {
                cnt = 0;
                break;
            }
            let lvl = self.items_in_play[i].level as i32;
            if !(1..=29).contains(&lvl) || lvl == self.current_level {
                i += 1;
                continue;
            }
            let diff = (self.current_level - lvl).abs();
            if diff <= cnt {
                i += 1;
                continue;
            }
            let mut t = false;
            let mut ii = i as u16;
            let mut guard = 0;
            while ii != 0 && !t && guard < 400 {
                guard += 1;
                t = self.is_item_moveable(ii);
                ii = self.items_in_play[ii as usize].obj.next_assigned_object;
            }
            if t {
                cnt = diff;
                r = i;
            }
            i += 1;
        }
        let mut slot = i.min(399);
        if cnt != 0 {
            slot = 0;
            if self.is_item_moveable(r as u16) {
                let n = self.items_in_play[r].obj.next_assigned_object;
                if n != 0 {
                    self.items_in_play[n as usize].level = self.items_in_play[r].level;
                }
                self.delete_item(r as u16);
                slot = r;
            } else {
                let mut ii = self.items_in_play[r].obj.next_assigned_object;
                let mut guard = 0;
                while ii != 0 && guard < 400 {
                    guard += 1;
                    if self.is_item_moveable(ii) {
                        self.items_in_play[r].obj.next_assigned_object =
                            self.items_in_play[ii as usize].obj.next_assigned_object;
                        self.delete_item(ii);
                        slot = ii as usize;
                        break;
                    }
                    ii = self.items_in_play[ii as usize].obj.next_assigned_object;
                }
            }
        }
        self.items_in_play[slot] = LolItem::default();
        self.items_in_play[slot].item_property_index = item_type as u16;
        self.items_in_play[slot].shp_cur_frame_flg = ((cur_frame & 0x1FFF) | flags) as u16;
        self.items_in_play[slot].level = -1;
        slot as Item
    }

    pub fn place_move_level_item(
        &mut self,
        item: Item,
        level: i32,
        block: i32,
        x_offs: i32,
        y_offs: i32,
        flying_height: i32,
    ) {
        let (x, y) = self.calc_coordinates(block as u16, x_offs as u16, y_offs as u16);
        self.items_in_play[item as usize].obj.x = x;
        self.items_in_play[item as usize].obj.y = y;
        let b = self.items_in_play[item as usize].obj.block;
        if b != 0 {
            self.remove_level_item(item, b as i32);
        }
        if self.current_level == level {
            self.set_item_position(item, x, y, flying_height, 1);
        } else {
            let it = &mut self.items_in_play[item as usize];
            it.level = level as i8;
            it.obj.block = block as u16;
            it.obj.flying_height = flying_height as u8;
            it.shp_cur_frame_flg |= 0x4000;
        }
    }

    pub fn add_item_to_inventory(&mut self, item: Item) -> bool {
        let mut pos = 0;
        let mut i = 0;
        while i < 48 {
            pos = self.inventory_cur_item + i;
            if pos > 47 {
                pos -= 48;
            }
            if self.inventory[pos as usize] == 0 {
                break;
            }
            i += 1;
        }
        if i == 48 {
            return false;
        }
        let mut guard = 0;
        while (self.inventory_cur_item > pos || self.inventory_cur_item + 9 <= pos) && guard < 100 {
            guard += 1;
            self.inventory_cur_item += 1;
            if self.inventory_cur_item > 47 {
                self.inventory_cur_item -= 48;
            }
            self.gui_draw_inventory();
        }
        self.inventory[pos as usize] = item;
        self.gui_draw_inventory();
        true
    }

    pub fn is_item_moveable(&self, item: Item) -> bool {
        let it = &self.items_in_play[item as usize];
        if it.shp_cur_frame_flg & 0x4000 == 0 {
            return false;
        }
        self.item_properties
            .get(it.item_property_index as usize)
            .map_or(0, |p| p.flags)
            & 4
            == 0
    }

    pub fn delete_item(&mut self, item: Item) {
        self.items_in_play[item as usize] = LolItem::default();
        self.items_in_play[item as usize].shp_cur_frame_flg |= 0x8000;
    }

    /// `runItemScript`: the item's function in ITEM.INF with the event in `flags`.
    pub fn run_item_script(&mut self, char_num: i32, item: Item, flags: i32, next: i32, reg4: i32) {
        let func = if item != 0 {
            let pi = self.items_in_play[item as usize].item_property_index as usize;
            self.item_properties
                .get(pi)
                .map_or(0xFF, |p| p.item_script_func)
        } else {
            3
        };
        if func == 0xFF {
            return;
        }
        let Some(script) = self.item_script.clone() else {
            return;
        };
        let mut st = lol_data::emc::State::default();
        if !script.start(&mut st, func as usize) {
            return;
        }
        st.regs[0] = flags as i16;
        st.regs[1] = char_num as i16;
        st.regs[2] = item as i16;
        st.regs[3] = next as i16;
        st.regs[4] = reg4 as i16;
        if (script.entry_flags(&st) as i32) & flags != 0 {
            self.run_script_to_end(&script, &mut st);
        }
    }

    pub fn set_hand_item(&mut self, item: Item) {
        let mut item = item;
        if item != 0 {
            let pi = self.items_in_play[item as usize].item_property_index as usize;
            if self.item_properties.get(pi).map_or(0, |p| p.flags) & 0x80 != 0 {
                self.run_item_script(-1, item, 0x400, 0, 0);
                if self.items_in_play[item as usize].shp_cur_frame_flg & 0x8000 != 0 {
                    item = 0;
                }
            }
        }
        let mut mouse_offs = 0;
        if item != 0 && self.flags_table[31] & 0x02 == 0 {
            mouse_offs = 10;
            if self.current_control_mode == 0 || self.text_enabled() {
                let pi = self.items_in_play[item as usize].item_property_index as usize;
                let name_id = self.item_properties.get(pi).map_or(0, |p| p.name_string_id);
                let f = self.get_lang_string(0x403E);
                let n = self.get_lang_string(name_id);
                self.print_message(0, &f, &[Arg::Str(n)]);
            }
        }
        self.item_in_hand = item;
        let shp = self.get_item_icon_shape(item);
        self.screen.set_mouse_cursor(mouse_offs, mouse_offs, &shp);
    }

    pub fn item_equipped(&self, char_num: usize, item_type: u16) -> bool {
        if char_num > 3 || self.characters[char_num].flags & 1 == 0 {
            return false;
        }
        self.characters[char_num]
            .items
            .iter()
            .any(|&it| it != 0 && self.items_in_play[it as usize].item_property_index == item_type)
    }

    pub fn set_item_position(
        &mut self,
        item: Item,
        x: u16,
        y: u16,
        flying_height: i32,
        moveable: i32,
    ) {
        let (mut x, mut y) = (x, y);
        if flying_height == 0 {
            x = (x & 0xFFC0) | 0x40;
            y = (y & 0xFFC0) | 0x40;
        }
        let block = Self::calc_block_index(x, y);
        {
            let it = &mut self.items_in_play[item as usize];
            it.obj.x = x;
            it.obj.y = y;
            it.obj.block = block;
            it.obj.flying_height = flying_height as u8;
            if moveable != 0 {
                it.shp_cur_frame_flg |= 0x4000;
            } else {
                it.shp_cur_frame_flg &= 0xBFFF;
            }
        }
        self.assign_item_to_block(block as usize, item);
        let cd = self.current_direction;
        self.reassign_draw_objects(cd, item, block as usize, false);
        if moveable != 0 {
            self.run_level_script_custom(block as i32, 0x80, -1, item as i32, 0, 0);
        }
        self.check_scene_update_need(block as i32);
    }

    pub fn remove_level_item(&mut self, item: Item, block: i32) {
        self.remove_assigned_object_from_block(block as usize, item);
        self.remove_draw_object_from_block(block as usize, item);
        self.run_level_script_custom(block, 0x100, -1, item as i32, 0, 0);
        self.items_in_play[item as usize].obj.block = 0;
        self.items_in_play[item as usize].level = 0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn launch_object(
        &mut self,
        object_type: i32,
        item: Item,
        start_x: i32,
        start_y: i32,
        flying_height: i32,
        direction: i32,
        _unk: i32,
        attacker_id: i32,
        c: i32,
    ) -> bool {
        let mut sp = Self::check_draw_object_space(
            self.party_pos_x as i32,
            self.party_pos_y as i32,
            start_x,
            start_y,
        );
        let mut slot: i32 = -1;
        let mut i = 0;
        while i < 8 {
            let t = self.flying_objects[i];
            if t.enable == 0 {
                sp = -1;
                break;
            }
            let csp = Self::check_draw_object_space(
                self.party_pos_x as i32,
                self.party_pos_y as i32,
                t.x as i32,
                t.y as i32,
            );
            if csp > sp {
                sp = csp;
                slot = i as i32;
            }
            i += 1;
        }
        if sp != -1 && slot != -1 {
            i = slot as usize;
            self.end_object_flight(i, start_x, start_y, 8);
        }
        if i == 8 {
            return false;
        }
        let t = &mut self.flying_objects[i];
        t.enable = 1;
        t.object_type = object_type as u8;
        t.item = item;
        t.x = start_x as u16;
        t.y = start_y as u16;
        t.flying_height = flying_height as u8;
        t.direction = direction as u8;
        t.distance = 255;
        t.attacker_id = attacker_id as u16;
        t.flags = 7;
        t.wall_flags = 2;
        t.c = c as u8;
        if attacker_id != -1 {
            if attacker_id & 0x8000 != 0 {
                t.flags &= 0xFD;
            } else {
                t.flags &= 0xFB;
                self.increase_experience(attacker_id as usize & 3, 1, 2);
            }
        }
        self.update_object_flight_position(i);
        true
    }

    pub fn end_object_flight(&mut self, ti: usize, x: i32, y: i32, collision_type: i32) {
        let (mut cx, mut cy) = (x, y);
        let t = self.flying_objects[ti];
        let block = Self::calc_block_index(t.x, t.y) as usize;
        self.remove_assigned_object_from_block(block, t.item);
        self.remove_draw_object_from_block(block, t.item);
        if collision_type == 1 {
            cx = t.x as i32;
            cy = t.y as i32;
        }
        if t.object_type == 0 || t.object_type == 1 {
            self.object_flight_process_hits(ti, cx, cy, collision_type);
            let t = &mut self.flying_objects[ti];
            t.x = ((cx & 0xFFC0) | 0x40) as u16;
            t.y = ((cy & 0xFFC0) | 0x40) as u16;
            t.flying_height = 0;
            self.update_object_flight_position(ti);
        }
        self.flying_objects[ti].enable = 0;
    }

    pub fn process_object_flight(&mut self, ti: usize, x: i32, y: i32) {
        let t = self.flying_objects[ti];
        let bl = Self::calc_block_index(t.x, t.y) as usize;
        self.remove_assigned_object_from_block(bl, t.item);
        self.remove_draw_object_from_block(bl, t.item);
        self.flying_objects[ti].x = x as u16;
        self.flying_objects[ti].y = y as u16;
        self.update_object_flight_position(ti);
        self.check_scene_update_need(bl as i32);
    }

    pub fn update_object_flight_position(&mut self, ti: usize) {
        let t = self.flying_objects[ti];
        if t.object_type == 0 {
            self.set_item_position(
                t.item,
                t.x,
                t.y,
                t.flying_height as i32,
                if t.flying_height == 0 { 1 } else { 0 },
            );
        } else if t.object_type == 1 {
            if t.flying_height == 0 {
                self.delete_item(t.item);
                self.check_scene_update_need(Self::calc_block_index(t.x, t.y) as i32);
            } else {
                self.set_item_position(t.item, t.x, t.y, t.flying_height as i32, 0);
            }
        }
    }

    pub fn object_flight_process_hits(&mut self, ti: usize, x: i32, y: i32, collision_type: i32) {
        let t = self.flying_objects[ti];
        let it = self.items_in_play[t.item as usize];
        let pflags = self
            .item_properties
            .get(it.item_property_index as usize)
            .map_or(0, |p| p.flags);
        match collision_type {
            1 => {
                let b = self.calc_new_block_position(it.obj.block, (t.direction >> 1) as u16);
                self.run_level_script_custom(b as i32, 0x8000, -1, t.item as i32, 0, 0);
            }
            2 => {
                if pflags & 0x4000 != 0 {
                    let mut obj =
                        self.level_block_properties[it.obj.block as usize].assigned_objects;
                    let mut guard = 0;
                    while obj & 0x8000 != 0 && guard < 100 {
                        guard += 1;
                        self.run_item_script(
                            t.attacker_id as i16 as i32,
                            t.item,
                            0x8000,
                            obj as i32,
                            0,
                        );
                        obj = self.obj(obj).next_assigned_object;
                    }
                } else {
                    let n = self.get_nearest_monster_from_pos(x, y);
                    self.run_item_script(t.attacker_id as i16 as i32, t.item, 0x8000, n as i32, 0);
                }
            }
            4 => {
                self.party_awake = true;
                if pflags & 0x4000 != 0 {
                    for i in 0..4 {
                        if self.characters[i].flags & 1 != 0 {
                            self.run_item_script(
                                t.attacker_id as i16 as i32,
                                t.item,
                                0x8000,
                                i as i32,
                                0,
                            );
                        }
                    }
                } else {
                    let n = self.get_nearest_party_member_from_pos(x, y);
                    self.run_item_script(t.attacker_id as i16 as i32, t.item, 0x8000, n as i32, 0);
                }
            }
            _ => {}
        }
    }

    pub fn update_flying_object(&mut self, ti: usize) {
        let t = self.flying_objects[ti];
        let (x, y) = self.get_next_step_coords(t.x as i32, t.y as i32, t.direction as i32);
        let collision = self.check_block_before_object_placement(
            x as u16,
            y as u16,
            63,
            t.flags as u16,
            t.wall_flags as u16,
        );
        if collision != 0 {
            self.end_object_flight(ti, x, y, collision);
        } else {
            self.flying_objects[ti].distance = self.flying_objects[ti].distance.wrapping_sub(1);
            if self.flying_objects[ti].distance != 0 {
                self.process_object_flight(ti, x, y);
            } else {
                self.end_object_flight(ti, x, y, 8);
            }
        }
    }

    /// `checkSceneForItems`: the `color`th item among the objects drawn in a block.
    pub fn check_scene_for_items(&self, block: usize, mut color: i32) -> i32 {
        let mut s = self.level_block_properties[block].draw_objects;
        let mut guard = 0;
        while s != 0 && guard < 1000 {
            guard += 1;
            if s & 0x8000 == 0 {
                color -= 1;
                if color == 0 {
                    return s as i32;
                }
            }
            s = self.obj(s).next_draw_object;
        }
        -1
    }
}
