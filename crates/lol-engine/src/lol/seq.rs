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

//! The sequences (ScummVM's `sequences_lol.cpp` and `MainMenu`): the title and its menu, the
//! intro, King Richard's choice of a hero, the "Lore of the Lands", the end and the credits.

use lol_data::tables::HISTORY;

use super::tim::{TimInterp, TimOpcodes, TimRef, WSA_SLOTS};
use super::types::*;
use super::wsa::Wsa;
use super::{Lol, TICK};
use crate::platform::Event;
use crate::screen::{CR_NO_P_CHECK, FontId, Pal, Screen};
use crate::shape::{self, ShapeArgs};

const CHAR_PREVIEWS: [(i32, i32, [i32; 3]); 4] = [
    (0x60, 0x7F, [0x0F, 0x08, 0x05]),
    (0x9A, 0x7F, [0x06, 0x0A, 0x0F]),
    (0xD4, 0x7F, [0x08, 0x06, 0x08]),
    (0x10F, 0x7F, [0x0A, 0x0C, 0x0A]),
];
const CHAR_PREVIEW_NAMES: [&str; 4] = ["Ak'shel", "Michael", "Kieran", "Conrad"];
const CHARGEN_FRAMES: [i32; 30] = [
    0, 1, 2, 3, 4, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17,
    18,
];
const SELECTION_POS: [i32; 32] = [
    0x6F, 0x00, 0x8F, 0x00, 0xAF, 0x00, 0xCF, 0x00, 0xEF, 0x00, 0x6F, 0x20, 0x8F, 0x20, 0xAF, 0x20,
    0xCF, 0x20, 0xEF, 0x20, 0x6F, 0x40, 0x8F, 0x40, 0xAF, 0x40, 0xCF, 0x40, 0xEF, 0x40, 0x10F,
    0x00,
];
const SEL_IDX: [[usize; 27]; 4] = [
    [
        0, 0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 0, 0, 5, 5, 5, 5, 5, 5, 5, 0, 0, 5, 5, 5, 5, 5,
    ],
    [
        1, 1, 6, 6, 1, 1, 6, 6, 6, 6, 6, 6, 6, 1, 1, 6, 6, 6, 1, 1, 6, 6, 6, 6, 6, 6, 6,
    ],
    [
        2, 2, 7, 7, 7, 7, 2, 2, 7, 7, 7, 7, 7, 7, 7, 2, 2, 7, 7, 7, 7, 2, 2, 7, 7, 7, 7,
    ],
    [
        3, 3, 8, 8, 8, 8, 3, 3, 8, 8, 3, 3, 8, 8, 8, 8, 8, 8, 8, 8, 8, 3, 3, 8, 8, 8, 8,
    ],
];
const REMINDER_IDX: [[usize; 17]; 4] = [
    [4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5],
    [9, 9, 9, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6],
    [0xE, 0xE, 0xE, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7],
    [0xF, 0xF, 0xF, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8],
];
const SELECTION_ANIM_INDEX: [usize; 8] = [0, 5, 1, 6, 2, 7, 3, 8];
const CHAR_INFO_FRAMES: [usize; 32] = [
    0, 7, 8, 9, 10, 11, 10, 9, 8, 7, 0, 0, 7, 8, 9, 10, 11, 10, 9, 8, 7, 0, 0, 7, 8, 9, 10, 11, 10,
    9, 8, 7,
];
const OUTRO_SHAPE_FILES: [&str; 28] = [
    "AMAZON.SHP",
    "ARCHRSLG.SHP",
    "AVIANWRM.SHP",
    "BANDIT.SHP",
    "BOAR.SHP",
    "CABAL.SHP",
    "GUARD.SHP",
    "HAG.SHP",
    "HORNET.SHP",
    "HURZELL.SHP",
    "IRONGRZR.SHP",
    "KNOWLES.SHP",
    "LIZARD.SHP",
    "MANTHA.SHP",
    "MINOTAUR.SHP",
    "MORIBUND.SHP",
    "ORC.SHP",
    "ORCLDR.SHP",
    "PENTROG.SHP",
    "RATMAN.SHP",
    "ROCKLING.SHP",
    "SCAVNGR.SHP",
    "STARK.SHP",
    "SWAMPCIT.SHP",
    "SWAMPMON.SHP",
    "THUG.SHP",
    "VIPER.SHP",
    "XEOB.SHP",
];
const OUTRO_FRAMES: [usize; 24] = [
    0, 0, 0, 0, 0, 1, 2, 3, 0, 1, 2, 3, 8, 9, 10, 11, 8, 9, 10, 11, 4, 5, 6, 7,
];
const OUTRO_RIGHT_MONSTER: [i32; 48] = [
    205, 55, 205, 55, 205, 55, 205, 55, 205, 56, 207, 57, 208, 58, 210, 59, 213, 60, 216, 61, 220,
    61, 225, 61, 230, 61, 235, 61, 240, 61, 240, 61, 240, 61, 240, 61, 240, 61, 240, 61, 240, 61,
    265, 61, 290, 61, 315, 61,
];
const OUTRO_LEFT_MONSTER: [i32; 48] = [
    92, 55, 92, 55, 92, 55, 92, 55, 92, 56, 90, 57, 85, 58, 77, 59, 67, 60, 57, 61, 47, 61, 35, 61,
    35, 61, 35, 61, 35, 61, 35, 61, 35, 61, 35, 61, 35, 61, 35, 61, 35, 61, 10, 61, -20, 61, -45,
    61,
];
const OUTRO_RIGHT_DOOR: [i32; 48] = [
    200, 41, 200, 29, 200, 17, 200, 5, 200, -7, 200, -7, 200, -7, 200, -7, 200, 5, 200, 17, 200,
    29, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41, 200, 41,
    200, 41, 200, 41, 200, 41,
];
const OUTRO_LEFT_DOOR: [i32; 48] = [
    72, 41, 72, 29, 72, 17, 72, 5, 72, -7, 72, -7, 72, -7, 72, -7, 72, 5, 72, 17, 72, 29, 72, 41,
    72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41, 72, 41,
];
const OUTRO_SCALE_X: [i32; 24] = [
    0x50, 0x50, 0x50, 0x50, 0x50, 0x5D, 0x70, 0x85, 0xA0, 0xC0, 0xE2, 0x100, 0x100, 0x100, 0x100,
    0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100,
];
const OUTRO_SCALE_Y: [i32; 24] = [
    0x4C, 0x4C, 0x4C, 0x4C, 0x4C, 0x59, 0x6B, 0x80, 0x99, 0xB8, 0xD9, 0x100, 0x100, 0x100, 0x100,
    0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100, 0x100,
];

const PROLOGUE_PAKS: [&str; 13] = [
    "GENERAL.PAK",
    "INTROVOC.PAK",
    "STARTUP.PAK",
    "INTRO1.PAK",
    "INTRO2.PAK",
    "INTRO3.PAK",
    "INTRO4.PAK",
    "INTRO5.PAK",
    "INTRO6.PAK",
    "INTRO7.PAK",
    "INTRO8.PAK",
    "INTRO9.PAK",
    "HISTORY.PAK",
];
const EPILOGUE_PAKS: [&str; 6] = [
    "GENERAL.PAK",
    "INTROVOC.PAK",
    "STARTUP.PAK",
    "FINALE.PAK",
    "FINALE1.PAK",
    "FINALE2.PAK",
];

/// What the character selection keeps (`_chargenWSA`, the selection's state).
#[derive(Default)]
pub struct SelectionState {
    pub chargen: Option<Wsa>,
    pub info_result: i32,
    pub anim_frames: [usize; 4],
    pub anim_timers: [u32; 4],
}

impl Lol {
    /// `processPrologue`: the title and its menu; 0 a new game, 3 a game loaded, -1 quit.
    pub fn process_prologue(&mut self) -> i32 {
        self.res.load_pak("GENERAL.PAK");
        self.setup_prologue_data(true);
        if self.savegame_list().is_empty() {
            self.show_intro();
        }
        self.pre_init();
        let version = format!("RS {}", env!("CARGO_PKG_VERSION"));
        let mut process_selection = -1;
        while !self.should_quit() && process_selection == -1 {
            if let Some(d) = self.res.file_data("TITLE.CPS") {
                let mut p0 = self.screen.palette(0).clone();
                self.screen.load_bitmap(&d, 2, Some(&mut p0));
                *self.screen.palette_mut(0) = p0;
            }
            self.screen
                .copy_region(0, 0, 0, 0, 320, 200, 2, 0, CR_NO_P_CHECK);
            self.screen.set_font(FontId::F6);
            let w = self.screen.text_width(version.as_bytes());
            self.screen
                .fprint_string(version.as_bytes(), 320 - w, 193, 0x67, 0x00, 0x04);
            self.screen.set_font(FontId::F9);
            let p0 = self.screen.palette(0).clone();
            self.screen.fade_palette(&p0, 0x1E);
            self.screen.update_screen();
            self.event_list.clear();
            let selection = self.title_main_menu();
            if selection != 3 {
                self.screen.hide_mouse();
                *self.screen.palette_mut(0) = Pal::default();
                self.screen.fade_to_black(0x54);
            }
            match selection {
                -1 => {}
                0 => process_selection = 0,
                1 => self.show_intro(),
                2 => self.play_history(),
                3 => {
                    if self.run_menu(MenuCb::LoadMenu) != 0 && self.game_to_load != -1 {
                        process_selection = 3;
                    }
                }
                _ => {
                    self.quit_game();
                    self.update_input();
                }
            }
        }
        if process_selection == 0 {
            self.restart_play_timer_at(0);
            self.snd_load_sound_file_named("LOREINTR");
            self.screen.system.mixer().play_music(6);
            self.choose_character();
            self.screen.system.mixer().play_music(1);
            self.screen.fade_to_black(0x54);
        }
        self.setup_prologue_data(false);
        process_selection
    }

    pub fn setup_prologue_data(&mut self, load: bool) {
        for f in PROLOGUE_PAKS {
            if load {
                self.res.load_pak(f);
            } else {
                self.res.unload_pak(f);
            }
        }
        self.screen.clear_page(0);
        self.screen.clear_page(3);
        if load {
            self.selection = SelectionState {
                info_result: -1,
                anim_frames: [0, 1, 0, 1],
                ..Default::default()
            };
            *self.screen.palette_mut(1) = Pal::default();
            self.snd_load_sound_file_named("LOREINTR");
        } else {
            self.selection.chargen = None;
            *self.screen.palette_mut(0) = Pal::default();
            let p0 = self.screen.palette(0).clone();
            self.screen.set_screen_palette(&p0);
            if self.should_quit() {
                return;
            }
            self.event_list.clear();
        }
    }

    /// `mainMenu` with `MainMenu::handle`.
    fn title_main_menu(&mut self) -> i32 {
        let has_save = !self.savegame_list().is_empty();
        let ids: [u16; 5] = if has_save {
            [0x4248, 0x4249, 0x42DD, 0x4001, 0x424A]
        } else {
            [0x4248, 0x4249, 0x42DD, 0x424A, 0]
        };
        let strings: Vec<Vec<u8>> = ids
            .iter()
            .map(|&i| {
                if i != 0 {
                    self.get_lang_string(i)
                } else {
                    Vec::new()
                }
            })
            .collect();
        let count = if has_save { 5 } else { 4 };
        let dim = if has_save { 19 } else { 6 };
        const MENU_TABLE: [i32; 7] = [0x01, 0x04, 0x0C, 0x04, 0x00, 0x3D, 0x9F];
        const COLORS: [u8; 4] = [0x2C, 0x19, 0x48, 0x2C];
        self.screen.set_text_color_map(&[0; 16]);
        let old_font = self.screen.set_font(FontId::F9);
        self.screen.set_screen_dim(dim);
        let d = self.screen.cur_dim();
        self.screen
            .copy_region(d.sx, d.sy, d.sx, d.sy, d.w, d.h, 0, 3, 0);
        let (x, y, width, height) = (d.sx << 3, d.sy, d.w << 3, d.h);
        let draw_box = |s: &mut Screen, x: i32, y: i32, w: i32, h: i32, fill: bool| {
            let (w, h) = (w - 1, h - 1);
            if fill {
                s.fill_rect(x, y, x + w, y + h, COLORS[0], None);
            }
            s.draw_clipped_line(x, y + h, x + w, y + h, COLORS[1]);
            s.draw_clipped_line(x + w, y, x + w, y + h, COLORS[1]);
            s.draw_clipped_line(x, y, x + w, y, COLORS[2]);
            s.draw_clipped_line(x, y, x, y + h, COLORS[2]);
            let p = s.cur_page;
            s.set_page_pixel(p, x, y + h, COLORS[3]);
            s.set_page_pixel(p, x + w, y, COLORS[3]);
        };
        draw_box(&mut self.screen, x, y, width, height, true);
        draw_box(&mut self.screen, x + 1, y + 1, width - 2, height - 2, false);
        let fh = self.screen.font_height();
        let text_pos = ((d.w >> 1) + d.sx) << 3;
        let print = |s: &mut Screen, t: &[u8], x: i32, y: i32, col: i32| {
            let x = x - (s.text_width(t) >> 1);
            s.print_text(t, x - 1, y, 1, 0);
            s.print_text(t, x, y + 1, 1, 0);
            s.print_text(t, x, y, col as u8, 0);
        };
        let top = d.sy + MENU_TABLE[1];
        for (i, s) in strings.iter().enumerate().take(count) {
            let col = if i == 0 { MENU_TABLE[6] } else { MENU_TABLE[5] };
            print(&mut self.screen, s, text_pos, top + i as i32 * fh, col);
        }
        while !self.screen.is_mouse_visible() {
            self.screen.show_mouse();
        }
        let (rx1, ry1, rx2, ry2) = (x + 16, y + 4, x + width - 16, y + 4 + fh * count as i32);
        let mut selected = 0usize;
        let mut command = -1;
        while !self.should_quit() {
            self.screen.update_screen();
            self.update_input();
            let pressed = self
                .event_list
                .iter()
                .any(|e| matches!(e.event, Event::MouseUp { button: 0, .. }));
            self.event_list.clear();
            let (mx, my) = self.get_mouse_pos();
            if mx >= rx1 && mx < rx2 && my >= ry1 && my < ry2 {
                let item = (((my - ry1) / fh.max(1)) as usize).min(count - 1);
                if item != selected {
                    print(
                        &mut self.screen,
                        &strings[selected],
                        text_pos,
                        ry1 + selected as i32 * fh,
                        MENU_TABLE[5],
                    );
                    print(
                        &mut self.screen,
                        &strings[item],
                        text_pos,
                        ry1 + item as i32 * fh,
                        MENU_TABLE[6],
                    );
                    selected = item;
                }
                if pressed {
                    for _ in 0..3 {
                        print(
                            &mut self.screen,
                            &strings[selected],
                            text_pos,
                            ry1 + selected as i32 * fh,
                            MENU_TABLE[5],
                        );
                        self.screen.update_screen();
                        self.screen.system.delay_millis(50);
                        print(
                            &mut self.screen,
                            &strings[selected],
                            text_pos,
                            ry1 + selected as i32 * fh,
                            MENU_TABLE[6],
                        );
                        self.screen.update_screen();
                        self.screen.system.delay_millis(50);
                    }
                    command = item as i32;
                    break;
                }
            }
            self.screen.system.delay_millis(10);
        }
        if self.should_quit() {
            command = -1;
        }
        self.screen
            .copy_region(d.sx, d.sy, d.sx, d.sy, d.w, d.h, 3, 0, 0);
        self.screen.set_font(old_font);
        self.screen.set_screen_dim(0);
        if !has_save && command == 3 {
            command = 4;
        }
        command
    }

    /// A sequence's own TIM interpreter, swapped in for the game's.
    fn with_seq_interp(&mut self, f: impl FnOnce(&mut Self)) {
        let saved = std::mem::replace(&mut self.tim, TimInterp::new(false));
        f(self);
        self.tim = saved;
    }

    /// The loop that runs a sequence's script (`showIntro`, `showOutro`).
    fn run_seq_tim(&mut self, update_pages: bool) {
        let mut pal_next = 0u32;
        while !self.tim.finished && !self.should_quit() && !self.skip_flag() {
            self.update_input();
            self.tim_exec(TimRef::Seq, false);
            if update_pages {
                self.screen.checked_page_update(8, 4);
            }
            if self.tim.pal_diff != 0 && pal_next < self.millis() {
                self.tim.pal_delay_acc += self.tim.pal_delay_inc;
                pal_next = self.millis() + ((self.tim.pal_delay_acc >> 8) as u32 * TICK);
                self.tim.pal_delay_acc &= 0xFF;
                let p0 = self.screen.palette(0).clone();
                if !self.screen.fade_pal_step(&p0, self.tim.pal_diff) {
                    self.screen.set_screen_palette(&p0);
                    self.tim.pal_diff = 0;
                }
            }
            self.screen.system.delay_millis(10);
            self.screen.update_screen();
        }
    }

    pub fn show_intro(&mut self) {
        self.with_seq_interp(|s| {
            *s.screen.palette_mut(0) = Pal::default();
            let p0 = s.screen.palette(0).clone();
            s.screen.set_screen_palette(&p0);
            s.screen.clear_page(0);
            s.screen.clear_page(4);
            s.screen.clear_page(8);
            s.seq_tim = s.tim_load("LOLINTRO.TIM", TimOpcodes::Intro);
            if let Some(d) = s.res.file_data("NEW8P.FNT") {
                s.screen.load_font(FontId::F8, &d);
            }
            if let Some(d) = s.res.file_data("INTRO.FNT") {
                s.screen.load_font(FontId::Intro, &d);
            }
            s.screen.set_font(FontId::F8);
            s.tim.finished = false;
            s.tim.lang_data = s.res.file_data("LOLINTRO.DIP");
            s.screen.hide_mouse();
            s.run_seq_tim(true);
            s.screen.show_mouse();
            s.screen
                .system
                .mixer()
                .stop_voices(crate::audio::VoiceKind::Speech);
            s.screen
                .system
                .mixer()
                .stop_voices(crate::audio::VoiceKind::Effect);
            s.screen.system.mixer().fade_out_music();
            s.event_list.clear();
            s.seq_tim = None;
            s.tim.lang_data = None;
            for i in 0..WSA_SLOTS {
                s.tim_free_anim_struct(i);
            }
        });
        let p1 = self.screen.palette(1).clone();
        self.screen.fade_palette(&p1, 30);
    }

    fn tim_table(&self, i: u16) -> Vec<u8> {
        self.tim.table_entry(i).unwrap_or_default()
    }

    fn chargen_frame(&mut self, frame: i32) {
        if let Some(w) = self.selection.chargen.as_mut() {
            w.display_frame(&mut self.screen, frame, 0, 113, 0, 0, None, None);
        }
    }

    /// `chooseCharacter`.
    pub fn choose_character(&mut self) -> i32 {
        self.with_seq_interp(|s| s.choose_character_inner());
        self.char_selection
    }

    fn choose_character_inner(&mut self) {
        self.tim.lang_data = self.res.file_data("LOLINTRO.DIP");
        if let Some(d) = self.res.file_data("FONT9P.FNT") {
            self.screen.load_font(FontId::F9, &d);
        }
        if let Some(d) = self.res.file_data("ITEMICN.SHP") {
            self.screen.load_bitmap(&d, 3, None);
            let p = self.screen.page(3).to_vec();
            let c = shape::shape_copy(&p, 0).unwrap_or_default();
            self.screen.set_mouse_cursor(0, 0, &c);
        }
        while !self.screen.is_mouse_visible() {
            self.screen.show_mouse();
        }
        let mut p0 = self.screen.palette(0).clone();
        if let Some(d) = self.res.file_data("CHAR.CPS") {
            self.screen.load_bitmap(&d, 2, Some(&mut p0));
        }
        if let Some(d) = self.res.file_data("BACKGRND.CPS") {
            self.screen.load_bitmap(&d, 4, Some(&mut p0));
        }
        *self.screen.palette_mut(0) = p0;
        self.selection.chargen = self.open_wsa("CHARGEN.WSA", 1, false);
        if let Some(w) = self.selection.chargen.as_mut() {
            w.display_frame(&mut self.screen, 0, 2, 113, 0, 0, None, None);
        }
        self.screen.set_font(FontId::F9);
        self.screen.cur_page = 2;
        for (i, (x, y, attrib)) in CHAR_PREVIEWS.iter().enumerate() {
            self.screen.fprint_string_intro(
                CHAR_PREVIEW_NAMES[i].as_bytes(),
                x + 16,
                y + 36,
                0xC0,
                0x00,
                0x9C,
                0x120,
            );
            for (k, a) in attrib.iter().enumerate() {
                let s = a.to_string();
                self.screen.fprint_string_intro(
                    s.as_bytes(),
                    x + 21,
                    y + 48 + 8 * k as i32,
                    0x98,
                    0x00,
                    0x9C,
                    0x220,
                );
            }
        }
        for (k, idx) in [51u16, 53, 55].iter().enumerate() {
            let t = self.tim_table(*idx);
            self.screen
                .fprint_string_intro(&t, 36, 173 + 8 * k as i32, 0x98, 0x00, 0x9C, 0x20);
        }
        self.screen
            .copy_region(0, 0, 0, 0, 320, 200, 2, 0, CR_NO_P_CHECK);
        self.screen.cur_page = 0;
        let p0 = self.screen.palette(0).clone();
        self.screen.fade_palette(&p0, 30);
        let mut king_intro = true;
        while !self.should_quit() {
            if king_intro {
                self.king_selection_intro();
            }
            if self.char_selection < 0 {
                self.process_character_selection();
            }
            if self.should_quit() {
                break;
            }
            if self.char_selection == 100 {
                king_intro = true;
                self.char_selection = -1;
                continue;
            }
            self.screen
                .copy_region(0, 0, 0, 0, 112, 120, 4, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            self.screen.show_mouse();
            let c = self.char_selection;
            if self.selection_char_info(c) == -1 {
                self.char_selection = -1;
                king_intro = false;
            } else {
                break;
            }
            self.delay(10, false);
        }
        if self.should_quit() {
            return;
        }
        let wait = self.millis() + 420 * TICK;
        while wait > self.millis() && !self.skip_flag() && !self.should_quit() {
            self.update_input();
            self.screen.system.delay_millis(10);
        }
        self.event_list.clear();
        self.tim.lang_data = None;
    }

    fn speech_playing(&self) -> bool {
        self.speech_handle != 0 && self.screen.system.mixer().voice_playing(self.speech_handle)
    }

    fn play_speech_file(&mut self, file: &str) {
        let Some((s, r)) = self.voice_samples(file) else {
            self.speech_handle = 0;
            return;
        };
        self.speech_handle = self
            .screen
            .system
            .mixer()
            .play_voice_stream(s, r, 255, 255, crate::audio::VoiceKind::Speech)
            .unwrap_or(0);
    }

    fn stop_speech_handle(&mut self) {
        let h = self.speech_handle;
        self.screen.system.mixer().stop_voice(h);
    }

    fn copy_previews(&mut self, idx: [usize; 4]) {
        for (k, &i) in idx.iter().enumerate() {
            let (x, y, _) = CHAR_PREVIEWS[k];
            self.screen.copy_region(
                SELECTION_POS[i * 2],
                SELECTION_POS[i * 2 + 1],
                x,
                y,
                32,
                32,
                4,
                0,
                0,
            );
        }
    }

    fn king_selection_intro(&mut self) {
        self.screen
            .copy_region(0, 0, 0, 0, 112, 120, 4, 0, CR_NO_P_CHECK);
        for i in 0..5 {
            let t = self.tim_table(57 + i);
            self.screen
                .fprint_string_intro(&t, 8, 38 + i as i32 * 10, 0x32, 0x00, 0x9C, 0x20);
        }
        self.play_speech_file("KING01");
        let mut index = 4usize;
        while (!self.speech_enabled() || self.speech_playing())
            && self.char_selection == -1
            && !self.should_quit()
            && !self.skip_flag()
        {
            index = index.max(4);
            self.chargen_frame(CHARGEN_FRAMES[index]);
            self.copy_previews([
                SEL_IDX[0][index],
                SEL_IDX[1][index],
                SEL_IDX[2][index],
                SEL_IDX[3][index],
            ]);
            self.screen.update_screen();
            let wait = self.millis() + 7 * TICK;
            while wait > self.millis()
                && self.char_selection == -1
                && !self.should_quit()
                && !self.skip_flag()
            {
                self.char_selection = self.get_char_selection();
                self.screen.system.delay_millis(10);
            }
            if self.speech_enabled() {
                index = (index + 1) % 22;
            } else {
                index += 1;
                if index >= 27 {
                    break;
                }
            }
        }
        self.reset_skip_flag(true);
        self.chargen_frame(0x10);
        self.screen.update_screen();
        self.stop_speech_handle();
    }

    fn king_selection_reminder(&mut self) {
        self.screen
            .copy_region(0, 0, 0, 0, 112, 120, 4, 0, CR_NO_P_CHECK);
        let t = self.tim_table(62);
        self.screen
            .fprint_string_intro(&t, 8, 48, 0x32, 0x00, 0x9C, 0x20);
        let t = self.tim_table(63);
        self.screen
            .fprint_string_intro(&t, 8, 58, 0x32, 0x00, 0x9C, 0x20);
        self.play_speech_file("KING02");
        let mut index = 0usize;
        while (!self.speech_enabled() || self.speech_playing())
            && self.char_selection == -1
            && !self.should_quit()
            && index < 15
        {
            self.chargen_frame(CHARGEN_FRAMES[index + 9]);
            self.copy_previews([
                REMINDER_IDX[0][index],
                REMINDER_IDX[1][index],
                REMINDER_IDX[2][index],
                REMINDER_IDX[3][index],
            ]);
            self.screen.update_screen();
            let wait = self.millis() + 8 * TICK;
            while wait > self.millis() && !self.should_quit() {
                self.char_selection = self.get_char_selection();
                self.screen.system.delay_millis(10);
            }
            if self.speech_enabled() {
                index = (index + 1) % 22;
            } else {
                index += 1;
                if index >= 27 {
                    break;
                }
            }
        }
        self.stop_speech_handle();
    }

    fn king_selection_outro(&mut self) {
        self.play_speech_file("KING03");
        let mut index = 0usize;
        while (!self.speech_enabled() || self.speech_playing())
            && !self.should_quit()
            && !self.skip_flag()
        {
            index = index.max(4);
            self.chargen_frame(CHARGEN_FRAMES[index]);
            self.screen.update_screen();
            let wait = self.millis() + 8 * TICK;
            while wait > self.millis() && !self.should_quit() && !self.skip_flag() {
                self.update_input();
                self.screen.system.delay_millis(10);
            }
            if self.speech_enabled() {
                index = (index + 1) % 22;
            } else {
                index += 1;
                if index >= 27 {
                    break;
                }
            }
        }
        self.reset_skip_flag(true);
        self.chargen_frame(0x10);
        self.screen.update_screen();
        self.stop_speech_handle();
    }

    fn process_character_selection(&mut self) {
        self.char_selection = -1;
        while !self.should_quit() && self.char_selection == -1 {
            let next = self.millis() + 900 * TICK;
            while next > self.millis() && self.char_selection == -1 && !self.should_quit() {
                self.update_selection_anims();
                self.char_selection = self.get_char_selection();
                self.screen.system.delay_millis(10);
            }
            if self.char_selection == -1 {
                self.king_selection_reminder();
            }
        }
    }

    fn update_selection_anims(&mut self) {
        for i in 0..4 {
            if self.millis() < self.selection.anim_timers[i] {
                continue;
            }
            let index = SELECTION_ANIM_INDEX[self.selection.anim_frames[i] + i * 2];
            let (x, y, _) = CHAR_PREVIEWS[i];
            self.screen.copy_region(
                SELECTION_POS[index * 2],
                SELECTION_POS[index * 2 + 1],
                x,
                y,
                32,
                32,
                4,
                0,
                0,
            );
            let delay = if self.selection.anim_frames[i] == 1 {
                self.rnd.range(0, 31) + 80
            } else {
                self.rnd.range(0, 3) + 10
            };
            self.selection.anim_timers[i] = self.millis() + delay as u32 * TICK;
            self.selection.anim_frames[i] = (self.selection.anim_frames[i] + 1) % 2;
        }
        self.screen.update_screen();
    }

    fn selection_char_info(&mut self, character: i32) -> i32 {
        if character < 0 {
            return -1;
        }
        let (file, voc) = match character {
            0 => ("FACE09.SHP", 'A'),
            1 => ("FACE01.SHP", 'M'),
            2 => ("FACE08.SHP", 'K'),
            _ => ("FACE05.SHP", 'C'),
        };
        let faces = self.res.file_data(file).map(|d| {
            self.screen.load_bitmap(&d, 9, None);
            self.screen.page(9).to_vec()
        });
        self.screen
            .copy_region(0, 122, 0, 122, 320, 78, 4, 0, CR_NO_P_CHECK);
        let (px, py, _) = CHAR_PREVIEWS[character as usize];
        self.screen
            .copy_region(px - 3, py - 3, 8, 127, 38, 38, 2, 0, 0);
        const INFO_IDX: [u16; 4] = [0x1D, 0x22, 0x27, 0x2C];
        let idx = INFO_IDX[character as usize];
        for i in 0..5 {
            let t = self.tim_table(idx + i);
            self.screen
                .fprint_string_intro(&t, 50, 127 + i as i32 * 10, 0x53, 0x00, 0xCF, 0x20);
        }
        let t = self.tim_table(69);
        self.screen
            .fprint_string_intro(&t, 100, 168, 0x32, 0x00, 0xCF, 0x20);
        let faces = faces.unwrap_or_default();
        self.selection_char_info_intro(voc, &faces);
        if self.selection.info_result == -1 {
            while self.selection.info_result == -1 && !self.should_quit() {
                self.selection.info_result = self.selection_char_accept();
                self.screen.system.delay_millis(10);
            }
        }
        if self.selection.info_result != 1 {
            self.selection.info_result = -1;
            self.screen
                .copy_region(0, 122, 0, 122, 320, 78, 2, 0, CR_NO_P_CHECK);
            self.screen.update_screen();
            return -1;
        }
        self.screen
            .copy_region(48, 127, 48, 127, 272, 60, 4, 0, CR_NO_P_CHECK);
        self.screen.hide_mouse();
        self.screen
            .copy_region(48, 127, 48, 160, 272, 35, 4, 0, CR_NO_P_CHECK);
        self.screen
            .copy_region(0, 0, 0, 0, 112, 120, 4, 0, CR_NO_P_CHECK);
        for i in 0..5 {
            let t = self.tim_table(64 + i);
            self.screen
                .fprint_string_intro(&t, 3, 28 + i as i32 * 10, 0x32, 0x00, 0x9C, 0x20);
        }
        self.reset_skip_flag(true);
        self.king_selection_outro();
        character
    }

    fn selection_char_info_intro(&mut self, voc: char, faces: &[u8]) {
        let mut index = 0u8;
        let mut process_anim = true;
        let face = |i: usize| shape::shape_copy(faces, i).unwrap_or_default();
        while self.selection.info_result == -1 && !self.should_quit() {
            let file = format!("000{voc}{}", (b'0' + index) as char);
            if self.speech_enabled() && !self.is_voice_present(&file) {
                break;
            }
            self.play_speech_file(&file);
            let mut i = 0usize;
            while (!self.speech_enabled() || self.speech_playing())
                && self.selection.info_result == -1
                && !self.should_quit()
            {
                let f = face(CHAR_INFO_FRAMES[i]);
                self.screen
                    .draw_shape(0, &f, 11, 130, 0, 0, ShapeArgs::none());
                self.screen.update_screen();
                let next = self.millis() + 8 * TICK;
                while next > self.millis()
                    && self.selection.info_result == -1
                    && !self.should_quit()
                {
                    self.selection.info_result = self.selection_char_accept();
                    self.screen.system.delay_millis(10);
                }
                if self.speech_enabled() || process_anim {
                    i = (i + 1) % 32;
                }
                if i == 0 {
                    process_anim = false;
                }
            }
            self.stop_speech_handle();
            index += 1;
            if index > 9 {
                break;
            }
        }
        let f = face(0);
        self.screen
            .draw_shape(0, &f, 11, 130, 0, 0, ShapeArgs::none());
        self.screen.update_screen();
    }

    fn get_char_selection(&mut self) -> i32 {
        let input = self.check_input(None, false, 0) & 0xCF;
        self.remove_input_top();
        if input == 200 {
            for (i, (x, y, _)) in CHAR_PREVIEWS.iter().enumerate() {
                if *x <= self.mouse_x
                    && self.mouse_x <= x + 31
                    && *y <= self.mouse_y
                    && self.mouse_y <= y + 31
                {
                    return i as i32;
                }
            }
        }
        -1
    }

    fn selection_char_accept(&mut self) -> i32 {
        let input = self.check_input(None, false, 0) & 0xCF;
        self.remove_input_top();
        if input == 200 {
            let (mx, my) = (self.mouse_x, self.mouse_y);
            if (88..=128).contains(&mx) && (180..=194).contains(&my) {
                return 1;
            }
            if (196..=236).contains(&mx) && (180..=194).contains(&my) {
                return 0;
            }
        }
        -1
    }

    // ---- the Lore of the Lands

    /// `HistoryPlayer::play`.
    pub fn play_history(&mut self) {
        let data = &HISTORY;
        if let Some(d) = self.res.file_data("FONT9P.FNT") {
            self.screen.load_font(FontId::F9, &d);
        }
        let mut pal = Pal::default();
        self.screen.fade_palette(&pal, 0x1E);
        if let Some(d) = self.res.file_data("BACKGND.CPS") {
            self.screen.load_bitmap(&d, 8, Some(&mut pal));
        }
        self.screen
            .copy_region(0, 0, 0, 0, 320, 200, 8, 0, CR_NO_P_CHECK);
        self.screen
            .copy_region(0, 0, 0, 0, 320, 200, 8, 2, CR_NO_P_CHECK);
        self.screen.update_screen();
        self.screen.fade_palette(&pal, 0x82);
        let mut h = History::default();
        self.screen
            .copy_region(h.x, h.y, h.x, h.y, h.w, h.h, 2, 0, 0);
        self.screen.update_screen();
        self.screen.set_font(FontId::F9);
        let mut voice = *b"PS_1a";
        let mut part = 0usize;
        let entry = |part: usize| -> String {
            let e = &data[(part * 15 + 2).min(data.len())..];
            let end = e.iter().position(|&b| b == 0).unwrap_or(e.len()).min(13);
            String::from_utf8_lossy(&e[..end]).to_string()
        };
        while voice[3] <= b'9' && !self.should_quit() && !self.skip_flag() {
            while !self.should_quit() && !self.skip_flag() {
                let vf = String::from_utf8_lossy(&voice).to_string();
                if !self.is_voice_present(&vf) {
                    break;
                }
                if data.get(part * 15) == Some(&voice[3])
                    && data.get(part * 15 + 1) == Some(&voice[4])
                {
                    match part {
                        0 => {
                            self.history_load_wsa(&mut h, &entry(part));
                            self.history_play_wsa(&mut h, true);
                            self.snd_voice_play(&vf, 255, false);
                        }
                        1 | 2 | 8 | 16 | 25 => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, true);
                        }
                        3 | 7 | 10 | 17 | 23 | 26 => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, true);
                            self.history_restore_bkgd(&h);
                            self.history_load_wsa(&mut h, &entry(part));
                            self.history_play_wsa(&mut h, true);
                        }
                        6 => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, false);
                            self.history_restore_bkgd(&h);
                            self.history_load_wsa(&mut h, &entry(part));
                            self.history_play_wsa(&mut h, true);
                            self.delay_with_ticks(30);
                            self.history_play_wsa(&mut h, true);
                        }
                        9 => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_load_wsa(&mut h, &entry(part));
                            self.history_play_wsa(&mut h, true);
                        }
                        22 => {
                            self.history_play_wsa(&mut h, false);
                            self.history_restore_bkgd(&h);
                            self.history_load_wsa(&mut h, &entry(part));
                            self.delay_with_ticks(30);
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, true);
                            let mut tmp = entry(part).into_bytes();
                            for i in 1..4 {
                                if self.should_quit() {
                                    break;
                                }
                                let next = self.millis() + 30 * TICK;
                                if tmp.len() > 6 {
                                    tmp[6] = b'a' + i;
                                }
                                self.history_load_wsa(&mut h, &String::from_utf8_lossy(&tmp));
                                self.delay_until(next, false);
                                self.history_play_wsa(&mut h, true);
                            }
                            if tmp.len() > 6 {
                                tmp[6] = b'e';
                            }
                            self.history_load_wsa(&mut h, &String::from_utf8_lossy(&tmp));
                        }
                        29 => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, false);
                            self.history_restore_bkgd(&h);
                            self.history_load_wsa(&mut h, &entry(part));
                            h.fire = self.open_wsa("FIRE.WSA", 0, false);
                            self.history_play_wsa(&mut h, true);
                            h.fire_frame = 0;
                            for _ in 0..12 {
                                self.history_fire_step(&mut h, true);
                            }
                            let mut pal = Pal::default();
                            if let Some(d) = self.res.file_data("DRACPAL.PAL") {
                                pal.load(&d);
                            }
                            self.history_fade_with_fire(&mut h, &pal, 0x78);
                            while self
                                .screen
                                .system
                                .mixer()
                                .any_voice_playing(crate::audio::VoiceKind::Speech)
                                && !self.should_quit()
                            {
                                // Here the frame counts on before it is shown.
                                let next = self.millis() + 3 * TICK;
                                h.fire_frame += 1;
                                if h.fire_frame > 4 {
                                    h.fire_frame = 0;
                                }
                                if let Some(w) = h.fire.as_mut() {
                                    w.display_frame(
                                        &mut self.screen,
                                        h.fire_frame,
                                        0,
                                        75,
                                        51,
                                        0,
                                        None,
                                        None,
                                    );
                                }
                                self.screen.update_screen();
                                self.delay_until(next, false);
                            }
                            h.fire_frame = 0;
                            for _ in 0..10 {
                                self.history_fire_step(&mut h, true);
                            }
                        }
                        _ => {
                            self.snd_voice_play(&vf, 255, false);
                            self.history_play_wsa(&mut h, false);
                            self.history_restore_bkgd(&h);
                            self.history_load_wsa(&mut h, &entry(part));
                            self.history_play_wsa(&mut h, true);
                        }
                    }
                    part += 1;
                } else {
                    self.snd_voice_play(&vf, 255, false);
                }
                while self
                    .screen
                    .system
                    .mixer()
                    .any_voice_playing(crate::audio::VoiceKind::Speech)
                    && !self.should_quit()
                    && !self.skip_flag()
                {
                    self.delay(10, false);
                }
                if self.skip_flag() {
                    self.screen
                        .system
                        .mixer()
                        .stop_voices(crate::audio::VoiceKind::Speech);
                }
                voice[4] += 1;
            }
            voice[3] += 1;
            voice[4] = b'a';
        }
        if self.skip_flag() {
            self.event_list.clear();
        }
        let mut pal = Pal::default();
        pal.fill(0, 256, 63);
        if h.fire.is_some() {
            self.history_fade_with_fire(&mut h, &pal, 0x3C);
        } else {
            self.screen.fade_palette(&pal, 0x3C);
        }
        self.screen.clear_page(0);
        self.screen.fade_palette(&Pal::default(), 0x3C);
        if self.skip_flag() {
            self.event_list.clear();
        }
    }

    fn history_load_wsa(&mut self, h: &mut History, name: &str) {
        h.wsa = self.open_wsa(name, 3, true);
        if let Some(w) = &h.wsa {
            if let Some(p) = &w.palette {
                self.screen.set_screen_palette(p);
            }
            h.x = w.x_add;
            h.y = w.y_add;
            h.w = w.width;
            h.h = w.height;
        }
        h.frame = 1;
    }

    fn history_play_wsa(&mut self, h: &mut History, forward: bool) {
        for _ in 0..15 {
            if self.should_quit() {
                break;
            }
            let next = self.millis() + 3 * TICK;
            if let Some(w) = h.wsa.as_mut() {
                w.display_frame(&mut self.screen, h.frame, 2, 0, 0, 0, None, None);
            }
            self.screen
                .copy_region(h.x, h.y, h.x, h.y, h.w, h.h, 2, 0, 0);
            self.screen.update_screen();
            self.delay_until(next, false);
            h.frame += if forward { 1 } else { -1 };
        }
    }

    fn history_restore_bkgd(&mut self, h: &History) {
        self.screen
            .copy_region(h.x, h.y, h.x, h.y, h.w, h.h, 8, 0, 0);
        self.screen
            .copy_region(h.x, h.y, h.x, h.y, h.w, h.h, 8, 2, 0);
        self.screen.update_screen();
    }

    fn history_fire_step(&mut self, h: &mut History, wait: bool) {
        let next = self.millis() + 3 * TICK;
        if h.fire_frame > 4 {
            h.fire_frame = 0;
        }
        if let Some(w) = h.fire.as_mut() {
            w.display_frame(&mut self.screen, h.fire_frame, 0, 75, 51, 0, None, None);
        }
        h.fire_frame += 1;
        self.screen.update_screen();
        if wait {
            self.delay_until(next, false);
        }
    }

    fn history_fade_with_fire(&mut self, h: &mut History, pal: &Pal, delay: i32) {
        let tick = TICK;
        let mut fire = h.fire.take();
        let mut frame = h.fire_frame;
        let mut next = h.next_fire_time;
        self.screen
            .fade_palette_with(pal, delay, &mut |s: &mut Screen| {
                if s.system.millis() > next {
                    if let Some(w) = fire.as_mut() {
                        w.display_frame(s, frame, 0, 75, 51, 0, None, None);
                    }
                    frame = (frame + 1) % 5;
                    next = s.system.millis() + 4 * tick;
                }
                s.update_screen();
            });
        h.fire = fire;
        h.fire_frame = frame;
        h.next_fire_time = next;
    }

    // ---- the end

    pub fn setup_epilogue_data(&mut self, load: bool) {
        for f in EPILOGUE_PAKS {
            if load {
                self.res.load_pak(f);
            } else {
                self.res.unload_pak(f);
            }
        }
        self.screen.clear_page(0);
        self.screen.clear_page(3);
        if !load {
            *self.screen.palette_mut(0) = Pal::default();
            let p0 = self.screen.palette(0).clone();
            self.screen.set_screen_palette(&p0);
            if self.should_quit() {
                return;
            }
            self.event_list.clear();
        }
    }

    /// `showOutro`.
    pub fn show_outro(&mut self, character: i32, max_difficulty: bool) {
        self.setup_epilogue_data(true);
        self.with_seq_interp(|s| {
            *s.screen.palette_mut(0) = Pal::default();
            let p0 = s.screen.palette(0).clone();
            s.screen.set_screen_palette(&p0);
            s.screen.clear_page(0);
            s.screen.clear_page(4);
            s.screen.clear_page(8);
            s.seq_tim = s.tim_load("LOLFINAL.TIM", TimOpcodes::Outro);
            if let Some(t) = s.seq_tim.as_mut() {
                t.lol_character = character as u8;
            }
            if let Some(d) = s.res.file_data("NEW6P.FNT") {
                s.screen.load_font(FontId::F6, &d);
            }
            if let Some(d) = s.res.file_data("INTRO.FNT") {
                s.screen.load_font(FontId::Intro, &d);
            }
            s.tim.finished = false;
            s.tim.lang_data = s.res.file_data("LOLFINAL.DIP");
            s.screen.hide_mouse();
            s.run_seq_tim(false);
            s.remove_input_top();
            s.screen.show_mouse();
            s.screen
                .system
                .mixer()
                .stop_voices(crate::audio::VoiceKind::Speech);
            s.screen.system.mixer().fade_out_music();
            s.event_list.clear();
            s.seq_tim = None;
            for i in 0..WSA_SLOTS {
                s.tim_free_anim_struct(i);
            }
            s.screen.fade_to_black(30);
            if !s.should_quit() {
                s.show_credits();
            }
            s.event_list.clear();
            if !s.should_quit() {
                let file = match character {
                    0 => Some("KIERAN.CPS"),
                    1 => Some("AK'SHEL.CPS"),
                    2 => Some("MICHAEL.CPS"),
                    3 => Some("CONRAD.CPS"),
                    _ => None,
                };
                // The picture's palette goes into palette 0 (kept if it brings none).
                let mut p0 = s.screen.palette(0).clone();
                match file.and_then(|f| s.res.file_data(f)) {
                    Some(d) => s.screen.load_bitmap(&d, 3, Some(&mut p0)),
                    None => {
                        s.screen.clear_page(3);
                        p0 = Pal::default();
                    }
                }
                *s.screen.palette_mut(0) = p0.clone();
                s.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 2, 0, CR_NO_P_CHECK);
                if max_difficulty {
                    s.tim_display_text_outro(0x8000, 0, 0xDC);
                }
                s.screen.update_screen();
                s.screen.fade_palette(&p0, 30);
                while s.check_input(None, true, 0) == 0 && !s.should_quit() {
                    s.delay(TICK, false);
                }
                s.screen.fade_to_black(30);
            }
            s.tim.lang_data = None;
        });
        self.setup_epilogue_data(false);
    }

    pub fn show_credits(&mut self) {
        let mut table: Vec<u8> = (0..=255u8).collect();
        table[255] = 0;
        self.screen.system.mixer().halt_music();
        self.snd_load_sound_file_named("LOREFINL");
        self.screen.system.mixer().play_music(4);
        self.screen.hide_mouse();
        const COLOR_MAP: [u8; 16] = [
            0x00, 0x00, 0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x72, 0x6F,
            0x6F, 0x6D,
        ];
        self.screen.char_spacing = 0;
        let mut p0 = self.screen.palette(0).clone();
        if let Some(d) = self.res.file_data("ROOM.CPS") {
            self.screen.load_bitmap(&d, 2, Some(&mut p0));
        }
        self.screen.set_text_color_map(&COLOR_MAP);
        p0.fill(255, 1, 0);
        *self.screen.palette_mut(0) = p0;
        self.screen.fade_to_black(30);
        self.screen
            .copy_region(0, 0, 0, 0, 320, 200, 2, 0, CR_NO_P_CHECK);
        self.screen.line_spacing = 0;
        let credits = self.res.file_data("CREDITS.TXT").unwrap_or_default();
        self.process_credits(&credits, 21, 4, 5, &table);
        let end = self.millis() + 120 * TICK;
        while end > self.millis() && !self.should_quit() {
            if self.check_input(None, true, 0) != 0 {
                break;
            }
            self.delay(TICK, false);
        }
        self.screen.system.mixer().fade_out_music();
        self.screen.fade_to_black(30);
        self.screen.clear_cur_page();
        self.screen.update_screen();
        self.screen.show_mouse();
    }

    fn load_outro_shapes(&mut self, file: usize) -> Vec<Vec<u8>> {
        let Some(d) = self.res.file_data(OUTRO_SHAPE_FILES[file % 28]) else {
            return vec![Vec::new(); 12];
        };
        self.screen.load_bitmap(&d, 5, None);
        let p = self.screen.page(5).to_vec();
        (0..12)
            .map(|i| shape::shape_copy(&p, if i < 8 { i } else { i + 4 }).unwrap_or_default())
            .collect()
    }

    fn process_credits(
        &mut self,
        text: &[u8],
        dim_state: usize,
        page: usize,
        delay_time: u32,
        table: &[u8],
    ) {
        if text.is_empty() {
            return;
        }
        self.screen.set_screen_dim(dim_state);
        self.screen.clear_page(page);
        self.screen.clear_page(6);
        let door = self.res.file_data("DOOR.SHP").map(|d| {
            self.screen.load_bitmap(&d, 5, None);
            shape::shape_copy(self.screen.page(5), 0).unwrap_or_default()
        });
        let door = door.unwrap_or_default();
        self.screen
            .draw_shape(0, &door, 0, 0, 22, 0x10, ShapeArgs::none());
        self.screen
            .draw_shape(0, &door, 0, 0, 23, 0x11, ShapeArgs::none());
        let mut cur_shape_file = 0usize;
        let mut shapes = self.load_outro_shapes(cur_shape_file);
        cur_shape_file += 1;
        let monster_pal = self.res.file_data("MONSTERS.PAL").unwrap_or_default();
        {
            let mut p0 = self.screen.palette(0).clone();
            let n = 120.min(monster_pal.len());
            p0.0[88 * 3..88 * 3 + n].copy_from_slice(&monster_pal[..n]);
            *self.screen.palette_mut(0) = p0;
        }
        let p0 = self.screen.palette(0).clone();
        self.screen.fade_palette(&p0, 30);
        let mut wait_timer = self.millis();
        #[derive(Clone, Default)]
        struct Cs {
            x: i32,
            y: i32,
            s: Vec<u8>,
            code: u8,
            height: i32,
        }
        let mut strings: Vec<Cs> = vec![Cs::default()];
        let mut pos = 0usize;
        let mut frame_counter = 0;
        let mut monster_frame = 0usize;
        let mut need_new_shape = false;
        let mut door_redraw = true;
        let mut anim_block = vec![0u8; 40960 * 2];
        let d = self.screen.cur_dim();
        loop {
            while self.millis() < wait_timer && !self.should_quit() {
                self.delay(TICK, false);
            }
            wait_timer = self.millis() + delay_time * TICK;
            while strings.len() - 1 < 35 && pos < text.len() && text[pos] != 0 {
                let n = strings.len() - 1;
                let mut y = if n == 0 {
                    d.h
                } else {
                    let p = &strings[n];
                    p.y + p.height + (p.height >> 3)
                };
                let start = pos;
                let end = text[pos..]
                    .iter()
                    .position(|&c| c == 5 || c == 0x0D || c == 0)
                    .map_or(text.len(), |e| pos + e);
                let code = text.get(end).copied().unwrap_or(0);
                pos = if code != 0 { end + 1 } else { end };
                let mut cur = &text[start..end];
                let mut alignment = 0;
                if cur.first() == Some(&3) || cur.first() == Some(&4) {
                    alignment = cur[0];
                    cur = &cur[1..];
                }
                self.screen.set_font(FontId::F6);
                if cur.first() == Some(&1) || cur.first() == Some(&2) {
                    cur = &cur[1..];
                }
                let height = self.screen.font_height();
                let tw = self.screen.text_width(cur);
                let x = match alignment {
                    3 => 0,
                    4 => 300 - tw,
                    _ => ((d.w << 3) - tw) / 2,
                };
                if strings[n].code == 5 {
                    y -= strings[n].height + (strings[n].height >> 3);
                }
                let mut s = Cs {
                    x,
                    y,
                    s: cur.to_vec(),
                    code,
                    height,
                };
                if s.x + tw > 320 {
                    // The original leaves out lines too long; broken in two as ScummVM does:
                    // at the last space that lets the first part fit (centred).
                    let mut split = None;
                    let mut end = s.s.len();
                    let mut x = s.x;
                    while x + self.screen.text_width(&s.s[..end]) > 320 {
                        match s.s[..end].iter().rposition(|&c| c == b' ') {
                            Some(sp) => {
                                split = Some(sp);
                                end = sp;
                                x = (((d.w << 3) - self.screen.text_width(&s.s[..end])) / 2).max(0);
                            }
                            None => {
                                split = None;
                                break;
                            }
                        }
                    }
                    if let Some(sp) = split {
                        let rest = s.s[sp + 1..].to_vec();
                        s.s.truncate(sp);
                        s.x = ((d.w << 3) - self.screen.text_width(&s.s)) / 2;
                        let ny = s.y + s.height + (s.height >> 3);
                        let nx = ((d.w << 3) - self.screen.text_width(&rest)) / 2;
                        let c = s.code;
                        strings.push(s);
                        strings.push(Cs {
                            x: nx,
                            y: ny,
                            s: rest,
                            code: c,
                            height,
                        });
                        continue;
                    }
                }
                strings.push(s);
            }
            frame_counter += 1;
            if frame_counter % 3 != 0 {
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 6, page, CR_NO_P_CHECK);
            } else if monster_frame == 0 && door_redraw {
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 2, page, CR_NO_P_CHECK);
                self.screen
                    .draw_shape(page, &door, 0, 0, 22, 0x10, ShapeArgs::none());
                self.screen
                    .draw_shape(page, &door, 0, 0, 23, 0x11, ShapeArgs::none());
                frame_counter -= 1;
                door_redraw = false;
            } else {
                if monster_frame == 0 {
                    let p0 = self.screen.palette(0).clone();
                    self.screen.set_screen_palette(&p0);
                }
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 2, page, CR_NO_P_CHECK);
                let monster = shapes[OUTRO_FRAMES[monster_frame]].clone();
                let right = ((cur_shape_file as i32 - 1) & 1) != 0;
                let (door_sd, dx, dy, mx, my) = if right {
                    self.screen
                        .draw_shape(page, &door, 0, 0, 22, 0x10, ShapeArgs::none());
                    (
                        23,
                        OUTRO_RIGHT_DOOR[monster_frame * 2],
                        OUTRO_RIGHT_DOOR[monster_frame * 2 + 1],
                        OUTRO_RIGHT_MONSTER[monster_frame * 2],
                        OUTRO_RIGHT_MONSTER[monster_frame * 2 + 1],
                    )
                } else {
                    self.screen
                        .draw_shape(page, &door, 0, 0, 23, 0x11, ShapeArgs::none());
                    (
                        22,
                        OUTRO_LEFT_DOOR[monster_frame * 2],
                        OUTRO_LEFT_DOOR[monster_frame * 2 + 1],
                        OUTRO_LEFT_MONSTER[monster_frame * 2],
                        OUTRO_LEFT_MONSTER[monster_frame * 2 + 1],
                    )
                };
                let door_flip = if door_sd == 22 { 0 } else { 1 };
                if monster_frame >= 8 {
                    self.screen.draw_shape(
                        page,
                        &door,
                        dx,
                        dy,
                        door_sd,
                        door_flip,
                        ShapeArgs::none(),
                    );
                }
                let flip = if !right || monster_frame < 20 { 0 } else { 1 };
                self.screen.draw_shape(
                    page,
                    &monster,
                    mx,
                    my,
                    0,
                    0x104 | flip,
                    ShapeArgs {
                        fade: Some((table, 1)),
                        scale: (OUTRO_SCALE_X[monster_frame], OUTRO_SCALE_Y[monster_frame]),
                        ..ShapeArgs::none()
                    },
                );
                if monster_frame < 8 {
                    self.screen.draw_shape(
                        page,
                        &door,
                        dx,
                        dy,
                        door_sd,
                        door_flip,
                        ShapeArgs::none(),
                    );
                }
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, page, 6, CR_NO_P_CHECK);
                door_redraw = true;
                monster_frame = (monster_frame + 1) % 24;
                need_new_shape = monster_frame == 0;
            }
            for s in strings.iter_mut().skip(1) {
                if s.y < d.h {
                    let cp = self.screen.set_cur_page(page);
                    self.screen.set_font(FontId::F6);
                    self.screen
                        .print_text(&s.s, (d.sx << 3) + s.x, d.sy + s.y, 0xDC, 0x00);
                    self.screen.set_cur_page(cp);
                    self.screen.cur_page = 0;
                }
                s.y -= 1;
            }
            self.screen.copy_to_page0(d.sy, d.h, page, &mut anim_block);
            if strings.len() > 1 && strings[1].y < -10 {
                strings.remove(1);
            }
            if need_new_shape {
                cur_shape_file += 1;
                if cur_shape_file == 16 {
                    cur_shape_file += 2;
                }
                if cur_shape_file == 6 {
                    cur_shape_file += 2;
                }
                cur_shape_file %= 28;
                shapes = self.load_outro_shapes(cur_shape_file);
                let mut p0 = self.screen.palette(0).clone();
                let at = cur_shape_file * 120;
                if let Some(src) = monster_pal.get(at..at + 120) {
                    p0.0[88 * 3..88 * 3 + 120].copy_from_slice(src);
                }
                *self.screen.palette_mut(0) = p0.clone();
                self.screen.set_screen_palette(&p0);
                need_new_shape = false;
            }
            self.screen.update_screen();
            let input = self.check_input(None, true, 0);
            self.remove_input_top();
            let count = strings.len() - 1;
            if count == 0 || (input != 0 && input & 0x800 == 0) || self.should_quit() {
                break;
            }
        }
        self.remove_input_top();
    }
}

/// `HistoryPlayer`'s state.
#[derive(Default)]
struct History {
    wsa: Option<Wsa>,
    fire: Option<Wsa>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    frame: i32,
    fire_frame: i32,
    next_fire_time: u32,
}
