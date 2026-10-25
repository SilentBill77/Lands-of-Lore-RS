//! The monsters and what the view draws of objects (ScummVM's `sprites_lol.cpp`,
//! `sprites_rpg.cpp`): loading their figures, their modes and moves, their attacks from afar,
//! drawing monsters, items, doors and equipment scaled for their distance.

use lol_data::tables::{
    DSC_BLOCK_INDEX, DSC_DIM_MAP, DSC_DOOR_SCALE, DSC_DOOR_X, DSC_DOOR_Y, DSC_DOOR_Y2, DSC_DOOR4,
    DSC_OVL_INDEX, FLYING_OBJECT_SHP, GAME_SHAPE_MAP, MONSTER_DIR_FLAGS, MONSTER_MODIFIERS1,
    MONSTER_MODIFIERS2, MONSTER_MODIFIERS3, MONSTER_SCALE_WH, MONSTER_SCALE_X, MONSTER_SCALE_Y,
    MONSTER_SHIFT_OFFSETS, SCENE_ITEM_OFFS,
};

use super::Lol;
use super::types::*;
use crate::shape::{self, ShapeArgs};

/// `_sceneShpDim`: the window the view's figures are drawn into.
pub const SCENE_SHP_DIM: usize = 13;

impl Lol {
    // ---- the objects' chains

    /// `findObject`, as a copy.
    pub fn obj(&self, id: u16) -> Object {
        if id & 0x8000 != 0 {
            self.monsters
                .get((id & 0x7FFF) as usize)
                .map(|m| m.obj)
                .unwrap_or_default()
        } else {
            self.items_in_play
                .get(id as usize)
                .map(|i| i.obj)
                .unwrap_or_default()
        }
    }

    pub fn obj_mut(&mut self, id: u16) -> Option<&mut Object> {
        if id & 0x8000 != 0 {
            self.monsters
                .get_mut((id & 0x7FFF) as usize)
                .map(|m| &mut m.obj)
        } else {
            self.items_in_play.get_mut(id as usize).map(|i| &mut i.obj)
        }
    }

    fn link_get(&self, l: Link) -> u16 {
        match l {
            Link::Assigned(b) => self.level_block_properties[b].assigned_objects,
            Link::Draw(b) => self.level_block_properties[b].draw_objects,
            Link::NextAssigned(o) => self.obj(o).next_assigned_object,
            Link::NextDraw(o) => self.obj(o).next_draw_object,
            Link::MonsterItems(m) => self.monsters[m].assigned_items,
        }
    }

    fn link_set(&mut self, l: Link, v: u16) {
        match l {
            Link::Assigned(b) => self.level_block_properties[b].assigned_objects = v,
            Link::Draw(b) => self.level_block_properties[b].draw_objects = v,
            Link::NextAssigned(o) => {
                if let Some(x) = self.obj_mut(o) {
                    x.next_assigned_object = v;
                }
            }
            Link::NextDraw(o) => {
                if let Some(x) = self.obj_mut(o) {
                    x.next_draw_object = v;
                }
            }
            Link::MonsterItems(m) => self.monsters[m].assigned_items = v,
        }
    }

    /// `removeAssignedObjectFromBlock`.
    pub fn remove_assigned_object_from_block(&mut self, block: usize, id: u16) {
        let mut l = Link::Assigned(block);
        for _ in 0..2000 {
            let cur = self.link_get(l);
            if cur == 0 {
                return;
            }
            if cur == id {
                let next = self.obj(id).next_assigned_object;
                self.link_set(l, next);
                self.link_set(Link::NextAssigned(id), 0);
                return;
            }
            l = Link::NextAssigned(cur);
        }
    }

    /// `removeDrawObjectFromBlock`.
    pub fn remove_draw_object_from_block(&mut self, block: usize, id: u16) {
        let mut l = Link::Draw(block);
        for _ in 0..2000 {
            let cur = self.link_get(l);
            if cur == 0 {
                return;
            }
            if cur == id {
                let next = self.obj(id).next_draw_object;
                self.link_set(l, next);
                self.link_set(Link::NextDraw(id), 0);
                return;
            }
            l = Link::NextDraw(cur);
        }
    }

    /// `assignObjectToBlock`: at the head of the block's chain.
    pub fn assign_object_to_block(&mut self, block: usize, id: u16) {
        let head = self.level_block_properties[block].assigned_objects;
        self.link_set(Link::NextAssigned(id), head);
        self.level_block_properties[block].assigned_objects = id;
    }

    /// `assignItemToBlock`: behind the block's monsters.
    pub fn assign_item_to_block(&mut self, block: usize, id: u16) {
        let mut l = Link::Assigned(block);
        for _ in 0..2000 {
            let cur = self.link_get(l);
            if cur & 0x8000 == 0 {
                break;
            }
            l = Link::NextAssigned(cur);
        }
        let cur = self.link_get(l);
        self.link_set(Link::NextAssigned(id), cur);
        if id & 0x8000 == 0 {
            self.items_in_play[id as usize].level = -1;
        }
        self.link_set(l, id);
    }

    /// `assignBlockItem`: the item and the chain it heads at the end of the block's chain.
    pub fn assign_block_item(&mut self, block: usize, item: u16) {
        let mut l = Link::Assigned(block);
        for _ in 0..2000 {
            let cur = self.link_get(l);
            if cur & 0x8000 == 0 {
                break;
            }
            l = Link::NextAssigned(cur);
        }
        self.items_in_play[item as usize].level = -1;
        let ix = self.link_get(l);
        if ix == item {
            return;
        }
        self.link_set(l, item);
        let mut l = Link::NextAssigned(item);
        for _ in 0..2000 {
            let cur = self.link_get(l);
            if cur == 0 {
                break;
            }
            l = Link::NextAssigned(cur);
        }
        self.link_set(l, ix);
    }

    // ---- figures of monsters

    pub fn load_monster_shapes(&mut self, file: &str, monster_index: usize, anim_type: i32) {
        self.release_monster_shapes(monster_index);
        let Some(d) = self.res.file_data(file) else {
            return;
        };
        self.screen.load_bitmap(&d, 3, None);
        let p = self.screen.page(2).to_vec();
        let offset = |i: usize| -> Option<usize> {
            let count = u16::from_le_bytes([p[0], p[1]]) as usize;
            (i < count).then(|| {
                u32::from_le_bytes([p[2 + i * 4], p[3 + i * 4], p[4 + i * 4], p[5 + i * 4]])
                    as usize
            })
        };
        let pos = monster_index << 4;
        for i in 0..16 {
            let ts = offset(i);
            let dup = (0..i).find(|&ii| offset(ii) == ts);
            self.monster_shapes[pos + i] = match dup {
                Some(ii) => self.monster_shapes[pos + ii].clone(),
                None => shape::shape_copy(&p, i),
            };
            let size = (self.monster_shapes[pos + i]
                .as_ref()
                .and_then(|s| s.get(10).copied())
                .unwrap_or(0) as usize)
                << 3;
            self.monster_palettes[pos + i] = vec![0; size];
        }
        for i in 0..4 {
            for ii in 0..16 {
                let of = monster_index * 192 + i * 48 + ii * 3;
                let s = (i << 4) + ii + 17;
                self.monster_decoration_shapes[of] = shape::shape_copy(&p, s);
                self.monster_decoration_shapes[of + 1] = shape::shape_copy(&p, s + 1);
                self.monster_decoration_shapes[of + 2] = shape::shape_copy(&p, s + 2);
            }
        }
        self.monster_anim_type[monster_index] = (anim_type & 0xFF) as u8;
        let pal_shape = shape::shape_copy(&p, 16).unwrap_or_default();
        self.screen.clear_page(3);
        self.screen
            .draw_shape(2, &pal_shape, 0, 0, 0, 0, ShapeArgs::none());
        let p = self.screen.page(2).to_vec();
        let tmp_pal1: Vec<u8> = (0..64).map(|i| p[i * 320]).collect();
        for i in 0..16 {
            let pos = (monster_index << 4) + i;
            let Some(shp) = self.monster_shapes[pos].clone() else {
                continue;
            };
            let sz = (shape::size(&shp).saturating_sub(10))
                .min(256)
                .min(shp.len().saturating_sub(10));
            let mut tmp_pal2 = vec![0u8; 256];
            tmp_pal2[..sz].copy_from_slice(&shp[10..10 + sz]);
            let mut tmp_pal3 = [0xFFFFu16; 256];
            let num_col = tmp_pal2[0] as usize;
            for ii in 0..num_col {
                if let Some(cl) = tmp_pal1.iter().position(|&c| c == tmp_pal2[1 + ii]) {
                    tmp_pal3[ii] = cl as u16;
                }
            }
            for ii in 0..8 {
                let mut t2 = vec![0u8; 256];
                t2[..sz].copy_from_slice(&shp[10..10 + sz]);
                for iii in 0..num_col {
                    if tmp_pal3[iii] == 0xFFFF {
                        continue;
                    }
                    let v = p[tmp_pal3[iii] as usize * 320 + ii + 1];
                    if v != 0 {
                        t2[1 + iii] = v;
                    }
                }
                let dst = &mut self.monster_palettes[pos];
                let at = ii * num_col;
                if at + num_col <= dst.len() {
                    dst[at..at + num_col].copy_from_slice(&t2[1..1 + num_col]);
                }
            }
        }
    }

    pub fn release_monster_shapes(&mut self, monster_index: usize) {
        for i in 0..16 {
            let pos = (monster_index << 4) + i;
            self.monster_shapes[pos] = None;
            self.monster_palettes[pos] = Vec::new();
        }
        for i in 0..192 {
            self.monster_decoration_shapes[monster_index * 192 + i] = None;
        }
    }

    pub fn delete_monsters_from_block(&mut self, block: usize) -> i32 {
        let mut i = self.level_block_properties[block].assigned_objects;
        let mut cnt = 0;
        let mut guard = 0;
        while i != 0 && guard < 2000 {
            guard += 1;
            let next = self.obj(i).next_assigned_object;
            if i & 0x8000 == 0 {
                i = next;
                continue;
            }
            let m = (i & 0x7FFF) as usize;
            cnt += 1;
            self.set_monster_mode(m, 14);
            let b = self.monsters[m].obj.block;
            self.check_scene_update_need(b as i32);
            self.place_monster(m, 0, 0);
            i = next;
        }
        cnt
    }

    pub fn set_monster_mode(&mut self, m: usize, mode: i32) {
        if self.monsters[m].mode == 13 && mode != 14 {
            return;
        }
        if mode == 7 {
            self.monsters[m].dest_x = self.party_pos_x;
            self.monsters[m].dest_y = self.party_pos_y;
        }
        if self.monsters[m].mode == 1 && mode == 7 {
            // The original walks its list of monsters but looks at this one only.
            let mo = &mut self.monsters[m];
            mo.mode = mode as u8;
            mo.fight_cur_tick = 0;
            mo.dest_x = self.party_pos_x;
            mo.dest_y = self.party_pos_y;
            let (x, y, dx, dy) = (mo.obj.x, mo.obj.y, mo.dest_x, mo.dest_y);
            let d = self.calc_monster_direction(x, y, dx, dy);
            self.set_monster_direction(m, d);
        } else {
            self.monsters[m].mode = mode as u8;
            self.monsters[m].fight_cur_tick = 0;
            if mode == 14 {
                self.monsters[m].hit_points = 0;
            }
            if mode == 13 && self.monsters[m].flags & 0x20 != 0 {
                self.monsters[m].mode = 0;
                self.monster_drop_items(m);
                if self.current_level != 29 {
                    self.set_monster_mode(m, 14);
                }
                let id = self.monsters[m].id as i32;
                self.run_level_script_custom(0x404, -1, id, id, 0, 0);
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
                if self.monsters[m].mode == 14 {
                    self.place_monster(m, 0, 0);
                }
            }
        }
    }

    fn mprop(&self, m: usize) -> MonsterProperty {
        self.monster_properties[self.monsters[m].properties.min(4)]
    }

    pub fn update_monster_adjust_blocks(&mut self, m: usize) -> bool {
        const DIMS: [i32; 4] = [0, 13, 9, 3];
        let p = self.mprop(m);
        if p.flags & 8 != 0 {
            return true;
        }
        let mo = self.monsters[m];
        let x1 = (mo.obj.x & 0xFF00) | 0x80;
        let y1 = (mo.obj.y & 0xFF00) | 0x80;
        let mut dir: u16;
        if p.flags & 1 != 0 {
            dir = (mo.direction >> 1) as u16;
        } else {
            dir = self.calc_monster_direction(x1, y1, self.party_pos_x, self.party_pos_y) as u16;
            if p.flags & 2 != 0 && dir == (mo.direction ^ 4) as u16 {
                return false;
            }
            dir >>= 1;
        }
        let (mut x2, mut y2) = Self::calc_sprite_rel_position(
            x1,
            y1,
            self.party_pos_x as i32,
            self.party_pos_y as i32,
            dir,
        );
        x2 >>= 8;
        y2 >>= 8;
        if !(0..=3).contains(&y2) {
            return false;
        }
        if x2.abs() > y2 {
            return false;
        }
        for i in 0..18 {
            self.visible_blocks[i] = ((mo.obj.block as i32
                + DSC_BLOCK_INDEX[dir as usize + i] as i8 as i32)
                & 0x3FF) as usize;
        }
        let (fx1, fx2) = self.set_level_shapes_dim((x2 + DIMS[y2 as usize]) as usize, 13);
        fx1 < fx2
    }

    pub fn place_monster(&mut self, m: usize, x: u16, y: u16) {
        let mut cont = true;
        let t = self.monsters[m].obj.block;
        let id = self.monsters[m].id as u16 | 0x8000;
        if t != 0 {
            self.remove_assigned_object_from_block(t as usize, id);
            self.level_block_properties[t as usize].direction = 5;
            self.check_scene_update_need(t as i32);
        } else {
            cont = false;
        }
        self.monsters[m].obj.block = Self::calc_block_index(x, y);
        if self.monsters[m].obj.x != x || self.monsters[m].obj.y != y {
            self.monsters[m].obj.x = x;
            self.monsters[m].obj.y = y;
            self.monsters[m].current_sub_frame = (self.monsters[m].current_sub_frame + 1) & 3;
        }
        let nb = self.monsters[m].obj.block;
        if nb == 0 {
            return;
        }
        self.assign_object_to_block(nb as usize, id);
        self.level_block_properties[nb as usize].direction = 5;
        self.check_scene_update_need(nb as i32);
        let p = self.mprop(m);
        if p.sounds[0] == 0 || p.sounds[0] == 255 || !cont {
            return;
        }
        if (p.flags & 0x100 == 0 || (self.monsters[m].current_sub_frame & 1) == 0) && nb == t {
            return;
        }
        if nb != t {
            let mid = self.monsters[m].id as i32;
            self.run_level_script_custom(nb as i32, 0x800, -1, mid, 0, 0);
        }
        if self.update_flags & 1 != 0 {
            return;
        }
        self.snd_process_environmental_sound_effect(p.sounds[0] as i32, nb as i32);
    }

    pub fn calc_monster_direction(&self, x1: u16, y1: u16, x2: u16, y2: u16) -> i32 {
        let mut r: i16 = 0;
        let mut t1 = y1.wrapping_sub(y2) as i16;
        if t1 < 0 {
            r += 1;
            t1 = t1.wrapping_neg();
        }
        r <<= 1;
        let mut t2 = x2.wrapping_sub(x1) as i16;
        if t2 < 0 {
            r += 1;
            t2 = t2.wrapping_neg();
        }
        let f = (t1 > t2) as i16;
        if t2 >= t1 {
            std::mem::swap(&mut t1, &mut t2);
        }
        r = (r << 1) | f;
        t1 = ((t1 as i32 + 1) >> 1) as i16;
        let f = (t1 > t2) as i16;
        r = (r << 1) | f;
        const RET: [i32; 16] = [1, 2, 1, 0, 7, 6, 7, 0, 3, 2, 3, 4, 5, 6, 5, 4];
        RET[(r & 15) as usize]
    }

    pub fn set_monster_direction(&mut self, m: usize, dir: i32) {
        let mo = &mut self.monsters[m];
        mo.direction = dir as u8;
        if dir & 1 == 0 || (mo.direction as i32 - ((mo.facing as i32) << 1)) >= 2 {
            mo.facing = mo.direction >> 1;
        }
        let b = mo.obj.block;
        self.check_scene_update_need(b as i32);
    }

    pub fn monster_drop_items(&mut self, m: usize) {
        let mut a = self.monsters[m].assigned_items;
        let (x, y) = (self.monsters[m].obj.x, self.monsters[m].obj.y);
        let mut guard = 0;
        while a != 0 && guard < 400 {
            guard += 1;
            let b = a;
            a = self.items_in_play[a as usize].obj.next_assigned_object;
            self.set_item_position(b, x, y, 0, 1);
        }
    }

    pub fn check_block_before_object_placement(
        &mut self,
        x: u16,
        y: u16,
        object_width: u16,
        test_flag: u16,
        wall_flag: u16,
    ) -> i32 {
        self.object_last_direction = 0;
        let (mut x2, mut y2) = (0u16, 0u16);
        let (mut x_offs, mut y_offs) = (0i32, 0i32);
        let mut flag = 0;
        let r = self.test_block_passability(
            Self::calc_block_index(x, y) as i32,
            x as i32,
            y as i32,
            object_width as i32,
            test_flag as i32,
            wall_flag as i32,
        );
        if r != 0 {
            return r;
        }
        if self.check_block_occupied_by_party(x as i32, y as i32, test_flag as i32) {
            return 4;
        }
        if x & 0x80 != 0 {
            if ((x & 0xFF) + object_width) & 0xFF00 != 0 {
                x_offs = 1;
                self.object_last_direction = 2;
                x2 = x.wrapping_add(object_width);
                let r = self.test_block_passability(
                    Self::calc_block_index(x2, y) as i32,
                    x as i32,
                    y as i32,
                    object_width as i32,
                    test_flag as i32,
                    wall_flag as i32,
                );
                if r != 0 {
                    return r;
                }
                if self.check_block_occupied_by_party(x as i32 + x_offs, y as i32, test_flag as i32)
                {
                    return 4;
                }
                flag = 1;
            }
        } else if ((x & 0xFF).wrapping_sub(object_width)) & 0xFF00 != 0 {
            x_offs = -1;
            self.object_last_direction = 6;
            x2 = x.wrapping_sub(object_width);
            let r = self.test_block_passability(
                Self::calc_block_index(x2, y) as i32,
                x as i32,
                y as i32,
                object_width as i32,
                test_flag as i32,
                wall_flag as i32,
            );
            if r != 0 {
                return r;
            }
            if self.check_block_occupied_by_party(x as i32 + x_offs, y as i32, test_flag as i32) {
                return 4;
            }
            flag = 1;
        }
        if y & 0x80 != 0 {
            if ((y & 0xFF) + object_width) & 0xFF00 != 0 {
                y_offs = 1;
                self.object_last_direction = 4;
                y2 = y.wrapping_add(object_width);
                let r = self.test_block_passability(
                    Self::calc_block_index(x, y2) as i32,
                    x as i32,
                    y as i32,
                    object_width as i32,
                    test_flag as i32,
                    wall_flag as i32,
                );
                if r != 0 {
                    return r;
                }
                if self.check_block_occupied_by_party(x as i32, y as i32 + y_offs, test_flag as i32)
                {
                    return 4;
                }
                flag &= 1;
            } else {
                flag = 0;
            }
        } else if ((y & 0xFF).wrapping_sub(object_width)) & 0xFF00 != 0 {
            y_offs = -1;
            self.object_last_direction = 0;
            y2 = y.wrapping_sub(object_width);
            let r = self.test_block_passability(
                Self::calc_block_index(x, y2) as i32,
                x as i32,
                y as i32,
                object_width as i32,
                test_flag as i32,
                wall_flag as i32,
            );
            if r != 0 {
                return r;
            }
            if self.check_block_occupied_by_party(x as i32, y as i32 + y_offs, test_flag as i32) {
                return 4;
            }
            flag &= 1;
        } else {
            flag = 0;
        }
        if flag == 0 {
            return 0;
        }
        let r = self.test_block_passability(
            Self::calc_block_index(x2, y2) as i32,
            x as i32,
            y as i32,
            object_width as i32,
            test_flag as i32,
            wall_flag as i32,
        );
        if r != 0 {
            return r;
        }
        if self.check_block_occupied_by_party(
            x as i32 + x_offs,
            y as i32 + y_offs,
            test_flag as i32,
        ) {
            return 4;
        }
        0
    }

    pub fn test_block_passability(
        &mut self,
        block: i32,
        x: i32,
        y: i32,
        object_width: i32,
        mut test_flag: i32,
        wall_flag: i32,
    ) -> i32 {
        if block == self.current_block as i32 {
            test_flag &= 0xFFFE;
        }
        if test_flag & 1 != 0 {
            self.monster_cur_block = block as u16;
            if self.test_wall_flag(block as u16, -1, wall_flag) {
                return 1;
            }
            self.monster_cur_block = 0;
        }
        if test_flag & 2 == 0 {
            return 0;
        }
        let mut obj = self.level_block_properties[block as usize].assigned_objects;
        let mut guard = 0;
        while obj & 0x8000 != 0 && guard < 100 {
            guard += 1;
            let mi = (obj & 0x7FFF) as usize;
            let mo = self.monsters[mi];
            if mo.mode < 13 {
                let r = Self::check_draw_object_space(x, y, mo.obj.x as i32, mo.obj.y as i32);
                if object_width + self.mprop(mi).max_width as i32 > r {
                    return 2;
                }
            }
            obj = mo.obj.next_assigned_object;
        }
        0
    }

    pub fn calc_monster_skill_level(&self, id: i32, a: i32) -> i32 {
        let c = self.get_character_or_monster_stats(id);
        let mut r = (a << 8) / (c[4].max(1) as i32);
        if id & 0x8000 != 0 {
            r = (r * MONSTER_MODIFIERS2[self.monster_difficulty.clamp(0, 2) as usize] as i32) >> 8;
        } else {
            let s = self.characters[(id & 3) as usize].skill_levels[1];
            if s > 7 {
                r -= r >> 1;
            } else if s > 3 {
                r -= r >> 2;
            }
        }
        r
    }

    pub fn check_block_occupied_by_party(&self, x: i32, y: i32, test_flag: i32) -> bool {
        test_flag & 4 != 0 && self.current_block == Self::calc_block_index(x as u16, y as u16)
    }

    // ---- drawing

    pub fn draw_block_objects(&mut self, block_array_index: usize) {
        let bi = self.visible_blocks[block_array_index];
        let cd = self.current_direction;
        if self.level_block_properties[bi].direction as u16 != cd {
            self.level_block_properties[bi].draw_objects = 0;
            self.level_block_properties[bi].direction = cd as u8;
            let mut s = self.level_block_properties[bi].assigned_objects;
            let mut guard = 0;
            while s != 0 && guard < 1000 {
                guard += 1;
                self.reassign_draw_objects(cd, s, bi, true);
                s = self.obj(s).next_assigned_object;
            }
        }
        let mut s = self.level_block_properties[bi].draw_objects;
        let mut guard = 0;
        while s != 0 && guard < 1000 {
            guard += 1;
            if s & 0x8000 != 0 {
                let mi = (s & 0x7FFF) as usize;
                if block_array_index < 15 {
                    self.draw_monster(mi);
                }
                s = self.monsters[mi].obj.next_draw_object;
                continue;
            }
            let it = self.items_in_play[s as usize];
            let fx = (SCENE_ITEM_OFFS[(s & 7) as usize] as i8 as i32) << 1;
            let mut fy = SCENE_ITEM_OFFS[((s >> 1) & 7) as usize] as i8 as i32 + 5;
            if it.obj.flying_height >= 2 && block_array_index >= 15 {
                s = it.obj.next_draw_object;
                continue;
            }
            let mut shp: Option<Vec<u8>> = None;
            let mut flg = 0;
            if it.obj.flying_height >= 2 {
                fy -= (it.obj.flying_height as i32 - 1) * 6;
            }
            let ip = self
                .item_properties
                .get(it.item_property_index as usize)
                .copied()
                .unwrap_or_default();
            if ip.flags & 0x1000 != 0 && it.shp_cur_frame_flg & 0xC000 == 0 {
                let mut shp_index = if ip.flags & 0x800 != 0 {
                    7
                } else {
                    ip.shp_index as usize
                };
                let ii = (0..8).find(|&ii| {
                    self.flying_objects[ii].enable != 0 && self.flying_objects[ii].item == s
                });
                let fos = FLYING_OBJECT_SHP[shp_index.min(FLYING_OBJECT_SHP.len() - 1)];
                if fos.flip_flags != 0 && (it.obj.x ^ it.obj.y) & 0x20 != 0 {
                    flg |= 0x20;
                }
                flg |= fos.draw_flags as i32;
                if let Some(ii) = ii {
                    match self.current_direction as i32
                        - (self.flying_objects[ii].direction as i32 >> 1)
                        + 3
                    {
                        1 | 5 => shp_index = fos.shape_front as usize,
                        3 => shp_index = fos.shape_back as usize,
                        2 | 6 => {
                            flg |= 0x10;
                            shp_index = fos.shape_left as usize;
                        }
                        0 | 4 => shp_index = fos.shape_left as usize,
                        _ => {}
                    }
                    shp = self.thrown_shapes.get(shp_index).cloned();
                }
                if let Some(s) = &shp {
                    fy += (s.get(2).copied().unwrap_or(0) >> 2) as i32;
                }
            } else {
                shp = if ip.flags & 0x40 != 0 {
                    self.game_shapes.get(ip.shp_index as usize).cloned()
                } else {
                    self.item_shapes
                        .get(GAME_SHAPE_MAP[(ip.shp_index as usize) << 1] as usize)
                        .cloned()
                };
            }
            if let Some(shp) = shp
                && !shp.is_empty()
            {
                self.draw_item_or_monster(
                    &shp,
                    None,
                    it.obj.x as i32,
                    it.obj.y as i32,
                    fx,
                    fy,
                    flg,
                    -1,
                    false,
                );
            }
            s = it.obj.next_draw_object;
        }
    }

    pub fn draw_monster(&mut self, id: usize) {
        let m = self.monsters[id];
        let p = self.mprop(id);
        let mut flg = MONSTER_DIR_FLAGS
            [((self.current_direction as usize) << 2) + (m.facing as usize & 3)]
            as i32;
        let cur_frm = self.get_monster_cur_frame(id, (flg & 0xFFEF) as u16);
        let shift = |i: usize| MONSTER_SHIFT_OFFSETS[i] as i8 as i32;
        let mx = m.obj.x as i32 + shift((m.shift_step as usize & 15) << 1);
        let my = m.obj.y as i32 + shift(((m.shift_step as usize & 15) << 1) + 1);
        let shp: Vec<u8>;
        if cur_frm == -1 {
            shp = self.monster_shapes[(p.shape_index as usize) << 4]
                .clone()
                .unwrap_or_default();
            let (x2, y2, w, h, _) = self.calc_drawing_layer_parameters(mx, my, &shp, false);
            self.shp_dm_x = x2;
            self.shp_dm_y = y2;
            self.dm_scale_w = w;
            self.dm_scale_h = h;
        } else {
            let d = (m.flags & 7) as usize;
            let flip = p.flags & 0x200 != 0;
            flg &= 0x10;
            let idx = ((p.shape_index as usize) << 4) + cur_frm as usize;
            shp = self
                .monster_shapes
                .get(idx)
                .cloned()
                .flatten()
                .unwrap_or_default();
            if p.flags & 0x800 != 0 {
                flg |= 0x20;
            }
            let pal = if d != 0 {
                let base = self
                    .monster_palettes
                    .get(((p.shape_index as usize) << 4) + (cur_frm as usize & 0x0F))
                    .cloned()
                    .unwrap_or_default();
                let at = shp.get(10).copied().unwrap_or(0) as usize * (d - 1);
                Some(base.get(at..).unwrap_or(&[]).to_vec())
            } else {
                None
            };
            let ovl =
                self.draw_item_or_monster(&shp, pal.as_deref(), mx, my, 0, 0, flg | 1, -1, flip);
            for i in 0..4 {
                let v = m.equipment_shapes[i] as i32 - 1;
                if v == -1 {
                    break;
                }
                let at = p.shape_index as usize * 192 + v as usize * 48 + cur_frm as usize * 3;
                let Some(shp2) = self.monster_decoration_shapes.get(at).cloned().flatten() else {
                    continue;
                };
                let (x, y) = (self.shp_dm_x, self.shp_dm_y);
                self.draw_door_or_monster_equipment(&shp2, None, x, y, flg | 1, ovl);
            }
        }
        if m.damage_received == 0 {
            return;
        }
        let mut dw = shape::scaled_width(&shp, self.dm_scale_w as i32) >> 1;
        let mut dh = shape::scaled_height(&shp, self.dm_scale_h as i32) >> 1;
        let blood_amount = if m.mode == 13 {
            (m.fight_cur_tick as i32) << 1
        } else {
            p.hit_points as i32 / ((m.damage_received & 0x7FFF).max(1) as i32)
        };
        let blood = self.game_shapes.get(6).cloned().unwrap_or_default();
        let blood_type: i32 = match p.flags & 0xC000 {
            0x4000 => 63,
            0x8000 => 15,
            0xC000 => 74,
            _ => 0,
        };
        let mut tbl = [0u8; 256];
        if blood_type != 0 {
            for (i, t) in tbl.iter_mut().enumerate() {
                *t = i as u8;
                if (2..=7).contains(&i) {
                    *t = (i as i32 + blood_type) as u8;
                }
            }
        }
        dw += m.hit_offs_x as i32;
        dh += m.hit_offs_y as i32;
        let blood_amount = blood_amount.clamp(1, 4);
        let sw = self.dm_scale_w as i32 / blood_amount;
        let sh = self.dm_scale_h as i32 / blood_amount;
        let page = self.scene_draw_page1;
        let (x, y) = (self.shp_dm_x + dw, self.shp_dm_y + dh);
        let flags = 0x124;
        let fade_level = if blood_type != 0 { 1 } else { 0 };
        self.screen.draw_shape(
            page,
            &blood,
            x,
            y,
            SCENE_SHP_DIM,
            flags,
            ShapeArgs {
                fade: Some((&tbl, fade_level)),
                scale: (sw, sh),
                ..ShapeArgs::none()
            },
        );
    }

    pub fn get_monster_cur_frame(&self, m: usize, dir_flags: u16) -> i32 {
        let mo = &self.monsters[m];
        let p = self.mprop(m);
        match self
            .monster_anim_type
            .get(p.shape_index as usize)
            .copied()
            .unwrap_or(0)
        {
            0 => {
                if dir_flags != 0 {
                    if mo.mode == 13 {
                        -1
                    } else {
                        dir_flags as i32 + mo.current_sub_frame as i32
                    }
                } else {
                    if mo.damage_received != 0 {
                        return 12;
                    }
                    match mo.mode as i32 - 5 {
                        0 => {
                            if p.flags & 4 != 0 {
                                13
                            } else {
                                0
                            }
                        }
                        3 => mo.fight_cur_tick as i32 + 13,
                        6 => 14,
                        8 => -1,
                        _ => mo.current_sub_frame as i32,
                    }
                }
            }
            1 => {
                let mut tmp = p.hit_points as i32;
                tmp = (tmp
                    * MONSTER_MODIFIERS1[self.monster_difficulty.clamp(0, 2) as usize] as i32)
                    >> 8;
                let tmp = if mo.hit_points as i32 > (tmp >> 1) {
                    0
                } else if mo.hit_points as i32 > (tmp >> 2) {
                    4
                } else {
                    8
                };
                match mo.mode {
                    8 => mo.fight_cur_tick as i32 + tmp,
                    11 => 12,
                    13 => mo.fight_cur_tick as i32 + 12,
                    _ => tmp,
                }
            }
            2 => {
                if mo.fight_cur_tick >= 13 {
                    13
                } else {
                    mo.fight_cur_tick as i32
                }
            }
            3 => match mo.mode {
                5 => {
                    if mo.damage_received != 0 {
                        5
                    } else {
                        6
                    }
                }
                8 => mo.fight_cur_tick as i32 + 6,
                11 => 5,
                _ => {
                    if mo.damage_received != 0 {
                        5
                    } else {
                        mo.current_sub_frame as i32
                    }
                }
            },
            _ => 0,
        }
    }

    /// `calcObjectPosition`: the drawing order (height, then distance).
    pub fn calc_object_position(&self, o: &Object, direction: u16) -> i32 {
        let (_, mut y) = Self::calc_sprite_rel_position(
            self.party_pos_x,
            self.party_pos_y,
            o.x as i32,
            o.y as i32,
            direction,
        );
        if y < 0 {
            y = 0;
        }
        ((o.flying_height as i32) << 12) | (4095 - y)
    }

    pub fn reassign_draw_objects(
        &mut self,
        direction: u16,
        item_index: u16,
        block: usize,
        flag: bool,
    ) {
        if self.level_block_properties[block].direction as u16 != direction {
            self.level_block_properties[block].direction = 5;
            return;
        }
        let new_object = self.obj(item_index);
        let r = self.calc_object_position(&new_object, direction);
        let mut l = Link::Draw(block);
        let mut guard = 0;
        loop {
            guard += 1;
            let cur = self.link_get(l);
            if cur == 0 || guard > 1000 {
                break;
            }
            let last = self.obj(cur);
            let pos = self.calc_object_position(&last, direction);
            if (flag && pos >= r) || (!flag && pos > r) {
                break;
            }
            l = Link::NextDraw(cur);
        }
        let cur = self.link_get(l);
        self.link_set(Link::NextDraw(item_index), cur);
        self.link_set(l, item_index);
    }

    pub fn redraw_scene_item(&mut self) {
        let (cb, cd) = (self.current_block, self.current_direction);
        self.assign_visible_blocks(cb as i32, cd as i32);
        self.screen.fill_rect(112, 0, 287, 119, 0, None);
        const TILES: [usize; 2] = [13, 16];
        for (i, &tile) in TILES.iter().enumerate() {
            self.set_level_shapes_dim(tile, 13);
            let mut s = self.level_block_properties[self.visible_blocks[tile]].draw_objects;
            let mut t = ((i as i32) << 7) + 1;
            let mut guard = 0;
            while s != 0 && guard < 1000 {
                guard += 1;
                if s & 0x8000 != 0 {
                    s = self.monsters[(s & 0x7FFF) as usize].obj.next_draw_object;
                    continue;
                }
                let it = self.items_in_play[s as usize];
                if it.shp_cur_frame_flg & 0x4000 != 0
                    && Self::check_draw_object_space(
                        it.obj.x as i32,
                        it.obj.y as i32,
                        self.party_pos_x as i32,
                        self.party_pos_y as i32,
                    ) < 320
                {
                    let fx = (SCENE_ITEM_OFFS[(s & 7) as usize] as i8 as i32) << 1;
                    let mut fy = SCENE_ITEM_OFFS[((s >> 1) & 7) as usize] as i8 as i32 + 5;
                    if it.obj.flying_height > 1 {
                        fy -= (it.obj.flying_height as i32 - 1) * 6;
                    }
                    let ip = self
                        .item_properties
                        .get(it.item_property_index as usize)
                        .copied()
                        .unwrap_or_default();
                    let shp = if ip.flags & 0x40 != 0 {
                        self.game_shapes.get(ip.shp_index as usize).cloned()
                    } else {
                        self.item_shapes
                            .get(GAME_SHAPE_MAP[(ip.shp_index as usize) << 1] as usize)
                            .cloned()
                    }
                    .unwrap_or_default();
                    self.draw_item_or_monster(
                        &shp,
                        None,
                        it.obj.x as i32,
                        it.obj.y as i32,
                        fx,
                        fy,
                        0,
                        t,
                        false,
                    );
                    self.screen.update_screen();
                }
                s = it.obj.next_draw_object;
                t += 1;
            }
        }
    }

    /// `calcSpriteRelPosition`: `(x2, y2)` relative to `(x1, y1)` seen facing `direction`.
    pub fn calc_sprite_rel_position(
        x1: u16,
        y1: u16,
        x2: i32,
        y2: i32,
        direction: u16,
    ) -> (i32, i32) {
        let mut a = x2 - x1 as i32;
        let mut b = y1 as i32 - y2;
        if direction != 0 {
            if direction != 2 {
                std::mem::swap(&mut a, &mut b);
            }
            if direction != 3 {
                a = -a;
                if direction != 1 {
                    b = -b;
                }
            } else {
                b = -b;
            }
        }
        (a, b)
    }

    pub fn draw_door(
        &mut self,
        shape: Option<&[u8]>,
        door_palette: Option<&[u8]>,
        index: usize,
        unk2: usize,
        w: i32,
        h: i32,
        flags: i32,
    ) {
        let Some(shape) = shape else { return };
        let c = DSC_DOOR_Y2[((self.current_direction as usize) << 5) + unk2] as usize;
        let r = (c / 5) + 5 * DSC_DIM_MAP[index] as usize;
        let mut d = DSC_OVL_INDEX.get(r).copied().unwrap_or(0) as usize;
        let t = (index << 5) + c;
        self.shp_dm_y = DSC_DOOR_Y[t] as i16 as i32 + 120;
        let mut u = 0;
        if flags & 2 != 0 {
            let dim_w = DSC_DIM_MAP[index] as usize;
            self.dm_scale_w = DSC_DOOR_SCALE[dim_w << 1];
            self.dm_scale_h = DSC_DOOR_SCALE[(dim_w << 1) + 1];
            u = DSC_DOOR4[dim_w] as i32;
        }
        d += 2;
        if self.dm_scale_w == 0 || self.dm_scale_h == 0 {
            return;
        }
        let s = shape::scaled_height(shape, self.dm_scale_h as i32) >> 1;
        let w = if w != 0 {
            (w * self.dm_scale_w as i32) >> 8
        } else {
            0
        };
        let h = if h != 0 {
            (h * self.dm_scale_h as i32) >> 8
        } else {
            0
        };
        self.shp_dm_x = DSC_DOOR_X[t] as i16 as i32 + w + 200;
        self.shp_dm_y = self.shp_dm_y + 4 - s + h - u;
        let d = d.min(7);
        let sw = shape::scaled_width(shape, self.dm_scale_w as i32);
        self.shp_dm_x -= sw >> 1;
        self.shp_dm_y -= s;
        let (x, y) = (self.shp_dm_x, self.shp_dm_y);
        self.draw_door_or_monster_equipment(shape, door_palette, x, y, flags, d);
    }

    pub fn draw_door_or_monster_equipment(
        &mut self,
        shape: &[u8],
        object_palette: Option<&[u8]>,
        x: i32,
        y: i32,
        flags: i32,
        brightness: usize,
    ) {
        let mut flg = 0;
        if flags & 0x10 != 0 {
            flg |= 1;
        }
        if flags & 0x20 != 0 {
            flg |= 0x1000;
        }
        if flags & 0x40 != 0 {
            flg |= 2;
        }
        let ovl = self.screen.level_overlays[brightness.min(7)].clone();
        let (t1, t2) = (
            self.transparency_table1.clone(),
            self.transparency_table2.clone(),
        );
        let page = self.scene_draw_page1;
        let scale = (self.dm_scale_w as i32, self.dm_scale_h as i32);
        let mut args = ShapeArgs {
            color: object_palette,
            fade: Some((&ovl, 1)),
            scale,
            ..ShapeArgs::none()
        };
        let mut f = flg | 0x104;
        if object_palette.is_some() {
            f |= 0x8000;
        }
        if flg & 0x1000 != 0 {
            args.transparency = Some((&t1, &t2));
        }
        self.screen
            .draw_shape(page, shape, x, y, SCENE_SHP_DIM, f, args);
    }

    /// `drawItemOrMonster`: the figure at its place in the view; the brightness table used.
    pub fn draw_item_or_monster(
        &mut self,
        shape: &[u8],
        monster_palette: Option<&[u8]>,
        x: i32,
        y: i32,
        fine_x: i32,
        fine_y: i32,
        mut flags: i32,
        tbl_value: i32,
        vflip: bool,
    ) -> usize {
        let mut monster_palette = monster_palette.map(|p| p.to_vec());
        let ovl2: Vec<u8> = if flags & 0x80 != 0 {
            flags &= 0xFF7F;
            monster_palette.take().unwrap_or_default()
        } else {
            self.screen.level_overlays[4].clone()
        };
        let (x2, y2, w, h, mut r) = self.calc_drawing_layer_parameters(x, y, shape, vflip);
        self.shp_dm_x = x2;
        self.shp_dm_y = y2;
        self.dm_scale_w = w;
        self.dm_scale_h = h;
        let brightness;
        if tbl_value == -1 {
            r = 7 - ((r / 3) - 1);
            r = r.clamp(0, 7);
            brightness = r as usize;
        } else {
            let mut t = vec![tbl_value as u8; 16];
            t[0] = 0;
            monster_palette = Some(t);
            brightness = 7;
        }
        let mut flg = if flags & 0x10 != 0 { 1 } else { 0 };
        if flags & 0x20 != 0 {
            flg |= 0x1000;
        }
        if flags & 0x40 != 0 {
            flg |= 2;
        }
        if self.current_level == 22 {
            self.screen.level_overlays[brightness][255] = 0;
        } else {
            flg |= 0x2000;
        }
        self.shp_dm_x += (self.dm_scale_w as i32 * fine_x) >> 8;
        self.shp_dm_y += (self.dm_scale_h as i32 * fine_y) >> 8;
        let dh = shape::scaled_height(shape, self.dm_scale_h as i32) >> 1;
        let ovl = self.screen.level_overlays[brightness].clone();
        let (t1, t2) = (
            self.transparency_table1.clone(),
            self.transparency_table2.clone(),
        );
        let page = self.scene_draw_page1;
        let scale = (self.dm_scale_w as i32, self.dm_scale_h as i32);
        let transparent = flg & 0x1000 != 0;
        let mut args = ShapeArgs {
            color: monster_palette.as_deref(),
            fade: Some((&ovl, if transparent { 0 } else { 1 })),
            scale,
            background_fade: Some(&ovl2),
            ..ShapeArgs::none()
        };
        let mut f = flg | 0x124;
        if monster_palette.is_some() {
            f |= 0x8000;
        }
        if transparent {
            args.transparency = Some((&t1, &t2));
        }
        let (sx, sy) = (self.shp_dm_x, self.shp_dm_y);
        self.screen
            .draw_shape(page, shape, sx, sy, SCENE_SHP_DIM, f, args);
        self.shp_dm_x -= shape::scaled_width(shape, self.dm_scale_w as i32) >> 1;
        self.shp_dm_y -= dh;
        brightness
    }

    /// `calcDrawingLayerParameters`: where on the screen and how large; the distance row.
    pub fn calc_drawing_layer_parameters(
        &self,
        x1: i32,
        y1: i32,
        shape: &[u8],
        vflip: bool,
    ) -> (i32, i32, u16, u16, i32) {
        let (x1, y1) = Self::calc_sprite_rel_position(
            self.party_pos_x,
            self.party_pos_y,
            x1,
            y1,
            self.current_direction,
        );
        if y1 < 0 {
            return (0, 0, 0, 0, 0);
        }
        let l = y1 >> 5;
        let mut y2 = MONSTER_SCALE_Y.get(l as usize).copied().unwrap_or(0) as i32;
        let x2 = ((MONSTER_SCALE_X.get(l as usize).copied().unwrap_or(0) as i32 * x1) >> 8) + 200;
        let wh = if y2 > 120 {
            0x100
        } else {
            MONSTER_SCALE_WH
                .get((y2 - 56).max(0) as usize)
                .copied()
                .unwrap_or(0x100)
        };
        if vflip {
            y2 = ((120 - y2) >> 1) + (shape::scaled_height(shape, wh as i32) >> 1);
        } else {
            y2 -= shape::scaled_height(shape, wh as i32) >> 1;
        }
        (x2, y2, wh, wh, l)
    }

    // ---- the monsters' moves

    pub fn update_monster(&mut self, m: usize) {
        const FLAGS: [i32; 15] = [1, 0, 1, 3, 3, 0, 0, 3, 4, 1, 0, 0, 4, 0, 0];
        if self.monsters[m].mode > 14 {
            return;
        }
        let mut f = FLAGS[self.monsters[m].mode as usize];
        let p = self.mprop(m);
        let st = self.monsters[m].speed_tick;
        self.monsters[m].speed_tick = st.wrapping_add(1);
        if st < p.speed_total_wait_ticks && f & 4 == 0 {
            return;
        }
        self.monsters[m].speed_tick = 0;
        if p.flags & 0x40 != 0 {
            let add = self.roll_dice(1, 8, 0);
            let mo = &mut self.monsters[m];
            mo.hit_points = (mo.hit_points as i32 + add).min(p.hit_points as i32) as i16;
        }
        if self.monsters[m].flags & 8 != 0 {
            self.monsters[m].dest_x = self.party_pos_x;
            self.monsters[m].dest_y = self.party_pos_y;
        }
        if f & 2 != 0 && self.update_monster_adjust_blocks(m) {
            self.set_monster_mode(m, 7);
            f &= 6;
        }
        if f & 1 != 0 && self.monsters[m].flags & 0x10 != 0 {
            self.set_monster_mode(m, 7);
        }
        if self.monsters[m].mode != 11 && self.monsters[m].mode != 14 && self.rnd.get(255) & 3 == 0
        {
            self.monsters[m].shift_step = (self.monsters[m].shift_step + 1) & 0x0F;
            let b = self.monsters[m].obj.block;
            self.check_scene_update_need(b as i32);
        }
        match self.monsters[m].mode {
            0 | 1 => {
                if self.monsters[m].flags & 0x10 != 0 {
                    for i in 0..30 {
                        if self.monsters[i].mode == 1 {
                            self.set_monster_mode(i, 7);
                        }
                    }
                } else if self.monsters[m].mode == 1 {
                    self.move_monster(m);
                }
            }
            2 => self.move_monster(m),
            3 => {
                if self.update_monster_adjust_blocks(m) {
                    self.set_monster_mode(m, 7);
                }
                for i in 0..4 {
                    if self.calc_new_block_position(self.monsters[m].obj.block, i)
                        == self.current_block
                    {
                        self.set_monster_mode(m, 7);
                    }
                }
            }
            4 => self.move_straying_monster(m),
            5 => {
                self.party_awake = true;
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_sub(1);
                let mo = self.monsters[m];
                if mo.fight_cur_tick <= 0
                    || Self::check_draw_object_space(
                        self.party_pos_x as i32,
                        self.party_pos_y as i32,
                        mo.obj.x as i32,
                        mo.obj.y as i32,
                    ) > 256
                    || mo.flags & 8 != 0
                {
                    self.set_monster_mode(m, 7);
                } else {
                    self.align_monster_to_party(m);
                }
            }
            6 => {
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_sub(1);
                if self.monsters[m].fight_cur_tick <= 0 {
                    self.set_monster_mode(m, 7);
                }
            }
            7 => {
                if !self.chase_party_with_distance_attacks(m) {
                    self.chase_party_with_close_attacks(m);
                }
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
            }
            8 => {
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_add(1);
                if self.monsters[m].fight_cur_tick > 2 {
                    self.set_monster_mode(m, 5);
                    let v = (((8 << 8) / (p.fighting_stats[4].max(1) as i32))
                        * MONSTER_MODIFIERS3[self.monster_difficulty.clamp(0, 2) as usize] as i32)
                        >> 8;
                    self.monsters[m].fight_cur_tick = v as i8;
                }
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
            }
            9 => {
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_sub(1);
                if self.monsters[m].fight_cur_tick != 0 {
                    self.chase_party_with_close_attacks(m);
                } else {
                    self.set_monster_mode(m, 7);
                    self.monsters[m].flags &= 0xFFF7;
                }
            }
            12 => {
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_add(1);
                if self.monsters[m].fight_cur_tick > 13 {
                    let id = self.monsters[m].id as i32;
                    self.run_level_script_custom(0x404, -1, id, id, 0, 0);
                }
            }
            13 => {
                self.monsters[m].fight_cur_tick = self.monsters[m].fight_cur_tick.wrapping_add(1);
                if self.monsters[m].fight_cur_tick > 2 {
                    self.kill_monster(m);
                }
                let b = self.monsters[m].obj.block;
                self.check_scene_update_need(b as i32);
            }
            14 => self.monsters[m].damage_received = 0,
            _ => {}
        }
        if self.monsters[m].damage_received != 0 {
            if self.monsters[m].damage_received & 0x8000 != 0 {
                self.monsters[m].damage_received &= 0x7FFF;
            } else {
                self.monsters[m].damage_received = 0;
            }
            let b = self.monsters[m].obj.block;
            self.check_scene_update_need(b as i32);
        }
        self.monsters[m].flags &= 0xFFEF;
    }

    pub fn move_monster(&mut self, m: usize) {
        const TURN_POS: [i32; 17] = [0, 2, 6, 6, 0, 2, 4, 4, 2, 2, 4, 6, 0, 0, 4, 6, 0];
        let mo = self.monsters[m];
        if mo.obj.x != mo.dest_x || mo.obj.y != mo.dest_y {
            self.walk_monster(m);
        } else if mo.direction != mo.dest_direction {
            let i = ((mo.facing as usize) << 2) + (mo.dest_direction as usize >> 1);
            self.set_monster_direction(m, TURN_POS[i.min(16)]);
        }
    }

    pub fn walk_monster(&mut self, m: usize) {
        if self.mprop(m).flags & 0x400 != 0 {
            return;
        }
        let s = self.walk_monster_calc_next_step(m);
        if s == -1 {
            let (x, y) = (self.monsters[m].obj.x, self.monsters[m].obj.y);
            if self.walk_monster_check_dest(x as i32, y as i32, m, 4) != 1 {
                return;
            }
            self.object_last_direction ^= 4;
            let d = self.object_last_direction;
            self.set_monster_direction(m, d);
        } else {
            self.set_monster_direction(m, s);
            if self.monsters[m].num_dist_attacks != 0 {
                let mb = self.monsters[m].obj.block;
                if Self::get_block_distance(mb, self.current_block) >= 2 {
                    let dir = self.monsters[m].direction as i32;
                    if self.check_for_possible_distance_attack(mb, dir, 3, self.current_block) != 5
                        && self.monsters[m].dist_attack_tick != 0
                    {
                        return;
                    }
                }
            }
        }
        let (x, y) = (self.monsters[m].obj.x as i32, self.monsters[m].obj.y as i32);
        let d = if s == -1 {
            self.object_last_direction
        } else {
            s
        };
        let (fx, fy) = self.get_next_step_coords(x, y, d);
        self.place_monster(m, fx as u16, fy as u16);
    }

    pub fn chase_party_with_distance_attacks(&mut self, m: usize) -> bool {
        if self.monsters[m].num_dist_attacks == 0 {
            return false;
        }
        if self.monsters[m].dist_attack_tick > 0 {
            self.monsters[m].dist_attack_tick -= 1;
            return false;
        }
        let mb = self.monsters[m].obj.block;
        let facing = self.monsters[m].facing as i32;
        let dir = self.check_for_possible_distance_attack(mb, facing, 4, self.current_block);
        if dir == 5 {
            return false;
        }
        let p = self.mprop(m);
        let s = if self.monsters[m].flags & 0x10 != 0 {
            if p.num_dist_weapons != 0 {
                self.roll_dice(1, p.num_dist_weapons as i32, 0)
            } else {
                0
            }
        } else {
            let s = self.monsters[m].cur_dist_weapon as i32;
            self.monsters[m].cur_dist_weapon += 1;
            if self.monsters[m].cur_dist_weapon as u16 >= p.num_dist_weapons {
                self.monsters[m].cur_dist_weapon = 0;
            }
            s
        };
        let mut flying_object = p.dist_weapons.get(s as usize).copied().unwrap_or(0) as i32;
        if flying_object & 0xC000 != 0 {
            if Self::get_block_distance(mb, self.current_block) > 1 {
                let t = if flying_object & 0x4000 != 0 { 0 } else { 1 };
                flying_object = self.make_item(flying_object & 0x3FFF, 0, 0) as i32;
                if flying_object != 0 {
                    let (x, y) = (self.monsters[m].obj.x as i32, self.monsters[m].obj.y as i32);
                    let id = self.monsters[m].id as i32 | 0x8000;
                    if !self.launch_object(
                        t,
                        flying_object as u16,
                        x,
                        y,
                        12,
                        dir << 1,
                        -1,
                        id,
                        0x3F,
                    ) {
                        self.delete_item(flying_object as u16);
                    }
                }
            }
        } else if flying_object & 0x2000 == 0 {
            if Self::get_block_distance(mb, self.current_block) > 1 {
                return false;
            }
            if flying_object == 1 {
                self.snd_play_sound_effect(147, -1);
                self.shake_scene(10, 2, 2, 1);
                for i in 0..4 {
                    if self.characters[i].flags & 1 == 0 {
                        continue;
                    }
                    let item = self.remove_character_item(i, 15);
                    if item != 0 {
                        let (px, py) = (self.party_pos_x, self.party_pos_y);
                        self.set_item_position(item, px, py, 0, 1);
                    }
                    self.inflict_damage(i as i32, 20, 0xFFFF, 0, 2);
                }
            } else if flying_object == 3 {
                for i in 0..30 {
                    if Self::get_block_distance(mb, self.monsters[i].obj.block) < 7 {
                        self.set_monster_mode(m, 7);
                    }
                }
                let s = self.get_lang_string(0x401A);
                self.print_message(2, b"%s", &[Arg::Str(s)]);
            } else if flying_object == 4 {
                self.launch_magic_viper();
            } else {
                return false;
            }
        }
        if self.monsters[m].num_dist_attacks != 255 {
            self.monsters[m].num_dist_attacks = self.monsters[m].num_dist_attacks.wrapping_sub(1);
        }
        self.monsters[m].dist_attack_tick = ((p.fighting_stats[4] as i32 * 8) >> 8) as i8;
        true
    }

    pub fn chase_party_with_close_attacks(&mut self, m: usize) {
        if self.monsters[m].flags & 8 == 0 {
            let mo = self.monsters[m];
            let dir = self.calc_monster_direction(
                mo.obj.x & 0xFF00,
                mo.obj.y & 0xFF00,
                self.party_pos_x & 0xFF00,
                self.party_pos_y & 0xFF00,
            );
            let (x1, y1) = Self::calc_sprite_rel_position(
                mo.obj.x,
                mo.obj.y,
                self.party_pos_x as i32,
                self.party_pos_y as i32,
                (dir >> 1) as u16,
            );
            if y1 <= 160 && x1.abs() <= 80 {
                if mo.direction as i32 == dir && mo.facing as i32 == dir >> 1 {
                    let dst = self
                        .get_nearest_party_member_from_pos(mo.obj.x as i32, mo.obj.y as i32)
                        as i16;
                    let snd = self.mprop(m).sounds[1] as i32;
                    self.snd_play_sound_effect(snd, -1);
                    let mid = (mo.id as u16 | 0x8000) as i16;
                    let hit = self.battle_hit_skill_test(mid, dst, 0);
                    if hit != 0 {
                        let mx = self.calc_inflictable_damage(mid, dst, hit);
                        let dmg = self.roll_dice(2, mx, 0);
                        self.inflict_damage(dst as i32, dmg, mid as u16 as i32, 0, 0);
                        self.apply_monster_attack_skill(m, dst, dmg as i16);
                    }
                    self.set_monster_mode(m, 8);
                    self.check_scene_update_need(mo.obj.block as i32);
                } else {
                    self.set_monster_direction(m, dir);
                    self.check_scene_update_need(mo.obj.block as i32);
                }
                return;
            }
        }
        let mo = self.monsters[m];
        if mo.obj.x != mo.dest_x || mo.obj.y != mo.dest_y {
            self.walk_monster(m);
        } else {
            self.set_monster_direction(m, mo.dest_direction as i32);
            let mode = if self.roll_dice(1, 100, 0) <= 50 {
                4
            } else {
                3
            };
            self.set_monster_mode(m, mode);
        }
    }

    pub fn walk_monster_calc_next_step(&mut self, m: usize) -> i32 {
        const T1: [i32; 8] = [7, -6, 5, -4, 3, -2, 1, 0];
        const T2: [i32; 8] = [-7, 6, -5, 4, -3, 2, -1, 0];
        self.monster_step_counter += 1;
        if self.monster_step_counter > 10 {
            self.monster_step_counter = 0;
            self.monster_step_mode ^= 1;
        }
        let tbl = if self.monster_step_mode != 0 { T2 } else { T1 };
        let mo = self.monsters[m];
        let (sx, sy) = (mo.obj.x as i32, mo.obj.y as i32);
        let mut s = mo.direction as i32;
        let mut d = self.calc_monster_direction(mo.obj.x, mo.obj.y, mo.dest_x, mo.dest_y);
        if mo.flags & 8 != 0 {
            d ^= 4;
        }
        d = (d - s) & 7;
        if d >= 5 {
            s = (s - 1) & 7;
        } else if d != 0 {
            s = (s + 1) & 7;
        }
        let pflags = self.mprop(m).flags;
        for i in (0..8).rev() {
            s = (s + tbl[i]) & 7;
            let (fx, fy) = self.get_next_step_coords(sx, sy, s);
            let d = self.walk_monster_check_dest(fx, fy, m, 4);
            if d == 0 {
                return s;
            }
            if d != 1 || s & 1 != 0 || pflags & 0x80 == 0 {
                continue;
            }
            let w = self.level_block_properties[self.monster_cur_block as usize].walls
                [((s >> 1) ^ 2) as usize];
            if self.wll_wall_flags[w as usize] & 0x20 != 0
                && self.special_wall_types[w as usize] == 5
            {
                let b = self.monster_cur_block as i32;
                self.open_close_door(b, 1);
                return -1;
            }
            if self.wll_wall_flags[w as usize] & 8 != 0 {
                return -1;
            }
        }
        -1
    }

    pub fn check_for_possible_distance_attack(
        &self,
        monster_block: u16,
        direction: i32,
        distance: i32,
        cur_block: u16,
    ) -> i32 {
        let mdist = Self::get_block_distance(cur_block, monster_block);
        if mdist > distance {
            return 5;
        }
        let dir = self.calc_monster_direction(
            monster_block & 0x1F,
            monster_block >> 5,
            cur_block & 0x1F,
            cur_block >> 5,
        );
        if dir & 1 != 0 || dir != direction << 1 {
            return 5;
        }
        if (monster_block & 0x1F) != (cur_block & 0x1F)
            && (monster_block & 0xFFE0) != (cur_block & 0xFFE0)
        {
            return 5;
        }
        if distance < 0 || direction > 3 {
            return 5;
        }
        let mut p = monster_block;
        for _ in 0..distance {
            p = self.calc_new_block_position(p, direction as u16);
            if p == cur_block {
                return direction;
            }
            let w = self.level_block_properties[p as usize].walls[(direction ^ 2) as usize];
            if self.wll_wall_flags[w as usize] & 2 != 0 {
                return 5;
            }
            if self.level_block_properties[p as usize].assigned_objects & 0x8000 != 0 {
                return 5;
            }
        }
        5
    }

    pub fn walk_monster_check_dest(&mut self, x: i32, y: i32, m: usize, unk: i32) -> i32 {
        let mode = self.monsters[m].mode;
        self.monsters[m].mode = 15;
        let p = self.mprop(m);
        let wf = if p.flags & 0x1000 != 0 { 32 } else { unk };
        let r = self.check_block_before_object_placement(
            x as u16,
            y as u16,
            p.max_width as u16,
            7,
            wf as u16,
        );
        self.monsters[m].mode = mode;
        r
    }

    pub fn get_next_step_coords(&self, src_x: i32, src_y: i32, direction: i32) -> (i32, i32) {
        const AX: [i32; 8] = [0, 32, 32, 32, 0, -32, -32, -32];
        const AY: [i32; 8] = [-32, -32, 0, 32, 32, 32, 0, -32];
        let d = (direction & 7) as usize;
        let sx = src_x as i16 as i32;
        let sy = src_y as i16 as i32;
        ((sx + AX[d]) & 0x1FFF, (sy + AY[d]) & 0x1FFF)
    }

    pub fn align_monster_to_party(&mut self, m: usize) {
        let mo = self.monsters[m];
        let mut mdir = mo.direction >> 1;
        let (mut mx, mut my) = (mo.obj.x, mo.obj.y);
        let use_y = mdir & 1 != 0;
        let pos = if use_y { my } else { mx };
        let centered = pos & 0x7F == 0;
        let mut pos_flag = true;
        if self.mprop(m).max_width <= 63 {
            if centered {
                let mut r = false;
                if mo.obj.next_assigned_object & 0x8000 != 0 {
                    r = true;
                } else {
                    let id = self.level_block_properties[mo.obj.block as usize].assigned_objects;
                    let id = if id & 0x8000 != 0 {
                        id & 0x7FFF
                    } else {
                        0xFFFF
                    };
                    if id != mo.id as u16 {
                        r = true;
                    } else {
                        for _ in 0..3 {
                            mdir = (mdir + 1) & 3;
                            let b = self.calc_new_block_position(mo.obj.block, mdir as u16);
                            let id = self.level_block_properties[b as usize].assigned_objects;
                            let id = if id & 0x8000 != 0 {
                                id & 0x7FFF
                            } else {
                                0xFFFF
                            };
                            if id != 0xFFFF {
                                r = true;
                                break;
                            }
                        }
                    }
                }
                if r {
                    pos_flag = false;
                }
            } else {
                pos_flag = false;
            }
        }
        if centered && pos_flag {
            return;
        }
        let mut v = pos;
        if pos_flag {
            if v & 0x80 != 0 {
                v = v.wrapping_sub(32);
            } else {
                v = v.wrapping_add(32);
            }
        } else if v & 0x80 != 0 {
            v = v.wrapping_add(32);
        } else {
            v = v.wrapping_sub(32);
        }
        if use_y {
            my = v;
        } else {
            mx = v;
        }
        if self.walk_monster_check_dest(mx as i32, my as i32, m, 4) != 0 {
            return;
        }
        let (fx, fy) = Self::calc_sprite_rel_position(
            mx,
            my,
            self.party_pos_x as i32,
            self.party_pos_y as i32,
            (mo.direction >> 1) as u16,
        );
        if fy > 160 || fx.abs() > 80 {
            return;
        }
        self.place_monster(m, mx, my);
    }

    pub fn move_straying_monster(&mut self, m: usize) {
        if self.monsters[m].fight_cur_tick != 0 {
            let fct = self.monsters[m].fight_cur_tick as i32;
            let mut d = (self.monsters[m].direction as i32 - fct) & 6;
            let mut id = self.monsters[m].id;
            for i in 0..7 {
                let (x, y) = self.get_next_step_coords(
                    self.monsters[m].obj.x as i32,
                    self.monsters[m].obj.y as i32,
                    d,
                );
                if self.walk_monster_check_dest(x, y, m, 4) == 0 {
                    self.place_monster(m, x as u16, y as u16);
                    self.set_monster_direction(m, d);
                    if i == 0 {
                        id = id.wrapping_add(1);
                        if id > 3 {
                            self.monsters[m].fight_cur_tick = 0;
                        }
                    }
                    return;
                }
                d = (d + fct) & 6;
            }
            self.set_monster_mode(m, 3);
        } else {
            self.monsters[m].direction &= 6;
            let d = self.monsters[m].direction as i32;
            let (x, y) = self.get_next_step_coords(
                self.monsters[m].obj.x as i32,
                self.monsters[m].obj.y as i32,
                d,
            );
            if self.walk_monster_check_dest(x, y, m, 4) == 0 {
                self.place_monster(m, x as u16, y as u16);
            } else {
                // `getRandomBit`.
                let t: i8 = if self.rnd.get(1) != 0 { 2 } else { -2 };
                self.monsters[m].fight_cur_tick = t;
                self.monsters[m].direction =
                    ((self.monsters[m].direction as i32 + t as i32) & 6) as u8;
            }
        }
    }

    pub fn kill_monster(&mut self, m: usize) {
        self.set_monster_mode(m, 14);
        self.monster_drop_items(m);
        let b = self.monsters[m].obj.block as usize;
        self.check_scene_update_need(b as i32);
        let w = self.level_block_properties[b].walls[0];
        let f = self.level_block_properties[b].flags;
        if self.wll_vmp_map[w as usize] == 0
            && self.wll_shape_map[w as usize] == 0
            && f & 0x40 == 0
            && self.mprop(m).flags & 0x1000 == 0
        {
            self.level_block_properties[b].flags |= 0x80;
        }
        self.place_monster(m, 0, 0);
    }

    /// `getBlockDistance`.
    pub fn get_block_distance(block1: u16, block2: u16) -> i32 {
        let (b1x, b1y) = ((block1 & 0x1F) as i32, (block1 >> 5) as i32);
        let (b2x, b2y) = ((block2 & 0x1F) as i32, (block2 >> 5) as i32);
        let mut dy = (b2y - b1y).abs();
        let mut dx = (b2x - b1x).abs();
        if dx > dy {
            std::mem::swap(&mut dx, &mut dy);
        }
        (dx >> 1) + dy
    }

    pub fn check_draw_object_space(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        (x1 - x2).abs() + (y1 - y2).abs()
    }
}

/// Where a chain of objects continues (the original's `uint16 *` into blocks and objects).
#[derive(Debug, Clone, Copy)]
pub enum Link {
    Assigned(usize),
    Draw(usize),
    NextAssigned(u16),
    NextDraw(u16),
    MonsterItems(usize),
}
