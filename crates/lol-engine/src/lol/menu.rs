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

//! The game's menus (ScummVM's `GUI_LoL::runMenu` with `GUI_v1`'s menu drawing): the main
//! menu, loading, saving and deleting games, the options, the volumes, death and the choice
//! asked before quitting or deleting.

use super::types::*;
use super::{ButtonListId, Lol, TICK};
use crate::platform::{Event, Key};
use crate::screen::FontId;
use crate::shape::ShapeArgs;

#[derive(Debug, Clone, Default)]
pub struct MenuItem {
    pub enabled: bool,
    pub item_id: u16,
    pub item_string: Vec<u8>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text_color: u8,
    pub highlight_color: u8,
    pub title_x: i32,
    pub bkgd_color: u8,
    pub color1: u8,
    pub color2: u8,
    pub save_slot: i32,
    pub label_id: u16,
    pub label_x: i32,
    pub label_y: i32,
    pub key_code: u16,
}

#[derive(Debug, Clone, Default)]
pub struct Menu {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub bkgd_color: u8,
    pub color1: u8,
    pub color2: u8,
    pub menu_name_id: u16,
    pub highlighted_item: u8,
    pub title_x: i32,
    pub title_y: i32,
    pub text_color: u8,
    pub scroll_up_x: i32,
    pub scroll_up_y: i32,
    pub scroll_down_x: i32,
    pub scroll_down_y: i32,
    /// `numberOfItems`: the items `initMenu` and `processHighlights` handle (the audio menu
    /// has only its OK button there; its three sliders are drawn by `runMenu`).
    pub number_of_items: usize,
    pub items: Vec<MenuItem>,
}

/// `GUI_LoL`'s menus and state.
#[derive(Debug, Clone, Default)]
pub struct MenuState {
    pub buttons: Vec<Button>,
    pub menus: Vec<Menu>,
    pub current: usize,
    pub last: usize,
    pub new: Option<usize>,
    pub display_menu: bool,
    pub menu_result: i32,
    pub savegame_offset: i32,
    pub menu_buttons: Vec<Button>,
    pub scroll_up_button: Button,
    pub scroll_down_button: Button,
    pub save_description: Vec<u8>,
    pub save_slots: Vec<i32>,
    pub savegame_list: Vec<Vec<u8>>,
    pub slider_sfx: i32,
    pub typed: Option<(Key, u8)>,
}

fn idx(m: MenuCb) -> usize {
    m as usize
}

fn cb_of(i: usize) -> MenuCb {
    [
        MenuCb::MainMenu,
        MenuCb::LoadMenu,
        MenuCb::SaveMenu,
        MenuCb::DeleteMenu,
        MenuCb::OptionsMenu,
        MenuCb::AudioMenu,
        MenuCb::DeathMenu,
        MenuCb::SavenameMenu,
        MenuCb::ChoiceMenu,
    ][i.min(8)]
}

impl Lol {
    /// `GUI_LoL::initStaticData`.
    pub fn menu_init(&mut self) {
        let dim_menu = |s: &Self, dim: usize, name: u16, hl: u8, e: i32, f: i32, g: i32, i: i32| {
            let d = s.screen.dim(dim);
            Menu {
                x: d.sx << 3,
                y: d.sy,
                width: d.w << 3,
                height: d.h,
                bkgd_color: 225,
                color1: 223,
                color2: 227,
                menu_name_id: name,
                highlighted_item: hl,
                title_x: (d.sx << 3) + (d.w << 2),
                title_y: 6,
                text_color: 254,
                scroll_up_x: e,
                scroll_up_y: f,
                scroll_down_x: g,
                scroll_down_y: i,
                number_of_items: 0,
                items: Vec::new(),
            }
        };
        let item = |id: u16, x: i32, y: i32, w: i32, h: i32, label: u16, key: u16| MenuItem {
            enabled: true,
            item_id: id,
            item_string: Vec::new(),
            x,
            y,
            width: w,
            height: h,
            text_color: 204,
            highlight_color: 254,
            title_x: -1,
            bkgd_color: 225,
            color1: 223,
            color2: 227,
            save_slot: 0,
            label_id: label,
            label_x: 0,
            label_y: 0,
            key_code: key,
        };
        let (bh, step, skip, list_y, list_ok) = (15, 17, 22, 39, 118);
        let esc = Self::KEY_ESCAPE;
        let mut menus = vec![Menu::default(); 9];
        let mut m = dim_menu(self, 9, 0x4000, 0, -1, -1, -1, -1);
        let mut row = 0;
        while row < 4 {
            m.items.push(item(
                0x4001 + row as u16,
                16,
                skip + 1 + row * step,
                176,
                bh,
                0,
                0,
            ));
            row += 1;
        }
        m.items
            .push(item(0x42D9, 16, skip + 1 + row * step, 176, bh, 0, 0));
        row += 1;
        m.items
            .push(item(0x4006, 16, skip + 1 + row * step, 176, bh, 0, 0));
        row += 1;
        m.items
            .push(item(0x4005, 88, skip + 1 + row * step + 2, 104, bh, 0, esc));
        menus[idx(MenuCb::MainMenu)] = m;
        for (cb, name) in [
            (MenuCb::LoadMenu, 0x400E),
            (MenuCb::SaveMenu, 0x400D),
            (MenuCb::DeleteMenu, 0x400F),
        ] {
            let mut m = dim_menu(self, 10, name, 1, 128, 20, 128, 118);
            for r in 0..4 {
                m.items
                    .push(item(0xFFFE - r as u16, 8, list_y + r * step, 256, bh, 0, 0));
            }
            m.items.push(item(0x4011, 168, list_ok, 96, bh, 0, esc));
            menus[idx(cb)] = m;
        }
        let mut m = dim_menu(self, 17, 0x400C, 0, -1, -1, -1, -1);
        let mut row = 0;
        for (id, label) in [
            (0xFFF7, 0x406E),
            (0xFFF6, 0x406C),
            (0xFFF5, 0x406D),
            (0xFFF4, 0x42D5),
            (0xFFF3, 0x42D2),
        ] {
            m.items
                .push(item(id, 120, skip + row * step, 80, bh, label, 0));
            row += 1;
        }
        m.items
            .push(item(0x4072, 104, skip + row * step + 3, 96, bh, 0, esc));
        menus[idx(MenuCb::OptionsMenu)] = m;
        let mut m = dim_menu(self, 18, 0x42D9, 2, -1, -1, -1, -1);
        m.items.push(item(0x4072, 152, 76, 96, bh, 0, esc));
        m.items.push(item(3, 128, 22, 114, 14, 0x42DB, 0));
        m.items.push(item(4, 128, 39, 114, 14, 0x42DA, 0));
        m.items.push(item(5, 128, 56, 114, 14, 0x42DC, 0));
        menus[idx(MenuCb::AudioMenu)] = m;
        let mut m = dim_menu(self, 11, 0x4013, 0, -1, -1, -1, -1);
        m.items.push(item(0x4006, 8, 30, 104, bh, 0, 0));
        m.items.push(item(0x4001, 176, 30, 104, bh, 0, 0));
        menus[idx(MenuCb::DeathMenu)] = m;
        let mut m = dim_menu(self, 7, 0x4053, 0, -1, -1, -1, -1);
        m.items
            .push(item(0x4012, 8, 38, 72, bh, 0, Self::KEY_RETURN));
        m.items.push(item(0x4011, 176, 38, 72, bh, 0, esc));
        menus[idx(MenuCb::SavenameMenu)] = m;
        let mut m = dim_menu(self, 11, 0, 0, -1, -1, -1, -1);
        m.items.push(item(0x4007, 8, 30, 72, bh, 0, 0));
        m.items.push(item(0x4008, 208, 30, 72, bh, 0, 0));
        menus[idx(MenuCb::ChoiceMenu)] = m;
        for m in menus.iter_mut() {
            m.number_of_items = m.items.len();
        }
        menus[idx(MenuCb::AudioMenu)].number_of_items = 1;
        let mk = |index: u16, key: u16, d0v1: u8, flags: u16, w: u16, h: u16| {
            let mut b = Button::blank();
            b.index = index;
            b.key_code = key;
            b.data0_val1 = d0v1;
            b.data1_val1 = d0v1;
            b.data2_val1 = d0v1;
            b.flags = flags;
            b.width = w;
            b.height = h;
            b
        };
        self.menu.scroll_up_button = mk(20, 96, 1, 0x4487, 25, 16);
        self.menu.scroll_down_button = mk(21, 98, 1, 0x4487, 25, 16);
        self.menu.menu_buttons = (0..10).map(|i| mk(i, 0, 0, 0x4487, 0, 0)).collect();
        self.menu.menus = menus;
        self.menu.slider_sfx = 11;
    }

    fn cur_menu(&self) -> &Menu {
        &self.menu.menus[self.menu.current]
    }

    fn print_menu_text(&mut self, s: &[u8], x: i32, y: i32, c0: u8, c1: u8, flags: i32) {
        self.screen.fprint_string(s, x, y, c0, c1, flags);
    }

    fn menu_center_x(&self, s: &[u8], x1: i32, x2: i32) -> i32 {
        if s.is_empty() {
            return 0;
        }
        let w = x2 - x1 + 1;
        x1 + (w - self.screen.text_width(s)) / 2
    }

    fn menu_title(&self, m: &Menu) -> Vec<u8> {
        if m.menu_name_id == 0 {
            Vec::new()
        } else {
            self.get_lang_string(m.menu_name_id)
        }
    }

    fn menu_item_title(&self, it: &MenuItem) -> Vec<u8> {
        if it.item_id & 0x8000 != 0 && !it.item_string.is_empty() {
            it.item_string.clone()
        } else if it.item_id & 0x8000 != 0 || it.item_id == 0 {
            Vec::new()
        } else {
            self.get_lang_string(it.item_id)
        }
    }

    fn menu_item_label(&self, it: &MenuItem) -> Vec<u8> {
        if it.label_id & 0x8000 != 0 || it.label_id == 0 {
            Vec::new()
        } else {
            self.get_lang_string(it.label_id)
        }
    }

    /// `GUI_v1::initMenu`.
    fn init_menu(&mut self) {
        self.menu.buttons.clear();
        let m = self.cur_menu().clone();
        let (mx2, my2) = (m.width + m.x - 1, m.height + m.y - 1);
        self.screen
            .fill_rect(m.x + 2, m.y + 2, mx2 - 2, my2 - 2, m.bkgd_color, None);
        self.screen
            .draw_shaded_box(m.x, m.y, mx2, my2, m.color1, m.color2);
        let title = self.menu_title(&m);
        let tx = if m.title_x != -1 {
            m.title_x
        } else {
            self.menu_center_x(&title, m.x, mx2)
        };
        self.print_menu_text(&title, tx, m.y + m.title_y, m.text_color, 0, 9);
        for (i, it) in m.items.iter().enumerate().take(m.number_of_items) {
            if !it.enabled {
                continue;
            }
            let (x1, y1) = (m.x + it.x, m.y + it.y);
            let (x2, y2) = (x1 + it.width - 1, y1 + it.height - 1);
            let mut b = self.menu.menu_buttons[i].clone();
            b.x = x1 as i16;
            b.y = y1 as i16;
            b.width = (it.width - 1) as u16;
            b.height = (it.height - 1) as u16;
            b.button_callback = Callback::Menu(cb_of(self.menu.current));
            b.key_code = it.key_code;
            b.key_code2 = 0;
            b.arg = it.item_id;
            self.menu.menu_buttons[i] = b.clone();
            self.menu.buttons.push(b);
            self.screen.fill_rect(x1, y1, x2, y2, it.bkgd_color, None);
            self.screen
                .draw_shaded_box(x1, y1, x2, y2, it.color1, it.color2);
            let t = self.menu_item_title(it);
            if !t.is_empty() {
                let tx = if it.title_x != -1 {
                    x1 + it.title_x + 3
                } else {
                    self.menu_center_x(&t, x1, x2)
                };
                let col = if i as u8 == m.highlighted_item {
                    it.highlight_color
                } else {
                    it.text_color
                };
                self.print_menu_text(&t, tx, y1 + 3, col, 0, 8);
            }
        }
        for i in 0..m.number_of_items.min(m.items.len()) {
            let it = self.menu.menus[self.menu.current].items[i].clone();
            let l = self.menu_item_label(&it);
            if !l.is_empty() {
                let (lx, ly) = (it.x - 1, it.y + 3);
                let cur = self.menu.current;
                self.menu.menus[cur].items[i].label_x = lx;
                self.menu.menus[cur].items[i].label_y = ly;
                self.print_menu_text(&l, m.x + lx, m.y + ly, it.text_color, 0, 10);
            }
        }
        if m.scroll_up_x != -1 {
            let mut up = self.menu.scroll_up_button.clone();
            up.x = (m.scroll_up_x + m.x) as i16;
            up.y = (m.scroll_up_y + m.y) as i16;
            up.button_callback = Callback::ScrollUp;
            up.mouse_wheel = -1;
            self.menu.buttons.push(up.clone());
            self.update_menu_button(&up);
            let mut down = self.menu.scroll_down_button.clone();
            down.x = (m.scroll_down_x + m.x) as i16;
            down.y = (m.scroll_down_y + m.y) as i16;
            down.button_callback = Callback::ScrollDown;
            down.mouse_wheel = 1;
            self.menu.buttons.push(down.clone());
            self.update_menu_button(&down);
        }
        self.screen.update_screen();
    }

    fn find_menu_button(&self, b: &Button) -> Option<usize> {
        self.menu.buttons.iter().position(|x| {
            x.x == b.x
                && x.y == b.y
                && x.arg == b.arg
                && x.index == b.index
                && x.button_callback == b.button_callback
        })
    }

    /// `GUI_v1::updateMenuButton`.
    pub fn update_menu_button(&mut self, b: &Button) {
        if !self.menu.display_menu {
            return;
        }
        self.update_button(b);
    }

    /// `GUI_v1::updateButton`.
    pub fn update_button(&mut self, b: &Button) {
        if b.flags & 8 != 0 {
            return;
        }
        let i = self.find_menu_button(b);
        let mut nb = match i {
            Some(i) => self.menu.buttons[i].clone(),
            None => b.clone(),
        };
        if nb.flags2 & 1 != 0 {
            nb.flags2 &= 0xFFF7;
        } else {
            nb.flags2 |= 8;
        }
        nb.flags2 &= 0xFFFC;
        if nb.flags2 & 4 != 0 {
            nb.flags2 |= 0x10;
        } else {
            nb.flags2 &= 0xEEEF;
        }
        nb.flags2 &= 0xFFFB;
        if let Some(i) = i {
            self.menu.buttons[i] = nb.clone();
        }
        self.process_button(&nb);
    }

    pub fn menu_redraw_button(&mut self, b: &Button, shaded: bool) -> i32 {
        if !self.menu.display_menu {
            return 0;
        }
        let (x, y, w, h) = (b.x as i32, b.y as i32, b.width as i32, b.height as i32);
        if shaded {
            self.screen.draw_shaded_box(x, y, x + w, y + h, 223, 227);
        } else {
            self.screen
                .draw_box(x + 1, y + 1, x + w - 1, y + h - 1, 225);
        }
        0
    }

    fn redraw_menu_item_text(&mut self, i: usize, highlight: bool) {
        let m = self.cur_menu().clone();
        let Some(it) = m.items.get(i) else { return };
        let x1 = m.x + it.x;
        let y1 = m.y + it.y;
        let x2 = x1 + it.width - 1;
        let t = self.menu_item_title(it);
        let tx = if it.title_x >= 0 {
            x1 + it.title_x + 3
        } else {
            self.menu_center_x(&t, x1, x2)
        };
        let col = if highlight {
            it.highlight_color
        } else {
            it.text_color
        };
        self.print_menu_text(&t, tx, y1 + 3, col, 0, 8);
    }

    /// `GUI_v1::processHighlights`.
    fn process_highlights(&mut self) {
        let (mx, my) = self.get_mouse_pos();
        let m = self.cur_menu().clone();
        if m.highlighted_item != 255
            && m.items
                .get(m.highlighted_item as usize)
                .is_some_and(|it| it.enabled)
        {
            self.redraw_menu_item_text(m.highlighted_item as usize, false);
        }
        for (i, it) in m.items.iter().enumerate().take(m.number_of_items) {
            if !it.enabled {
                continue;
            }
            let (x1, y1) = (m.x + it.x, m.y + it.y);
            let (x2, y2) = (x1 + it.width, y1 + it.height);
            if mx > x1 && mx < x2 && my > y1 && my < y2 {
                let cur = self.menu.current;
                self.menu.menus[cur].highlighted_item = i as u8;
                self.redraw_menu_item_text(i, true);
            }
        }
        self.screen.update_screen();
    }

    fn setup_save_menu_slots(&mut self, num: usize) {
        let cur = self.menu.current;
        for i in 0..num {
            self.menu.menus[cur].items[i].save_slot = -1;
            self.menu.menus[cur].items[i].enabled = false;
        }
        let is_save = cur == idx(MenuCb::SaveMenu);
        let (start, offs) = if is_save {
            (if self.menu.savegame_offset == 0 { 1 } else { 0 }, 1)
        } else {
            (0, 0)
        };
        let max_len = (self.screen.dim(8).w << 3) - self.screen.char_width(b'W');
        let mut i = start;
        while i < num {
            let k = i as i32 + self.menu.savegame_offset - offs;
            if k < 0 || k as usize >= self.menu.savegame_list.len() {
                break;
            }
            // Trimmed in the current font, the one the menu prints with.
            let mut s = self.menu.savegame_list[k as usize].clone();
            while !s.is_empty() && self.screen.text_width(&s) >= max_len {
                s.pop();
            }
            let slot = self.menu.save_slots[k as usize];
            let it = &mut self.menu.menus[cur].items[i];
            it.item_string = s;
            it.save_slot = slot;
            it.enabled = true;
            i += 1;
        }
        if self.menu.savegame_offset == 0 && is_save {
            let s = self.get_lang_string(0x4010);
            let it = &mut self.menu.menus[cur].items[0];
            it.item_string = s;
            it.save_slot = -3;
            it.enabled = true;
        }
    }

    fn update_save_slots_list(&mut self) {
        let list = self.savegame_list();
        self.menu.save_slots = list.iter().map(|s| s.slot).collect();
        self.menu.savegame_list = list.into_iter().map(|s| s.description).collect();
    }

    /// `GUI_LoL::getInput`: true when a key was typed into the name.
    fn menu_get_input(&mut self) -> bool {
        if !self.menu.display_menu {
            return false;
        }
        let (mx, my) = self.get_mouse_pos();
        self.mouse_x = mx;
        self.mouse_y = my;
        let savename = self.menu.current == idx(MenuCb::SavenameMenu);
        if savename {
            self.update_input();
            for e in self.event_list.iter() {
                if let Event::KeyDown { key, ascii, .. } = e.event {
                    self.menu.typed = Some((key, ascii));
                }
            }
        }
        let mut input = self.check_input(Some(ButtonListId::Menu), true, 0);
        if savename && let Some((key, ascii)) = self.menu.typed {
            let c = dos_char(ascii);
            if c > 31 && c < 226 {
                self.menu.save_description.push(c);
                input |= 0x8000;
            } else if key == Key::Backspace && !self.menu.save_description.is_empty() {
                self.menu.save_description.pop();
                input |= 0x8000;
            }
        }
        self.remove_input_top();
        self.menu.typed = None;
        if self.should_quit() {
            self.menu.display_menu = false;
        }
        self.delay(8, false);
        input & 0x8000 != 0
    }

    /// `GUI_LoL::runMenu`.
    pub fn run_menu(&mut self, which: MenuCb) -> i32 {
        if self.menu.menus.is_empty() {
            self.menu_init();
        }
        self.menu.current = idx(which);
        self.menu.last = self.menu.current;
        self.menu.new = None;
        self.menu.display_menu = true;
        self.menu.menu_result = 1;
        self.menu.savegame_offset = 0;
        self.screen.copy_page(0, 7);
        let d = self.screen.dim(8);
        let of = self.screen.set_font(FontId::F9);
        let w_w = self.screen.char_width(b'W');
        self.screen.set_font(of);
        let f_w = (d.w << 3) - w_w;
        let mut f_c = 0;
        let mut cursor_timer = 0u32;
        let mut cursor_status = 1u8;
        while self.menu.display_menu && !self.should_quit() {
            self.mouse_x = 0;
            self.mouse_y = 0;
            let cur = self.menu.current;
            if cur == idx(MenuCb::LoadMenu)
                || cur == idx(MenuCb::SaveMenu)
                || cur == idx(MenuCb::DeleteMenu)
            {
                self.update_save_slots_list();
                self.setup_save_menu_slots(4);
            }
            let special = self.menu.menus[cur].highlighted_item;
            self.menu.menus[cur].highlighted_item = 255;
            if cur == idx(MenuCb::OptionsMenu) {
                let strs = [
                    self.get_lang_string(0x406F + self.monster_difficulty as u16),
                    self.get_lang_string(if self.smooth_scrolling {
                        0x4068
                    } else {
                        0x4069
                    }),
                    self.get_lang_string(if self.floating_cursors {
                        0x4068
                    } else {
                        0x4069
                    }),
                    self.get_lang_string(0x42D6 + self.lang_index as u16),
                    self.get_lang_string(if self.text_enabled() { 0x4068 } else { 0x4069 }),
                ];
                for (k, s) in strs.into_iter().enumerate() {
                    self.menu.menus[cur].items[k].item_string = s;
                }
            }
            if special == 1 {
                let gs = |i: usize| self.game_shapes.get(i).cloned();
                if self.menu.savegame_offset == 0 {
                    self.menu.scroll_up_button.data0_shape = None;
                    self.menu.scroll_up_button.data1_shape = None;
                    self.menu.scroll_up_button.data2_shape = None;
                } else {
                    self.menu.scroll_up_button.data0_shape = gs(17);
                    self.menu.scroll_up_button.data1_shape = gs(19);
                    self.menu.scroll_up_button.data2_shape = gs(19);
                }
                let slot_offs = if cur == idx(MenuCb::SaveMenu) { 1 } else { 0 };
                let n = self.menu.save_slots.len() as i32;
                if self.menu.savegame_offset == n - (4 - slot_offs) || n < 5 - slot_offs {
                    self.menu.scroll_down_button.data0_shape = None;
                    self.menu.scroll_down_button.data1_shape = None;
                    self.menu.scroll_down_button.data2_shape = None;
                } else {
                    self.menu.scroll_down_button.data0_shape = gs(18);
                    self.menu.scroll_down_button.data1_shape = gs(20);
                    self.menu.scroll_down_button.data2_shape = gs(20);
                }
            }
            let nitems = self.menu.menus[cur].number_of_items;
            for i in 0..nitems.min(10) {
                let b = &mut self.menu.menu_buttons[i];
                b.data0_val1 = 4;
                b.data1_val1 = 4;
                b.data2_val1 = 4;
                b.data0_callback = Callback::RedrawShadedButton;
                b.data1_callback = Callback::RedrawButton;
                b.data2_callback = Callback::RedrawButton;
                b.flags = 0x4487;
                b.flags2 = 0;
            }
            self.init_menu();
            if (cur == idx(MenuCb::LoadMenu) || cur == idx(MenuCb::DeleteMenu))
                && self.menu.save_slots.is_empty()
            {
                let m = self.cur_menu().clone();
                let s = self.get_lang_string(0x4009);
                self.screen
                    .fprint_string(&s, m.x + m.width / 2, m.y + 42, 204, 0, 9);
            }
            if special == 2 {
                const OX: [i32; 3] = [0, 10, 124];
                const OW: [i32; 3] = [10, 114, 10];
                let m = self.cur_menu().clone();
                for i in 1..4 {
                    let it = m.items[i].clone();
                    let (tx, ty) = (m.x + it.x, m.y + it.y);
                    for ii in 0..3 {
                        let mut b = Button::blank();
                        b.index = ii as u16;
                        b.x = (tx + OX[ii]) as i16;
                        b.y = ty as i16;
                        b.width = OW[ii] as u16;
                        b.height = it.height as u16;
                        b.flags = if ii == 1 { 0x6606 } else { 0x4406 };
                        b.button_callback = Callback::Menu(MenuCb::AudioMenu);
                        b.arg = it.item_id;
                        self.menu.buttons.push(b.clone());
                        self.process_button(&b);
                        self.update_button(&b);
                    }
                    let (lx, ly) = (it.x - 5, it.y + 3);
                    let cur = self.menu.current;
                    self.menu.menus[cur].items[i].label_x = lx;
                    self.menu.menus[cur].items[i].label_y = ly;
                    let l = self.menu_item_label(&it);
                    self.print_menu_text(&l, m.x + lx, m.y + ly, it.text_color, 0, 10);
                    let volume = self.get_volume(i as i32 - 1);
                    let page = self.screen.cur_page;
                    let s85 = self.game_shapes.get(85).cloned().unwrap_or_default();
                    let s87 = self.game_shapes.get(87).cloned().unwrap_or_default();
                    let s86 = self.game_shapes.get(86).cloned().unwrap_or_default();
                    self.screen
                        .draw_shape(page, &s85, tx, ty, 0, 0x10, ShapeArgs::none());
                    self.screen.draw_shape(
                        page,
                        &s87,
                        tx + 2 + OX[1],
                        ty,
                        0,
                        0x10,
                        ShapeArgs::none(),
                    );
                    self.screen.draw_shape(
                        page,
                        &s86,
                        tx + OX[1] + volume,
                        ty,
                        0,
                        0x10,
                        ShapeArgs::none(),
                    );
                }
                self.screen.update_screen();
            }
            if cur == idx(MenuCb::MainMenu) {
                let m = self.cur_menu().clone();
                let f = self.screen.set_font(FontId::F6);
                let v = format!("v{}", env!("CARGO_PKG_VERSION"));
                self.screen
                    .fprint_string(v.as_bytes(), m.x + 8, m.y + m.height - 12, 204, 0, 8);
                self.screen.set_font(f);
                self.screen.update_screen();
            }
            if cur == idx(MenuCb::SavenameMenu) {
                let (mx, my, mw, mh) = ((d.sx << 3) - 1, d.sy - 1, (d.w << 3) + 1, d.h + 1);
                self.screen
                    .draw_shaded_box(mx, my, mx + mw, my + mh, 227, 223);
                let pg = self.screen.set_cur_page(0);
                self.txt_clear_dim(8);
                cursor_timer = 0;
                cursor_status = 0;
                let f = self.screen.set_font(FontId::F9);
                while !self.menu.save_description.is_empty()
                    && self.screen.text_width(&self.menu.save_description.clone()) >= f_w
                {
                    self.menu.save_description.pop();
                }
                f_c = self.screen.text_width(&self.menu.save_description.clone());
                let sd = self.menu.save_description.clone();
                self.screen
                    .fprint_string(&sd, d.sx << 3, d.sy + 2, d.col1 as u8, d.col2 as u8, 0);
                self.screen.set_font(f);
                self.screen.fill_rect(
                    (d.sx << 3) + f_c,
                    d.sy,
                    (d.sx << 3) + f_c + w_w,
                    d.sy + d.h - 1,
                    d.col1 as u8,
                    Some(0),
                );
                self.screen.set_cur_page(pg);
            }
            while self.menu.new.is_none() && self.menu.display_menu && !self.should_quit() {
                let frame_end = self.millis() + 16;
                self.process_highlights();
                let savename = self.menu.current == idx(MenuCb::SavenameMenu);
                if savename && cursor_timer <= self.millis() {
                    let f = self.screen.set_font(FontId::F9);
                    f_c = self.screen.text_width(&self.menu.save_description.clone());
                    cursor_status ^= 1;
                    cursor_timer = self.millis() + 20 * TICK;
                    let col = if cursor_status != 0 { d.col1 } else { d.col2 } as u8;
                    self.screen.fill_rect(
                        (d.sx << 3) + f_c,
                        d.sy,
                        (d.sx << 3) + f_c + w_w,
                        d.sy + d.h - 1,
                        col,
                        Some(0),
                    );
                    self.screen.update_screen();
                    self.screen.set_font(f);
                }
                if self.menu_get_input() {
                    if self.menu.new.is_none() {
                        if self.menu.current == idx(MenuCb::SavenameMenu) {
                            let f = self.screen.set_font(FontId::F9);
                            // The cursor erased where it was drawn (`fC` before the key).
                            self.screen.fill_rect(
                                (d.sx << 3) + f_c,
                                d.sy,
                                (d.sx << 3) + f_c + w_w,
                                d.sy + d.h - 1,
                                d.col2 as u8,
                                Some(0),
                            );
                            while !self.menu.save_description.is_empty()
                                && self.screen.text_width(&self.menu.save_description.clone())
                                    >= f_w
                            {
                                self.menu.save_description.pop();
                            }
                            f_c = self.screen.text_width(&self.menu.save_description.clone());
                            let sd = self.menu.save_description.clone();
                            self.screen.fprint_string(
                                &sd,
                                d.sx << 3,
                                d.sy + 2,
                                d.col1 as u8,
                                d.col2 as u8,
                                0,
                            );
                            let col = if cursor_status != 0 { d.col1 } else { d.col2 } as u8;
                            self.screen.fill_rect(
                                (d.sx << 3) + f_c,
                                d.sy,
                                (d.sx << 3) + f_c + w_w,
                                d.sy + d.h - 1,
                                col,
                                Some(0),
                            );
                            self.screen.set_font(f);
                            cursor_timer = 0;
                            cursor_status = 0;
                        } else if self.menu.current != idx(MenuCb::AudioMenu) {
                            self.menu.new = Some(self.menu.current);
                        }
                    } else if self.menu.menu_result != -1 {
                        self.menu.last = self.menu.current;
                    }
                }
                if self.menu.menu_result == 0 {
                    self.menu.display_menu = false;
                }
                self.delay_until(frame_end, false);
            }
            if self.menu.new != Some(self.menu.current) || !self.menu.display_menu {
                self.screen.copy_page(7, 0);
                self.screen.update_screen();
            }
            let cur = self.menu.current;
            self.menu.menus[cur].highlighted_item = special;
            if let Some(n) = self.menu.new.take() {
                self.menu.current = n;
            }
        }
        self.menu.buttons.clear();
        self.gui_notify_button_list_changed();
        self.menu.menu_result
    }

    pub fn run_menu_callback(&mut self, m: MenuCb, b: &Button) -> i32 {
        self.update_menu_button(b);
        let arg = b.arg;
        let item_index = |arg: u16| (-(arg as i16) - 2).max(0) as usize;
        match m {
            MenuCb::MainMenu => {
                match arg {
                    0x4001 => {
                        self.menu.savegame_offset = 0;
                        self.menu.new = Some(idx(MenuCb::LoadMenu));
                    }
                    0x4002 => {
                        self.menu.savegame_offset = 0;
                        self.menu.new = Some(idx(MenuCb::SaveMenu));
                    }
                    0x4003 => {
                        self.menu.savegame_offset = 0;
                        self.menu.new = Some(idx(MenuCb::DeleteMenu));
                    }
                    0x4004 => self.menu.new = Some(idx(MenuCb::OptionsMenu)),
                    0x42D9 => self.menu.new = Some(idx(MenuCb::AudioMenu)),
                    0x4006 => {
                        self.menu.menus[idx(MenuCb::ChoiceMenu)].menu_name_id = 0x400A;
                        self.menu.new = Some(idx(MenuCb::ChoiceMenu));
                    }
                    0x4005 => self.menu.display_menu = false,
                    _ => {}
                }
                1
            }
            MenuCb::LoadMenu => {
                if arg == 0x4011 {
                    if self.menu.current != self.menu.last {
                        self.menu.new = Some(self.menu.last);
                    } else {
                        self.menu.menu_result = 0;
                    }
                    return 1;
                }
                let k = item_index(arg);
                self.game_to_load = self.menu.menus[idx(MenuCb::LoadMenu)]
                    .items
                    .get(k)
                    .map_or(-1, |i| i.save_slot);
                self.menu.display_menu = false;
                1
            }
            MenuCb::SaveMenu => {
                if arg == 0x4011 {
                    self.menu.new = Some(idx(MenuCb::MainMenu));
                    return 1;
                }
                self.menu.new = Some(idx(MenuCb::SavenameMenu));
                let k = item_index(arg);
                let it = self.menu.menus[idx(MenuCb::SaveMenu)]
                    .items
                    .get(k)
                    .cloned()
                    .unwrap_or_default();
                self.menu.menu_result = it.save_slot + 1;
                self.menu.save_description.clear();
                if it.save_slot != -3 {
                    self.menu.save_description = it.item_string.clone();
                    self.menu.save_description.truncate(79);
                }
                1
            }
            MenuCb::DeleteMenu => {
                if arg == 0x4011 {
                    self.menu.new = Some(idx(MenuCb::MainMenu));
                    return 1;
                }
                self.menu.menus[idx(MenuCb::ChoiceMenu)].menu_name_id = 0x400B;
                self.menu.new = Some(idx(MenuCb::ChoiceMenu));
                let k = item_index(arg);
                self.menu.menu_result = self.menu.menus[idx(MenuCb::DeleteMenu)]
                    .items
                    .get(k)
                    .map_or(0, |i| i.save_slot)
                    + 1;
                1
            }
            MenuCb::OptionsMenu => {
                match arg {
                    0xFFF7 => self.monster_difficulty = (self.monster_difficulty + 1) % 3,
                    0xFFF6 => self.smooth_scrolling ^= true,
                    0xFFF5 => self.floating_cursors ^= true,
                    0xFFF4 => self.set_language_index((self.lang_index + 1) % 3),
                    0xFFF3 => self.config_voice ^= 3,
                    0x4072 => {
                        let ext = self.language_ext();
                        let f = format!("LEVEL{:02}.{ext}", self.current_level);
                        self.level_lang_file = self.res.file_data(&f).unwrap_or_default();
                        let f = format!("LANDS.{ext}");
                        self.lands_file = self.res.file_data(&f).unwrap_or_default();
                        self.menu.new = Some(self.menu.last);
                    }
                    _ => {}
                }
                1
            }
            MenuCb::AudioMenu => {
                if arg == 0x4072 {
                    self.menu.new = Some(self.menu.last);
                    return 1;
                }
                let mut tx = b.x as i32;
                let entry = arg as i32 - 3;
                let old = self.get_volume(entry);
                let mut new = old;
                match b.index {
                    0 => {
                        new -= 10;
                        tx += 10;
                    }
                    1 => new = self.mouse_x - (tx + 7),
                    2 => {
                        new += 10;
                        tx -= 114;
                    }
                    _ => {}
                }
                let new = new.clamp(2, 102);
                if new == old {
                    self.screen.update_screen();
                    return 0;
                }
                let s87 = self.game_shapes.get(87).cloned().unwrap_or_default();
                let s86 = self.game_shapes.get(86).cloned().unwrap_or_default();
                self.screen
                    .draw_shape(0, &s87, tx + old, b.y as i32, 0, 0x10, ShapeArgs::none());
                // The knob where the volume will read back (`convertVolumeFromMixer(
                // convertVolumeToMixer(newVolume))`).
                let draw_x = (((new - 2) * 255) / 100) * 100 / 255 + 2;
                self.screen.draw_shape(
                    0,
                    &s86,
                    tx + draw_x,
                    b.y as i32,
                    0,
                    0x10,
                    ShapeArgs::none(),
                );
                self.screen.update_screen();
                self.snd_stop_speech(false);
                self.set_volume(entry, new);
                if arg == 4 {
                    let s = self.menu.slider_sfx;
                    self.snd_play_sound_effect(s, -1);
                    let idx_of = |s: i32| {
                        lol_data::tables::INGAME_SFX_INDEX
                            .get((s.max(0) as usize) * 2)
                            .copied()
                            .map_or(-1, |v| v as i16 as i32)
                    };
                    let mut voc = idx_of(self.menu.slider_sfx);
                    let mut guard = 0;
                    loop {
                        guard += 1;
                        if guard > 500 {
                            break;
                        }
                        self.menu.slider_sfx += 1;
                        if self.menu.slider_sfx < 47 {
                            self.menu.slider_sfx += 1;
                        }
                        if voc == 199 {
                            self.menu.slider_sfx = 11;
                        }
                        voc = idx_of(self.menu.slider_sfx);
                        if voc == -1 {
                            continue;
                        }
                        if self
                            .ingame_sound_list
                            .get(voc as usize)
                            .is_some_and(|s| s.eq_ignore_ascii_case("EMPTY"))
                        {
                            continue;
                        }
                        break;
                    }
                } else if arg == 5 {
                    self.last_speech_id = -1;
                    self.snd_play_character_speech(0x42E0, 0, false);
                }
                1
            }
            MenuCb::DeathMenu => {
                let items = &self.menu.menus[idx(MenuCb::DeathMenu)].items;
                if arg == items[0].item_id {
                    self.quit_game();
                } else if arg == items[1].item_id {
                    self.menu.new = Some(idx(MenuCb::LoadMenu));
                }
                1
            }
            MenuCb::SavenameMenu => {
                let items = self.menu.menus[idx(MenuCb::SavenameMenu)].items.clone();
                if arg == items[0].item_id {
                    let slot = if self.menu.menu_result == -2 {
                        self.next_savegame_slot()
                    } else {
                        self.menu.menu_result - 1
                    };
                    let d = self.menu.save_description.clone();
                    self.save_game_state(slot, &d);
                    self.menu.display_menu = false;
                } else if arg == items[1].item_id {
                    self.menu.new = Some(idx(MenuCb::SaveMenu));
                }
                1
            }
            MenuCb::ChoiceMenu => {
                let items = self.menu.menus[idx(MenuCb::ChoiceMenu)].items.clone();
                if arg == items[0].item_id {
                    if self.menu.last == idx(MenuCb::MainMenu) {
                        self.quit_game();
                    } else if self.menu.last == idx(MenuCb::DeleteMenu) {
                        let s = self.menu.menu_result - 1;
                        self.delete_savegame(s);
                        self.menu.new = Some(idx(MenuCb::MainMenu));
                    }
                } else if arg == items[1].item_id {
                    self.menu.new = Some(idx(MenuCb::MainMenu));
                }
                1
            }
        }
    }

    pub fn menu_scroll_up(&mut self, b: &Button) -> i32 {
        if self.menu.scroll_up_button.data0_shape.is_none() {
            return 0;
        }
        self.update_button(b);
        if self.menu.savegame_offset > 0 {
            self.menu.savegame_offset -= 1;
            self.menu.new = Some(self.menu.current);
            self.menu.menu_result = -1;
        }
        1
    }

    pub fn menu_scroll_down(&mut self, b: &Button) -> i32 {
        if self.menu.scroll_down_button.data0_shape.is_none() {
            return 0;
        }
        self.update_button(b);
        let lim = self.menu.save_slots.len() as i32
            - if self.menu.current == idx(MenuCb::SaveMenu) {
                3
            } else {
                4
            };
        if self.menu.savegame_offset < lim {
            self.menu.savegame_offset += 1;
            self.menu.new = Some(self.menu.current);
            self.menu.menu_result = -1;
        }
        1
    }

    /// `getVolume`: 0 music, 1 sounds, 2 speech, in the original's steps 2–102 (speech 2 while
    /// `speechEnabled()` is false).
    pub fn get_volume(&self, entry: i32) -> i32 {
        if entry >= 2 && !self.speech_enabled() {
            return 2;
        }
        let m = self.screen.system.mixer();
        let v = match entry {
            0 => {
                if m.music_enabled {
                    m.music_volume
                } else {
                    0
                }
            }
            1 => {
                if m.sfx_enabled {
                    m.sfx_volume
                } else {
                    0
                }
            }
            _ => {
                if m.speech_enabled {
                    m.speech_volume
                } else {
                    0
                }
            }
        };
        (v as i32 * 100) / 255 + 2
    }

    pub fn set_volume(&mut self, entry: i32, value: i32) {
        let v = (((value - 2).clamp(0, 100) * 255) / 100) as u8;
        let mut m = self.screen.system.mixer();
        match entry {
            0 => {
                m.music_volume = v;
                m.music_enabled = v > 0;
            }
            1 => {
                m.sfx_volume = v;
                m.sfx_enabled = v > 0;
            }
            _ => {
                m.speech_volume = v;
                m.speech_enabled = v > 0;
            }
        }
        // `_configVoice` is left alone: `clickedOptions` adjusts it around the menu.
    }
}

/// `Util::convertISOToDOS` for the letters the game's fonts have.
pub fn dos_char(c: u8) -> u8 {
    match c {
        0xE4 => 0x84,
        0xF6 => 0x94,
        0xFC => 0x81,
        0xC4 => 0x8E,
        0xD6 => 0x99,
        0xDC => 0x9A,
        0xDF => 0xE1,
        0xE9 => 0x82,
        0xE8 => 0x8A,
        0xEA => 0x88,
        0xE0 => 0x85,
        0xE2 => 0x83,
        0xE7 => 0x87,
        0xF4 => 0x93,
        0xFB => 0x96,
        0xEE => 0x8C,
        0xEF => 0x8B,
        0xEB => 0x89,
        0xF9 => 0x97,
        _ => c,
    }
}
