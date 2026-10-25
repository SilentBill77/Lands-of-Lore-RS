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

//! Westwood's AdLib driver, in the version Lands of Lore uses (4; Eye of the Beholder used 1
//! and 2, Kyrandia 1 to 3): sound data are programs of opcodes, one per channel (nine OPL
//! channels and a control channel that starts the others), run 72 times a second. A port of
//! ScummVM's `AdLibDriver` (Kyra engine), its safety checks and fixes included; taken over
//! from EOBRS.
//!
//! A sound file (`.ADL`) begins with its track table: the sound for each track number, in
//! version 4 250 of them as u16 (`FFFF` none), before that 120 bytes (`FF` none). Then the
//! sound data: offsets (u16) of the programs (500 in version 4, 150 in version 1, else 250),
//! then of the instruments, then both.

use crate::opl::Opl;

/// How often the driver runs, per second.
pub const CALLBACKS_PER_SECOND: u32 = 72;
/// The tracks in the table in front of the sound data (version 4; 120 before).
pub const TRACKS: usize = 250;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Primary {
    #[default]
    None,
    Slide,
    Vibrato,
}

#[derive(Clone, Default)]
struct Channel {
    op_extra_level2: u8,
    dataptr: Option<usize>,
    duration: u8,
    repeat_counter: u8,
    base_octave: i8,
    priority: u8,
    stack_pos: usize,
    stack: [usize; 4],
    base_note: i8,
    slide_tempo: u8,
    slide_timer: u8,
    slide_step: i16,
    vibrato_step: i16,
    vibrato_step_range: u8,
    vibrato_steps_countdown: u8,
    vibrato_num_steps: u8,
    vibrato_delay: u8,
    vibrato_tempo: u8,
    vibrato_timer: u8,
    vibrato_delay_countdown: u8,
    op_extra_level1: u8,
    spacing2: u8,
    base_freq: u8,
    tempo: u8,
    timer: u8,
    reg_ax: u8,
    reg_bx: u8,
    primary: Primary,
    secondary: bool,
    fractional_spacing: u8,
    op_level1: u8,
    op_level2: u8,
    op_extra_level3: u8,
    two_chan: u8,
    spacing1: u8,
    duration_randomness: u8,
    secondary_tempo: u8,
    secondary_timer: u8,
    secondary_size: i8,
    secondary_pos: i8,
    secondary_regbase: u8,
    secondary_data: u16,
    tempo_reset: u8,
    raw_note: u8,
    pitch_bend: i8,
    volume_modifier: u8,
}

#[derive(Clone, Copy, Default)]
struct QueueEntry {
    data: Option<usize>,
    id: u16,
    volume: u8,
}

pub struct AdLibDriver {
    pub opl: Opl,
    version: u8,
    num_programs: usize,
    data: Vec<u8>,
    channels: [Channel; 10],
    cur_channel: usize,
    cur_reg_offset: u8,
    rnd: u16,
    tempo: u8,
    beat_divider: u8,
    beat_div_cnt: u8,
    callback_timer: u8,
    beat_counter: u8,
    beat_waiting: u8,
    op_level_bd: u8,
    op_level_hh: u8,
    op_level_sd: u8,
    op_level_tt: u8,
    op_level_cy: u8,
    extra1: [u8; 5],
    extra2: [u8; 5],
    vibrato_am_bits: u8,
    rhythm_bits: u8,
    queue: [QueueEntry; 16],
    queue_start: usize,
    queue_end: usize,
    start_timeout: u32,
    retry_sounds: bool,
    sfx_pointer: Option<usize>,
    sfx_priority: u8,
    sfx_velocity: u8,
    music_volume: u8,
    sfx_volume: u8,
    table2_first: u8,
    /// Jumps back (a program starting over) per channel, for knowing when music repeats.
    pub jumps_back: [u32; 10],
    pub sound_trigger: u8,
}

const REG_OFFSET: [u8; 9] = [0x00, 0x01, 0x02, 0x08, 0x09, 0x0a, 0x10, 0x11, 0x12];
const FREQ_TABLE: [u16; 12] = [
    0x0134, 0x0147, 0x015a, 0x016f, 0x0184, 0x019c, 0x01b4, 0x01ce, 0x01e9, 0x0207, 0x0225, 0x0246,
];
/// The first entries of the driver's `_unkTable2` tables (only those are ever used).
const TABLE2_FIRST: [u8; 6] = [0x50, 0x00, 0x50, 0x00, 0x40, 0x00];
const PITCH_BEND: [[u8; 32]; 14] = [
    [
        0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 25, 26,
        27, 28, 29, 30, 31, 32, 33,
    ],
    [
        0, 1, 2, 3, 4, 6, 7, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 26, 27,
        28, 29, 30, 31, 32, 34, 36,
    ],
    [
        0, 1, 2, 3, 4, 6, 8, 9, 10, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23, 25, 26, 28, 29, 30,
        31, 32, 33, 34, 36, 37, 38,
    ],
    [
        0, 1, 2, 3, 4, 6, 8, 10, 11, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24, 26, 28, 29,
        30, 31, 32, 33, 35, 37, 39, 40,
    ],
    [
        0, 1, 2, 3, 4, 6, 8, 10, 11, 12, 13, 14, 15, 17, 19, 21, 22, 23, 24, 25, 27, 29, 31, 32,
        33, 34, 35, 36, 37, 38, 40, 42,
    ],
    [
        0, 1, 2, 3, 5, 7, 9, 11, 12, 13, 14, 15, 16, 17, 19, 21, 22, 23, 24, 25, 27, 29, 31, 32,
        33, 34, 35, 37, 39, 41, 43, 45,
    ],
    [
        0, 1, 2, 3, 5, 7, 9, 11, 12, 13, 14, 15, 16, 17, 19, 21, 22, 23, 24, 26, 28, 30, 33, 36,
        37, 38, 39, 41, 43, 45, 47, 48,
    ],
    [
        0, 1, 2, 4, 6, 8, 10, 12, 13, 14, 15, 16, 17, 19, 21, 24, 25, 26, 28, 29, 31, 33, 35, 37,
        38, 39, 41, 43, 45, 47, 48, 50,
    ],
    [
        0, 1, 2, 4, 6, 8, 10, 13, 14, 15, 16, 17, 18, 20, 23, 26, 25, 26, 28, 30, 32, 34, 37, 40,
        41, 42, 43, 45, 47, 49, 51, 53,
    ],
    [
        0, 1, 3, 5, 7, 9, 11, 14, 15, 16, 18, 20, 22, 24, 26, 27, 28, 29, 30, 32, 34, 36, 38, 41,
        42, 44, 46, 48, 50, 52, 54, 57,
    ],
    [
        0, 1, 3, 5, 7, 9, 11, 14, 15, 16, 18, 20, 22, 25, 27, 30, 31, 33, 35, 37, 39, 41, 43, 45,
        46, 47, 49, 50, 52, 54, 57, 60,
    ],
    [
        0, 1, 3, 5, 7, 10, 12, 15, 16, 17, 19, 21, 23, 25, 27, 30, 31, 32, 34, 36, 38, 40, 43, 46,
        47, 48, 50, 52, 54, 57, 60, 63,
    ],
    [
        0, 2, 4, 6, 8, 11, 13, 16, 17, 18, 20, 22, 24, 27, 30, 33, 34, 35, 37, 39, 41, 44, 47, 50,
        51, 52, 54, 56, 59, 52, 65, 68,
    ],
    [
        0, 2, 4, 6, 8, 11, 13, 17, 18, 19, 21, 23, 26, 29, 32, 35, 36, 37, 39, 41, 44, 47, 50, 53,
        54, 55, 57, 59, 62, 65, 68, 71,
    ],
];

/// Opcodes 0x80 … 0xCA: their function and how many value bytes follow.
#[derive(Clone, Copy)]
enum Op {
    SetRepeat,
    CheckRepeat,
    SetupProgram,
    SetNoteSpacing,
    Jump,
    JumpToSubroutine,
    ReturnFromSubroutine,
    SetBaseOctave,
    StopChannel,
    PlayRest,
    WriteAdLib,
    SetupNoteAndDuration,
    SetBaseNote,
    SetupSecondaryEffect1,
    StopOtherChannel,
    WaitForEndOfProgram,
    SetupInstrument,
    SetupPrimaryEffectSlide,
    RemovePrimaryEffectSlide,
    SetBaseFreq,
    SetupPrimaryEffectVibrato,
    SetPriority,
    SetBeat,
    WaitForNextBeat,
    SetExtraLevel1,
    SetupDuration,
    PlayNote,
    SetFractionalNoteSpacing,
    SetTempo,
    RemoveSecondaryEffect1,
    SetChannelTempo,
    SetExtraLevel3,
    SetExtraLevel2,
    ChangeExtraLevel2,
    SetAmDepth,
    SetVibratoDepth,
    ChangeExtraLevel1,
    ClearChannel,
    ChangeNoteRandomly,
    RemovePrimaryEffectVibrato,
    PitchBend,
    ResetToGlobalTempo,
    Nop,
    SetDurationRandomness,
    ChangeChannelTempo,
    Callback46,
    SetupRhythmSection,
    PlayRhythmSection,
    RemoveRhythmSection,
    SetRhythmLevel2,
    ChangeRhythmLevel1,
    SetRhythmLevel1,
    SetSoundTrigger,
    SetTempoReset,
    Callback56,
}

const OPCODES: [(Op, usize); 75] = {
    use Op::*;
    [
        (SetRepeat, 1),
        (CheckRepeat, 2),
        (SetupProgram, 1),
        (SetNoteSpacing, 1),
        (Jump, 2),
        (JumpToSubroutine, 2),
        (ReturnFromSubroutine, 0),
        (SetBaseOctave, 1),
        (StopChannel, 0),
        (PlayRest, 1),
        (WriteAdLib, 2),
        (SetupNoteAndDuration, 2),
        (SetBaseNote, 1),
        (SetupSecondaryEffect1, 5),
        (StopOtherChannel, 1),
        (WaitForEndOfProgram, 1),
        (SetupInstrument, 1),
        (SetupPrimaryEffectSlide, 3),
        (RemovePrimaryEffectSlide, 0),
        (SetBaseFreq, 1),
        (StopChannel, 0),
        (SetupPrimaryEffectVibrato, 4),
        (StopChannel, 0),
        (StopChannel, 0),
        (StopChannel, 0),
        (StopChannel, 0),
        (SetPriority, 1),
        (StopChannel, 0),
        (SetBeat, 1),
        (WaitForNextBeat, 1),
        (SetExtraLevel1, 1),
        (StopChannel, 0),
        (SetupDuration, 1),
        (PlayNote, 1),
        (StopChannel, 0),
        (StopChannel, 0),
        (SetFractionalNoteSpacing, 1),
        (StopChannel, 0),
        (SetTempo, 1),
        (RemoveSecondaryEffect1, 0),
        (StopChannel, 0),
        (SetChannelTempo, 1),
        (StopChannel, 0),
        (SetExtraLevel3, 1),
        (SetExtraLevel2, 2),
        (ChangeExtraLevel2, 2),
        (SetAmDepth, 1),
        (SetVibratoDepth, 1),
        (ChangeExtraLevel1, 1),
        (StopChannel, 0),
        (StopChannel, 0),
        (ClearChannel, 1),
        (StopChannel, 0),
        (ChangeNoteRandomly, 2),
        (RemovePrimaryEffectVibrato, 0),
        (StopChannel, 0),
        (StopChannel, 0),
        (PitchBend, 1),
        (ResetToGlobalTempo, 0),
        (Nop, 0),
        (SetDurationRandomness, 1),
        (ChangeChannelTempo, 1),
        (StopChannel, 0),
        (Callback46, 2),
        (Nop, 0),
        (SetupRhythmSection, 9),
        (PlayRhythmSection, 1),
        (RemoveRhythmSection, 0),
        (SetRhythmLevel2, 2),
        (ChangeRhythmLevel1, 2),
        (SetRhythmLevel1, 2),
        (SetSoundTrigger, 1),
        (SetTempoReset, 1),
        (Callback56, 2),
        (StopChannel, 0),
    ]
};

/// The rhythm instruments' register for their level, in the order of the bits in an
/// opcode's mask (hi-hat, cymbal, tom, snare, bass drum).
const RHYTHM_LEVEL_REG: [u8; 5] = [0x51, 0x55, 0x52, 0x54, 0x53];

/// Clamps a combined level to 0–63.
fn check(value: i32) -> u8 {
    value.clamp(0, 0x3f) as u8
}

/// Adds `tempo` to `timer`; whether it wrapped round (time for the next step).
fn advance(timer: &mut u8, tempo: u8) -> bool {
    let old = *timer;
    *timer = timer.wrapping_add(tempo);
    *timer < old
}

impl AdLibDriver {
    /// A driver of that version (4 for Lands of Lore) on a chip sampled `rate` times a second.
    pub fn new(version: u8, rate: u32) -> Self {
        let mut driver = Self {
            opl: Opl::new(rate),
            version,
            num_programs: match version {
                1 => 150,
                4 => 500,
                _ => 250,
            },
            data: Vec::new(),
            channels: Default::default(),
            cur_channel: 0,
            cur_reg_offset: 0,
            rnd: 0x1234,
            tempo: 0,
            beat_divider: 0,
            beat_div_cnt: 0,
            callback_timer: 0xff,
            beat_counter: 0,
            beat_waiting: 0,
            op_level_bd: 0,
            op_level_hh: 0,
            op_level_sd: 0,
            op_level_tt: 0,
            op_level_cy: 0,
            extra1: [0; 5],
            extra2: [0; 5],
            vibrato_am_bits: 0,
            rhythm_bits: 0,
            queue: [QueueEntry::default(); 16],
            queue_start: 0,
            queue_end: 0,
            start_timeout: 0,
            retry_sounds: false,
            sfx_pointer: None,
            sfx_priority: 0,
            sfx_velocity: 0,
            music_volume: 0,
            sfx_volume: 0,
            table2_first: 0,
            jumps_back: [0; 10],
            sound_trigger: 0,
        };
        driver.reset_adlib_state();
        driver.set_volume(0xff);
        driver
    }

    /// Takes a sound file's data (after its track table).
    pub fn set_sound_data(&mut self, data: &[u8]) {
        self.queue = [QueueEntry::default(); 16];
        self.queue_start = 0;
        self.queue_end = 0;
        self.sfx_pointer = None;
        self.data = data.to_vec();
    }

    /// The program of this number in the data, if there is one.
    pub fn program(&self, id: usize) -> Option<usize> {
        if id >= self.data.len() / 2 {
            return None;
        }
        let offset = u16::from_le_bytes([self.data[2 * id], self.data[2 * id + 1]]) as usize;
        (offset != 0 && offset < self.data.len()).then_some(offset)
    }

    fn instrument(&self, id: u8) -> Option<usize> {
        self.program(self.num_programs + id as usize)
    }

    /// `ptr + n` if that stays within the data.
    fn offset(&self, ptr: Option<usize>, n: isize) -> Option<usize> {
        let p = ptr? as isize + n;
        (p >= 0 && p <= self.data.len() as isize).then_some(p as usize)
    }

    fn byte(&self, at: usize) -> u8 {
        self.data.get(at).copied().unwrap_or(0)
    }

    fn word(&self, at: usize) -> i16 {
        i16::from_le_bytes([self.byte(at), self.byte(at + 1)])
    }

    /// Whether any channel still runs a program.
    pub fn playing(&self) -> bool {
        self.channels.iter().any(|c| c.dataptr.is_some())
    }

    /// Which channels still run a program.
    pub fn active(&self) -> [bool; 10] {
        std::array::from_fn(|i| self.channels[i].dataptr.is_some())
    }

    /// The music's volume; before version 4 the sounds' as well (channels 6 to 8).
    pub fn set_volume(&mut self, volume: u8) {
        self.music_volume = volume;
        let channels = if self.version < 4 {
            self.sfx_volume = volume;
            9
        } else {
            6
        };
        for (i, offset) in REG_OFFSET.iter().enumerate().take(channels) {
            self.channels[i].volume_modifier = volume;
            let level1 = Self::op_level1(&self.channels[i]);
            let level2 = Self::op_level2(&self.channels[i]);
            self.write_opl(0x40 + offset, level1);
            self.write_opl(0x43 + offset, level2);
        }
    }

    /// Queues a sound (program number) to start.
    pub fn start_sound(&mut self, id: u16, volume: u8) {
        let Some(data) = self.program(id as usize) else {
            return;
        };
        self.queue[self.queue_end] = QueueEntry {
            data: Some(data),
            id,
            volume,
        };
        self.queue_end = (self.queue_end + 1) & 15;
    }

    pub fn stop_all(&mut self) {
        for c in 0..=9 {
            self.cur_channel = c;
            self.channels[c].priority = 0;
            self.channels[c].dataptr = None;
            if c != 9 {
                self.note_off(c);
            }
        }
        self.retry_sounds = false;
        self.queue_start = 0;
        self.queue_end = 0;
        self.queue[0] = QueueEntry::default();
        self.start_timeout = 0;
    }

    /// One run of the driver (72 a second).
    pub fn callback(&mut self) {
        if self.start_timeout > 0 {
            self.start_timeout -= 1;
        } else {
            self.setup_programs();
        }
        self.execute_programs();
        if advance(&mut self.callback_timer, self.tempo) {
            self.beat_div_cnt = self.beat_div_cnt.wrapping_sub(1);
            if self.beat_div_cnt == 0 {
                self.beat_div_cnt = self.beat_divider;
                self.beat_counter = self.beat_counter.wrapping_add(1);
            }
        }
    }

    fn write_opl(&mut self, reg: u8, value: u8) {
        self.opl.write(reg, value);
    }

    fn reset_adlib_state(&mut self) {
        self.rnd = 0x1234;
        self.write_opl(0x01, 0x20);
        self.write_opl(0x08, 0x00);
        self.write_opl(0xbd, 0x00);
        Self::init_channel(&mut self.channels[9]);
        for c in (0..9).rev() {
            self.write_opl(0x40 + REG_OFFSET[c], 0x3f);
            self.write_opl(0x43 + REG_OFFSET[c], 0x3f);
            Self::init_channel(&mut self.channels[c]);
        }
    }

    fn init_channel(channel: &mut Channel) {
        let backup = channel.op_extra_level2;
        *channel = Channel {
            op_extra_level2: backup,
            tempo: 0xff,
            spacing1: 1,
            ..Default::default()
        };
    }

    fn setup_programs(&mut self) {
        let entry = self.queue[self.queue_start];
        if self.queue_start == self.queue_end && entry.data.is_none() {
            return;
        }
        let mut retry = QueueEntry::default();
        if entry.id == 0 {
            self.retry_sounds = true;
        } else if self.retry_sounds {
            retry = entry;
        }
        self.queue[self.queue_start].data = None;
        self.queue_start = (self.queue_start + 1) & 15;

        let ptr = entry.data;
        if self.offset(ptr, 2).is_none() {
            return;
        }
        let mut p = ptr.expect("checked");
        let chan = self.byte(p) as usize;
        if chan > 9 || (chan < 9 && self.offset(ptr, 4).is_none()) {
            return;
        }
        self.adjust_sfx_data(p, entry.volume);
        p += 1;
        let priority = self.byte(p);
        p += 1;
        if priority >= self.channels[chan].priority {
            self.start_channel(chan, priority, p);
            self.start_timeout = 2;
            retry = QueueEntry::default();
        }
        if retry.data.is_some() {
            self.start_sound(retry.id, retry.volume);
        }
    }

    fn start_channel(&mut self, chan: usize, priority: u8, data: usize) {
        let volume = if chan <= 5 {
            self.music_volume
        } else {
            self.sfx_volume
        };
        let channel = &mut self.channels[chan];
        Self::init_channel(channel);
        channel.priority = priority;
        channel.dataptr = Some(data);
        channel.tempo = 0xff;
        channel.timer = 0xff;
        channel.duration = 1;
        channel.volume_modifier = volume;
        self.init_adlib_channel(chan);
    }

    fn adjust_sfx_data(&mut self, ptr: usize, volume: u8) {
        if let Some(old) = self.sfx_pointer.take()
            && old + 3 < self.data.len()
        {
            self.data[old + 1] = self.sfx_priority;
            self.data[old + 3] = self.sfx_velocity;
        }
        if self.byte(ptr) == 9 || ptr + 3 >= self.data.len() {
            return;
        }
        self.sfx_pointer = Some(ptr);
        self.sfx_priority = self.data[ptr + 1];
        self.sfx_velocity = self.data[ptr + 3];
        if volume != 0xff {
            if self.version >= 3 {
                let new = (((self.data[ptr + 3] as i32 + 63) * volume as i32) >> 8) & 0xff;
                self.data[ptr + 3] = (63 - new) as u8;
                self.data[ptr + 1] = ((self.data[ptr + 1] as i32 * volume as i32) >> 8) as u8;
            } else {
                let new = (((self.sfx_velocity as i32) << 2) ^ 0xff) * volume as i32;
                self.data[ptr + 3] = ((new >> 10) ^ 0x3f) as u8;
                self.data[ptr + 1] = (new >> 11) as u8;
            }
        }
    }

    fn execute_programs(&mut self) {
        for c in (0..=9).rev() {
            self.cur_channel = c;
            if self.channels[c].dataptr.is_none() {
                continue;
            }
            self.cur_reg_offset = if c == 9 { 0 } else { REG_OFFSET[c] };
            if self.channels[c].tempo_reset != 0 {
                self.channels[c].tempo = self.tempo;
            }
            let mut result = 1;
            let tempo = self.channels[c].tempo;
            if advance(&mut self.channels[c].timer, tempo) {
                let ch = &mut self.channels[c];
                ch.duration = ch.duration.wrapping_sub(1);
                if ch.duration != 0 {
                    let (duration, spacing1, spacing2) = (ch.duration, ch.spacing1, ch.spacing2);
                    if duration == spacing2 {
                        self.note_off(c);
                    }
                    if duration == spacing1 && c != 9 {
                        self.note_off(c);
                    }
                } else {
                    result = 0;
                }
            }
            while result == 0 {
                let Some(ptr) = self.channels[c].dataptr else {
                    break;
                };
                let mut opcode = 0xffu8;
                if self.offset(Some(ptr), 1).is_some() && ptr < self.data.len() {
                    opcode = self.data[ptr];
                    self.channels[c].dataptr = Some(ptr + 1);
                }
                let ptr = self.channels[c].dataptr.expect("set");
                if opcode & 0x80 != 0 {
                    let index = ((opcode & 0x7f) as usize).min(OPCODES.len() - 1);
                    let (op, values) = OPCODES[index];
                    if self.offset(Some(ptr), values as isize).is_none() {
                        result = self.stop_channel(c);
                        break;
                    }
                    self.channels[c].dataptr = Some(ptr + values);
                    result = self.run(op, c, ptr);
                } else {
                    if ptr >= self.data.len() {
                        result = self.stop_channel(c);
                        break;
                    }
                    let duration = self.data[ptr];
                    self.channels[c].dataptr = Some(ptr + 1);
                    self.setup_note(opcode, c, false);
                    self.note_on(c);
                    self.setup_duration(duration, c);
                    result = (duration != 0) as i32;
                }
            }
            if result == 1 {
                match self.channels[c].primary {
                    Primary::Slide => self.primary_slide(c),
                    Primary::Vibrato => self.primary_vibrato(c),
                    Primary::None => {}
                }
                if self.channels[c].secondary {
                    self.secondary_effect(c);
                }
            }
        }
    }

    fn note_off(&mut self, c: usize) {
        if self.cur_channel >= 9 || (self.rhythm_bits != 0 && self.cur_channel >= 6) {
            return;
        }
        let ch = &mut self.channels[c];
        ch.reg_bx &= 0xdf;
        let value = ch.reg_bx;
        self.write_opl(0xb0 + self.cur_channel as u8, value);
    }

    fn init_adlib_channel(&mut self, chan: usize) {
        if chan >= 9 || (self.rhythm_bits != 0 && chan >= 6) {
            return;
        }
        let offset = REG_OFFSET[chan];
        self.write_opl(0x60 + offset, 0xff);
        self.write_opl(0x63 + offset, 0xff);
        self.write_opl(0x80 + offset, 0xff);
        self.write_opl(0x83 + offset, 0xff);
        self.write_opl(0xb0 + chan as u8, 0x00);
        self.write_opl(0xb0 + chan as u8, 0x20);
    }

    fn random(&mut self) -> u16 {
        self.rnd = self.rnd.wrapping_add(0x9248);
        let low = self.rnd & 7;
        self.rnd >>= 3;
        self.rnd |= low << 13;
        self.rnd
    }

    fn setup_duration(&mut self, duration: u8, c: usize) {
        if self.channels[c].duration_randomness != 0 {
            let mask = self.channels[c].duration_randomness;
            let random = self.random() as u8;
            self.channels[c].duration = duration.wrapping_add(random & mask);
            return;
        }
        let ch = &mut self.channels[c];
        if ch.fractional_spacing != 0 {
            ch.spacing2 = (duration >> 3).wrapping_mul(ch.fractional_spacing);
        }
        ch.duration = duration;
    }

    fn setup_note(&mut self, raw_note: u8, c: usize, flag: bool) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        ch.raw_note = raw_note;
        let mut note = ((raw_note & 0x0f) as i32 + ch.base_note as i32) as i8 as i32;
        let mut octave = (((raw_note as i32 + ch.base_octave as i32) >> 4) & 0x0f) as i8 as i32;
        if note >= 12 {
            octave += note / 12;
            note %= 12;
        } else if note < 0 {
            let octaves = -(note + 1) / 12 + 1;
            octave -= octaves;
            note += 12 * octaves;
        }
        let mut freq = FREQ_TABLE[note.clamp(0, 11) as usize].wrapping_add(ch.base_freq as u16);
        if ch.pitch_bend != 0 || flag {
            let index = ((raw_note & 0x0f) as usize).min(11);
            if ch.pitch_bend >= 0 {
                freq = freq
                    .wrapping_add(PITCH_BEND[index + 2][(ch.pitch_bend as usize).min(31)] as u16);
            } else {
                let bend = (-(ch.pitch_bend as i32)).clamp(0, 31) as usize;
                freq = freq.wrapping_sub(PITCH_BEND[index][bend] as u16);
            }
        }
        ch.reg_ax = (freq & 0xff) as u8;
        ch.reg_bx = ((ch.reg_bx & 0x20) as i32 | (octave << 2) | ((freq >> 8) & 3) as i32) as u8;
        let (ax, bx) = (ch.reg_ax, ch.reg_bx);
        self.write_opl(0xa0 + self.cur_channel as u8, ax);
        self.write_opl(0xb0 + self.cur_channel as u8, bx);
    }

    fn setup_instrument(&mut self, reg_offset: u8, ptr: usize, c: usize) {
        if self.cur_channel >= 9 || self.offset(Some(ptr), 11).is_none() {
            return;
        }
        let d: [u8; 11] = std::array::from_fn(|i| self.byte(ptr + i));
        self.write_opl(0x20 + reg_offset, d[0]);
        self.write_opl(0x23 + reg_offset, d[1]);
        self.write_opl(0xc0 + self.cur_channel as u8, d[2]);
        self.channels[c].two_chan = d[2] & 1;
        self.write_opl(0xe0 + reg_offset, d[3]);
        self.write_opl(0xe3 + reg_offset, d[4]);
        self.channels[c].op_level1 = d[5];
        self.channels[c].op_level2 = d[6];
        let level1 = Self::op_level1(&self.channels[c]);
        let level2 = Self::op_level2(&self.channels[c]);
        self.write_opl(0x40 + reg_offset, level1);
        self.write_opl(0x43 + reg_offset, level2);
        self.write_opl(0x60 + reg_offset, d[7]);
        self.write_opl(0x63 + reg_offset, d[8]);
        self.write_opl(0x80 + reg_offset, d[9]);
        self.write_opl(0x83 + reg_offset, d[10]);
    }

    fn note_on(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        ch.reg_bx |= 0x20;
        let bx = ch.reg_bx;
        let shift = 9 - (ch.vibrato_step_range as i8).clamp(0, 9) as u32;
        let freq = (((ch.reg_bx as u16) << 8) | ch.reg_ax as u16) & 0x3ff;
        ch.vibrato_step = ((freq >> shift) & 0xff) as i16;
        ch.vibrato_delay_countdown = ch.vibrato_delay;
        self.write_opl(0xb0 + self.cur_channel as u8, bx);
    }

    fn adjust_volume(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let offset = REG_OFFSET[self.cur_channel];
        let level2 = Self::op_level2(&self.channels[c]);
        self.write_opl(0x43 + offset, level2);
        if self.channels[c].two_chan != 0 {
            let level1 = Self::op_level1(&self.channels[c]);
            self.write_opl(0x40 + offset, level1);
        }
    }

    fn primary_slide(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        if !advance(&mut ch.slide_timer, ch.slide_tempo) {
            return;
        }
        let mut freq = (((ch.reg_bx & 3) as i16) << 8) | ch.reg_ax as i16;
        let mut octave = ch.reg_bx & 0x1c;
        let note_on = ch.reg_bx & 0x20;
        freq += ch.slide_step.clamp(-0x3ff, 0x3ff);
        if ch.slide_step >= 0 && freq >= 734 {
            freq >>= 1;
            if freq & 0x3ff == 0 {
                freq += 1;
            }
            octave = octave.wrapping_add(4);
        } else if ch.slide_step < 0 && freq < 388 {
            if freq < 0 {
                freq = 0;
            }
            freq <<= 1;
            if freq & 0x3ff == 0 {
                freq -= 1;
            }
            octave = octave.wrapping_sub(4);
        }
        ch.reg_ax = (freq & 0xff) as u8;
        ch.reg_bx = note_on | (octave & 0x1c) | ((freq >> 8) & 3) as u8;
        let (ax, bx) = (ch.reg_ax, ch.reg_bx);
        self.write_opl(0xa0 + self.cur_channel as u8, ax);
        self.write_opl(0xb0 + self.cur_channel as u8, bx);
    }

    fn primary_vibrato(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        if ch.vibrato_delay_countdown != 0 {
            ch.vibrato_delay_countdown -= 1;
            return;
        }
        if advance(&mut ch.vibrato_timer, ch.vibrato_tempo) {
            ch.vibrato_steps_countdown = ch.vibrato_steps_countdown.wrapping_sub(1);
            if ch.vibrato_steps_countdown == 0 {
                ch.vibrato_step = ch.vibrato_step.wrapping_neg();
                ch.vibrato_steps_countdown = ch.vibrato_num_steps;
            }
            let mut freq = (((ch.reg_bx as u16) << 8) | ch.reg_ax as u16) & 0x3ff;
            freq = freq.wrapping_add(ch.vibrato_step as u16);
            ch.reg_ax = (freq & 0xff) as u8;
            ch.reg_bx = ((ch.reg_bx & 0xfc) as u16 | (freq >> 8)) as u8;
            let (ax, bx) = (ch.reg_ax, ch.reg_bx);
            self.write_opl(0xa0 + self.cur_channel as u8, ax);
            self.write_opl(0xb0 + self.cur_channel as u8, bx);
        }
    }

    fn secondary_effect(&mut self, c: usize) {
        if self.cur_channel >= 9 {
            return;
        }
        let ch = &mut self.channels[c];
        if advance(&mut ch.secondary_timer, ch.secondary_tempo) {
            ch.secondary_pos = ch.secondary_pos.wrapping_sub(1);
            if ch.secondary_pos < 0 {
                ch.secondary_pos = ch.secondary_size;
            }
            let reg = ch.secondary_regbase.wrapping_add(self.cur_reg_offset);
            let at = ch.secondary_data as usize + ch.secondary_pos as usize;
            let value = self.byte(at);
            self.write_opl(reg, value);
        }
    }

    fn level3(ch: &Channel) -> u8 {
        let mut level3 = (ch.op_extra_level3 ^ 0x3f) as u16 * ch.volume_modifier as u16;
        if level3 != 0 {
            level3 += 0x3f;
            level3 >>= 8;
        }
        (level3 ^ 0x3f) as u8
    }

    fn op_level1(ch: &Channel) -> u8 {
        let mut value = ch.op_level1 & 0x3f;
        if ch.two_chan != 0 {
            value = value
                .wrapping_add(ch.op_extra_level1)
                .wrapping_add(ch.op_extra_level2)
                .wrapping_add(Self::level3(ch));
        }
        value = value.min(0x3f);
        if ch.volume_modifier == 0 {
            value = 0x3f;
        }
        value | (ch.op_level1 & 0xc0)
    }

    fn op_level2(ch: &Channel) -> u8 {
        let mut value = (ch.op_level2 & 0x3f)
            .wrapping_add(ch.op_extra_level1)
            .wrapping_add(ch.op_extra_level2)
            .wrapping_add(Self::level3(ch));
        value = value.min(0x3f);
        if ch.volume_modifier == 0 {
            value = 0x3f;
        }
        value | (ch.op_level2 & 0xc0)
    }

    fn stop_channel(&mut self, c: usize) -> i32 {
        self.channels[c].priority = 0;
        if self.cur_channel != 9 {
            self.note_off(c);
        }
        self.channels[c].dataptr = None;
        2
    }

    /// Runs an opcode whose values begin at `v`; 0 go on, 1 stop, 2 stop without effects.
    fn run(&mut self, op: Op, c: usize, v: usize) -> i32 {
        let values: [u8; 9] = std::array::from_fn(|i| self.byte(v + i));
        let b = |i: usize| values[i];
        let (v0, v1) = (b(0), b(1));
        match op {
            Op::SetRepeat => self.channels[c].repeat_counter = v0,
            Op::CheckRepeat => {
                let ch = &mut self.channels[c];
                ch.repeat_counter = ch.repeat_counter.wrapping_sub(1);
                if ch.repeat_counter != 0 {
                    let add = self.word(v) as isize;
                    if let Some(p) = self.offset(self.channels[c].dataptr, add) {
                        self.channels[c].dataptr = Some(p);
                    }
                }
            }
            Op::SetupProgram => {
                if v0 == 0xff {
                    return 0;
                }
                let ptr = self.program(v0 as usize);
                if self.offset(ptr, 2).is_none() {
                    return 0;
                }
                let p = ptr.expect("checked");
                let chan = self.byte(p) as usize;
                let priority = self.byte(p + 1);
                if chan > 9 {
                    return 0;
                }
                if priority >= self.channels[chan].priority {
                    let backup = self.channels[c].dataptr;
                    self.start_timeout = 2;
                    self.start_channel(chan, priority, p + 2);
                    self.channels[c].dataptr = backup;
                }
            }
            Op::SetNoteSpacing => self.channels[c].spacing1 = v0,
            Op::Jump => {
                let add = self.word(v) as isize;
                let from = self.channels[c].dataptr;
                let target = if self.version == 1 {
                    self.offset(Some(0), add - 191)
                } else {
                    self.offset(from, add)
                };
                match target {
                    Some(t) => {
                        if from.is_some_and(|f| t < f) {
                            self.jumps_back[c] += 1;
                        }
                        self.channels[c].dataptr = Some(t);
                    }
                    None => return self.stop_channel(c),
                }
            }
            Op::JumpToSubroutine => {
                let add = self.word(v) as isize;
                let ch = &self.channels[c];
                if ch.stack_pos >= ch.stack.len() {
                    return 0;
                }
                let from = ch.dataptr.expect("running");
                let target = if self.version < 3 {
                    self.offset(Some(0), add - 191)
                } else {
                    self.offset(Some(from), add)
                };
                if let Some(t) = target {
                    let ch = &mut self.channels[c];
                    ch.stack[ch.stack_pos] = from;
                    ch.stack_pos += 1;
                    ch.dataptr = Some(t);
                }
            }
            Op::ReturnFromSubroutine => {
                let ch = &mut self.channels[c];
                if ch.stack_pos == 0 {
                    return self.stop_channel(c);
                }
                ch.stack_pos -= 1;
                ch.dataptr = Some(ch.stack[ch.stack_pos]);
            }
            Op::SetBaseOctave => self.channels[c].base_octave = v0 as i8,
            Op::StopChannel => return self.stop_channel(c),
            Op::PlayRest => {
                self.setup_duration(v0, c);
                self.note_off(c);
                return (v0 != 0) as i32;
            }
            Op::WriteAdLib => self.write_opl(v0, v1),
            Op::SetupNoteAndDuration => {
                self.setup_note(v0, c, false);
                self.setup_duration(v1, c);
                return (v1 != 0) as i32;
            }
            Op::SetBaseNote => self.channels[c].base_note = v0 as i8,
            Op::SetupSecondaryEffect1 => {
                let data = (self.word(v + 3) as i32 - 191) as u16;
                let ch = &mut self.channels[c];
                ch.secondary_timer = v0;
                ch.secondary_tempo = v0;
                ch.secondary_size = v1 as i8;
                ch.secondary_pos = v1 as i8;
                ch.secondary_regbase = b(2);
                ch.secondary_data = data;
                let start = data as i16 as i32 + v1 as i8 as i32;
                ch.secondary = start >= 0 && start < self.data.len() as i32;
            }
            Op::StopOtherChannel => {
                if v0 > 9 {
                    return 0;
                }
                let backup = self.channels[c].dataptr;
                let other = &mut self.channels[v0 as usize];
                other.duration = 0;
                other.priority = 0;
                other.dataptr = None;
                self.channels[c].dataptr = backup;
            }
            Op::WaitForEndOfProgram => {
                let Some(p) = self.program(v0 as usize) else {
                    return 0;
                };
                let chan = self.byte(p) as usize;
                if chan > 9 || self.channels[chan].dataptr.is_none() {
                    return 0;
                }
                let ch = &mut self.channels[c];
                ch.dataptr = ch.dataptr.map(|p| p - 2);
                return 2;
            }
            Op::SetupInstrument => {
                if let Some(p) = self.instrument(v0) {
                    self.setup_instrument(self.cur_reg_offset, p, c);
                }
            }
            Op::SetupPrimaryEffectSlide => {
                let ch = &mut self.channels[c];
                ch.slide_tempo = v0;
                ch.slide_step = i16::from_be_bytes([v1, b(2)]);
                ch.primary = Primary::Slide;
                ch.slide_timer = 0xff;
            }
            Op::RemovePrimaryEffectSlide => {
                let ch = &mut self.channels[c];
                ch.primary = Primary::None;
                ch.slide_step = 0;
            }
            Op::SetBaseFreq => self.channels[c].base_freq = v0,
            Op::SetupPrimaryEffectVibrato => {
                let ch = &mut self.channels[c];
                ch.vibrato_tempo = v0;
                ch.vibrato_step_range = v1;
                ch.vibrato_steps_countdown = b(2).wrapping_add(1);
                ch.vibrato_num_steps = b(2).wrapping_shl(1);
                ch.vibrato_delay = b(3);
                ch.primary = Primary::Vibrato;
            }
            Op::SetPriority => self.channels[c].priority = v0,
            Op::SetBeat => {
                self.beat_divider = v0 >> 1;
                self.beat_div_cnt = v0 >> 1;
                self.callback_timer = 0xff;
                self.beat_counter = 0;
                self.beat_waiting = 0;
            }
            Op::WaitForNextBeat => {
                if self.beat_counter & v0 != 0 && self.beat_waiting != 0 {
                    self.beat_waiting = 0;
                    return 0;
                }
                if self.beat_counter & v0 == 0 {
                    self.beat_waiting = self.beat_waiting.wrapping_add(1);
                }
                let ch = &mut self.channels[c];
                ch.dataptr = ch.dataptr.map(|p| p - 2);
                ch.duration = 1;
                return 2;
            }
            Op::SetExtraLevel1 => {
                self.channels[c].op_extra_level1 = v0;
                self.adjust_volume(c);
            }
            Op::SetupDuration => {
                self.setup_duration(v0, c);
                return (v0 != 0) as i32;
            }
            Op::PlayNote => {
                self.setup_duration(v0, c);
                self.note_on(c);
                return (v0 != 0) as i32;
            }
            Op::SetFractionalNoteSpacing => self.channels[c].fractional_spacing = v0 & 7,
            Op::SetTempo => self.tempo = v0,
            Op::RemoveSecondaryEffect1 => self.channels[c].secondary = false,
            Op::SetChannelTempo => self.channels[c].tempo = v0,
            Op::SetExtraLevel3 => self.channels[c].op_extra_level3 = v0,
            Op::SetExtraLevel2 | Op::ChangeExtraLevel2 => {
                if v0 > 9 {
                    return 0;
                }
                let backup = self.cur_channel;
                self.cur_channel = v0 as usize;
                let other = &mut self.channels[v0 as usize];
                other.op_extra_level2 = if matches!(op, Op::SetExtraLevel2) {
                    v1
                } else {
                    other.op_extra_level2.wrapping_add(v1)
                };
                self.adjust_volume(v0 as usize);
                self.cur_channel = backup;
            }
            Op::SetAmDepth => {
                if v0 & 1 != 0 {
                    self.vibrato_am_bits |= 0x80;
                } else {
                    self.vibrato_am_bits &= 0x7f;
                }
                self.write_opl(0xbd, self.vibrato_am_bits);
            }
            Op::SetVibratoDepth => {
                if v0 & 1 != 0 {
                    self.vibrato_am_bits |= 0x40;
                } else {
                    self.vibrato_am_bits &= 0xbf;
                }
                self.write_opl(0xbd, self.vibrato_am_bits);
            }
            Op::ChangeExtraLevel1 => {
                let ch = &mut self.channels[c];
                ch.op_extra_level1 = ch.op_extra_level1.wrapping_add(v0);
                self.adjust_volume(c);
            }
            Op::ClearChannel => {
                if v0 > 9 {
                    return 0;
                }
                let backup = self.channels[c].dataptr;
                let target = v0 as usize;
                let other = &mut self.channels[target];
                other.duration = 0;
                other.priority = 0;
                other.dataptr = None;
                other.op_extra_level2 = 0;
                if target != 9 {
                    let offset = REG_OFFSET[target];
                    self.write_opl(0xc0 + target as u8, 0x00);
                    self.write_opl(0x43 + offset, 0x3f);
                    self.write_opl(0x83 + offset, 0xff);
                    self.write_opl(0xb0 + target as u8, 0x00);
                }
                self.channels[c].dataptr = backup;
            }
            Op::ChangeNoteRandomly => {
                if self.cur_channel >= 9 {
                    return 0;
                }
                let mask = u16::from_be_bytes([v0, v1]);
                let ch = &self.channels[c];
                let mut note = (((ch.reg_bx & 0x1f) as u16) << 8) | ch.reg_ax as u16;
                let key = ((ch.reg_bx & 0x20) as u16) << 8;
                note = note.wrapping_add(mask & self.random());
                note |= key;
                self.write_opl(0xa0 + self.cur_channel as u8, (note & 0xff) as u8);
                self.write_opl(0xb0 + self.cur_channel as u8, (note >> 8) as u8);
            }
            Op::RemovePrimaryEffectVibrato => self.channels[c].primary = Primary::None,
            Op::PitchBend => {
                self.channels[c].pitch_bend = v0 as i8;
                let raw = self.channels[c].raw_note;
                self.setup_note(raw, c, true);
            }
            Op::ResetToGlobalTempo => self.channels[c].tempo = self.tempo,
            Op::Nop => {}
            Op::SetDurationRandomness => self.channels[c].duration_randomness = v0,
            Op::ChangeChannelTempo => {
                let ch = &mut self.channels[c];
                ch.tempo = (ch.tempo as i32 + v0 as i8 as i32).clamp(1, 255) as u8;
            }
            Op::Callback46 => {
                let entry = v1 as usize;
                if entry + 2 > TABLE2_FIRST.len() {
                    return 0;
                }
                self.table2_first = TABLE2_FIRST[entry + 1];
                if v0 == 2 {
                    self.write_opl(0xa0, self.table2_first);
                }
            }
            Op::SetupRhythmSection => {
                let (channel_backup, offset_backup) = (self.cur_channel, self.cur_reg_offset);
                for (k, chan) in [6usize, 7, 8].into_iter().enumerate() {
                    self.cur_channel = chan;
                    self.cur_reg_offset = REG_OFFSET[chan];
                    if let Some(p) = self.instrument(b(k)) {
                        self.setup_instrument(self.cur_reg_offset, p, c);
                    }
                    let ch = &self.channels[c];
                    match chan {
                        6 => self.op_level_bd = ch.op_level2,
                        7 => {
                            self.op_level_hh = ch.op_level1;
                            self.op_level_sd = ch.op_level2;
                        }
                        _ => {
                            self.op_level_tt = ch.op_level1;
                            self.op_level_cy = ch.op_level2;
                        }
                    }
                }
                for (k, chan) in [6usize, 7, 8].into_iter().enumerate() {
                    let bx = b(3 + k * 2) & 0x2f;
                    self.channels[chan].reg_bx = bx;
                    self.write_opl(0xb0 + chan as u8, bx);
                    self.write_opl(0xa0 + chan as u8, b(4 + k * 2));
                }
                self.rhythm_bits = 0x20;
                self.cur_reg_offset = offset_backup;
                self.cur_channel = channel_backup;
            }
            Op::PlayRhythmSection => {
                self.write_opl(0xbd, (self.rhythm_bits & !(v0 & 0x1f)) | 0x20);
                self.rhythm_bits |= v0;
                self.write_opl(0xbd, self.vibrato_am_bits | 0x20 | self.rhythm_bits);
            }
            Op::RemoveRhythmSection => {
                self.rhythm_bits = 0;
                self.write_opl(0xbd, self.vibrato_am_bits);
            }
            Op::SetRhythmLevel2 | Op::ChangeRhythmLevel1 | Op::SetRhythmLevel1 => {
                let levels = [
                    self.op_level_hh,
                    self.op_level_cy,
                    self.op_level_tt,
                    self.op_level_sd,
                    self.op_level_bd,
                ];
                for i in 0..5 {
                    if v0 & (1 << i) == 0 {
                        continue;
                    }
                    let base = levels[i] as i32;
                    let value = match op {
                        Op::SetRhythmLevel2 => {
                            self.extra2[i] = v1;
                            check(v1 as i32 + base + self.extra1[i] as i32 + self.extra2[i] as i32)
                        }
                        Op::ChangeRhythmLevel1 => {
                            self.extra1[i] = check(
                                v1 as i32 + base + self.extra1[i] as i32 + self.extra2[i] as i32,
                            );
                            self.extra1[i]
                        }
                        _ => {
                            self.extra1[i] = v1;
                            check(v1 as i32 + base + self.extra2[i] as i32)
                        }
                    };
                    self.write_opl(RHYTHM_LEVEL_REG[i], value);
                }
            }
            Op::SetSoundTrigger => self.sound_trigger = v0,
            Op::SetTempoReset => self.channels[c].tempo_reset = v0,
            Op::Callback56 => {}
        }
        0
    }
}

/// The sound numbers of a sound file's tracks (`None` where a track is unused) and the sound
/// data after the table, for a driver of that version.
pub fn split_sound_file(version: u8, bytes: &[u8]) -> Option<(Vec<Option<u16>>, &[u8])> {
    let table = if version == 4 { 2 * TRACKS } else { 120 };
    if bytes.len() <= table {
        return None;
    }
    let tracks = if version == 4 {
        bytes[..table]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .map(|id| (id != 0xffff).then_some(id))
            .collect()
    } else {
        bytes[..table]
            .iter()
            .map(|&id| (id != 0xff).then_some(id as u16))
            .collect()
    };
    Some((tracks, &bytes[table..]))
}
