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

//! The game's text (ScummVM's `TextDisplayer_rpg` and `TextDisplayer_LoL`): the text field
//! under the view, messages in their colours, dialogue texts with the script's parameters,
//! lines broken at spaces, scrolling, and the page break that waits for the "more" button.

use super::types::*;
use super::{Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId};

/// What the text keeps per screen window (`TextDimData`).
#[derive(Debug, Clone, Copy, Default)]
pub struct TextDimData {
    pub color1: u8,
    pub color2: u8,
    pub line: i32,
    pub column: i32,
}

/// `TextDisplayer_rpg` / `TextDisplayer_LoL`'s state.
#[derive(Debug, Clone)]
pub struct TextState {
    pub line_count: i32,
    pub print_flag: bool,
    pub line_width: i32,
    pub num_chars_total: i32,
    pub allow_page_break: bool,
    pub num_chars_left: i32,
    pub num_chars_printed: i32,
    pub dim_data: Vec<TextDimData>,
    pub current_line: Vec<u8>,
    pub script_text_parameter: i32,
    pub page_break_string: Vec<u8>,
    pub wait_button_space: i32,
    ctrl: [u8; 3],
    temp1: Option<(Vec<u8>, usize)>,
    temp2: Option<(Vec<u8>, usize)>,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            line_count: 0,
            print_flag: false,
            line_width: 0,
            num_chars_total: 0,
            allow_page_break: true,
            num_chars_left: 0,
            num_chars_printed: 0,
            dim_data: Vec::new(),
            current_line: Vec::new(),
            script_text_parameter: 0,
            page_break_string: Vec::new(),
            wait_button_space: 0,
            ctrl: [0; 3],
            temp1: None,
            temp2: None,
        }
    }
}

impl Lol {
    /// The text's windows as the screen defines them (the constructor).
    pub fn txt_init(&mut self) {
        self.txt.dim_data = (0..self.screen.dim_count())
            .map(|i| {
                let d = self.screen.dim(i);
                TextDimData {
                    color1: d.col1 as u8,
                    color2: d.col2 as u8,
                    line: d.line,
                    column: d.column,
                }
            })
            .collect();
    }

    fn tdim(&mut self, i: usize) -> &mut TextDimData {
        if self.txt.dim_data.len() <= i {
            self.txt.dim_data.resize(i + 1, TextDimData::default());
        }
        &mut self.txt.dim_data[i]
    }

    /// `copyRegionToBuffer` into a buffer of the game's, keeping what lies beyond.
    pub fn copy_region_into(
        &self,
        page: usize,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        buf: &mut Vec<u8>,
        offset: usize,
    ) {
        let v = self.screen.copy_region_to_buffer(page, x, y, w, h);
        if buf.len() < offset + v.len() {
            buf.resize(offset + v.len(), 0);
        }
        buf[offset..offset + v.len()].copy_from_slice(&v);
    }

    pub fn txt_setup_field(&mut self, mode: bool) {
        if self.text_enabled() {
            let (y, h, step_y, step_h) = (142, 37, 3, 1);
            if mode {
                let mut pb1 = std::mem::take(&mut self.page_buffer1);
                self.copy_region_into(3, 0, 0, 320, 40, &mut pb1, 0);
                self.screen
                    .copy_region(80, y, 0, 0, 240, h, 0, 3, CR_NO_P_CHECK);
                let mut pb2 = std::mem::take(&mut self.page_buffer2);
                self.copy_region_into(3, 0, 0, 320, 40, &mut pb2, 0);
                self.screen.copy_block_to_page(3, 0, 0, 320, 40, &pb1);
                self.page_buffer1 = pb1;
                self.page_buffer2 = pb2;
            } else {
                let d = self.txt_clear_dim(4);
                self.screen.set_screen_dim(d as usize);
                let cp = self.screen.set_cur_page(2);
                let mut pb1 = std::mem::take(&mut self.page_buffer1);
                self.copy_region_into(3, 0, 0, 320, 40, &mut pb1, 0);
                let pb2 = self.page_buffer2.clone();
                self.screen.copy_block_to_page(3, 0, 0, 320, 40, &pb2);
                let cur = self.screen.cur_page;
                self.screen
                    .copy_region(0, 0, 80, y, 240, h, 3, cur, CR_NO_P_CHECK);
                let mut end = self.millis();
                for i in (142..=177).rev() {
                    end += TICK;
                    self.screen.copy_region(
                        83,
                        i - step_h + 1,
                        83,
                        i - step_h,
                        235,
                        step_y,
                        0,
                        0,
                        CR_NO_P_CHECK,
                    );
                    self.screen
                        .copy_region(83, i + 1, 83, i + 1, 235, 1, 2, 0, CR_NO_P_CHECK);
                    self.update_input();
                    self.screen.update_screen();
                    self.delay_until(end, false);
                }
                self.screen.copy_block_to_page(3, 0, 0, 320, 200, &pb1);
                self.page_buffer1 = pb1;
                self.screen.set_cur_page(cp);
                self.update_flags &= 0xFFFD;
            }
        } else {
            if !mode {
                let d = self.txt_clear_dim(4);
                self.screen.set_screen_dim(d as usize);
            }
            self.toggle_selected_character_frame(true);
        }
    }

    pub fn txt_expand_field(&mut self) {
        if self.text_enabled() {
            self.fade_text = false;
            self.text_color_flag = 0;
            self.timers.disable(11);
            let d = self.txt_clear_dim(3);
            self.screen.set_screen_dim(d as usize);
            let mut pb1 = std::mem::take(&mut self.page_buffer1);
            self.copy_region_into(3, 0, 0, 320, 10, &mut pb1, 13000);
            let (y, h, step_h) = (140, 3, 0);
            self.screen
                .copy_region(83, y, 0, 0, 235, h, 0, 2, CR_NO_P_CHECK);
            let mut end = self.millis();
            for i in 140..177 {
                end += TICK;
                self.screen
                    .copy_region(0, 0, 83, i - step_h, 235, h, 2, 0, CR_NO_P_CHECK);
                self.update_input();
                self.screen.update_screen();
                self.delay_until(end, false);
            }
            self.screen
                .copy_block_to_page(3, 0, 0, 320, 10, &pb1[13000..13000 + 3200]);
            self.page_buffer1 = pb1;
            self.update_flags |= 2;
        } else {
            self.txt_clear_dim(3);
            self.toggle_selected_character_frame(false);
        }
    }

    /// `printDialogueText2`: a text of the game with the script's parameters (`params` from
    /// `param_index` on) into window `dim`.
    pub fn txt_print_dialogue_text2(
        &mut self,
        dim: i32,
        s: &[u8],
        params: &[i32],
        param_index: i32,
    ) {
        let old_dim;
        if dim == 3 {
            if self.update_flags & 2 != 0 {
                old_dim = self.txt_clear_dim(4);
                let c2 = self.screen.cur_dim().col2 as u8;
                let t = self.tdim(4);
                t.color1 = 254;
                t.color2 = c2;
            } else {
                old_dim = self.txt_clear_dim(3);
                let c2 = self.screen.cur_dim().col2 as u8;
                let t = self.tdim(3);
                t.color1 = 192;
                t.color2 = c2;
                self.screen.copy_color(192, 254);
                self.enable_timer(11);
                self.text_color_flag = 0;
                self.fade_text = false;
            }
        } else {
            old_dim = self.screen.cur_dim_index as i32;
            self.screen.set_screen_dim(dim as usize);
            self.txt.line_count = 0;
            let c2 = self.screen.cur_dim().col2 as u8;
            let t = self.tdim(dim as usize);
            t.color1 = 254;
            t.color2 = c2;
        }
        let cp = self.screen.set_cur_page(0);
        let of = self.screen.set_font(FontId::F9);
        let buf = self.txt_preprocess_string(s, params, param_index);
        if std::env::var_os("LOL_DEBUG").is_some() {
            eprintln!("Text ({dim}): {}", String::from_utf8_lossy(&buf));
        }
        self.txt.num_chars_total = buf.len() as i32;
        self.txt_display_text(&buf, &[]);
        self.screen.set_screen_dim(old_dim as usize);
        self.screen.set_cur_page(cp);
        self.screen.set_font(of);
        self.txt.line_count = 0;
        self.fade_text = false;
    }

    /// `TextDisplayer_LoL::printMessage`: the message types 0–4 in their colours (bit 15: no
    /// sound, bit 2: the speaking portrait goes on).
    pub fn print_message(&mut self, type_: u16, format: &[u8], args: &[Arg]) {
        const COLORS: [u8; 5] = [0xFE, 0xA2, 0x84, 0x97, 0x9F];
        const SFX: [u8; 5] = [0x0B, 0x00, 0x2B, 0x1B, 0x00];
        let mut type_ = type_;
        if type_ & 4 != 0 {
            type_ &= !4;
        } else {
            self.stop_portrait_speech_anim();
        }
        let index = ((type_ & 0x7FFF) as usize).min(4);
        let col = COLORS[index];
        let od = self.screen.cur_dim_index;
        if self.update_flags & 2 != 0 {
            self.txt_clear_dim(4);
            self.tdim(4).color1 = col;
        } else {
            self.txt_clear_dim(3);
            self.screen.copy_color(192, col as usize);
            self.tdim(3).color1 = 192;
            self.enable_timer(11);
        }
        let mut buf = sprintf(format, args);
        buf.truncate(239);
        self.txt_display_text(&buf, &[]);
        self.screen.set_screen_dim(od);
        self.txt.line_count = 0;
        // `sound()->playSoundEffect`: the driver's track directly (not `snd_playSoundEffect`,
        // which would map it through the game's list to a voice file).
        if type_ & 0x8000 == 0 && SFX[index] != 0 {
            self.screen
                .system
                .mixer()
                .play_adlib_effect(SFX[index], 0xFF);
        }
        self.text_color_flag = index as i32;
        self.fade_text = false;
    }

    fn txt_preprocess_string(&self, s: &[u8], params: &[i32], param_index: i32) -> Vec<u8> {
        let mut dst = Vec::new();
        let mut i = 0;
        let at = |i: usize| s.get(i).copied().unwrap_or(0);
        let param = || params.get(param_index.max(0) as usize).copied();
        while at(i) != 0 {
            if at(i) != b'%' {
                dst.push(at(i));
                i += 1;
                continue;
            }
            i += 1;
            let mut para = at(i);
            let mut eos = false;
            match para {
                0 => eos = true,
                b'#' => {
                    i += 1;
                    para = at(i);
                    if !matches!(
                        para,
                        b'E' | b'G' | b'X' | b'c' | b'd' | b'e' | b'f' | b'g' | b's' | b'u' | b'x'
                    ) {
                        eos = true;
                    }
                }
                b' ' | b'+' | b'-' => i += 1,
                _ => {}
            }
            if eos {
                continue;
            }
            para = at(i);
            match para {
                0 => continue,
                b'0' => i += 1,
                _ => {
                    while para != 0 && para.is_ascii_digit() {
                        i += 1;
                        para = at(i);
                    }
                }
            }
            para = at(i);
            i += 1;
            match para {
                b'a' => {
                    dst.extend_from_slice(self.txt.script_text_parameter.to_string().as_bytes())
                }
                b'n' => {
                    if let Some(p) = param() {
                        let c = &self.characters[(p as usize).min(3)];
                        dst.extend_from_slice(&c.name);
                    }
                }
                b's' => {
                    if let Some(p) = param() {
                        dst.extend_from_slice(&self.get_lang_string(p as u16));
                    }
                }
                b'X' | b'd' | b'u' | b'x' => {
                    if let Some(p) = param() {
                        dst.extend_from_slice((p as i16).to_string().as_bytes());
                    }
                }
                _ => continue,
            }
        }
        dst.truncate(2559);
        dst
    }

    fn txt_read_next_para(&mut self) {
        let mut d = 0u8;
        if let Some((s, p)) = &mut self.txt.temp2 {
            if *p < s.len() && s[*p] != 0 {
                d = s[*p];
                *p += 1;
            } else {
                self.txt.temp2 = None;
                d = self.txt.ctrl[0];
            }
        }
        if d == 0
            && let Some((s, p)) = &mut self.txt.temp1
        {
            if *p < s.len() && s[*p] != 0 {
                d = s[*p];
                *p += 1;
            } else {
                self.txt.temp1 = None;
            }
        }
        self.txt.ctrl[1] = d;
        self.txt.ctrl[2] = 0;
    }

    fn txt_parse_command(&mut self) -> u8 {
        if self.txt.ctrl[1] == 0 {
            self.txt_read_next_para();
        }
        let res = self.txt.ctrl[1];
        self.txt.ctrl[1] = self.txt.ctrl[2];
        self.txt.ctrl[2] = 0;
        if self.txt.ctrl[1] == 0 {
            self.txt_read_next_para();
        }
        res
    }

    /// `displayText`: the text into the current window, lines broken; `%d`/`%s` in it take
    /// `args`.
    pub fn txt_display_text(&mut self, s: &[u8], args: &[Arg]) {
        self.txt.print_flag = false;
        self.txt.line_width = 0;
        self.txt.num_chars_left = 0;
        self.txt.num_chars_printed = 0;
        self.txt.temp1 = Some((s.to_vec(), 0));
        self.txt.temp2 = None;
        self.txt.current_line.clear();
        self.txt.ctrl = [0; 3];
        let mut next_arg = 0;
        let mut c = self.txt_parse_command();
        let sd = self.screen.cur_dim();
        let sdx = self.screen.cur_dim_index;
        let of = self.screen.current_font;
        let chars_per_line =
            (sd.w << 3) / (self.screen.font_width() + self.screen.char_spacing).max(1);
        let mut guard = 0;
        while c != 0 && guard < 20_000 {
            guard += 1;
            let a = self.txt.ctrl[1].to_ascii_lowercase();
            if self.txt.temp2.is_none() && c == b'%' {
                let sub = match (a, args.get(next_arg)) {
                    (b'd', Some(Arg::Int(v))) => v.to_string().into_bytes(),
                    (b's', Some(Arg::Str(v))) => v.clone(),
                    _ => break,
                };
                next_arg += 1;
                self.txt.temp2 = Some((sub, 0));
                self.txt.ctrl[0] = self.txt.ctrl[2];
                self.txt.ctrl[2] = 0;
                self.txt.ctrl[1] = 0;
                c = self.txt_parse_command();
            }
            match c as i32 - 1 {
                0 => {
                    self.txt_print_line();
                    self.screen.update_screen();
                    self.txt_text_page_break();
                    self.txt.num_chars_printed = 0;
                }
                1 => {
                    self.txt_print_line();
                    let v = self.txt_parse_command();
                    self.tdim(sdx).color2 = v;
                }
                5 => {
                    self.txt_print_line();
                    let v = self.txt_parse_command();
                    self.tdim(sdx).color1 = v;
                }
                8 => {
                    self.txt_print_line();
                    let fw = (self.screen.font_width() + self.screen.char_spacing).max(1);
                    let mut dv = self.tdim(sdx).column / fw;
                    dv = ((dv + 8) & 0xFFF8) - 1;
                    if dv >= chars_per_line {
                        dv = 0;
                    }
                    self.tdim(sdx).column = fw * dv;
                }
                12 => {
                    self.txt_print_line();
                    self.txt.line_count += 1;
                    let t = self.tdim(sdx);
                    t.column = 0;
                    t.line += 1;
                }
                11 | 18 | 23 | 24 | 26 | 28 => {}
                _ => {
                    let cw = self.screen.char_width(c);
                    self.txt.line_width += cw;
                    self.txt.current_line.push(c);
                    self.txt.num_chars_left += 1;
                    if self.tdim(sdx).column + self.txt.line_width > (sd.w << 3) {
                        self.txt_print_line();
                    }
                }
            }
            c = self.txt_parse_command();
        }
        if self.txt.num_chars_left != 0 {
            self.txt_print_line();
        }
        self.screen.set_font(of);
        self.screen.update_screen();
    }

    fn txt_print_line(&mut self) {
        let mut guard = 0;
        loop {
            guard += 1;
            if guard > 64 || !self.txt_print_line_once() {
                break;
            }
        }
    }

    /// One pass of `printLine`; true when the rest of the line must be printed as well.
    fn txt_print_line_once(&mut self) -> bool {
        let sd = self.screen.cur_dim();
        let sdx = self.screen.cur_dim_index;
        let fh = self.screen.font_height() + self.screen.line_spacing;
        let lines = (sd.h - self.screen.line_spacing) / fh.max(1);
        let mut guard = 0;
        while self.tdim(sdx).line >= lines && guard < 100 {
            guard += 1;
            if lines - self.txt.wait_button_space <= self.txt.line_count
                && self.txt.allow_page_break
            {
                self.txt.line_count = 0;
                self.screen.update_screen();
                self.txt_text_page_break();
                self.txt.num_chars_printed = 0;
            }
            let h1 = ((sd.h / fh.max(1)) - 1) * fh;
            let h2 = sd.h - fh;
            let page = self.screen.cur_page;
            if h2 > 0 {
                self.screen.copy_region(
                    sd.sx << 3,
                    sd.sy + fh,
                    sd.sx << 3,
                    sd.sy,
                    sd.w << 3,
                    h2,
                    page,
                    page,
                    CR_NO_P_CHECK,
                );
            }
            let c2 = self.tdim(sdx).color2;
            self.screen.fill_rect(
                sd.sx << 3,
                sd.sy + h1,
                ((sd.sx + sd.w) << 3) - 1,
                sd.sy + sd.h - 1,
                c2,
                None,
            );
            if self.tdim(sdx).line != 0 {
                self.tdim(sdx).line -= 1;
            }
        }
        let x1 = (sd.sx << 3) + self.tdim(sdx).column;
        let y = sd.sy + fh * self.tdim(sdx).line;
        let mut w = sd.w << 3;
        let mut lw = self.txt.line_width;
        let mut s = self
            .txt
            .num_chars_left
            .min(self.txt.current_line.len() as i32);
        let str_ = self.txt.current_line.clone();
        let column = self.tdim(sdx).column;
        if lw + column >= w {
            if lines - 1 <= self.txt.line_count && self.txt.allow_page_break {
                // `waitReserve`: room for the "more" button.
                w -= 80;
            }
            w -= column;
            let mut line_last_char_pos = 0;
            let mut str_pos = s - 1;
            while str_pos > 0 {
                let c = str_[str_pos as usize];
                lw -= self.screen.char_width(c);
                if line_last_char_pos == 0 && lw <= w {
                    line_last_char_pos = str_pos;
                }
                if line_last_char_pos != 0 && c == b' ' {
                    s = str_pos;
                    self.txt.print_flag = false;
                    break;
                }
                str_pos -= 1;
            }
            if str_pos == 0 {
                if column != 0 && !self.txt.print_flag {
                    s = 0;
                    lw = 0;
                    self.txt.print_flag = true;
                } else {
                    s = line_last_char_pos;
                }
            }
        }
        let s = s.max(0) as usize;
        let printed: Vec<u8> = str_[..s.min(str_.len())].to_vec();
        let col1 = self.tdim(sdx).color1;
        let col2 = self.tdim(sdx).color2;
        self.screen.print_text(&printed, x1, y, col1, col2);
        self.tdim(sdx).column += lw;
        self.txt.num_chars_printed += printed.len() as i32;
        let mut rest_start = s;
        for _ in 0..2 {
            if str_.get(rest_start) == Some(&b' ') {
                rest_start += 1;
            }
        }
        let rest: Vec<u8> = str_.get(rest_start..).unwrap_or(&[]).to_vec();
        self.txt.current_line = rest;
        self.txt.num_chars_left = self.txt.current_line.len() as i32;
        self.txt.line_width = self.screen.text_width(&self.txt.current_line.clone());
        if self.txt.num_chars_left == 0 && self.tdim(sdx).column <= (sd.w << 3) {
            return false;
        }
        let t = self.tdim(sdx);
        t.column = 0;
        t.line += 1;
        self.txt.line_count += 1;
        true
    }

    /// `clearDim`: the window cleared; the window that was current.
    pub fn txt_clear_dim(&mut self, dim: i32) -> i32 {
        let res = self.screen.cur_dim_index as i32;
        self.screen.set_screen_dim(dim as usize);
        let d = self.screen.cur_dim();
        let t = self.tdim(dim as usize);
        t.color1 = d.col1 as u8;
        t.color2 = d.col2 as u8;
        self.txt_clear_cur_dim();
        res
    }

    pub fn txt_clear_cur_dim(&mut self) {
        let d = self.screen.cur_dim_index;
        let tmp = self.screen.dim(d);
        let c2 = self.tdim(d).color2;
        self.screen.fill_rect(
            tmp.sx << 3,
            tmp.sy,
            ((tmp.sx + tmp.w) << 3) - 1,
            tmp.sy + tmp.h - 1,
            c2,
            None,
        );
        self.txt.line_count = 0;
        let t = self.tdim(d);
        t.column = 0;
        t.line = 0;
    }

    pub fn txt_reset_dim_text_positions(&mut self, dim: i32) {
        let t = self.tdim(dim as usize);
        t.column = 0;
        t.line = 0;
    }

    pub fn txt_set_page_break_flag(&mut self) {
        self.txt.allow_page_break = true;
        self.txt.line_count = 0;
    }

    pub fn txt_remove_page_break_flag(&mut self) {
        self.txt.allow_page_break = false;
    }

    /// `textPageBreak`: the "more" button, waited for (or for the speech's part of the text).
    pub fn txt_text_page_break(&mut self) {
        self.txt.page_break_string = self.get_lang_string(0x4073);
        let cp = self.screen.set_cur_page(0);
        let cf = self.screen.set_font(FontId::F6);
        let now = self.millis();
        self.timers.pause_single(11, true, now);
        self.fade_text = false;
        let mut reset_portrait = 0;
        let mut duration = 0;
        if self.update_char_num != -1 {
            reset_portrait = self.reset_portrait_after_speech_anim;
            self.reset_portrait_after_speech_anim = 0;
            duration = self.update_portrait_speech_anim_duration;
            if self.update_portrait_speech_anim_duration > 36 {
                self.update_portrait_speech_anim_duration = 36;
            }
        }
        let mut speech_part_time = 0u32;
        if self.speech_enabled()
            && self.active_voice_file_total_time != 0
            && self.txt.num_chars_total != 0
        {
            speech_part_time = self.millis()
                + ((self.txt.num_chars_printed.max(0) as u32 * self.active_voice_file_total_time)
                    / self.txt.num_chars_total as u32);
        }
        let sdx = self.screen.cur_dim_index;
        let dim = self.screen.dim(sdx);
        let bw = self.dialogue_button_width as i32;
        let mut x = ((dim.sx + dim.w) << 3) - (bw + 3);
        let y;
        let w = bw;
        if self.need_scene_restore != 0 && self.update_flags & 2 != 0 {
            if self.current_control_mode != 0 || self.update_flags & 2 == 0 {
                y = dim.sy + dim.h - 5;
            } else {
                x += 6;
                y = dim.sy + dim.h - 2;
            }
        } else {
            y = dim.sy + dim.h - 10;
        }
        // `guiSettings()`: frame 136/251, no fill (-1), height 9, label colour 144, text 2 down.
        self.gui_draw_box(x, y, w, 9, 136, 251, -1);
        let pbs = self.txt.page_break_string.clone();
        let tw = self.screen.text_width(&pbs);
        self.screen
            .print_text(&pbs, x + (w >> 1) - (tw >> 1), y + 2, 144, 0);
        self.remove_input_top();
        let mut lp = true;
        let mut target = false;
        while lp && !self.should_quit() {
            let mut input = self.check_input(None, false, 0) & 0xFF;
            self.remove_input_top();
            while input == 0 && !self.should_quit() {
                self.update();
                if self.speech_enabled()
                    && speech_part_time != 0
                    && (self.millis() > speech_part_time || self.snd_update_character_speech() != 2)
                {
                    lp = false;
                    input = Self::KEY_RETURN as i32;
                    break;
                }
                input = self.check_input(None, false, 0) & 0xFF;
                self.remove_input_top();
            }
            self.gui_notify_button_list_changed();
            if input == Self::KEY_SPACE as i32 || input == Self::KEY_RETURN as i32 {
                lp = false;
            } else if input == 199 || input == 201 {
                if Self::pos_within_rect(self.mouse_x, self.mouse_y, x, y, x + w, y + 9) {
                    target = true;
                }
            } else if (input == 200 || input == 202) && target {
                lp = false;
            }
        }
        let c2 = self.tdim(sdx).color2;
        self.screen.fill_rect(x, y, x + w - 1, y + 9 - 1, c2, None);
        self.txt_clear_cur_dim();
        self.screen.update_screen();
        let now = self.millis();
        self.timers.pause_single(11, false, now);
        if self.update_char_num != -1 {
            self.reset_portrait_after_speech_anim = reset_portrait;
            if duration > 36 {
                duration -= 36;
            } else {
                duration >>= 1;
            }
            self.update_portrait_speech_anim_duration = duration;
        }
        self.screen.set_font(cf);
        self.screen.set_cur_page(cp);
        self.remove_input_top();
    }
}
