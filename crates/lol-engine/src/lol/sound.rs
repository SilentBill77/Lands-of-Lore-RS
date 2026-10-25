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

//! Sound (ScummVM's `sound_lol.cpp` and the environmental sounds of `kyra_rpg.cpp`): the
//! characters' speech from the `TLK` archives, sound effects as voices or AdLib tracks, the
//! music files and tracks, sounds of the level fading with distance and walls.

use lol_data::tables::{INGAME_SFX_INDEX, MUSIC_TRACK_MAP};

use super::{Lol, TICK};
use crate::audio::VoiceKind;

impl Lol {
    /// `Sound::getVoiceStream`: a voice file (without extension) as samples.
    /// (`_supportedCodecs`: the name with `.VOC`, then as it is.)
    pub fn voice_samples(&self, file: &str) -> Option<(Vec<i16>, u32)> {
        for name in [format!("{file}.VOC"), file.to_string()] {
            if let Some(bytes) = self.res.file_data(&name) {
                let v = lol_data::voc::parse(&name, &bytes).ok()?;
                return Some((v.samples, v.rate));
            }
        }
        None
    }

    pub fn is_voice_present(&self, file: &str) -> bool {
        self.res.exists(&format!("{file}.VOC")) || self.res.exists(file)
    }

    /// `Sound::voicePlay`: the voice's length in milliseconds.
    pub fn snd_voice_play(&mut self, file: &str, volume: i32, is_sfx: bool) -> u32 {
        self.snd_voice_play_prio(file, volume, 255, is_sfx).1
    }

    fn snd_voice_play_prio(
        &mut self,
        file: &str,
        volume: i32,
        priority: i32,
        is_sfx: bool,
    ) -> (Option<u32>, u32) {
        let Some((samples, rate)) = self.voice_samples(file) else {
            return (None, 0);
        };
        let len = (samples.len() as u64 * 1000 / rate.max(1) as u64) as u32;
        let kind = if is_sfx {
            VoiceKind::Effect
        } else {
            VoiceKind::Speech
        };
        let h = self.screen.system.mixer().play_voice_stream(
            samples,
            rate,
            volume.clamp(0, 255) as u8,
            priority.clamp(0, 255) as u8,
            kind,
        );
        (h, len)
    }

    fn voice_is_playing(&self, handle: u32) -> bool {
        handle != 0 && self.screen.system.mixer().voice_playing(handle)
    }

    /// `snd_playCharacterSpeech`: the speech `id` by `speaker` (a character's index or the
    /// first letter of the name); false if there is none.
    pub fn snd_play_character_speech(&mut self, id: i32, speaker: i32, _unused: bool) -> bool {
        if !self.speech_enabled() {
            return false;
        }
        let mut speaker = speaker as i8 as i32;
        if speaker < 65 {
            let c = &self.characters[(speaker.max(0) as usize).min(3)];
            speaker = if c.flags & 1 != 0 {
                c.name.first().copied().unwrap_or(0) as i32
            } else {
                0
            };
        }
        if self.last_speech_id == id && speaker == self.last_speaker {
            return true;
        }
        self.last_speech_id = id;
        self.last_speaker = speaker;
        self.next_speech_id = -1;
        self.next_speaker = -1;
        let pattern2 = format!(
            "{:02}",
            if id & 0x4000 != 0 {
                0
            } else {
                self.cur_tlk_file
            }
        );
        let mut list: Vec<(Vec<i16>, u32)> = Vec::new();
        let sp = speaker as u8 as char;
        let mut pattern1 = String::new();
        let mut file3 = String::new();
        if id & 0x4000 != 0 {
            pattern1 = format!("{:03X}", id & 0x3FFF);
        } else if id < 1000 {
            pattern1 = format!("{id:03}");
        } else {
            file3 = format!("@{:04}{sp}.{pattern2}", id - 1000);
            if let Some(v) = self.voice_samples(&file3) {
                list.push(v);
            }
        }
        if file3.is_empty() {
            for i in 0..36u8 {
                let symbol = (b'0' + i) as char;
                let f1 = format!("{pattern1}{sp}{symbol}.{pattern2}");
                let f2 = format!("{pattern1}_{symbol}.{pattern2}");
                if let Some(v) = self.voice_samples(&f1) {
                    list.push(v);
                } else if let Some(v) = self.voice_samples(&f2) {
                    list.push(v);
                } else {
                    break;
                }
            }
        }
        if list.is_empty() {
            return false;
        }
        while self.voice_is_playing(self.speech_handle) && !self.should_quit() {
            self.delay(TICK, true);
        }
        while self.screen.system.mixer().all_voice_channels_playing() && !self.should_quit() {
            self.delay(TICK, false);
        }
        self.active_voice_file_total_time = list
            .iter()
            .map(|(s, r)| (s.len() as u64 * 1000 / (*r).max(1) as u64) as u32)
            .sum();
        self.speech_list = list.into_iter().collect();
        if let Some((s, r)) = self.speech_list.pop_front() {
            self.speech_handle = self
                .screen
                .system
                .mixer()
                .play_voice_stream(s, r, 255, 255, VoiceKind::Speech)
                .unwrap_or(0);
        }
        if self.active_voice_file_total_time == 0 {
            return false;
        }
        self.tim.abort_flag = 0;
        true
    }

    /// `snd_updateCharacterSpeech`: 2 while speech plays.
    pub fn snd_update_character_speech(&mut self) -> i32 {
        if self.voice_is_playing(self.speech_handle) {
            return 2;
        }
        if let Some((s, r)) = self.speech_list.pop_front() {
            self.speech_handle = self
                .screen
                .system
                .mixer()
                .play_voice_stream(s, r, 255, 255, VoiceKind::Speech)
                .unwrap_or(0);
            return 2;
        } else if self.next_speech_id != -1 {
            self.last_speech_id = -1;
            self.last_speaker = -1;
            self.active_voice_file_total_time = 0;
            let (id, sp) = (self.next_speech_id, self.next_speaker);
            if self.snd_play_character_speech(id, sp, false) {
                return 2;
            }
        }
        self.last_speech_id = -1;
        self.last_speaker = -1;
        self.active_voice_file_total_time = 0;
        0
    }

    pub fn snd_stop_speech(&mut self, set_flag: bool) {
        if !self.voice_is_playing(self.speech_handle) {
            return;
        }
        let h = self.speech_handle;
        self.screen.system.mixer().stop_voice(h);
        self.active_voice_file_total_time = 0;
        self.next_speech_id = -1;
        self.next_speaker = -1;
        self.speech_list.clear();
        if set_flag {
            self.tim.abort_flag = 1;
        }
    }

    /// `snd_playSoundEffect`: a sound of the game's list (a voice file or an AdLib track).
    pub fn snd_play_sound_effect(&mut self, track: i32, volume: i32) {
        if (track == 1 && (self.last_sfx_track == -1 || self.last_sfx_track == 1))
            || self.should_quit()
        {
            return;
        }
        self.last_sfx_track = track;
        if track == -1 || track >= (INGAME_SFX_INDEX.len() / 2) as i32 || track < 0 {
            return;
        }
        let mut volume = volume & 0xFF;
        let pr_index = INGAME_SFX_INDEX[track as usize * 2 + 1] as i16 as i32;
        let priority = if pr_index > 0 {
            (pr_index * volume) >> 8
        } else {
            -pr_index
        } as u16;
        const V1: [i32; 6] = [223, 159, 95, 47, 15, 0];
        const V2: [i32; 6] = [255, 191, 127, 63, 30, 0];
        for i in 0..6 {
            if V1[i] < volume {
                volume = V2[i];
                break;
            }
        }
        let voc_index = INGAME_SFX_INDEX[track as usize * 2] as i16 as i32;
        let voc = if voc_index != -1 {
            self.ingame_sound_list
                .get(voc_index as usize)
                .filter(|s| !s.eq_ignore_ascii_case("EMPTY"))
                .cloned()
        } else {
            None
        };
        if let Some(v) = voc {
            if self.is_voice_present(&v) {
                self.snd_voice_play_prio(&v, volume, priority as i32 & 0xFF, true);
            }
        } else {
            let t = if track == 168 { 167 } else { track };
            self.screen
                .system
                .mixer()
                .play_adlib_effect(t as u8, volume as u8);
        }
    }

    /// `KyraRpgEngine::snd_processEnvironmentalSoundEffect`, then `LoLEngine`'s.
    pub fn snd_process_environmental_sound_effect(&mut self, sound_id: i32, block: i32) -> bool {
        if !self.screen.system.mixer().sfx_enabled || self.should_quit() {
            return false;
        }
        if self.environment_sfx != 0 {
            let (s, v) = (self.environment_sfx, self.environment_sfx_vol);
            self.snd_play_sound_effect(s, v);
        }
        let mut dist = 0;
        if block != 0 {
            dist = Self::get_block_distance(self.current_block, block as u16);
            if dist > self.env_sfx_dist_threshold {
                self.environment_sfx = 0;
                return false;
            }
        }
        self.environment_sfx = sound_id;
        self.environment_sfx_vol = (15 - if block != 0 || dist < 2 { dist } else { 0 }) << 4;
        if block != self.current_block as i32 {
            const SHIFT: [i32; 8] = [-32, -31, 1, 33, 32, 31, -1, -33];
            let mut cbl = self.current_block;
            for _ in 0..3 {
                let dir = self.calc_monster_direction(
                    cbl & 0x1F,
                    cbl >> 5,
                    (block & 0x1F) as u16,
                    (block >> 5) as u16,
                );
                cbl = ((cbl as i32 + SHIFT[dir as usize & 7]) & 0x3FF) as u16;
                if cbl as i32 == block {
                    break;
                }
                if self.test_wall_flag(cbl, 0, 1) {
                    self.environment_sfx_vol >>= 1;
                }
            }
        }
        if sound_id == 0 || self.scene_update_required {
            return false;
        }
        self.snd_process_environmental_sound_effect(0, 0)
    }

    pub fn snd_update_environmental_sfx(&mut self, sound_id: i32) {
        let cb = self.current_block as i32;
        self.snd_process_environmental_sound_effect(sound_id, cb);
    }

    pub fn snd_queue_environmental_sound_effect(&mut self, sound_id: i32, block: i32) {
        if self.env_sfx_use_queue && self.env_sfx_num_tracks_in_queue < 10 {
            let n = self.env_sfx_num_tracks_in_queue;
            self.env_sfx_queued_tracks[n] = sound_id as u16;
            self.env_sfx_queued_blocks[n] = block as u16;
            self.env_sfx_num_tracks_in_queue += 1;
        } else {
            self.snd_process_environmental_sound_effect(sound_id, block);
        }
    }

    pub fn snd_play_queued_effects(&mut self) {
        for i in 0..self.env_sfx_num_tracks_in_queue {
            let (t, b) = (
                self.env_sfx_queued_tracks[i] as i32,
                self.env_sfx_queued_blocks[i] as i32,
            );
            self.snd_process_environmental_sound_effect(t, b);
        }
        self.env_sfx_num_tracks_in_queue = 0;
    }

    /// `snd_loadSoundFile`: the music file of a track (250 on).
    pub fn snd_load_sound_file(&mut self, track: i32) {
        if !self.screen.system.mixer().music_enabled {
            return;
        }
        self.snd_stop_music();
        let t = (track - 250) * 3;
        if t < 0 || t as usize + 1 >= MUSIC_TRACK_MAP.len() {
            return;
        }
        let (idx, ext) = (
            MUSIC_TRACK_MAP[t as usize] as i32,
            MUSIC_TRACK_MAP[t as usize + 1],
        );
        if self.cur_music_file_index == idx && self.cur_music_file_ext == ext {
            return;
        }
        let name = format!("LORE{idx:02}{}", ext as char);
        self.snd_load_sound_file_named(&name);
        self.cur_music_file_index = idx;
        self.cur_music_file_ext = ext;
    }

    /// `Sound::loadSoundFile` for a name (the AdLib file `.ADL`).
    pub fn snd_load_sound_file_named(&mut self, name: &str) {
        let base = name.split('.').next().unwrap_or(name);
        let file = format!("{base}.ADL");
        if self.screen.system.mixer().loaded_adlib() == Some(file.as_str()) {
            return;
        }
        // `internalLoadFile`: the loaded file's tracks halted, then `playSoundEffect(0)` twice
        // (full volume) before the new data goes in.
        if self.screen.system.mixer().loaded_adlib().is_some() {
            self.screen.system.mixer().halt_music();
        }
        if let Some(d) = self.res.file_data(&file) {
            let mut m = self.screen.system.mixer();
            m.play_adlib_effect(0, 0xFF);
            m.play_adlib_effect(0, 0xFF);
            m.load_adlib(&file, &d);
        }
    }

    /// `snd_playTrack`: the track before.
    pub fn snd_play_track(&mut self, track: i32) -> i32 {
        if track == -1 {
            return self.last_music_track;
        }
        let res = self.last_music_track;
        self.last_music_track = track;
        if self.screen.system.mixer().music_enabled {
            self.snd_load_sound_file(track);
            let t = (track - 250) * 3;
            if t >= 0 && (t as usize + 2) < MUSIC_TRACK_MAP.len() {
                let tr = MUSIC_TRACK_MAP[t as usize + 2];
                self.screen.system.mixer().play_music(tr);
            }
        }
        res
    }

    pub fn snd_stop_music(&mut self) -> i32 {
        let enabled = self.screen.system.mixer().music_enabled;
        if enabled {
            if self.screen.system.mixer().music_playing() {
                self.screen.system.mixer().fade_out_music();
                self.screen.system.delay_millis(3 * TICK);
            }
            self.screen.system.mixer().halt_music();
        }
        self.snd_play_track(-1)
    }

    /// `getVolume(kVolumeSpeech)` in the original's steps (2–…).
    pub fn speech_volume_level(&self) -> i32 {
        (self.screen.system.mixer().speech_volume as i32 * 100) / 255 + 2
    }
}
