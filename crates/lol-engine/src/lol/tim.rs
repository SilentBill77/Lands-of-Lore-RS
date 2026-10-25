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

//! The scene scripts (`.TIM`, ScummVM's `TIMInterpreter`, `TIMInterpreter_LoL` and
//! `TimAnimator`): up to ten functions running side by side, each a list of timed commands;
//! their animations, texts, voices and the game's own opcodes (`tlol_…`).

use super::wsa::Wsa;
use super::{Lol, TICK};
use crate::screen::{CR_NO_P_CHECK, FontId, Pal};

pub const WSA_SLOTS: usize = 6;
pub const ANIM_PARTS: usize = 10;
pub const COUNT_FUNCS: usize = 10;

/// Which table of the game's opcodes a script calls (`_timIntroOpcodes` …).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimOpcodes {
    Intro,
    Outro,
    Ingame,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TimFunction {
    pub ip: Option<usize>,
    pub last_time: u32,
    pub next_time: u32,
    pub loop_ip: Option<usize>,
    pub avtl: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WsaSlot {
    pub anim: i32,
    pub x: i16,
    pub y: i16,
    pub wsa_flags: u16,
    pub offscreen: u16,
}

/// A loaded script (`TIM`).
#[derive(Debug, Clone)]
pub struct Tim {
    pub filename: String,
    pub clicked_button: u16,
    pub dlg_func: i16,
    pub proc_func: i16,
    pub proc_param: u16,
    pub func: [TimFunction; COUNT_FUNCS],
    pub wsa: [WsaSlot; WSA_SLOTS],
    pub avtl: Vec<u16>,
    pub text: Vec<u8>,
    pub opcodes: TimOpcodes,
    pub is_lol_outro: bool,
    pub lol_character: u8,
}

impl Tim {
    /// A string of the script's `TEXT` (offsets little-endian).
    pub fn text_entry(&self, idx: u16) -> Vec<u8> {
        let at = idx as usize * 2;
        let Some(o) = self
            .text
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        else {
            return Vec::new();
        };
        let rest = self.text.get(o..).unwrap_or(&[]);
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        rest[..end].to_vec()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AnimPart {
    pub first_frame: u16,
    pub last_frame: u16,
    pub cycles: u16,
    pub next_part: i16,
    pub part_delay: i16,
    pub field_a: u16,
    pub sfx_index: i16,
    pub sfx_frame: u16,
}

#[derive(Default)]
pub struct TimAnimation {
    pub wsa: Option<Wsa>,
    pub x: i16,
    pub y: i16,
    pub next_frame: u32,
    pub enable: u8,
    pub field_d: u8,
    pub frame_delay: u8,
    pub cur_part: i8,
    pub cur_frame: u8,
    pub cycles_completed: u8,
    pub wsa_copy_params: u16,
    pub last_part: i8,
    pub parts: [AnimPart; ANIM_PARTS],
}

/// Which script is running (`_currentTim`): one of the level's slots, or a sequence's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimRef {
    Active(usize),
    Seq,
}

/// The interpreter's state (one for the game, others for the intro and the end).
pub struct TimInterp {
    /// `TIMInterpreter_LoL` (the game's) rather than the plain one.
    pub lol: bool,
    pub anims: Vec<TimAnimation>,
    pub use_parts: bool,
    pub draw_page2: usize,
    pub pal_delay_inc: i32,
    pub pal_diff: i32,
    pub pal_delay_acc: i32,
    pub abort_flag: i32,
    pub current_func: usize,
    pub finished: bool,
    pub lang_data: Option<Vec<u8>>,
    pub text_displayed: bool,
    pub text_area_buffer: Vec<u8>,
    pub voc_files: Vec<String>,
    pub current: Option<TimRef>,
}

impl TimInterp {
    pub fn new(lol: bool) -> Self {
        Self {
            lol,
            anims: (0..WSA_SLOTS)
                .map(|_| TimAnimation {
                    last_part: -1,
                    ..Default::default()
                })
                .collect(),
            use_parts: lol,
            draw_page2: if lol { 0 } else { 8 },
            pal_delay_inc: 0,
            pal_diff: 0,
            pal_delay_acc: 0,
            abort_flag: 0,
            current_func: 0,
            finished: false,
            lang_data: None,
            text_displayed: false,
            text_area_buffer: vec![0; 320 * 40],
            voc_files: vec![String::new(); 120],
            current: None,
        }
    }

    pub fn table_entry(&self, idx: u16) -> Option<Vec<u8>> {
        let d = self.lang_data.as_ref()?;
        let at = idx as usize * 2;
        let o = u16::from_le_bytes([*d.get(at)?, *d.get(at + 1)?]) as usize;
        let rest = d.get(o..)?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Some(rest[..end].to_vec())
    }
}

impl Lol {
    /// `TIMInterpreter::load`.
    pub fn tim_load(&mut self, filename: &str, opcodes: TimOpcodes) -> Option<Tim> {
        let bytes = self.res.file_data(filename)?;
        if bytes.len() < 12 {
            return None;
        }
        let mut text = Vec::new();
        let mut avtl = Vec::new();
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let size = u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
            let body = bytes
                .get(pos + 8..(pos + 8 + size).min(bytes.len()))
                .unwrap_or(&[]);
            match id {
                b"TEXT" => text = body.to_vec(),
                b"AVTL" => {
                    avtl = body
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|w| u16::from_le_bytes([w[0], w[1]]))
                        .collect()
                }
                _ => {}
            }
            pos += 8 + size + (size & 1);
        }
        if avtl.is_empty() {
            return None;
        }
        let mut func = [TimFunction::default(); COUNT_FUNCS];
        for (i, f) in func
            .iter_mut()
            .enumerate()
            .take(avtl.len().min(COUNT_FUNCS))
        {
            f.avtl = Some(avtl[i] as usize);
        }
        Some(Tim {
            filename: filename.to_string(),
            clicked_button: 0,
            dlg_func: 0,
            proc_func: -1,
            proc_param: 0,
            func,
            wsa: [WsaSlot::default(); WSA_SLOTS],
            avtl,
            text,
            opcodes,
            is_lol_outro: filename.eq_ignore_ascii_case("LOLFINAL.TIM"),
            lol_character: 0,
        })
    }

    pub fn tim_mut(&mut self, r: TimRef) -> Option<&mut Tim> {
        match r {
            TimRef::Active(i) => self.active_tim.get_mut(i).and_then(|t| t.as_mut()),
            TimRef::Seq => self.seq_tim.as_mut(),
        }
    }

    fn cur(&mut self) -> Option<&mut Tim> {
        let r = self.tim.current?;
        self.tim_mut(r)
    }

    fn avtl_word(&mut self, at: usize) -> u16 {
        self.cur()
            .and_then(|t| t.avtl.get(at).copied())
            .unwrap_or(0)
    }

    /// `TIMInterpreter::exec`: runs the script's functions until they wait (or to the end when
    /// `lp`); the button clicked in its dialogue.
    pub fn tim_exec(&mut self, r: TimRef, mut lp: bool) -> i32 {
        if self.tim_mut(r).is_none() {
            return 0;
        }
        let prev = self.tim.current.replace(r);
        let now = self.millis();
        if let Some(t) = self.tim_mut(r)
            && t.func[0].ip.is_none()
        {
            t.func[0].ip = t.func[0].avtl;
            t.func[0].next_time = now;
            t.func[0].last_time = now;
        }
        loop {
            self.tim_update();
            let mut cf = 0;
            while cf < COUNT_FUNCS {
                self.tim.current_func = cf;
                if self.cur().map_or(-1, |t| t.proc_func) != -1 {
                    let pp = self.cur().map_or(0, |t| t.proc_param);
                    self.tim_exec_command(28, &[pp]);
                }
                self.tim_update();
                self.tim_check_speech_progress();
                // `TIM::Function &cur`: bound here, it stays the same function even when a
                // command changes `_currentFunc` (to 11 on -1).
                let fi = cf;
                let mut running = true;
                let mut cnt = 0;
                while let Some(f) = self.cur().map(|t| t.func[fi]) {
                    let Some(ip) = f.ip else { break };
                    if !(f.next_time <= self.millis() && running) || self.should_quit() {
                        break;
                    }
                    if cnt > 0 {
                        if self.cur().map_or(-1, |t| t.proc_func) != -1 {
                            let pp = self.cur().map_or(0, |t| t.proc_param);
                            self.tim_exec_command(28, &[pp]);
                        }
                        self.tim_update();
                    }
                    cnt += 1;
                    let opcode = (self.avtl_word(ip + 2) & 0xFF) as i8 as i32;
                    let params: Vec<u16> = (0..16).map(|k| self.avtl_word(ip + 3 + k)).collect();
                    match self.tim_exec_command(opcode, &params) {
                        -1 => {
                            lp = false;
                            running = false;
                            self.tim.current_func = 11;
                        }
                        -2 => running = false,
                        -3 => {
                            let cur = self.tim.current_func as i16;
                            if let Some(t) = self.cur() {
                                t.proc_func = cur;
                                t.dlg_func = -1;
                            }
                        }
                        22 => {
                            if let Some(t) = self.cur() {
                                t.func[fi].loop_ip = None;
                            }
                        }
                        _ => {}
                    }
                    let c = fi;
                    let tick = TICK;
                    if let Some(t) = self.cur()
                        && let Some(ip) = t.func[c].ip
                    {
                        let len = t.avtl.get(ip).copied().unwrap_or(1).max(1) as usize;
                        let nip = ip + len;
                        t.func[c].ip = Some(nip);
                        t.func[c].last_time = t.func[c].next_time;
                        let d = t.avtl.get(nip + 1).copied().unwrap_or(0) as u32;
                        t.func[c].next_time += d * tick;
                    }
                }
                cf = self.tim.current_func + 1;
            }
            if !lp || self.should_quit() {
                break;
            }
            // The original loops at once; a tick of waiting keeps the machine responsive.
            self.screen.system.delay_millis(1);
        }
        let res = self.tim_mut(r).map_or(0, |t| t.clicked_button as i32);
        self.tim.current = prev;
        res
    }

    fn tim_update(&mut self) {
        if self.tim.lol {
            self.update();
        } else {
            self.update_input();
        }
    }

    fn tim_check_speech_progress(&mut self) {
        if !self.tim.lol {
            return;
        }
        let cf = self.tim.current_func.min(COUNT_FUNCS - 1);
        let (pp, lip) = match self.cur() {
            Some(t) => (t.proc_param, t.func[cf].loop_ip),
            None => return,
        };
        if self.speech_enabled()
            && pp > 1
            && lip.is_some()
            && self.snd_update_character_speech() != 2
        {
            if let Some(t) = self.cur() {
                t.func[cf].loop_ip = None;
                t.dlg_func = cf as i16;
            }
            self.tim_advance_to_opcode(21);
            if let Some(t) = self.cur() {
                t.dlg_func = -1;
            }
            self.anim_reset(5, false);
        }
    }

    fn tim_advance_to_opcode(&mut self, opcode: u16) {
        let now = self.millis();
        let Some(t) = self.cur() else { return };
        let df = t.dlg_func;
        if !(0..COUNT_FUNCS as i16).contains(&df) {
            return;
        }
        let f = &mut t.func[df as usize];
        let Some(mut ip) = f.ip else { return };
        let mut len = t.avtl.get(ip).copied().unwrap_or(0);
        let mut guard = 0;
        while (t.avtl.get(ip + 2).copied().unwrap_or(1) & 0xFF) != opcode && guard < 10_000 {
            guard += 1;
            if (t.avtl.get(ip + 2).copied().unwrap_or(1) & 0xFF) == 1 {
                if let Some(w) = t.avtl.get_mut(ip) {
                    *w = len;
                }
                break;
            }
            len = t.avtl.get(ip).copied().unwrap_or(1).max(1);
            ip += len as usize;
        }
        let f = &mut t.func[df as usize];
        f.ip = Some(ip);
        f.next_time = now;
    }

    pub fn tim_reset_dialogue_state(&mut self, r: TimRef) {
        let n = if self.dialogue_num_buttons != 0 {
            self.dialogue_num_buttons as u16
        } else {
            1
        };
        if let Some(t) = self.tim_mut(r) {
            t.proc_func = 0;
            t.proc_param = n;
            t.clicked_button = 0;
            t.dlg_func = -1;
        }
    }

    pub fn tim_stop_all_funcs(&mut self, r: TimRef) {
        if let Some(t) = self.tim_mut(r) {
            for f in t.func.iter_mut() {
                f.ip = None;
            }
        }
    }

    fn tim_exec_command(&mut self, cmd: i32, p: &[u16]) -> i32 {
        let lol = self.tim.lol;
        let now = self.millis();
        let cf = self.tim.current_func.min(COUNT_FUNCS - 1);
        let p0 = p.first().copied().unwrap_or(0);
        match cmd {
            0 => {
                if let Some(t) = self.cur() {
                    t.wsa = [WsaSlot::default(); WSA_SLOTS];
                    t.func[0].ip = t.func[0].avtl;
                    t.func[0].last_time = now;
                }
                1
            }
            1 => {
                if lol {
                    loop {
                        let (df, cb) = self
                            .cur()
                            .map_or((0, 1), |t| (t.dlg_func, t.clicked_button));
                        if df != -1 || cb != 0 || self.should_quit() {
                            break;
                        }
                        self.tim_update();
                        let b = self.process_dialogue();
                        if let Some(t) = self.cur() {
                            t.clicked_button = b as u16;
                        }
                    }
                    if let Some(t) = self.cur() {
                        for f in t.func.iter_mut() {
                            f.ip = None;
                        }
                    }
                    -1
                } else {
                    if let Some(t) = self.cur() {
                        t.func[cf].ip = None;
                    }
                    if self.tim.current_func == 0 {
                        self.tim.finished = true;
                    }
                    -2
                }
            }
            2 => {
                let index = p0 as usize % WSA_SLOTS;
                let (x, y, off, flags) = (p[2] as i16, p[3] as i16, p[4], p[5]);
                let name = self.cur().map(|t| t.text_entry(p[1])).unwrap_or_default();
                let name = String::from_utf8_lossy(&name).to_string();
                if let Some(t) = self.cur() {
                    t.wsa[index] = WsaSlot {
                        anim: 0,
                        x,
                        y,
                        wsa_flags: flags,
                        offscreen: off,
                    };
                }
                let a = self
                    .tim_init_anim_struct(index, &name, x as i32, y as i32, 10, off as i32, flags);
                if let Some(t) = self.cur() {
                    t.wsa[index].anim = a;
                }
                1
            }
            3 => {
                let index = p0 as usize % WSA_SLOTS;
                let slot = self.cur().map(|t| t.wsa[index]).unwrap_or_default();
                if slot.anim == 0 {
                    return 0;
                }
                if slot.offscreen != 0 {
                    self.anim_reset(index, false);
                    if let Some(t) = self.cur() {
                        t.wsa[index].anim = 0;
                    }
                } else {
                    self.anim_reset(index, true);
                    if let Some(t) = self.cur() {
                        t.wsa[index] = WsaSlot::default();
                    }
                }
                1
            }
            4 => {
                let func = (p0 as usize).min(COUNT_FUNCS - 1);
                if let Some(t) = self.cur() {
                    if t.func[func].avtl.is_some() {
                        t.func[func].ip = t.func[func].avtl;
                    } else {
                        let a = t.avtl.get(func).copied().unwrap_or(0) as usize;
                        t.func[func].avtl = Some(a);
                        t.func[func].ip = Some(a);
                    }
                }
                1
            }
            5 => {
                let func = (p0 as usize).min(COUNT_FUNCS - 1);
                if let Some(t) = self.cur() {
                    t.func[func].ip = None;
                }
                1
            }
            6 => {
                let dp = self.tim.draw_page2;
                self.anim_display_frame(p0 as usize % WSA_SLOTS, dp, p[1] as i32, -1);
                1
            }
            7 => {
                if lol {
                    return 0;
                }
                if self.cur().is_some_and(|t| t.is_lol_outro) {
                    self.tim_display_text_outro(p0, p[1] as i16, 0xF2);
                } else {
                    self.tim_display_text(p0, p[1] as i16);
                }
                1
            }
            8 => {
                let string_id = p0;
                let index = p[1] as usize % 120;
                let (mut name, outro, ch) = match self.cur() {
                    Some(t) => (
                        String::from_utf8_lossy(&t.text_entry(string_id)).to_string(),
                        t.is_lol_outro,
                        t.lol_character,
                    ),
                    None => return 1,
                };
                if index == 2 && outro {
                    let c = match ch {
                        0 => 'K',
                        1 => 'A',
                        2 => 'M',
                        3 => 'C',
                        _ => 'A',
                    };
                    name = format!("CONGRAT{c}.VOC");
                }
                for _ in 0..4 {
                    name.pop();
                }
                self.tim.voc_files[index] = name;
                1
            }
            9 => {
                self.tim.voc_files[p0 as usize % 120].clear();
                1
            }
            10 => {
                let index = p0 as usize;
                let volume = (p[1] as i32 * 255) / 100;
                if index < 120 && !self.tim.voc_files[index].is_empty() {
                    let f = self.tim.voc_files[index].clone();
                    self.snd_voice_play(&f, volume, true);
                } else {
                    // `Sound::playSoundEffect`: the track of the sound file, not the game's list.
                    self.screen
                        .system
                        .mixer()
                        .play_adlib_effect(index as u8, 0xFF);
                }
                1
            }
            12 => {
                let file = self.cur().map(|t| t.text_entry(p0)).unwrap_or_default();
                let file = String::from_utf8_lossy(&file).to_string();
                self.snd_load_sound_file_named(&file);
                1
            }
            13 | 16 | 17 | 24 => 1,
            28 | 29 if !lol => 1,
            14 => {
                // `Sound::playTrack`: the track of the loaded file (not `snd_playTrack`).
                self.screen.system.mixer().play_music(p0 as u8);
                1
            }
            20 => {
                let set = !lol || !self.speech_enabled() || self.snd_update_character_speech() == 2;
                if set {
                    if let Some(t) = self.cur() {
                        t.func[cf].loop_ip = t.func[cf].ip;
                    }
                } else {
                    // `advanceToOpcode` works on `dlgFunc`, which the original leaves as it is.
                    // While no dialogue function is set (-1, where ScummVM would read outside
                    // the functions) it is the current function, for this call only.
                    let df = self.cur().map_or(-1, |t| t.dlg_func);
                    let unset = !(0..COUNT_FUNCS as i16).contains(&df);
                    if unset && let Some(t) = self.cur() {
                        t.dlg_func = cf as i16;
                    }
                    self.tim_advance_to_opcode(21);
                    if unset && let Some(t) = self.cur() {
                        t.dlg_func = df;
                    }
                }
                1
            }
            21 => {
                let Some(lip) = self.cur().and_then(|t| t.func[cf].loop_ip) else {
                    return -2;
                };
                if let Some(t) = self.cur() {
                    t.func[cf].ip = Some(lip);
                }
                if !lol || self.snd_update_character_speech() != 2 {
                    let factor = p0 as u32;
                    if factor != 0 {
                        let random = self.rnd.range(0, 0x8000) as u32;
                        let wait = (random * factor) / 0x8000;
                        if let Some(t) = self.cur() {
                            t.func[cf].next_time += wait * TICK;
                        }
                    }
                }
                -2
            }
            22 => {
                if let Some(t) = self.cur() {
                    t.func[cf].loop_ip = None;
                }
                1
            }
            23 => {
                if let Some(t) = self.cur() {
                    for f in t.func.iter_mut() {
                        if f.ip.is_some() {
                            f.next_time = now;
                        }
                    }
                }
                1
            }
            25 => {
                let opcode = p0;
                let params: Vec<u16> = p[1..].to_vec();
                let table = self.cur().map(|t| t.opcodes);
                match table {
                    Some(tab) => self.tim_opcode(tab, opcode, &params),
                    None => 0,
                }
            }
            26 => {
                let func = (p0 as usize).min(COUNT_FUNCS - 1);
                if let Some(t) = self.cur() {
                    t.func[func].ip = t.func[func].avtl;
                    t.func[func].last_time = now;
                    t.func[func].next_time = now;
                }
                1
            }
            27 => {
                let func = (p0 as usize).min(COUNT_FUNCS - 1);
                if let Some(t) = self.cur() {
                    t.func[func].ip = None;
                    t.func[func].last_time = now;
                    t.func[func].next_time = now;
                }
                1
            }
            28 if lol => {
                let res = self.process_dialogue();
                let pp = self.cur().map_or(0, |t| t.proc_param);
                if res == 0 || pp == 0 {
                    return res;
                }
                self.snd_stop_speech(false);
                if let Some(t) = self.cur() {
                    let pf = t.proc_func;
                    if (0..COUNT_FUNCS as i16).contains(&pf) {
                        t.func[pf as usize].loop_ip = None;
                    }
                    t.dlg_func = pf;
                    t.proc_func = -1;
                    t.clicked_button = res as u16;
                }
                self.anim_reset(5, false);
                if pp != 0 {
                    self.tim_advance_to_opcode(21);
                }
                res
            }
            29 if lol => {
                let func = p0;
                if let Some(t) = self.cur() {
                    t.proc_param = func;
                    t.clicked_button = 0;
                }
                let mut strs: [Vec<u8>; 3] = Default::default();
                let mut cnt = 0;
                for i in 1..4 {
                    if p[i] != 0xFFFF {
                        strs[i - 1] = self.get_lang_string(p[i]);
                        cnt += 1;
                    }
                }
                let [a, b, c] = strs;
                self.setup_dialogue_buttons(cnt, a, b, c);
                self.gui_notify_button_list_changed();
                -3
            }
            30 => -1,
            _ => 0,
        }
    }

    // ---- animations

    /// `initAnimStruct` (the game's or the plain interpreter's).
    pub fn tim_init_anim_struct(
        &mut self,
        index: usize,
        filename: &str,
        x: i32,
        y: i32,
        frame_delay: i32,
        offscreen: i32,
        wsa_flags: u16,
    ) -> i32 {
        let file = format!("{filename}.WSA");
        if self.tim.lol {
            let mut open_flags = 0;
            if wsa_flags & 0x10 != 0 {
                open_flags |= 2;
            }
            if wsa_flags & 8 != 0 {
                open_flags |= 1;
            }
            let mut wsa = self.open_wsa(&file, open_flags, true);
            if let Some(w) = &wsa
                && let Some(p) = &w.palette
            {
                let mut p3 = self.screen.palette(3).clone();
                p3.0[..p.0.len().min(768)].copy_from_slice(&p.0[..p.0.len().min(768)]);
                *self.screen.palette_mut(3) = p3;
            }
            if wsa_flags & 1 != 0 {
                if self.screen.fade_flag != 1 {
                    self.screen.fade_clear_scene_window(10);
                }
                let p0 = self.screen.palette(0).clone();
                let mut p3 = self.screen.palette(3).clone();
                p3.copy_from(&p0, 128, 128, 128);
                *self.screen.palette_mut(3) = p3;
            } else if wsa_flags & 2 != 0 {
                self.screen.fade_to_black(10);
            }
            if let Some(w) = wsa.as_mut()
                && wsa_flags & 7 != 0
            {
                w.display_frame(&mut self.screen, 0, 0, x, y, 0, None, None);
            }
            if wsa_flags & 3 != 0 {
                let mut p3 = self.screen.palette(3).clone();
                self.screen.load_special_colors(&mut p3);
                *self.screen.palette_mut(3) = p3.clone();
                self.screen.fade_palette(&p3, 10);
                self.screen.fade_flag = 0;
            }
            self.anim_init(index, wsa, x, y, wsa_flags, frame_delay);
            return index as i32 + 1;
        }
        // The plain interpreter (intro, end).
        self.tim.draw_page2 = if self.cur().is_some_and(|t| t.is_lol_outro) {
            0
        } else {
            8
        };
        let mut open_flags = 0;
        if wsa_flags & 0x10 != 0 {
            open_flags |= 2;
        }
        open_flags |= 1;
        if offscreen == 2 {
            open_flags = 1;
        }
        let mut wsa = self.open_wsa(&file, open_flags, index == 1);
        if index == 1
            && let Some(p) = wsa.as_ref().and_then(|w| w.palette.clone())
        {
            let mut p0 = self.screen.palette(0).clone();
            let n = p.0.len().min(p0.0.len());
            p0.0[..n].copy_from_slice(&p.0[..n]);
            *self.screen.palette_mut(0) = p0;
        }
        let (x, y) = (if x == -1 { 0 } else { x }, if y == -1 { 0 } else { y });
        let dp = self.tim.draw_page2;
        if wsa_flags & 2 != 0 {
            let p1 = self.screen.palette(1).clone();
            self.screen.fade_palette(&p1, 15);
            self.screen.clear_page(dp);
            if dp != 0 {
                self.screen.checked_page_update(8, 4);
            }
            self.screen.update_screen();
        }
        let has_wsa = wsa.is_some();
        if (wsa_flags & 4 != 0) || !has_wsa {
            let cps = format!("{filename}.CPS");
            if let Some(d) = self.res.file_data(&cps) {
                let mut p0 = self.screen.palette(0).clone();
                self.screen.load_bitmap(&d, 3, Some(&mut p0));
                *self.screen.palette_mut(0) = p0;
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 2, dp, CR_NO_P_CHECK);
                if dp != 0 {
                    self.screen.checked_page_update(8, 4);
                }
                self.screen.update_screen();
            }
            if let Some(w) = wsa.as_mut() {
                w.display_frame(&mut self.screen, 0, 0, x, y, 0, None, None);
            }
        }
        if wsa_flags & 2 != 0 {
            let p0 = self.screen.palette(0).clone();
            self.screen.fade_palette(&p0, 30);
        }
        self.anim_init(index, wsa, x, y, wsa_flags, 0);
        index as i32 + 1
    }

    pub fn tim_free_anim_struct(&mut self, index: usize) -> i32 {
        self.anim_reset(index, true);
        1
    }

    pub fn anim_init(
        &mut self,
        index: usize,
        wsa: Option<Wsa>,
        x: i32,
        y: i32,
        copy_params: u16,
        frame_delay: i32,
    ) {
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.wsa = wsa;
        a.x = x as i16;
        a.y = y as i16;
        a.wsa_copy_params = copy_params;
        a.frame_delay = frame_delay as u8;
        a.enable = 0;
        a.last_part = -1;
    }

    pub fn anim_reset(&mut self, index: usize, clear: bool) {
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.field_d = 0;
        a.enable = 0;
        a.wsa = None;
        if clear {
            *a = TimAnimation::default();
        }
    }

    pub fn anim_display_frame(&mut self, index: usize, page: usize, frame: i32, flags: i32) {
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        let page = if a.wsa_copy_params & 0x4000 != 0 {
            2
        } else {
            page
        };
        let (x, y) = (a.x as i32, a.y as i32);
        let f = if flags == -1 {
            a.wsa_copy_params & 0xF0FF
        } else {
            flags as u16
        };
        if let Some(w) = a.wsa.as_mut() {
            w.display_frame(&mut self.screen, frame, page, x, y, f, None, None);
        }
        if page == 0 {
            self.screen.update_screen();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn anim_setup_part(
        &mut self,
        index: usize,
        part: usize,
        first: i32,
        last: i32,
        cycles: i32,
        next_part: i32,
        part_delay: i32,
        f: i32,
        sfx_index: i32,
        sfx_frame: i32,
    ) {
        let a = &mut self.tim.anims[index % WSA_SLOTS].parts[part % ANIM_PARTS];
        a.first_frame = first as u16;
        a.last_frame = last as u16;
        a.cycles = cycles as u16;
        a.next_part = next_part as i16;
        a.part_delay = part_delay as i16;
        a.field_a = f as u16;
        a.sfx_index = sfx_index as i16;
        a.sfx_frame = sfx_frame as u16;
    }

    pub fn anim_start(&mut self, index: usize, part: usize) {
        if !self.tim.use_parts {
            return;
        }
        let now = self.millis();
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.cur_part = part as i8;
        let p = a.parts[part % ANIM_PARTS];
        a.enable = 1;
        a.next_frame = now + a.frame_delay as u32 * TICK;
        a.cur_frame = p.first_frame as u8;
        a.cycles_completed = 0;
        let (x, y, cf) = (a.x as i32, a.y as i32, a.cur_frame as i32);
        if let Some(w) = a.wsa.as_mut() {
            w.display_frame(&mut self.screen, cf - 1, 0, x, y, 0, None, None);
        }
    }

    pub fn anim_stop(&mut self, index: usize) {
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.enable = 0;
        a.field_d = 0;
        if index == 5 {
            a.wsa = None;
        }
    }

    pub fn anim_update(&mut self, index: usize) {
        if !self.tim.use_parts {
            return;
        }
        let now = self.millis();
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        if a.enable == 0 || a.next_frame >= now {
            return;
        }
        let mut p = a.parts[(a.cur_part.max(0) as usize) % ANIM_PARTS];
        a.next_frame = 0;
        let step: i32 = if p.last_frame >= p.first_frame {
            a.cur_frame = a.cur_frame.wrapping_add(1);
            1
        } else {
            a.cur_frame = a.cur_frame.wrapping_sub(1);
            -1
        };
        if a.cur_frame as i32 == p.last_frame as i32 + step {
            a.cycles_completed = a.cycles_completed.wrapping_add(1);
            if a.cycles_completed as u16 > p.cycles || a.field_d != 0 {
                a.last_part = a.cur_part;
                if p.next_part == -1 || (a.field_d != 0 && p.field_a != 0) {
                    a.enable = 0;
                    a.field_d = 0;
                    return;
                }
                a.next_frame += p.part_delay as u32 * TICK;
                a.cur_part = p.next_part as i8;
                p = a.parts[(a.cur_part.max(0) as usize) % ANIM_PARTS];
                a.cur_frame = p.first_frame as u8;
                a.cycles_completed = 0;
            } else {
                a.cur_frame = p.first_frame as u8;
            }
        }
        let sfx =
            (p.sfx_index != -1 && p.sfx_frame == a.cur_frame as u16).then_some(p.sfx_index as i32);
        if let Some(s) = sfx {
            self.snd_play_sound_effect(s, -1);
        }
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.next_frame += a.frame_delay as u32 * TICK;
        let (x, y, cf) = (a.x as i32, a.y as i32, a.cur_frame as i32);
        if let Some(w) = a.wsa.as_mut() {
            w.display_frame(&mut self.screen, cf - 1, 0, x, y, 0, None, None);
        }
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        a.next_frame += self.screen.system.millis();
    }

    pub fn anim_play_part(&mut self, index: usize, first: i32, last: i32, delay: i32) {
        if !self.tim.use_parts || self.tim.anims[index % WSA_SLOTS].wsa.is_none() {
            return;
        }
        let step = if last >= first { 1 } else { -1 };
        let mut i = first;
        let (t1, t2) = (
            self.transparency_table1.clone(),
            self.transparency_table2.clone(),
        );
        while i != last + step && !self.should_quit() {
            let next = self.millis() + delay.max(0) as u32 * TICK;
            let a = &mut self.tim.anims[index % WSA_SLOTS];
            let (x, y, cp) = (a.x as i32, a.y as i32, a.wsa_copy_params);
            if cp & 0x4000 != 0 {
                self.screen.copy_region(112, 0, 112, 0, 176, 120, 6, 2, 0);
                let f = if cp & 0x1000 != 0 { 0x5000 } else { 0x4000 };
                if let Some(w) = self.tim.anims[index % WSA_SLOTS].wsa.as_mut() {
                    w.display_frame(&mut self.screen, i - 1, 2, x, y, f, Some(&t1), Some(&t2));
                }
                self.screen.copy_region(112, 0, 112, 0, 176, 120, 2, 0, 0);
            } else if let Some(w) = self.tim.anims[index % WSA_SLOTS].wsa.as_mut() {
                w.display_frame(&mut self.screen, i - 1, 0, x, y, 0, None, None);
            }
            self.screen.update_screen();
            let del = next as i64 - self.millis() as i64;
            if del > 0 {
                self.delay(del as u32, true);
            }
            i += step;
        }
    }

    pub fn anim_reset_last_part(&mut self, index: usize) -> i32 {
        let a = &mut self.tim.anims[index % WSA_SLOTS];
        std::mem::replace(&mut a.last_part, -1) as i32
    }

    // ---- texts of the plain interpreter

    pub fn tim_display_text(&mut self, text_id: u16, flags: i16) {
        let text = self.tim.table_entry(text_id);
        if self.tim.text_displayed {
            let b = self.tim.text_area_buffer.clone();
            self.screen.copy_block_to_page(0, 0, 160, 320, 40, &b);
            self.tim.text_displayed = false;
        }
        let Some(mut text) = text else { return };
        if text.is_empty() {
            return;
        }
        if text[0] == b'$'
            && let Some(end) = text[1..].iter().position(|&c| c == b'$')
        {
            let filename = String::from_utf8_lossy(&text[1..1 + end]).to_string();
            if self.speech_enabled() {
                self.snd_voice_play(&filename, 255, false);
            }
            text = text[end + 2..].to_vec();
        }
        self.tim_setup_text_palette(if flags < 0 { 1 } else { flags as usize }, 0);
        if flags < 0 {
            const MAP: [u8; 16] = [0x00, 0xF0, 0xFE, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
            self.screen.set_font(FontId::F8);
            self.screen.set_text_color_map(&MAP);
            self.screen.char_spacing = -2;
        }
        self.screen.line_spacing = -4;
        self.tim.text_area_buffer = self.screen.copy_region_to_buffer(0, 0, 160, 320, 40);
        self.tim.text_displayed = true;
        let mut height_add = 0;
        if self.text_enabled() {
            // Every line counts, empty ones too; only the end after a last '\r' does not.
            let lines: Vec<&[u8]> = text.split(|&c| c == b'\r').collect();
            let n = lines.len();
            for (i, &line) in lines.iter().enumerate() {
                if i + 1 == n && line.is_empty() {
                    break;
                }
                let width = self.screen.text_width(line);
                if flags >= 0 {
                    self.screen
                        .print_text(line, (320 - width) >> 1, 160 + height_add, 0xF0, 0);
                } else {
                    self.screen
                        .print_text(line, (320 - width) >> 1, 188, 0xF0, 0);
                }
                height_add += self.screen.font_height();
            }
        }
        self.screen.line_spacing = 0;
        if flags < 0 {
            const MAP: [u8; 16] = [
                0x00, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0, 0, 0, 0,
            ];
            self.screen.set_font(FontId::Intro);
            self.screen.set_text_color_map(&MAP);
            self.screen.char_spacing = 0;
        }
    }

    pub fn tim_display_text_outro(&mut self, text_id: u16, flags: i16, color: u8) {
        if !self.text_enabled() && text_id & 0x8000 == 0 {
            return;
        }
        let text = self.tim.table_entry(text_id & 0x7FFF).unwrap_or_default();
        if flags > 0 {
            let b = self.tim.text_area_buffer.clone();
            self.screen.copy_block_to_page(0, 0, 0, 320, 40, &b);
        }
        if flags == 255 {
            return;
        }
        self.screen.set_font(FontId::Intro);
        const MAP: [u8; 16] = [0x00, 0xA0, 0xA1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        self.screen.set_text_color_map(&MAP);
        self.screen.char_spacing = 0;
        self.screen.line_spacing = -4;
        if flags == 0 {
            self.tim.text_area_buffer = self.screen.copy_region_to_buffer(0, 0, 0, 320, 40);
        }
        let mut y = 0;
        let lines: Vec<&[u8]> = text.split(|&c| c == b'\r').collect();
        let n = lines.len();
        for (i, &line) in lines.iter().enumerate() {
            if i + 1 == n && line.is_empty() {
                break;
            }
            let width = self.screen.text_width(line);
            if flags >= 0 {
                self.screen
                    .print_text(line, (320 - width) >> 1, y, color, 0);
            } else {
                self.screen.print_text(line, 0, y, color, 0);
            }
            y += self.screen.font_height() - 4;
        }
    }

    pub fn tim_setup_text_palette(&mut self, index: usize, fade_palette: i32) {
        const PAL: [u16; 18] = [
            0x00, 0x00, 0x00, 0x64, 0x64, 0x64, 0x61, 0x51, 0x30, 0x29, 0x48, 0x64, 0x00, 0x4B,
            0x3B, 0x64, 0x1E, 0x1E,
        ];
        let index = index.min(5);
        let mut p0 = self.screen.palette(0).clone();
        for i in 0..15 {
            let at = (240 + i) * 3;
            for k in 0..3 {
                p0.0[at + k] = ((((15 - i as u16) << 2) * PAL[index * 3 + k]) / 100) as u8;
            }
        }
        *self.screen.palette_mut(0) = p0.clone();
        if fade_palette == 0 && self.tim.pal_diff == 0 {
            self.screen.set_screen_palette(&p0);
        } else {
            let (inc, diff) = self.screen.get_fade_params(&p0, fade_palette);
            self.tim.pal_delay_inc = inc;
            self.tim.pal_diff = diff;
            self.tim.pal_delay_acc = 0;
        }
    }

    // ---- the game's opcodes

    fn tim_opcode(&mut self, table: TimOpcodes, opcode: u16, p: &[u16]) -> i32 {
        let name = match table {
            TimOpcodes::Intro => lol_data::tables::TIM_INTRO_FUNCTIONS.get(opcode as usize),
            TimOpcodes::Outro => lol_data::tables::TIM_OUTRO_FUNCTIONS.get(opcode as usize),
            TimOpcodes::Ingame => lol_data::tables::TIM_INGAME_FUNCTIONS.get(opcode as usize),
        };
        let Some(&name) = name else { return 0 };
        if self.trace_calls {
            eprintln!("  TIM {name} {:?}", &p[..p.len().min(6)]);
        }
        let p0 = p.first().copied().unwrap_or(0);
        let param = |i: usize| p.get(i).copied().unwrap_or(0);
        match name {
            "setupPaletteFade" => {
                let p0p = self.screen.palette(0).clone();
                let (inc, diff) = self.screen.get_fade_params(&p0p, p0 as i32);
                self.tim.pal_delay_inc = inc;
                self.tim.pal_diff = diff;
                self.tim.pal_delay_acc = 0;
                1
            }
            "loadPalette" => {
                let f = self.cur().map(|t| t.text_entry(p0)).unwrap_or_default();
                let f = String::from_utf8_lossy(&f).to_string();
                if let Some(d) = self.res.file_data(&f) {
                    // (`Screen::loadPalette` sets only the colours the file has)
                    let mut p = self.screen.palette(0).clone();
                    p.load(&d);
                    *self.screen.palette_mut(0) = p;
                }
                1
            }
            "setupPaletteFadeEx" => {
                self.screen.copy_palette(0, 1);
                let p0p = self.screen.palette(0).clone();
                let (inc, diff) = self.screen.get_fade_params(&p0p, p0 as i32);
                self.tim.pal_delay_inc = inc;
                self.tim.pal_diff = diff;
                self.tim.pal_delay_acc = 0;
                1
            }
            "processWsaFrame" => {
                let anim_index = self
                    .cur()
                    .map_or(0, |t| t.wsa[p0 as usize % WSA_SLOTS].anim)
                    - 1;
                if anim_index < 0 {
                    return 1;
                }
                let ai = anim_index as usize;
                let frame = param(1) as i32;
                let (x2, y2) = (param(2) as i16 as i32, param(3) as i16 as i32);
                let factor = (param(4) as i16 as i32).max(0);
                let a = &self.tim.anims[ai % WSA_SLOTS];
                let (x1, y1) = (a.x as i32, a.y as i32);
                let (w1, h1) = a.wsa.as_ref().map_or((0, 0), |w| (w.width, w.height));
                let (w2, h2) = ((w1 * factor) / 100, (h1 * factor) / 100);
                self.anim_display_frame(ai, 2, frame, -1);
                self.screen
                    .wsa_frame_animation_step(x1, y1, x2, y2, w1, h1, w2, h2, 2, 8, 0);
                self.screen.checked_page_update(8, 4);
                self.screen.update_screen();
                1
            }
            "displayText" => {
                if self.cur().is_some_and(|t| t.is_lol_outro) {
                    self.tim_display_text_outro(p0, param(1) as i16, param(2) as u8);
                } else {
                    self.tim_display_text(p0, param(1) as i16);
                }
                1
            }
            "initSceneWindowDialogue" => {
                self.init_scene_window_dialogue(p0 as i32);
                1
            }
            "restoreAfterSceneWindowDialogue" => {
                self.restore_after_scene_window_dialogue(p0 != 0);
                1
            }
            "giveItem" => {
                let item = self.make_item(p0 as i32, param(1) as i32, param(2) as i32);
                if self.add_item_to_inventory(item) {
                    return 1;
                }
                self.delete_item(item);
                0
            }
            "setPartyPosition" => {
                if p0 == 1 {
                    self.current_direction = param(1);
                } else if p0 == 0 {
                    self.current_block = param(1);
                    let (x, y) = self.calc_coordinates(self.current_block, 0x80, 0x80);
                    self.party_pos_x = x;
                    self.party_pos_y = y;
                }
                1
            }
            "fadeClearWindow" => {
                match p0 {
                    0 => self.screen.fade_clear_scene_window(10),
                    1 => {
                        let p0p = self.screen.palette(0).clone();
                        let mut p3 = self.screen.palette(3).clone();
                        p3.copy_from(&p0p, 128, 128, 128);
                        self.screen.load_special_colors(&mut p3);
                        *self.screen.palette_mut(3) = p3.clone();
                        self.screen.fade_palette(&p3, 10);
                        self.screen.fade_flag = 0;
                    }
                    2 => self.screen.fade_to_black(10),
                    3 => {
                        let mut p3 = self.screen.palette(3).clone();
                        self.screen.load_special_colors(&mut p3);
                        *self.screen.palette_mut(3) = p3.clone();
                        self.screen.fade_palette(&p3, 10);
                        self.screen.fade_flag = 0;
                    }
                    4 => {
                        if self.screen.fade_flag != 2 {
                            self.screen.fade_clear_scene_window(10);
                        }
                        self.gui_draw_play_field();
                        let p0p = self.screen.palette(0).clone();
                        let (br, le) = (self.brightness, self.lamp_effect as i32);
                        self.set_palette_brightness(&p0p, br, le);
                        self.screen.fade_flag = 0;
                    }
                    5 => {
                        let mut p3 = self.screen.palette(3).clone();
                        self.screen.load_special_colors(&mut p3);
                        *self.screen.palette_mut(3) = p3;
                        let p1 = self.screen.palette(1).clone();
                        self.screen.fade_palette(&p1, 10);
                        self.screen.fade_flag = 0;
                    }
                    _ => {}
                }
                1
            }
            "copyRegion" => {
                let q = |i: usize| param(i) as i16 as i32;
                self.screen.copy_region(
                    q(0),
                    q(1),
                    q(2),
                    q(3),
                    q(4),
                    q(5),
                    param(6) as usize,
                    param(7) as usize,
                    CR_NO_P_CHECK,
                );
                if param(7) == 0 {
                    self.screen.update_screen();
                }
                1
            }
            "characterChat" => {
                let s = self.get_lang_string(param(2));
                let params: Vec<i32> = p.iter().map(|&v| v as i16 as i32).collect();
                self.play_character_script_chat(
                    p0 as i16 as i32,
                    param(1) as i16 as i32,
                    1,
                    &s,
                    &params,
                    3,
                );
                1
            }
            "drawScene" => {
                self.gui_draw_scene(p0 as usize);
                1
            }
            "update" => {
                self.update();
                1
            }
            "clearTextField" => {
                if self.current_control_mode != 0 && !self.text_enabled() {
                    return 1;
                }
                self.screen.set_screen_dim(5);
                // (dimension 5 is in pixels)
                let d = self.screen.cur_dim();
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
            "loadSoundFile" => {
                self.snd_load_sound_file(p0 as i32);
                1
            }
            "playMusicTrack" => {
                self.snd_play_track(p0 as i32);
                1
            }
            "playDialogueTalkText" => {
                if !self.snd_play_character_speech(p0 as i32, 0, false) || self.text_enabled() {
                    let s = self.get_lang_string(p0);
                    let params: Vec<i32> = p.iter().map(|&v| v as i16 as i32).collect();
                    self.txt_print_dialogue_text2(4, &s, &params, 1);
                }
                1
            }
            "playSoundEffect" => {
                self.snd_play_sound_effect(p0 as i16 as i32, -1);
                1
            }
            "startBackgroundAnimation" => {
                self.anim_start(p0 as usize, param(1) as usize);
                1
            }
            "stopBackgroundAnimation" => {
                self.anim_stop(p0 as usize);
                1
            }
            "fadeInScene" => {
                let (scene, overlay) = match self.cur() {
                    Some(t) => (t.text_entry(p0), t.text_entry(param(1))),
                    None => return 1,
                };
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 0, 2, CR_NO_P_CHECK);
                let file = format!("{}.CPS", String::from_utf8_lossy(&scene));
                if let Some(d) = self.res.file_data(&file) {
                    let mut p0 = self.screen.palette(0).clone();
                    self.screen.load_bitmap(&d, 5, Some(&mut p0));
                    *self.screen.palette_mut(0) = p0;
                }
                let ovl_name = format!(
                    "{}/{}",
                    self.language_ext(),
                    String::from_utf8_lossy(&overlay)
                );
                let ovl = self
                    .res
                    .file_data(&ovl_name)
                    .or_else(|| self.res.file_data(&String::from_utf8_lossy(&overlay)))
                    .unwrap_or_default();
                for _ in 0..3 {
                    let end = self.millis() + 10 * TICK;
                    self.screen.copy_block_and_apply_overlay_outro(4, 2, &ovl);
                    self.screen
                        .copy_region(0, 0, 0, 0, 320, 200, 2, 0, CR_NO_P_CHECK);
                    self.screen.update_screen();
                    self.delay_until(end, false);
                }
                self.screen
                    .copy_region(0, 0, 0, 0, 320, 200, 4, 0, CR_NO_P_CHECK);
                self.screen.update_screen();
                1
            }
            "unusedResourceFunc" => 1,
            "fadeInPalette" => {
                let bitmap = self.cur().map(|t| t.text_entry(p0)).unwrap_or_default();
                let mut pal = Pal::default();
                if let Some(d) = self.res.file_data(&String::from_utf8_lossy(&bitmap)) {
                    self.screen.load_bitmap(&d, 3, Some(&mut pal));
                }
                self.screen.fade_palette(&pal, param(1) as i32);
                1
            }
            "fadeOutSound" => {
                self.screen.system.mixer().fade_out_music();
                1
            }
            "displayAnimFrame" => {
                let anim_index = self
                    .cur()
                    .map_or(0, |t| t.wsa[p0 as usize % WSA_SLOTS].anim)
                    - 1;
                if anim_index < 0 {
                    return 1;
                }
                let ai = anim_index as usize;
                if param(1) == 0xFFFF {
                    self.screen
                        .copy_region(0, 0, 0, 0, 320, 200, 0, 2, CR_NO_P_CHECK);
                } else {
                    self.anim_display_frame(ai, 2, param(1) as i32, 0);
                    let a = &self.tim.anims[ai % WSA_SLOTS];
                    if let Some(w) = a.wsa.as_ref() {
                        let (xa, ya, ww, wh) = (w.x_add, w.y_add, w.width, w.height);
                        self.screen.copy_region(xa, ya, xa, ya, ww, wh, 2, 0, 0);
                    }
                }
                1
            }
            "delayForChat" => {
                if !self.speech_enabled() {
                    self.delay(p0 as u32, false);
                }
                1
            }
            _ => 0,
        }
    }
}
