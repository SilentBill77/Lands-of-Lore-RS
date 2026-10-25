// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
// Ported from ScummVM's copy of MAME's fmopl by Tatsuyuki Satoh (taken from AdPlug, LGPL),
// used here under the terms of the GPL as the LGPL allows.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! An emulated Yamaha YM3812 (OPL2), the chip on the AdLib card: nine channels of two
//! operators each, or six and a rhythm section. A port of ScummVM's copy of MAME's `fmopl`
//! (V0.37a, by Tatsuyuki Satoh, through AdPlug; LGPL), with its fix for the way Westwood's
//! driver keys notes off; at its best quality (4096 envelope steps).
//!
//! Only what music needs: the timers, IRQs and CSM mode are left out.

const CLOCK: f64 = 3_579_545.0;

/// Attack and decay rate time bases (RATE 4 = 2826.24 ms and 39280.64 ms at 3.6 MHz).
const AR_RATE: f64 = 141_280.0;
const DR_RATE: f64 = 1_956_000.0;

const FREQ_BITS: u32 = 24;
const FREQ_RATE: f64 = (1 << (FREQ_BITS - 20)) as f64;
const TL_BITS: u32 = FREQ_BITS + 2;
/// Final output shift to 16 bits, and its limits.
const OUT_SHIFT: u32 = TL_BITS + 3 - 16;
const MAX_OUT: i32 = 0x7fff << OUT_SHIFT;
const MIN_OUT: i32 = -(0x8000 << OUT_SHIFT);

const SIN_SHIFT: u32 = 11;
const SIN_ENT: usize = 1 << SIN_SHIFT;

const ENV_BITS: u32 = 16;
const EG_ENT: i32 = 4096;
const EG_OFF: i32 = (2 * EG_ENT) << ENV_BITS;
const EG_DED: i32 = EG_OFF;
const EG_DST: i32 = EG_ENT << ENV_BITS;
const EG_AED: i32 = EG_DST;
const EG_AST: i32 = 0;
/// The OPL's 0.1875 dB step, spread over the envelope entries.
const EG_STEP: f64 = 96.0 / EG_ENT as f64;
const TL_MAX: usize = EG_ENT as usize * 2;

const VIB_ENT: usize = 512;
const VIB_SHIFT: u32 = 32 - 9;
const AMS_ENT: usize = 512;
const AMS_SHIFT: u32 = 32 - 9;
const VIB_RATE_SHIFT: u32 = 8;
const VIB_RATE: f64 = (1 << VIB_RATE_SHIFT) as f64;

/// Envelope phases.
const ENV_RR: u8 = 0;
const ENV_DR: u8 = 1;
const ENV_AR: u8 = 2;

/// Register number to operator ("slot"): channel × 2 + operator.
const SLOT_ARRAY: [i8; 32] = [
    0, 2, 4, 1, 3, 5, -1, -1, 6, 8, 10, 7, 9, 11, -1, -1, 12, 14, 16, 13, 15, 17, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1,
];

const KSL_SEED: [f64; 128] = [
    0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000,
    0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.000, 0.750,
    1.125, 1.500, 1.875, 2.250, 2.625, 3.000, 0.000, 0.000, 0.000, 0.000, 0.000, 1.125, 1.875,
    2.625, 3.000, 3.750, 4.125, 4.500, 4.875, 5.250, 5.625, 6.000, 0.000, 0.000, 0.000, 1.875,
    3.000, 4.125, 4.875, 5.625, 6.000, 6.750, 7.125, 7.500, 7.875, 8.250, 8.625, 9.000, 0.000,
    0.000, 3.000, 4.875, 6.000, 7.125, 7.875, 8.625, 9.000, 9.750, 10.125, 10.500, 10.875, 11.250,
    11.625, 12.000, 0.000, 3.000, 6.000, 7.875, 9.000, 10.125, 10.875, 11.625, 12.000, 12.750,
    13.125, 13.500, 13.875, 14.250, 14.625, 15.000, 0.000, 6.000, 9.000, 10.875, 12.000, 13.125,
    13.875, 14.625, 15.000, 15.750, 16.125, 16.500, 16.875, 17.250, 17.625, 18.000, 0.000, 9.000,
    12.000, 13.875, 15.000, 16.125, 16.875, 17.625, 18.000, 18.750, 19.125, 19.500, 19.875, 20.250,
    20.625, 21.000,
];

/// Sustain levels 0–15: 0, 3, 6 … 42 and 93 dB.
const SL_SEED: [i32; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 31];

/// Frequency multiples (×2): ½, 1, 2 … 10, 10, 12, 12, 15, 15.
const MUL_TABLE: [u32; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];

#[derive(Clone, Default)]
struct Slot {
    tl: i32,
    tll: i32,
    /// Key scale rate shift (0 or 2).
    ksr_shift: u8,
    /// Where the attack and decay rates start in their tables (`None`: rate 0).
    ar: Option<usize>,
    dr: Option<usize>,
    sl: i32,
    rr: usize,
    /// Key scale level shift (31: none).
    ksl: u32,
    ksr: u8,
    mul: u32,
    cnt: u32,
    incr: u32,
    eg_sustain: bool,
    evm: u8,
    evc: i32,
    eve: i32,
    evs: i32,
    evsa: i32,
    evsd: i32,
    evsr: i32,
    ams: bool,
    vib: bool,
    /// Waveform (0–3).
    wave: usize,
}

#[derive(Clone, Default)]
struct Channel {
    slots: [Slot; 2],
    /// Both operators sound (else the first modulates the second).
    con: bool,
    /// Feedback shift (0: none).
    fb: u32,
    op1_out: [i32; 2],
    block_fnum: u32,
    kcode: u8,
    fc: u32,
    ksl_base: u32,
    keyon: bool,
}

pub struct Opl {
    tl_table: Vec<i32>,
    /// Per waveform and phase: where in `tl_table` the output for envelope 0 is.
    sin_table: Vec<usize>,
    ams_table: Vec<i32>,
    vib_table: Vec<i32>,
    env_curve: Vec<i32>,
    ksl_table: [u32; 128],
    sl_table: [i32; 16],
    ar_table: [i32; 76],
    dr_table: [i32; 76],
    fn_table: [u32; 1024],
    channels: [Channel; 9],
    rhythm: u8,
    wavesel: bool,
    /// AM depth 4.8 dB (else 1 dB) and vibrato 14 cent (else 7).
    ams_deep: bool,
    vib_deep: bool,
    ams_cnt: u32,
    vib_cnt: u32,
    ams_incr: u32,
    vib_incr: u32,
    noise: u32,
}

impl Opl {
    /// A chip whose output is sampled `rate` times a second.
    pub fn new(rate: u32) -> Self {
        let mut tl_table = vec![0i32; TL_MAX * 2];
        for t in 0..(EG_ENT - 1) as usize {
            let rate = ((1i64 << TL_BITS) - 1) as f64 / 10f64.powf(EG_STEP * t as f64 / 20.0);
            tl_table[t] = rate as i32;
            tl_table[TL_MAX + t] = -(rate as i32);
        }
        let mut sin = vec![0usize; SIN_ENT * 4];
        sin[0] = (EG_ENT - 1) as usize;
        sin[SIN_ENT / 2] = (EG_ENT - 1) as usize;
        for s in 1..=SIN_ENT / 4 {
            let pom = (2.0 * std::f64::consts::PI * s as f64 / SIN_ENT as f64).sin();
            let db = 20.0 * (1.0 / pom).log10();
            let j = (db / EG_STEP) as usize;
            sin[s] = j;
            sin[SIN_ENT / 2 - s] = j;
            sin[SIN_ENT / 2 + s] = TL_MAX + j;
            sin[SIN_ENT - s] = TL_MAX + j;
        }
        let off = EG_ENT as usize;
        for s in 0..SIN_ENT {
            sin[SIN_ENT + s] = if s < SIN_ENT / 2 { sin[s] } else { off };
            sin[SIN_ENT * 2 + s] = sin[s % (SIN_ENT / 2)];
            sin[SIN_ENT * 3 + s] = if (s / (SIN_ENT / 4)) & 1 != 0 {
                off
            } else {
                sin[SIN_ENT * 2 + s]
            };
        }
        let mut env_curve = vec![0i32; 2 * EG_ENT as usize + 1];
        for i in 0..EG_ENT as usize {
            let pom = ((EG_ENT as f64 - 1.0 - i as f64) / EG_ENT as f64).powi(8) * EG_ENT as f64;
            env_curve[i] = pom as i32;
            env_curve[(EG_DST >> ENV_BITS) as usize + i] = i as i32;
        }
        env_curve[(EG_OFF >> ENV_BITS) as usize] = EG_ENT - 1;
        let mut ams_table = vec![0i32; AMS_ENT * 2];
        for i in 0..AMS_ENT {
            let pom = (1.0 + (2.0 * std::f64::consts::PI * i as f64 / AMS_ENT as f64).sin()) / 2.0;
            ams_table[i] = ((1.0 / EG_STEP) * pom) as i32;
            ams_table[AMS_ENT + i] = ((4.8 / EG_STEP) * pom) as i32;
        }
        let mut vib_table = vec![0i32; VIB_ENT * 2];
        for i in 0..VIB_ENT {
            let pom =
                VIB_RATE * 0.06 * (2.0 * std::f64::consts::PI * i as f64 / VIB_ENT as f64).sin();
            vib_table[i] = (VIB_RATE + pom * 0.07) as i32;
            vib_table[VIB_ENT + i] = (VIB_RATE + pom * 0.14) as i32;
        }
        let ksl_table = std::array::from_fn(|i| (KSL_SEED[i] / (EG_STEP / 2.0)) as u32);
        let sl_table = std::array::from_fn(|i| {
            (SL_SEED[i] as f64 * ((3.0 / EG_STEP) * (1u32 << ENV_BITS) as f64)) as i32 + EG_DST
        });

        let freqbase = if rate > 0 {
            CLOCK / rate as f64 / 72.0
        } else {
            0.0
        };
        let mut ar_table = [0i32; 76];
        let mut dr_table = [0i32; 76];
        for i in 4..=60 {
            let mut r = freqbase;
            if i < 60 {
                r *= 1.0 + (i & 3) as f64 * 0.25;
            }
            r *= (1u64 << ((i >> 2) - 1)) as f64;
            r *= ((EG_ENT as i64) << ENV_BITS) as f64;
            ar_table[i] = (r / AR_RATE) as i32;
            dr_table[i] = (r / DR_RATE) as i32;
        }
        for i in 60..76 {
            ar_table[i] = EG_AED - 1;
            dr_table[i] = dr_table[60];
        }
        let fn_table =
            std::array::from_fn(|f| (freqbase * f as f64 * FREQ_RATE * 128.0 / 2.0) as u32);
        let scale = CLOCK / 3_600_000.0;
        let ams_incr =
            (AMS_ENT as f64 * (1u64 << AMS_SHIFT) as f64 / rate as f64 * 3.7 * scale) as i32;
        let vib_incr =
            (VIB_ENT as f64 * (1u64 << VIB_SHIFT) as f64 / rate as f64 * 6.4 * scale) as i32;

        let mut opl = Self {
            tl_table,
            sin_table: sin,
            ams_table,
            vib_table,
            env_curve,
            ksl_table,
            sl_table,
            ar_table,
            dr_table,
            fn_table,
            channels: Default::default(),
            rhythm: 0,
            wavesel: false,
            ams_deep: false,
            vib_deep: false,
            ams_cnt: 0,
            vib_cnt: 0,
            ams_incr: ams_incr as u32,
            vib_incr: vib_incr as u32,
            noise: 1,
        };
        opl.reset();
        opl
    }

    pub fn reset(&mut self) {
        self.write(0x01, 0);
        for r in (0x20..=0xff).rev() {
            self.write(r, 0);
        }
        for ch in &mut self.channels {
            for s in &mut ch.slots {
                s.wave = 0;
                s.evc = EG_OFF;
                s.eve = EG_OFF + 1;
                s.evs = 0;
            }
        }
    }

    fn key_on(slot: &mut Slot) {
        slot.cnt = 0;
        slot.evm = ENV_AR;
        slot.evs = slot.evsa;
        slot.evc = EG_AST;
        slot.eve = EG_AED;
    }

    fn key_off(slot: &mut Slot) {
        if slot.evm > ENV_RR {
            // ScummVM's fix: Westwood's driver keys a very low note on and at once off again
            // when a song starts; the envelope must not jump to the top of the release then.
            if slot.evm == ENV_AR && slot.evc == EG_AST {
                slot.evc = EG_DED;
            } else if slot.evc & EG_DST == 0 {
                slot.evc = EG_DST;
            }
            slot.eve = EG_DED;
            slot.evs = slot.evsr;
            slot.evm = ENV_RR;
        }
    }

    fn rate(table: &[i32; 76], base: Option<usize>, ksr: u8) -> i32 {
        base.map_or(0, |b| table[b + ksr as usize])
    }

    fn calc_fcslot(&mut self, ch: usize, s: usize) {
        let (fc, kcode, ksl_base) = {
            let c = &self.channels[ch];
            (c.fc, c.kcode, c.ksl_base)
        };
        let (ar_table, dr_table) = (&self.ar_table, &self.dr_table);
        let slot = &mut self.channels[ch].slots[s];
        slot.incr = fc.wrapping_mul(slot.mul);
        let ksr = kcode >> slot.ksr_shift;
        if slot.ksr != ksr {
            slot.ksr = ksr;
            slot.evsa = Self::rate(ar_table, slot.ar, ksr);
            slot.evsd = Self::rate(dr_table, slot.dr, ksr);
            slot.evsr = dr_table[slot.rr + ksr as usize];
        }
        slot.tll = slot.tl + (ksl_base >> slot.ksl) as i32;
    }

    /// Writes a register.
    pub fn write(&mut self, r: u8, v: u8) {
        let slot_of = |r: u8| {
            let s = SLOT_ARRAY[(r & 0x1f) as usize];
            (s >= 0).then_some((s as usize >> 1, s as usize & 1))
        };
        match r & 0xe0 {
            0x00 => {
                if r & 0x1f == 0x01 {
                    self.wavesel = v & 0x20 != 0;
                    if !self.wavesel {
                        for ch in &mut self.channels {
                            ch.slots[0].wave = 0;
                            ch.slots[1].wave = 0;
                        }
                    }
                }
            }
            0x20 => {
                let Some((ch, s)) = slot_of(r) else { return };
                {
                    let slot = &mut self.channels[ch].slots[s];
                    slot.mul = MUL_TABLE[(v & 0x0f) as usize];
                    slot.ksr_shift = if v & 0x10 != 0 { 0 } else { 2 };
                    slot.eg_sustain = v & 0x20 != 0;
                    slot.vib = v & 0x40 != 0;
                    slot.ams = v & 0x80 != 0;
                }
                self.calc_fcslot(ch, s);
            }
            0x40 => {
                let Some((ch, s)) = slot_of(r) else { return };
                let ksl_base = self.channels[ch].ksl_base;
                let slot = &mut self.channels[ch].slots[s];
                let ksl = v >> 6;
                slot.ksl = if ksl != 0 { 3 - ksl as u32 } else { 31 };
                slot.tl = ((v & 0x3f) as f64 * (0.75 / EG_STEP)) as i32;
                slot.tll = slot.tl + (ksl_base >> slot.ksl) as i32;
            }
            0x60 => {
                let Some((ch, s)) = slot_of(r) else { return };
                let (ar_table, dr_table) = (&self.ar_table, &self.dr_table);
                let slot = &mut self.channels[ch].slots[s];
                let (ar, dr) = ((v >> 4) as usize, (v & 0x0f) as usize);
                slot.ar = (ar != 0).then_some(ar << 2);
                slot.evsa = Self::rate(ar_table, slot.ar, slot.ksr);
                if slot.evm == ENV_AR {
                    slot.evs = slot.evsa;
                }
                slot.dr = (dr != 0).then_some(dr << 2);
                slot.evsd = Self::rate(dr_table, slot.dr, slot.ksr);
                if slot.evm == ENV_DR {
                    slot.evs = slot.evsd;
                }
            }
            0x80 => {
                let Some((ch, s)) = slot_of(r) else { return };
                let (sl_table, dr_table) = (&self.sl_table, &self.dr_table);
                let slot = &mut self.channels[ch].slots[s];
                slot.sl = sl_table[(v >> 4) as usize];
                if slot.evm == ENV_DR {
                    slot.eve = slot.sl;
                }
                slot.rr = ((v & 0x0f) as usize) << 2;
                slot.evsr = dr_table[slot.rr + slot.ksr as usize];
                if slot.evm == ENV_RR {
                    slot.evs = slot.evsr;
                }
            }
            0xa0 => {
                if r == 0xbd {
                    self.write_rhythm(v);
                    return;
                }
                if (r & 0x0f) > 8 {
                    return;
                }
                let c = (r & 0x0f) as usize;
                let ch = &mut self.channels[c];
                let block_fnum = if r & 0x10 == 0 {
                    (ch.block_fnum & 0x1f00) | v as u32
                } else {
                    let keyon = (v >> 5) & 1 != 0;
                    if ch.keyon != keyon {
                        ch.keyon = keyon;
                        if keyon {
                            ch.op1_out = [0, 0];
                            Self::key_on(&mut ch.slots[0]);
                            Self::key_on(&mut ch.slots[1]);
                        } else {
                            Self::key_off(&mut ch.slots[0]);
                            Self::key_off(&mut ch.slots[1]);
                        }
                    }
                    (((v & 0x1f) as u32) << 8) | (ch.block_fnum & 0xff)
                };
                if ch.block_fnum != block_fnum {
                    let block_rv = 7 - (block_fnum >> 10);
                    let fnum = (block_fnum & 0x3ff) as usize;
                    ch.block_fnum = block_fnum;
                    ch.ksl_base = self.ksl_table[(block_fnum >> 6) as usize];
                    ch.fc = self.fn_table[fnum] >> block_rv;
                    ch.kcode = (block_fnum >> 9) as u8;
                    self.calc_fcslot(c, 0);
                    self.calc_fcslot(c, 1);
                }
            }
            0xc0 => {
                if (r & 0x0f) > 8 {
                    return;
                }
                let ch = &mut self.channels[(r & 0x0f) as usize];
                let feedback = ((v >> 1) & 7) as u32;
                ch.fb = if feedback != 0 { 9 - feedback } else { 0 };
                ch.con = v & 1 != 0;
            }
            0xe0 => {
                let Some((ch, s)) = slot_of(r) else { return };
                if self.wavesel {
                    self.channels[ch].slots[s].wave = (v & 3) as usize;
                }
            }
            _ => {}
        }
    }

    fn write_rhythm(&mut self, v: u8) {
        let rkey = self.rhythm ^ v;
        self.ams_deep = v & 0x80 != 0;
        self.vib_deep = v & 0x40 != 0;
        self.rhythm = v & 0x3f;
        if self.rhythm & 0x20 == 0 {
            return;
        }
        let keys = [
            (0x10, 6, 0, true),
            (0x08, 7, 1, false),
            (0x04, 8, 0, false),
            (0x02, 8, 1, false),
            (0x01, 7, 0, false),
        ];
        for (bit, c, s, bass) in keys {
            if rkey & bit == 0 {
                continue;
            }
            let ch = &mut self.channels[c];
            if v & bit != 0 {
                if bass {
                    ch.op1_out = [0, 0];
                    Self::key_on(&mut ch.slots[0]);
                    Self::key_on(&mut ch.slots[1]);
                } else {
                    Self::key_on(&mut ch.slots[s]);
                }
            } else if bass {
                Self::key_off(&mut ch.slots[0]);
                Self::key_off(&mut ch.slots[1]);
            } else {
                Self::key_off(&mut ch.slots[s]);
            }
        }
    }

    /// Advances an operator's envelope; its output level.
    fn calc_slot(env_curve: &[i32], slot: &mut Slot, ams: i32) -> u32 {
        slot.evc = slot.evc.wrapping_add(slot.evs);
        if slot.evc >= slot.eve {
            match slot.evm {
                ENV_AR => {
                    slot.evm = ENV_DR;
                    slot.evc = EG_DST;
                    slot.eve = slot.sl;
                    slot.evs = slot.evsd;
                }
                ENV_DR => {
                    slot.evc = slot.sl;
                    slot.eve = EG_DED;
                    if slot.eg_sustain {
                        slot.evs = 0;
                    } else {
                        slot.evm = ENV_RR;
                        slot.evs = slot.evsr;
                    }
                }
                _ => {
                    slot.evc = EG_OFF;
                    slot.eve = EG_OFF + 1;
                    slot.evs = 0;
                }
            }
        }
        (slot.tll + env_curve[(slot.evc >> ENV_BITS) as usize] + if slot.ams { ams } else { 0 })
            as u32
    }

    fn op_out(&self, slot: &Slot, env: u32, phase_mod: i32) -> i32 {
        let phase = ((slot.cnt.wrapping_add(phase_mod as u32)) >> (24 - SIN_SHIFT)) as usize
            & (SIN_ENT - 1);
        self.tl_table[self.sin_table[slot.wave * SIN_ENT + phase] + env as usize]
    }

    fn advance_phase(slot: &mut Slot, vib: i32) {
        if slot.vib {
            slot.cnt = slot
                .cnt
                .wrapping_add(slot.incr.wrapping_mul(vib as u32) >> VIB_RATE_SHIFT);
        } else {
            slot.cnt = slot.cnt.wrapping_add(slot.incr);
        }
    }

    fn calc_channel(&mut self, c: usize, ams: i32, vib: i32, out: &mut i32) {
        let mut feedback2 = 0;
        let env = Self::calc_slot(&self.env_curve, &mut self.channels[c].slots[0], ams);
        if env < (EG_ENT - 1) as u32 {
            Self::advance_phase(&mut self.channels[c].slots[0], vib);
            let ch = &self.channels[c];
            let value = if ch.fb != 0 {
                let feedback1 = (ch.op1_out[0] + ch.op1_out[1]) >> ch.fb;
                let value = self.op_out(&ch.slots[0], env, feedback1);
                let ch = &mut self.channels[c];
                ch.op1_out[1] = ch.op1_out[0];
                ch.op1_out[0] = value;
                value
            } else {
                self.op_out(&ch.slots[0], env, 0)
            };
            if self.channels[c].con {
                *out += value;
            } else {
                feedback2 += value;
            }
        } else {
            let ch = &mut self.channels[c];
            ch.op1_out[1] = ch.op1_out[0];
            ch.op1_out[0] = 0;
        }
        let env = Self::calc_slot(&self.env_curve, &mut self.channels[c].slots[1], ams);
        if env < (EG_ENT - 1) as u32 {
            Self::advance_phase(&mut self.channels[c].slots[1], vib);
            *out += self.op_out(&self.channels[c].slots[1], env, feedback2);
        }
    }

    fn next_noise_bit(&mut self) -> i32 {
        // A small linear-feedback register, standing in for ScummVM's random source.
        let bit = (self.noise ^ (self.noise >> 14)) & 1;
        self.noise = (self.noise >> 1) | (bit << 22);
        bit as i32
    }

    fn calc_rhythm(&mut self, ams: i32, vib: i32, out: &mut i32) {
        let whitenoise = self.next_noise_bit() * (EG_ENT >> 4);
        // Bass drum: as a normal channel in serial mode, twice as loud.
        let mut feedback2 = 0;
        let env = Self::calc_slot(&self.env_curve, &mut self.channels[6].slots[0], ams);
        if env < (EG_ENT - 1) as u32 {
            Self::advance_phase(&mut self.channels[6].slots[0], vib);
            let ch = &self.channels[6];
            if ch.fb != 0 {
                let feedback1 = (ch.op1_out[0] + ch.op1_out[1]) >> ch.fb;
                let value = self.op_out(&ch.slots[0], env, feedback1);
                let ch = &mut self.channels[6];
                ch.op1_out[1] = ch.op1_out[0];
                ch.op1_out[0] = value;
                feedback2 = value;
            } else {
                feedback2 = self.op_out(&ch.slots[0], env, 0);
            }
        } else {
            let ch = &mut self.channels[6];
            ch.op1_out[1] = ch.op1_out[0];
            ch.op1_out[0] = 0;
        }
        let env = Self::calc_slot(&self.env_curve, &mut self.channels[6].slots[1], ams);
        if env < (EG_ENT - 1) as u32 {
            Self::advance_phase(&mut self.channels[6].slots[1], vib);
            *out += self.op_out(&self.channels[6].slots[1], env, feedback2) * 2;
        }

        let curve = &self.env_curve;
        let env_sd =
            Self::calc_slot(curve, &mut self.channels[7].slots[1], ams) + whitenoise as u32;
        let env_tam = Self::calc_slot(curve, &mut self.channels[8].slots[0], ams);
        let env_top = Self::calc_slot(curve, &mut self.channels[8].slots[1], ams);
        let env_hh =
            Self::calc_slot(curve, &mut self.channels[7].slots[0], ams) + whitenoise as u32;

        let (fc7, fc8) = (self.channels[7].fc, self.channels[8].fc);
        let vibu = vib as u32;
        {
            let s = &mut self.channels[7].slots[0];
            s.cnt = s.cnt.wrapping_add(if s.vib {
                s.incr.wrapping_mul(vibu) >> (VIB_RATE_SHIFT - 1)
            } else {
                s.incr.wrapping_mul(2)
            });
        }
        {
            let s = &mut self.channels[7].slots[1];
            s.cnt = s.cnt.wrapping_add(if s.vib {
                fc7.wrapping_mul(vibu) >> (VIB_RATE_SHIFT - 3)
            } else {
                fc7.wrapping_mul(8)
            });
        }
        Self::advance_phase(&mut self.channels[8].slots[0], vib);
        {
            let s = &mut self.channels[8].slots[1];
            s.cnt = s.cnt.wrapping_add(if s.vib {
                fc8.wrapping_mul(3).wrapping_mul(vibu) >> (VIB_RATE_SHIFT - 4)
            } else {
                fc8.wrapping_mul(48)
            });
        }
        let tone8 = self.op_out(&self.channels[8].slots[1], whitenoise as u32, 0);
        let limit = (EG_ENT - 1) as u32;
        if env_sd < limit {
            *out += self.op_out(&self.channels[7].slots[0], env_sd, 0) * 8;
        }
        if env_tam < limit {
            *out += self.op_out(&self.channels[8].slots[0], env_tam, 0) * 2;
        }
        if env_top < limit {
            *out += self.op_out(&self.channels[7].slots[1], env_top, tone8) * 2;
        }
        if env_hh < limit {
            *out += self.op_out(&self.channels[7].slots[1], env_hh, tone8) * 2;
        }
    }

    /// Fills `buffer` with the next samples.
    pub fn generate(&mut self, buffer: &mut [i16]) {
        let rhythm = self.rhythm & 0x20 != 0;
        let ams_base = if self.ams_deep { AMS_ENT } else { 0 };
        let vib_base = if self.vib_deep { VIB_ENT } else { 0 };
        for sample in buffer.iter_mut() {
            self.ams_cnt = self.ams_cnt.wrapping_add(self.ams_incr);
            self.vib_cnt = self.vib_cnt.wrapping_add(self.vib_incr);
            let ams = self.ams_table[ams_base + (self.ams_cnt >> AMS_SHIFT) as usize];
            let vib = self.vib_table[vib_base + (self.vib_cnt >> VIB_SHIFT) as usize];
            let mut out = 0i32;
            let melodic = if rhythm { 6 } else { 9 };
            for c in 0..melodic {
                self.calc_channel(c, ams, vib, &mut out);
            }
            if rhythm {
                self.calc_rhythm(ams, vib, &mut out);
            }
            *sample = (out.clamp(MIN_OUT, MAX_OUT) >> OUT_SHIFT) as i16;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyed_note_sounds_and_fades_after_key_off() {
        let mut opl = Opl::new(44_100);
        // Channel 0: carrier at full level, fast attack, sustain, quick release; a4.
        opl.write(0x20, 0x01);
        opl.write(0x23, 0x21);
        opl.write(0x40, 0x3f);
        opl.write(0x43, 0x00);
        opl.write(0x60, 0xff);
        opl.write(0x63, 0xf4);
        opl.write(0x80, 0x0f);
        opl.write(0x83, 0x0f);
        opl.write(0xa0, 0x41);
        opl.write(0xb0, 0x32);
        let mut buffer = vec![0i16; 4410];
        opl.generate(&mut buffer);
        let loud = buffer
            .iter()
            .map(|s| s.unsigned_abs() as u32)
            .max()
            .unwrap();
        assert!(loud > 5000, "zu leise: {loud}");
        opl.write(0xb0, 0x12);
        let mut tail = vec![0i16; 44_100];
        opl.generate(&mut tail);
        let end = tail[40_000..]
            .iter()
            .map(|s| s.unsigned_abs() as u32)
            .max()
            .unwrap();
        assert!(end < 50, "klingt nicht aus: {end}");
    }

    #[test]
    fn silence_without_notes() {
        let mut opl = Opl::new(44_100);
        let mut buffer = vec![1i16; 1000];
        opl.generate(&mut buffer);
        assert!(buffer.iter().all(|&s| s == 0));
    }
}
