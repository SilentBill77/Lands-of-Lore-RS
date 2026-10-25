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

//! The sound card as the game uses it: Westwood's AdLib driver (version 4) on the emulated
//! OPL2 for music and most sounds (ScummVM's `SoundPC_v1`), and digital voices for speech and
//! sampled sounds (`Sound::voicePlay`). The window's audio thread pulls the samples
//! ([`Mixer::render`]); the game thread sends commands.

use lol_sound::adlib::{AdLibDriver, CALLBACKS_PER_SECOND, split_sound_file};

/// The driver's version in Lands of Lore.
const VERSION: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceKind {
    Speech,
    Effect,
}

struct Voice {
    handle: u32,
    samples: Vec<i16>,
    rate: u32,
    /// Position in 16.16 fixed point.
    pos: u64,
    volume: u32,
    kind: VoiceKind,
    priority: u8,
}

/// `kNumChannelHandles`: the voices that play at once.
pub const VOICE_CHANNELS: usize = 4;

pub struct Mixer {
    rate: u32,
    driver: AdLibDriver,
    tracks: Vec<Option<u16>>,
    loaded: Option<String>,
    fraction: u32,
    pending: Vec<i16>,
    voices: Vec<Voice>,
    next_handle: u32,
    pub music_enabled: bool,
    pub sfx_enabled: bool,
    pub speech_enabled: bool,
    /// 0–255.
    pub music_volume: u8,
    pub sfx_volume: u8,
    pub speech_volume: u8,
}

impl Mixer {
    pub fn new(rate: u32) -> Self {
        Self {
            rate,
            driver: AdLibDriver::new(VERSION, rate),
            tracks: Vec::new(),
            loaded: None,
            fraction: 0,
            pending: Vec::new(),
            voices: Vec::new(),
            next_handle: 1,
            music_enabled: true,
            sfx_enabled: true,
            speech_enabled: true,
            music_volume: 255,
            sfx_volume: 255,
            speech_volume: 255,
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// `internalLoadFile`: a sound file (`LORE01A.ADL` …) into the driver, unless loaded.
    pub fn load_adlib(&mut self, name: &str, bytes: &[u8]) {
        if self.loaded.as_deref() == Some(name) {
            return;
        }
        let Some((tracks, data)) = split_sound_file(VERSION, bytes) else {
            return;
        };
        self.driver.stop_all();
        self.driver.set_sound_data(data);
        self.tracks = tracks;
        self.loaded = Some(name.to_string());
    }

    pub fn loaded_adlib(&self) -> Option<&str> {
        self.loaded.as_deref()
    }

    /// `SoundPC_v1::play`: the sound of a track number.
    pub fn play_adlib(&mut self, track: u8, volume: u8) {
        if let Some(Some(id)) = self.tracks.get(track as usize) {
            self.driver.start_sound(*id, volume);
        }
    }

    pub fn play_music(&mut self, track: u8) {
        if self.music_enabled {
            self.play_adlib(track, 0xff);
        }
    }

    pub fn play_adlib_effect(&mut self, track: u8, volume: u8) {
        if self.sfx_enabled {
            self.play_adlib(track, volume);
        }
    }

    /// `haltTrack`.
    pub fn halt_music(&mut self) {
        self.play_adlib(0, 0);
        self.play_adlib(0, 0);
    }

    /// Fades the music out (`beginFadeOut`: track 1).
    pub fn fade_out_music(&mut self) {
        self.play_adlib(1, 0xff);
    }

    /// Whether music still plays (channel 0 of the driver).
    pub fn music_playing(&self) -> bool {
        self.driver.active()[0]
    }

    /// Plays samples; returns a handle to ask after them.
    pub fn play_voice(&mut self, samples: Vec<i16>, rate: u32, volume: u8, kind: VoiceKind) -> u32 {
        let enabled = match kind {
            VoiceKind::Speech => self.speech_enabled,
            VoiceKind::Effect => self.sfx_enabled,
        };
        let handle = self.next_handle;
        self.next_handle = self.next_handle.wrapping_add(1).max(1);
        if !enabled || samples.is_empty() || rate == 0 {
            return handle;
        }
        self.voices.push(Voice {
            handle,
            samples,
            rate,
            pos: 0,
            volume: volume as u32,
            kind,
            priority: 0xff,
        });
        handle
    }

    /// `Sound::playVoiceStream`: into one of the four channels, a playing one of lower
    /// priority stopped if all are busy; `None` if none is free.
    pub fn play_voice_stream(
        &mut self,
        samples: Vec<i16>,
        rate: u32,
        volume: u8,
        priority: u8,
        kind: VoiceKind,
    ) -> Option<u32> {
        if self.voices.len() >= VOICE_CHANNELS {
            let victim = self.voices.iter().position(|v| v.priority <= priority)?;
            self.voices.remove(victim);
        }
        let h = self.play_voice(samples, rate, volume, kind);
        if let Some(v) = self.voices.iter_mut().find(|v| v.handle == h) {
            v.priority = priority;
        }
        Some(h)
    }

    /// `allVoiceChannelsPlaying`.
    pub fn all_voice_channels_playing(&self) -> bool {
        self.voices.len() >= VOICE_CHANNELS
    }

    pub fn voice_playing(&self, handle: u32) -> bool {
        self.voices.iter().any(|v| v.handle == handle)
    }

    pub fn any_voice_playing(&self, kind: VoiceKind) -> bool {
        self.voices.iter().any(|v| v.kind == kind)
    }

    pub fn voice_count(&self) -> usize {
        self.voices.len()
    }

    pub fn stop_voice(&mut self, handle: u32) {
        self.voices.retain(|v| v.handle != handle);
    }

    pub fn stop_voices(&mut self, kind: VoiceKind) {
        self.voices.retain(|v| v.kind != kind);
    }

    /// Milliseconds a voice still plays.
    pub fn voice_remaining_ms(&self, handle: u32) -> u32 {
        self.voices
            .iter()
            .find(|v| v.handle == handle)
            .map_or(0, |v| {
                let left = (v.samples.len() as u64).saturating_sub(v.pos >> 16);
                (left * 1000 / v.rate as u64) as u32
            })
    }

    /// Mono samples at the mixer's rate.
    pub fn render(&mut self, out: &mut [i16]) {
        // The AdLib driver runs 72 times a second; between its runs the chip sounds.
        let mut filled = 0;
        while filled < out.len() {
            if self.pending.is_empty() {
                self.driver.callback();
                self.fraction += self.rate;
                let n = (self.fraction / CALLBACKS_PER_SECOND) as usize;
                self.fraction %= CALLBACKS_PER_SECOND;
                self.pending.resize(n, 0);
                let mut buffer = std::mem::take(&mut self.pending);
                self.driver.opl.generate(&mut buffer);
                buffer.reverse();
                self.pending = buffer;
                if self.pending.is_empty() {
                    continue;
                }
            }
            let music = self.music_volume as i32;
            while filled < out.len() {
                let Some(s) = self.pending.pop() else { break };
                out[filled] = ((s as i32 * music) >> 8) as i16;
                filled += 1;
            }
        }
        // Digital voices, resampled to the mixer's rate.
        let rate = self.rate as u64;
        let (sfx, speech) = (self.sfx_volume as i32, self.speech_volume as i32);
        for voice in &mut self.voices {
            let step = ((voice.rate as u64) << 16) / rate;
            let gain = (voice.volume as i32
                * match voice.kind {
                    VoiceKind::Speech => speech,
                    VoiceKind::Effect => sfx,
                })
                >> 8;
            for sample in out.iter_mut() {
                let i = (voice.pos >> 16) as usize;
                let Some(&s) = voice.samples.get(i) else {
                    break;
                };
                let mixed = *sample as i32 + ((s as i32 * gain) >> 8);
                *sample = mixed.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
                voice.pos += step;
            }
        }
        self.voices
            .retain(|v| ((v.pos >> 16) as usize) < v.samples.len());
    }

    /// Test runs without sound: time passes for the voices and the driver.
    pub fn advance_virtual(&mut self, ms: u32) {
        for voice in &mut self.voices {
            voice.pos += ((voice.rate as u64 * ms as u64) / 1000) << 16;
        }
        self.voices
            .retain(|v| ((v.pos >> 16) as usize) < v.samples.len());
        let ticks = (ms * CALLBACKS_PER_SECOND).div_ceil(1000).min(1000);
        for _ in 0..ticks {
            self.driver.callback();
        }
    }
}
