//! The party and the game's texts (ScummVM's `lol.cpp`: `preInit`, `getLangString`,
//! `addCharacter`, the portraits' speech animation, the characters' values, brightness and
//! palette effects, the dialogue fields).

use lol_data::tables::{
    CHAR_DEFS_AKSHEL, CHAR_DEFS_KIERAN, CHAR_DEFS_MAN, CHAR_DEFS_WOMAN, EXP_REQUIREMENTS,
    character_defs,
};

use super::types::*;
use super::{Lol, TICK};
use crate::screen::{FontId, Pal};
use crate::shape::{self, ShapeArgs};

/// `Util::decodeString1`: Westwood's packing of common letter pairs.
pub fn decode_string1(src: &[u8]) -> Vec<u8> {
    const T1: [u8; 16] = [
        0x20, 0x65, 0x74, 0x61, 0x69, 0x6E, 0x6F, 0x73, 0x72, 0x6C, 0x68, 0x63, 0x64, 0x75, 0x70,
        0x6D,
    ];
    const T2: [u8; 128] = [
        0x74, 0x61, 0x73, 0x69, 0x6F, 0x20, 0x77, 0x62, 0x20, 0x72, 0x6E, 0x73, 0x64, 0x61, 0x6C,
        0x6D, 0x68, 0x20, 0x69, 0x65, 0x6F, 0x72, 0x61, 0x73, 0x6E, 0x72, 0x74, 0x6C, 0x63, 0x20,
        0x73, 0x79, 0x6E, 0x73, 0x74, 0x63, 0x6C, 0x6F, 0x65, 0x72, 0x20, 0x64, 0x74, 0x67, 0x65,
        0x73, 0x69, 0x6F, 0x6E, 0x72, 0x20, 0x75, 0x66, 0x6D, 0x73, 0x77, 0x20, 0x74, 0x65, 0x70,
        0x2E, 0x69, 0x63, 0x61, 0x65, 0x20, 0x6F, 0x69, 0x61, 0x64, 0x75, 0x72, 0x20, 0x6C, 0x61,
        0x65, 0x69, 0x79, 0x6F, 0x64, 0x65, 0x69, 0x61, 0x20, 0x6F, 0x74, 0x72, 0x75, 0x65, 0x74,
        0x6F, 0x61, 0x6B, 0x68, 0x6C, 0x72, 0x20, 0x65, 0x69, 0x75, 0x2C, 0x2E, 0x6F, 0x61, 0x6E,
        0x73, 0x72, 0x63, 0x74, 0x6C, 0x61, 0x69, 0x6C, 0x65, 0x6F, 0x69, 0x72, 0x61, 0x74, 0x70,
        0x65, 0x61, 0x6F, 0x69, 0x70, 0x20, 0x62, 0x6D,
    ];
    let mut out = Vec::with_capacity(src.len() * 2);
    for &c in src {
        if c == 0 {
            break;
        }
        if c & 0x80 != 0 {
            let c = c & 0x7F;
            out.push(T1[((c & 0x78) >> 3) as usize]);
            out.push(T2[c as usize]);
        } else {
            out.push(c);
        }
    }
    out
}

/// `Util::decodeString2`: `1B c` stands for `c + 7F`.
pub fn decode_string2(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() && src[i] != 0 {
        if src[i] == 0x1B && i + 1 < src.len() {
            i += 1;
            out.push(src[i].wrapping_add(0x7F));
        } else {
            out.push(src[i]);
        }
        i += 1;
    }
    out
}

/// `getTableEntry`: entry `id` of a table of offsets (u16) to NUL-terminated strings.
pub fn table_entry(buffer: &[u8], id: u16) -> &[u8] {
    let at = id as usize * 2;
    let Some(off) = buffer
        .get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
    else {
        return &[];
    };
    let rest = buffer.get(off..).unwrap_or(&[]);
    let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
    &rest[..end]
}

impl Lol {
    /// The extension of the language files (`_languageExt`).
    pub fn language_ext(&self) -> &'static str {
        ["ENG", "FRE", "GER"][self.lang_index]
    }

    /// `preInit`: the shared archives, fonts, the speech of everyone, the texts of the game,
    /// the icons.
    pub fn pre_init(&mut self) {
        self.res.load_pak("GENERAL.PAK");
        self.res.load_pak("STARTUP.PAK");
        if let Some(d) = self.res.file_data("FONT9P.FNT") {
            self.screen.load_font(FontId::F9, &d);
        }
        if let Some(d) = self.res.file_data("FONT6P.FNT") {
            self.screen.load_font(FontId::F6, &d);
        }
        self.load_talk_file(0);
        let file = format!("LANDS.{}", self.language_ext());
        self.lands_file = self.res.file_data(&file).unwrap_or_default();
        self.load_item_icon_shapes();
    }

    /// The shapes of a file of figures (`loadBitmap` and `makeShapeCopy` for each).
    pub fn load_shape_file(&mut self, name: &str) -> Vec<Vec<u8>> {
        let Some(d) = self.res.file_data(name) else {
            return Vec::new();
        };
        self.screen.load_bitmap(&d, 3, None);
        let page = self.screen.page(3).to_vec();
        let count = u16::from_le_bytes([page[0], page[1]]) as usize;
        (0..count)
            .map(|i| shape::shape_copy(&page, i).unwrap_or_default())
            .collect()
    }

    pub fn load_item_icon_shapes(&mut self) {
        self.item_icon_shapes = self.load_shape_file("ITEMICN.SHP");
        if let Some(s) = self.item_icon_shapes.first().cloned() {
            self.screen.set_mouse_cursor(0, 0, &s);
        }
        if self.game_shapes.is_empty() {
            self.game_shapes = self.load_shape_file("GAMESHP.SHP");
        }
    }

    pub fn set_mouse_cursor_to_icon(&mut self, icon: usize) {
        self.flags_table[31] |= 0x02;
        let pi = self.items_in_play[self.item_in_hand as usize].item_property_index as usize;
        let i = self.item_properties.get(pi).map_or(0, |p| p.shp_index) as usize;
        if i == icon {
            return;
        }
        if let Some(s) = self.item_icon_shapes.get(icon).cloned() {
            self.screen.set_mouse_cursor(0, 0, &s);
        }
    }

    pub fn set_mouse_cursor_to_item_in_hand(&mut self) {
        self.flags_table[31] &= 0xFD;
        let o = if self.item_in_hand == 0 { 0 } else { 10 };
        let s = self.get_item_icon_shape(self.item_in_hand);
        self.screen.set_mouse_cursor(o, o, &s);
    }

    /// `checkFloatingPointerRegions`: arrows as the pointer near the view's edges (if enabled).
    pub fn check_floating_pointer_regions(&mut self) {
        if !self.floating_cursors {
            return;
        }
        let mut t = -1;
        let (x, y) = self.get_mouse_pos();
        if self.update_flags & 4 == 0
            && self.floating_cursor_control == 0
            && Self::pos_within_rect(x, y, 96, 0, 303, 136)
            && !Self::pos_within_rect(x, y, 128, 16, 271, 119)
        {
            if Self::pos_within_rect(x, y, 112, 0, 287, 15) {
                t = 0;
            }
            if Self::pos_within_rect(x, y, 272, 88, 303, 319) {
                t = 1;
            }
            if Self::pos_within_rect(x, y, 112, 110, 287, 135) {
                t = 2;
            }
            if Self::pos_within_rect(x, y, 96, 88, 127, 119) {
                t = 3;
            }
            if Self::pos_within_rect(x, y, 96, 16, 127, 87) {
                t = 4;
            }
            if Self::pos_within_rect(x, y, 272, 16, 303, 87) {
                t = 5;
            }
            // As the original: `t < 4` includes -1 (the edge's corners), which then looks to
            // the left and can show the blocked pointer.
            if t < 4 {
                let d = ((self.current_direction as i32 + t) & 3) as u16;
                let nb = self.calc_new_block_position(self.current_block, d);
                if !self.check_block_passability(nb, d) {
                    t = 6;
                }
            }
        }
        if t == self.current_floating_cursor {
            return;
        }
        if t == -1 {
            self.set_mouse_cursor_to_item_in_hand();
        } else {
            const PX: [i32; 7] = [7, 13, 7, 0, 0, 15, 7];
            const PY: [i32; 7] = [0, 7, 12, 7, 6, 6, 7];
            let s = self.game_shapes[10 + t as usize].clone();
            self.screen
                .set_mouse_cursor(PX[t as usize], PY[t as usize], &s);
        }
        self.current_floating_cursor = t;
    }

    /// `getItemIconShapePtr`.
    pub fn get_item_icon_shape(&self, item: Item) -> Vec<u8> {
        let it = &self.items_in_play[item as usize];
        let p = self
            .item_properties
            .get(it.item_property_index as usize)
            .copied()
            .unwrap_or_default();
        let mut ix = p.shp_index as i32;
        if p.flags & 0x200 != 0 {
            ix += (it.shp_cur_frame_flg & 0x1FFF) as i32 - 1;
        }
        self.item_icon_shapes
            .get(ix.max(0) as usize)
            .cloned()
            .unwrap_or_default()
    }

    /// `getLangString`: a text of the game (bit 14: `LANDS.xxx`, else the level's file).
    pub fn get_lang_string(&self, id: u16) -> Vec<u8> {
        if id == 0xFFFF {
            return Vec::new();
        }
        let real = id & 0x3FFF;
        let buffer = if id & 0x4000 != 0 {
            &self.lands_file
        } else {
            &self.level_lang_file
        };
        if buffer.is_empty() {
            return Vec::new();
        }
        decode_string2(&decode_string1(table_entry(buffer, real)))
    }

    /// `addCharacter`: one of the heroes joins the party (from the program's table).
    pub fn add_character(&mut self, id: i32) -> bool {
        const CDF: [usize; 9] = [0, 0, 0, 1, 0, 0, 1, 2, 3];
        let mut num = self.count_active_characters();
        if num >= 3 {
            if self.characters[2].id == 4
                && id == 6
                && self.current_level == 16
                && self.current_block == 0x225
            {
                // As ScummVM: Lora leaves gracefully when Paulson would join a full party.
                let items = self.characters[2].items;
                for it in items {
                    if it != 0 {
                        let (l, b) = (self.current_level, self.current_block as i32);
                        self.place_move_level_item(it, l, b, 0, 0, 0);
                    }
                }
                num = 2;
            } else {
                return false;
            }
        }
        let defs = character_defs(self.lang);
        let Some(i) = defs.iter().position(|d| d.id as i32 == id) else {
            return false;
        };
        let mut c = Character::from_def(&defs[i]);
        c.default_modifiers = CDF[i.min(8)];
        self.characters[num] = c;
        self.load_char_face_shapes(num, id);
        self.characters[num].next_anim_update_countdown = (self.roll_dice(1, 12, 0) + 6) as i16;
        for k in 0..11 {
            let it = self.characters[num].items[k];
            if it != 0 {
                let made = self.make_item(it as i32, 0, 0);
                self.characters[num].items[k] = made;
                self.run_item_script(num as i32, made, 0x80, 0, 0);
            }
        }
        self.calc_char_portrait_xpos();
        if num > 0 {
            self.set_temporary_face_frame(num, 2, 6, false);
        }
        true
    }

    /// The default modifiers of a character's kind (`defaultModifiers`).
    pub fn default_modifiers(&self, char_num: usize) -> &'static [u16; 9] {
        match self.characters[char_num].default_modifiers {
            1 => &CHAR_DEFS_WOMAN,
            2 => &CHAR_DEFS_KIERAN,
            3 => &CHAR_DEFS_AKSHEL,
            _ => &CHAR_DEFS_MAN,
        }
    }

    pub fn set_temporary_face_frame(
        &mut self,
        char_num: usize,
        frame: u8,
        update_delay: i32,
        redraw: bool,
    ) {
        self.characters[char_num].temp_face_frame = frame;
        if frame != 0 || update_delay != 0 {
            self.set_character_update_event(char_num, 6, update_delay, 1);
        }
        if redraw {
            self.gui_draw_char_portrait_with_stats(char_num);
        }
    }

    pub fn set_temporary_face_frame_for_all_characters(
        &mut self,
        frame: u8,
        update_delay: i32,
        redraw: bool,
    ) {
        for i in 0..4 {
            self.set_temporary_face_frame(i, frame, update_delay, false);
        }
        if redraw {
            self.gui_draw_all_char_portraits_with_stats();
        }
    }

    pub fn set_character_update_event(
        &mut self,
        char_num: usize,
        update_type: u8,
        update_delay: i32,
        overwrite: i32,
    ) {
        let now = self.millis();
        let l = &mut self.characters[char_num];
        for i in 0..5 {
            if l.character_update_events[i] != 0
                && (overwrite == 0 || l.character_update_events[i] != update_type)
            {
                continue;
            }
            l.character_update_events[i] = update_type;
            // uint8 in the original: truncated, not clamped.
            l.character_update_delay[i] = update_delay as u8;
            self.timers.set_next_run(3, now, now);
            self.timers.reset_next_run();
            self.timers.enable(3);
            break;
        }
    }

    pub fn count_active_characters(&self) -> usize {
        let mut i = 0;
        while i < 4 && self.characters[i].flags & 1 != 0 {
            i += 1;
        }
        i
    }

    pub fn load_char_face_shapes(&mut self, char_num: usize, id: i32) {
        let file = format!("FACE{:02}.SHP", id.unsigned_abs());
        let shapes = self.load_shape_file(&file);
        for i in 0..40 {
            if char_num < 3 {
                self.character_face_shapes[i][char_num] = shapes.get(i).cloned();
            }
        }
    }

    /// `updatePortraitSpeechAnim`.
    pub fn update_portrait_speech_anim(&mut self) {
        let c = self.update_char_num;
        if !(0..3).contains(&c) {
            return;
        }
        let c = c as usize;
        let (mut x, mut y, mut redraw) = (0, 0, false);
        match self.portrait_speech_anim_mode {
            0 => {
                x = self.active_chars_xpos[c] as i32;
                y = 144;
                redraw = true;
            }
            1 => {
                if self.text_enabled() {
                    x = 90;
                    y = 130;
                } else {
                    x = self.active_chars_xpos[c] as i32;
                    y = 144;
                }
            }
            2 => {
                if self.text_enabled() {
                    x = 16;
                    y = 134;
                } else {
                    x = self.active_chars_xpos[c] as i32 + 10;
                    y = 145;
                }
            }
            _ => {}
        }
        let mut f = self.roll_dice(1, 6, 0) - 1;
        if f == self.characters[c].cur_face_frame as i32 {
            f += 1;
        }
        if f > 5 {
            f -= 5;
        }
        f += 7;
        if self.speech_enabled() {
            if self.snd_update_character_speech() == 2 {
                if self.reset_portrait_after_speech_anim == 2 {
                    self.reset_portrait_after_speech_anim = 1;
                } else {
                    self.update_portrait_speech_anim_duration = 2;
                }
            } else {
                self.update_portrait_speech_anim_duration = 1;
            }
        } else if self.reset_portrait_after_speech_anim == 2 {
            self.reset_portrait_after_speech_anim = 1;
        }
        self.update_portrait_speech_anim_duration -= 1;
        if self.update_portrait_speech_anim_duration != 0 {
            self.characters[c].cur_face_frame = f as u8;
            if redraw {
                self.gui_draw_char_portrait_with_stats(c);
            } else {
                self.gui_draw_char_face_shape(c, x, y, 0);
            }
            self.update_portrait_next = self.millis() + 10 * TICK;
        } else if self.reset_portrait_after_speech_anim != 0 {
            self.face_frame_refresh(c);
            if redraw {
                self.gui_draw_char_portrait_with_stats(c);
                self.init_text_fading(0, false);
            } else {
                self.gui_draw_char_face_shape(c, x, y, 0);
            }
            self.update_char_num = -1;
        }
    }

    pub fn stop_portrait_speech_anim(&mut self) {
        if self.update_char_num == -1 {
            return;
        }
        self.update_portrait_speech_anim_duration = 1;
        self.reset_portrait_after_speech_anim = 2;
        self.update_portrait_speech_anim();
        self.update_portrait_speech_anim_duration = 1;
        self.update_char_num = -1;
        if self.portrait_speech_anim_mode == 0 {
            self.init_text_fading(0, false);
        }
    }

    pub fn init_text_fading(&mut self, text_type: i32, clear_field: bool) {
        if self.text_color_flag == text_type || text_type == 0 {
            self.fade_text = true;
            self.pal_update_timer = self.millis();
        }
        if !clear_field {
            return;
        }
        self.stop_portrait_speech_anim();
        if self.need_scene_restore != 0 {
            let d = self.txt_clear_dim(3);
            self.screen.set_screen_dim(d as usize);
        }
        self.fade_text = false;
        self.timers.disable(11);
    }

    pub fn face_frame_refresh(&mut self, char_num: usize) {
        let c = &self.characters[char_num];
        if c.cur_face_frame == 1 {
            self.set_temporary_face_frame(char_num, 0, 0, false);
        } else if c.cur_face_frame == 6 {
            if c.temp_face_frame != 5 {
                self.set_temporary_face_frame(char_num, 0, 0, false);
            } else {
                self.characters[char_num].cur_face_frame = 5;
            }
        } else {
            self.characters[char_num].cur_face_frame = 0;
        }
    }

    pub fn recalc_character_stats(&mut self, char_num: usize) {
        for i in 0..5 {
            self.char_stats_temp[i] = self.calculate_character_stats(char_num, i);
        }
    }

    /// `calculateCharacterStats`: 0 might, 1 protection, 2–4 the skills.
    pub fn calculate_character_stats(&self, char_num: usize, index: usize) -> i32 {
        let c = &self.characters[char_num];
        let dm = self.default_modifiers(char_num);
        match index {
            0 => {
                let mut v: i32 = c.items_might.iter().map(|&m| m as i32).sum();
                if v != 0 {
                    v += c.might as i32;
                } else {
                    v = dm[8] as i32;
                }
                v = (v * dm[1] as i32) >> 8;
                (v * c.total_might_modifier as i32) >> 8
            }
            1 => self.calculate_protection(char_num as i32),
            2..=4 => c.skill_levels[index - 2] as i32 + c.skill_modifiers[index - 2] as i32,
            _ => -1,
        }
    }

    pub fn calculate_protection(&self, index: i32) -> i32 {
        if index & 0x8000 != 0 {
            let m = &self.monsters[(index & 0x7FFF) as usize];
            let p = &self.monster_properties[m.properties.min(4)];
            (p.item_protection as i32 * p.fighting_stats[2] as i32) >> 8
        } else {
            let c = &self.characters[index as usize];
            let dm = self.default_modifiers(index as usize);
            let mut v = c.item_protection as i32 + c.protection as i32;
            v = (v * dm[2] as i32) >> 8;
            (v * c.total_protection_modifier as i32) >> 8
        }
    }

    /// `setCharacterMagicOrHitPoints`: the bar moves to the new value.
    pub fn set_character_magic_or_hit_points(
        &mut self,
        char_num: usize,
        type_: i32,
        points: i32,
        mode: i32,
    ) {
        const BAR: [[u16; 5]; 2] = [
            [0x27, 0x9A, 0x98, 0x01, 0x4254],
            [0x21, 0xA2, 0xA0, 0x00, 0x4253],
        ];
        if char_num > 2 {
            return;
        }
        let c = &self.characters[char_num];
        if c.flags & 1 == 0 {
            return;
        }
        let max = if type_ != 0 {
            c.magic_points_max as i32
        } else {
            c.hit_points_max as i32
        };
        let cur = if type_ != 0 {
            c.magic_points_cur as i32
        } else {
            c.hit_points_cur as i32
        };
        let new = match mode {
            2 => max + points,
            0 => points,
            _ => cur + points,
        }
        .clamp(0, max);
        let c = &mut self.characters[char_num];
        if type_ != 0 {
            c.magic_points_cur = new as i16;
        } else {
            c.hit_points_cur = new as i16;
            if c.hit_points_cur < 1 {
                c.flags |= 8;
            }
        }
        if self.update_flags & 2 != 0 || max == 0 {
            return;
        }
        let cf = self.screen.set_font(FontId::F6);
        let cp = self.screen.set_cur_page(0);
        let s = 8192 / max;
        let pmax = (s * max) >> 8;
        let pcur = (s * cur) >> 8;
        let mut nv = (s * new) >> 8;
        let nvs = nv.clamp(0, pmax);
        let mut step = if nv > pcur { 2 } else { -2 };
        nv = (nv + step).clamp(0, pmax);
        let bar = BAR[type_.clamp(0, 1) as usize];
        if nv != pcur {
            step = if nv >= pcur { 2 } else { -2 };
            let mut i = pcur;
            let mut guard = 0;
            while (i != nv || nv != nvs) && guard < 1000 {
                guard += 1;
                if (i - nv).abs() < step.abs() {
                    step >>= 1;
                }
                if step == 0 {
                    break;
                }
                i += step;
                let timer = self.millis() + TICK;
                let x = bar[0] as i32 + self.active_chars_xpos[char_num] as i32;
                self.gui_draw_live_magic_bar(
                    x,
                    175,
                    i,
                    0,
                    pmax,
                    5,
                    32,
                    bar[1] as u8,
                    1,
                    bar[3] != 0,
                );
                let label = self.get_lang_string(bar[4]);
                self.screen.print_text(&label, x, 144, bar[2] as u8, 0);
                self.screen.update_screen();
                if i == nv {
                    nv = nvs;
                    step = -step;
                }
                self.delay_until(timer, false);
            }
        }
        self.screen.set_font(cf);
        self.screen.set_cur_page(cp);
    }

    /// `increaseExperience`: levels gained bring hit and magic points.
    pub fn increase_experience(&mut self, char_num: usize, skill: usize, points: u32) {
        if char_num & 0x8000 != 0 || char_num > 3 || self.characters[char_num].flags & 8 != 0 {
            return;
        }
        self.characters[char_num].experience_pts[skill] += points as i32;
        loop {
            let lvl = self.characters[char_num].skill_levels[skill] as usize;
            let req = EXP_REQUIREMENTS.get(lvl).copied().unwrap_or(u32::MAX) as i64;
            if (self.characters[char_num].experience_pts[skill] as i64) < req {
                break;
            }
            self.characters[char_num].skill_levels[skill] += 1;
            self.characters[char_num].flags |= 0x200 << skill;
            let name = self.characters[char_num].name.clone();
            match skill {
                0 => {
                    let fmt = self.get_lang_string(0x4023);
                    self.print_message(0x8003, &fmt, &[Arg::Str(name)]);
                    let inc = self.roll_dice(4, 6, 0);
                    self.add_hit_points_max(char_num, inc);
                }
                1 => {
                    let fmt = self.get_lang_string(0x4025);
                    self.print_message(0x8003, &fmt, &[Arg::Str(name)]);
                    let inc = self.roll_dice(2, 6, 0);
                    self.add_hit_points_max(char_num, inc);
                }
                2 => {
                    let fmt = self.get_lang_string(0x4024);
                    self.print_message(0x8003, &fmt, &[Arg::Str(name)]);
                    let dm = self.default_modifiers(char_num)[6] as i32;
                    let inc = (dm * (self.roll_dice(1, 8, 0) + 17)) >> 8;
                    let c = &mut self.characters[char_num];
                    c.magic_points_cur += inc as i16;
                    c.magic_points_max = (c.magic_points_max as i32 + inc) as u16;
                    let inc = self.roll_dice(1, 6, 0);
                    self.add_hit_points_max(char_num, inc);
                }
                _ => {}
            }
            self.snd_play_sound_effect(118, -1);
            self.gui_draw_char_portrait_with_stats(char_num);
        }
    }

    fn add_hit_points_max(&mut self, char_num: usize, inc: i32) {
        let c = &mut self.characters[char_num];
        c.hit_points_cur += inc as i16;
        c.hit_points_max = (c.hit_points_max as i32 + inc) as u16;
    }

    pub fn increase_character_hitpoints(
        &mut self,
        char_num: usize,
        points: i32,
        ignore_death: bool,
    ) {
        let c = &mut self.characters[char_num];
        if c.hit_points_cur <= 0 && !ignore_death {
            return;
        }
        let points = points.max(1);
        // CLIP<int16>(cur + points, 1, max): the sum and the maximum are taken as int16.
        let v = (c.hit_points_cur as i32 + points) as i16;
        let max = c.hit_points_max as i16;
        c.hit_points_cur = if v < 1 {
            1
        } else if v > max {
            max
        } else {
            v
        };
        c.flags &= 0xFFF7;
    }

    pub fn setup_screen_dims(&mut self) {
        if self.text_enabled() {
            self.screen.modify_screen_dim(4, 11, 124, 28, 45);
            self.screen.modify_screen_dim(5, 85, 123, 233, 54);
        } else {
            self.screen.modify_screen_dim(4, 11, 124, 28, 9);
            self.screen.modify_screen_dim(5, 85, 123, 233, 18);
        }
    }

    pub fn init_scene_window_dialogue(&mut self, control_mode: i32) {
        self.reset_portraits_and_disable_sys_timer();
        self.gui_prepare_for_sequence(112, 0, 176, 120, control_mode);
        self.update_flags |= 3;
        self.txt_setup_field(true);
        self.txt_expand_field();
        self.setup_screen_dims();
        self.gui_disable_controls(control_mode);
    }

    pub fn toggle_selected_character_frame(&mut self, mode: bool) {
        if self.count_active_characters() == 1 {
            return;
        }
        let col = if mode { 212 } else { 1 };
        let cp = self.screen.set_cur_page(0);
        let x = self.active_chars_xpos[self.selected_character.clamp(0, 2) as usize] as i32;
        self.screen.draw_box(x, 143, x + 65, 176, col);
        self.screen.set_cur_page(cp);
    }

    pub fn gui_prepare_for_sequence(&mut self, x: i32, y: i32, w: i32, h: i32, button_flags: i32) {
        self.set_sequence_buttons(x, y, w, h, button_flags);
        self.seq_window = (x, y, x + w, y + h);
        let offs = if self.item_in_hand != 0 { 10 } else { 0 };
        let s = self.get_item_icon_shape(self.item_in_hand);
        self.screen.set_mouse_cursor(offs, offs, &s);
        self.last_mouse_region = -1;
        if w == 320 {
            self.set_lamp_mode(false);
            self.lamp_status_suspended = true;
        }
    }

    pub fn gui_special_scene_suspend_controls(&mut self, control_mode: i32) {
        if control_mode != 0 {
            self.update_flags |= 4;
            self.set_lamp_mode(false);
        }
        self.update_flags |= 1;
        self.special_scene_flag = 1;
        self.current_control_mode = control_mode;
        self.calc_char_portrait_xpos();
        self.check_floating_pointer_regions();
    }

    pub fn gui_special_scene_restore_controls(&mut self, restore_lamp: i32) {
        if restore_lamp != 0 {
            self.update_flags &= 0xFFFA;
            self.reset_lamp_status();
        }
        self.update_flags &= 0xFFFE;
        self.special_scene_flag = 0;
        self.check_floating_pointer_regions();
    }

    pub fn restore_after_scene_window_dialogue(&mut self, redraw: bool) {
        self.gui_enable_controls();
        self.txt_setup_field(false);
        self.update_flags &= 0xFFDF;
        self.set_default_button_state();
        for i in 0..6 {
            self.tim_free_anim_struct(i);
        }
        self.update_flags = 0;
        if redraw {
            if self.screen.fade_flag != 2 {
                self.screen.fade_clear_scene_window(10);
            }
            self.gui_draw_play_field();
            let p = self.screen.palette(0).clone();
            let (b, l) = (self.brightness, self.lamp_effect as i32);
            self.set_palette_brightness(&p, b, l);
            self.screen.fade_flag = 0;
        }
        self.need_scene_restore = 0;
        self.enable_sys_timer(2);
    }

    pub fn init_dialogue_sequence(&mut self, control_mode: i32, page: usize) {
        if control_mode != 0 {
            self.timers.disable(11);
            self.fade_text = false;
            let cp = self.screen.set_cur_page(page);
            self.screen.fill_rect(0, 128, 319, 199, 1, None);
            self.gui_draw_box(0, 129, 320, 71, 136, 251, -1);
            self.gui_draw_box(1, 130, 318, 69, 136, 251, 252);
            self.screen.modify_screen_dim(5, 8, 131, 306, 66);
            self.screen.modify_screen_dim(4, 1, 133, 38, 60);
            self.txt_clear_dim(4);
            self.update_flags |= 2;
            self.current_control_mode = control_mode;
            self.calc_char_portrait_xpos();
            if !self.text_enabled() && control_mode & 2 == 0 {
                let nc = self.count_active_characters();
                for i in 0..nc {
                    self.portrait_speech_anim_mode = 2;
                    self.update_char_num = i as i32;
                    let s = self.game_shapes[88].clone();
                    let x = self.active_chars_xpos[i] as i32 + 8;
                    self.screen
                        .draw_shape(0, &s, x, 142, 0, 0, ShapeArgs::none());
                    self.stop_portrait_speech_anim();
                }
            }
            self.screen.set_cur_page(cp);
        } else {
            self.txt_setup_field(true);
            self.txt_expand_field();
            self.setup_screen_dims();
            self.txt_clear_dim(4);
        }
        self.current_control_mode = control_mode;
        self.dialogue_field = true;
    }

    pub fn restore_after_dialogue_sequence(&mut self, control_mode: i32) {
        if !self.dialogue_field {
            return;
        }
        self.stop_portrait_speech_anim();
        self.current_control_mode = control_mode;
        self.calc_char_portrait_xpos();
        if self.current_control_mode != 0 {
            self.screen.modify_screen_dim(4, 11, 124, 28, 45);
            self.screen.modify_screen_dim(5, 85, 123, 233, 54);
            self.update_flags &= 0xFFFD;
        } else {
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
            self.txt_setup_field(false);
        }
        self.dialogue_field = false;
    }

    pub fn reset_portraits_and_disable_sys_timer(&mut self) {
        self.need_scene_restore = 1;
        if !self.text_enabled() || self.current_control_mode & 2 == 0 {
            self.timer_update_portrait_animations(1);
        }
        self.disable_sys_timer(2);
    }

    /// `fadeText`: the message's colour fades out.
    pub fn fade_text_step(&mut self) {
        if !self.fade_text {
            return;
        }
        let elapsed = (self.millis().wrapping_sub(self.pal_update_timer)) / TICK;
        if self.screen.fade_color(192, 252, elapsed, 60) {
            return;
        }
        if self.need_scene_restore != 0 {
            return;
        }
        let d = self.txt_clear_dim(3);
        self.screen.set_screen_dim(d as usize);
        self.timers.disable(11);
        self.fade_text = false;
    }

    pub fn set_palette_brightness(&mut self, src: &Pal, brightness: i32, modifier: i32) {
        let dst = self.generate_brightness_palette(src, brightness, modifier);
        *self.screen.palette_mut(1) = dst.clone();
        self.screen.fade_palette(&dst, 5);
        self.screen.fade_flag = 0;
    }

    pub fn generate_brightness_palette(
        &mut self,
        src: &Pal,
        brightness: i32,
        modifier: i32,
    ) -> Pal {
        let modifier = modifier as i16 as i32;
        let mut dst = src.clone();
        self.screen.load_special_colors(&mut dst);
        let mut b = (8 - brightness) << 5;
        if (0..8).contains(&modifier) && self.flags_table[31] & 0x08 != 0 {
            b = 256 - ((((modifier & 0xFFFE) << 5) * (256 - b)) >> 8);
            if b < 0 {
                b = 0;
            }
        }
        for i in 0..384 {
            dst.0[i] = ((dst.0[i] as i32 * b) >> 8) as u8;
        }
        dst
    }

    pub fn generate_flash_palette(&self, src: &Pal, color_flags: i32) -> Pal {
        let mut dst = src.clone();
        for i in 2..128 {
            for ii in 0..3 {
                let mut t = src.0[i * 3 + ii] & 0x3F;
                if color_flags & (1 << ii) != 0 {
                    t += (0x3F - t) >> 1;
                } else {
                    t -= t >> 1;
                }
                dst.0[i * 3 + ii] = t;
            }
        }
        dst
    }

    /// `loadTalkFile`: the speech archive of a level (`00.TLK` stays loaded).
    pub fn load_talk_file(&mut self, index: i32) {
        if index == self.cur_tlk_file {
            return;
        }
        if self.cur_tlk_file > 0 && index > 0 {
            let old = format!("{:02}.TLK", self.cur_tlk_file);
            self.res.unload_pak(&old);
        }
        if index > 0 {
            self.cur_tlk_file = index;
        }
        self.res.load_pak(&format!("{index:02}.TLK"));
    }

    /// `characterSays`.
    pub fn character_says(&mut self, track: i32, char_id: i32, redraw: bool) -> i32 {
        let mut char_id = char_id;
        if char_id == 1 {
            char_id = self.selected_character;
        }
        if char_id <= 0 {
            char_id = 0;
        } else {
            match (0..4).find(|&i| {
                char_id == self.characters[i].id as i32 && self.characters[i].flags & 1 != 0
            }) {
                Some(i) => char_id = i as i32,
                None => return 0,
            }
        }
        let r = self.snd_play_character_speech(track, char_id, false);
        if r && redraw {
            self.stop_portrait_speech_anim();
            self.update_char_num = char_id;
            self.portrait_speech_anim_mode = 0;
            self.reset_portrait_after_speech_anim = 1;
            self.fade_text = false;
            self.update_portrait_speech_anim();
        }
        if r {
            if self.text_enabled() { 1 } else { 0 }
        } else {
            1
        }
    }

    /// `playCharacterScriptChat`: a character speaks a line of a script (`params`: the
    /// arguments the line's `%d`/`%s` take, the third being the speech's number).
    pub fn play_character_script_chat(
        &mut self,
        char_id: i32,
        mode: i32,
        restore_portrait: i32,
        s: &[u8],
        params: &[i32],
        param_index: i32,
    ) -> i32 {
        let mut ch = 0;
        let mut skip_anim = false;
        let mut char_id = char_id;
        if char_id == -1 || char_id & 0x70 == 0 {
            char_id = if char_id == 1 {
                if self.selected_character != 0 {
                    self.characters[self.selected_character.clamp(0, 3) as usize].id as i32
                } else {
                    0
                }
            } else {
                char_id
            };
            ch = char_id;
        } else {
            char_id ^= 0x70;
        }
        self.stop_portrait_speech_anim();
        if char_id < 0 {
            let n = self.count_active_characters().max(1) as u32;
            char_id = self.rnd.get(n - 1) as i32;
            ch = char_id;
        } else if char_id > 0 {
            match (0..4).find(|&i| {
                self.characters[i].id as i32 == char_id && self.characters[i].flags & 1 != 0
            }) {
                Some(i) => {
                    if char_id == ch {
                        ch = i as i32;
                    }
                    char_id = i as i32;
                }
                None => {
                    if char_id == 8 {
                        skip_anim = true;
                    } else {
                        return 0;
                    }
                }
            }
        }
        if !skip_anim && char_id < 3 {
            self.update_char_num = char_id;
            self.portrait_speech_anim_mode = mode;
            self.update_portrait_speech_anim_duration = (s.len() >> 1) as i32;
            self.reset_portrait_after_speech_anim = restore_portrait;
        }
        if let Some(&track) = params.get(2) {
            self.snd_play_character_speech(track, ch, false);
        }
        if self.text_enabled() {
            match mode {
                0 => self.txt_print_dialogue_text2(3, s, params, param_index),
                1 => {
                    self.txt_clear_dim(4);
                    self.screen.modify_screen_dim(4, 16, 123, 23, 47);
                    self.txt_print_dialogue_text2(4, s, params, param_index);
                    self.screen.modify_screen_dim(4, 11, 123, 28, 47);
                }
                2 => {
                    self.txt_clear_dim(4);
                    self.screen.modify_screen_dim(4, 9, 133, 30, 60);
                    self.txt_print_dialogue_text2(4, s, params, 3);
                    self.screen.modify_screen_dim(4, 1, 133, 37, 60);
                }
                _ => {}
            }
        }
        self.fade_text = false;
        if !skip_anim && char_id < 3 {
            self.update_portrait_speech_anim();
        }
        1
    }

    /// `setupDialogueButtons`.
    pub fn setup_dialogue_buttons(&mut self, num: i32, s1: Vec<u8>, s2: Vec<u8>, s3: Vec<u8>) {
        self.screen.set_screen_dim(5);
        if num == 1 && self.speech_enabled() {
            self.dialogue_num_buttons = 0;
            self.dialogue_button_string = Default::default();
        } else {
            self.dialogue_num_buttons = num;
            self.dialogue_button_string = [s1, s2, s3];
            self.dialogue_highlighted_button = 0;
            let d = self.screen.dim(5);
            let py = (d.sy + d.h - 9) as u8;
            self.dialogue_button_pos_y = [py; 3];
            if num == 1 {
                let x = (d.sx + d.w - (self.dialogue_button_width as i32 + 3)) as u16;
                self.dialogue_button_pos_x = [x; 3];
            } else {
                let x_offs = d.w / num.max(1);
                let p0 = (d.sx + (x_offs >> 1) - 37) as u16;
                let p1 = p0.wrapping_add(x_offs as u16);
                let p2 = p1.wrapping_add(x_offs as u16);
                self.dialogue_button_pos_x = [p0, p1, p2];
            }
            self.draw_dialogue_buttons();
        }
        if !self.should_quit() {
            self.remove_input_top();
        }
    }

    /// `KyraRpgEngine::drawDialogueButtons`.
    pub fn draw_dialogue_buttons(&mut self) {
        let cp = self.screen.set_cur_page(0);
        let of = self.screen.set_font(FontId::F6);
        for i in 0..self.dialogue_num_buttons.clamp(0, 3) as usize {
            let x = self.dialogue_button_pos_x[i] as i32;
            let y = self.dialogue_button_y_offs + self.dialogue_button_pos_y[i] as i32;
            let w = self.dialogue_button_width as i32;
            // LoL's `_guiSettings`: frame 136/251, fill -1 (the box is not filled).
            self.gui_draw_box(x, y, w, 9, 136, 251, -1);
            let s = self.dialogue_button_string[i].clone();
            let tw = self.screen.text_width(&s);
            let col = if self.dialogue_highlighted_button == i as i32 {
                144
            } else {
                254
            };
            self.screen
                .print_text(&s, x + (w >> 1) - tw / 2, y + 2, col, 0);
        }
        self.screen.set_font(of);
        self.screen.set_cur_page(cp);
    }

    /// `KyraRpgEngine::processDialogue`: 0 while undecided, else the button chosen (1–3).
    pub fn process_dialogue(&mut self) -> i32 {
        let df = self.dialogue_highlighted_button;
        let mut res = 0;
        let w = self.dialogue_button_width as i32;
        let (mx, my) = self.get_mouse_pos();
        for i in 0..self.dialogue_num_buttons.clamp(0, 3) as usize {
            let x = self.dialogue_button_pos_x[i] as i32 + self.dialogue_button_x_offs;
            let y = self.dialogue_button_y_offs + self.dialogue_button_pos_y[i] as i32;
            if Self::pos_within_rect(mx, my, x, y, x + w, y + 9) {
                self.dialogue_highlighted_button = i as i32;
                break;
            }
        }
        if self.dialogue_num_buttons == 0 {
            let e = self.check_input(None, false, 0) & 0xFF;
            self.remove_input_top();
            if e != 0 {
                self.gui_notify_button_list_changed();
                if e == Self::KEY_SPACE as i32 || e == Self::KEY_RETURN as i32 {
                    self.snd_stop_speech(true);
                }
            }
            if self.snd_update_character_speech() != 2 {
                res = 1;
                if !self.should_quit() {
                    self.remove_input_top();
                    self.gui_notify_button_list_changed();
                }
            }
        } else {
            let e = self.check_input(None, false, 0) & 0xFF;
            self.remove_input_top();
            if e != 0 {
                self.gui_notify_button_list_changed();
            }
            if e == 200 || e == 202 {
                for i in 0..self.dialogue_num_buttons.clamp(0, 3) as usize {
                    let x = self.dialogue_button_pos_x[i] as i32;
                    let y = self.dialogue_button_y_offs + self.dialogue_button_pos_y[i] as i32;
                    let (mx, my) = self.get_mouse_pos();
                    if Self::pos_within_rect(mx, my, x, y, x + w, y + 9) {
                        self.dialogue_highlighted_button = i as i32;
                        res = i as i32 + 1;
                        break;
                    }
                }
            } else if e == Self::KEY_SPACE as i32 || e == Self::KEY_RETURN as i32 {
                self.snd_stop_speech(true);
                res = self.dialogue_highlighted_button + 1;
            } else if e == Self::KEY_LEFT as i32 || e == Self::KEY_DOWN as i32 {
                if self.dialogue_num_buttons > 1 && self.dialogue_highlighted_button > 0 {
                    self.dialogue_highlighted_button -= 1;
                }
            } else if (e == Self::KEY_RIGHT as i32 || e == Self::KEY_UP as i32)
                && self.dialogue_num_buttons > 1
                && self.dialogue_highlighted_button < self.dialogue_num_buttons - 1
            {
                self.dialogue_highlighted_button += 1;
            }
        }
        if df != self.dialogue_highlighted_button {
            self.draw_dialogue_buttons();
        }
        self.screen.update_screen();
        if res == 0 {
            return 0;
        }
        self.stop_portrait_speech_anim();
        if !self.text_enabled() && self.current_control_mode != 0 {
            self.screen.set_screen_dim(5);
            let d = self.screen.dim(5);
            self.screen.fill_rect(
                d.sx,
                d.sy + d.h - 9,
                d.sx + d.w - 1,
                d.sy + d.h - 1,
                d.col2 as u8,
                None,
            );
        } else {
            let d = self.screen.cur_dim();
            self.screen.fill_rect(
                d.sx,
                d.sy,
                d.sx + d.w - 2,
                d.sy + d.h - 1,
                d.col2 as u8,
                None,
            );
            self.txt_clear_dim(4);
            self.txt_reset_dim_text_positions(4);
        }
        res
    }

    pub fn give_item_to_monster(&mut self, monster: usize, item: Item) {
        let mut cur = self.monsters[monster].assigned_items;
        if cur == 0 {
            self.monsters[monster].assigned_items = item;
        } else {
            while self.items_in_play[cur as usize].obj.next_assigned_object != 0 {
                cur = self.items_in_play[cur as usize].obj.next_assigned_object;
            }
            self.items_in_play[cur as usize].obj.next_assigned_object = item;
        }
        self.items_in_play[item as usize].obj.next_assigned_object = 0;
    }

    /// `getCharacterOrMonsterStats`.
    pub fn get_character_or_monster_stats(&self, id: i32) -> [u16; 9] {
        if id & 0x8000 != 0 {
            let m = &self.monsters[(id & 0x7FFF) as usize];
            self.monster_properties[m.properties.min(4)].fighting_stats
        } else {
            *self.default_modifiers(id as usize)
        }
    }

    pub fn get_character_or_monster_items_might(&self, id: i32) -> [u16; 8] {
        if id & 0x8000 != 0 {
            let m = &self.monsters[(id & 0x7FFF) as usize];
            self.monster_properties[m.properties.min(4)].items_might
        } else {
            self.characters[id as usize].items_might
        }
    }

    pub fn get_character_or_monster_protection_against_items(&self, id: i32) -> [u16; 8] {
        if id & 0x8000 != 0 {
            let m = &self.monsters[(id & 0x7FFF) as usize];
            self.monster_properties[m.properties.min(4)].protection_against_items
        } else {
            self.characters[id as usize].protection_against_items
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_packed_strings() {
        // 0x80: pair index 0 → " " + "t".
        assert_eq!(decode_string1(&[b'a', 0x80, b'b', 0]), b"a tb");
        assert_eq!(
            decode_string2(&[b'x', 0x1B, 0x05, b'y']),
            vec![b'x', 0x84, b'y']
        );
    }
}
