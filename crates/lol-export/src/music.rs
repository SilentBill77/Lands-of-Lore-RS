// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writing the music and sounds of the AdLib card out as WAV files (16 bit, mono, 44.1 kHz),
//! played by Westwood's driver (version 4) on the emulated OPL2: into `musik/` (pieces: music
//! repeating or playing longer than `PIECE` seconds) and `musik/klaenge/` (sound effects), one
//! file per sound, named after the sound file and the first track that plays it
//! (`LORE01A_03.wav`). Sounds heard before in another file are left out. Music is played once
//! through and faded out in its repeat; endless sounds are cut off and faded out after
//! `LONGEST` seconds.
//!
//! The files: every `.ADL` in the archives (`MUSIC.PAK` with `LORE01A.ADL` …, `LOREINTR.ADL`,
//! `LOREFINL.ADL`, the sounds `LORESFX.ADL`).

use std::path::Path;

use lol_data::Result;
use lol_data::assets::AssetStore;
use lol_data::paths::{Installation, Language};
use lol_sound::adlib::split_sound_file;

const RATE: u32 = 44_100;
/// The driver's version in Lands of Lore.
const VERSION: u8 = 4;
/// The longest a piece may play before it is cut off (seconds).
const LONGEST: f32 = 300.0;
/// Sounds longer than this (seconds) count as music.
const PIECE: f32 = 20.0;

pub fn all(copy: &Installation, target: &Path) -> Result<()> {
    let store = AssetStore::open(copy, Language::German)?;
    let music = target.join("musik");
    let sounds = music.join("klaenge");
    for dir in [&music, &sounds] {
        std::fs::create_dir_all(dir).map_err(|e| lol_data::Error::io(dir.display(), e))?;
    }
    let mut done_files: Vec<String> = Vec::new();
    let (mut pieces, mut effects) = (0, 0);
    let mut heard: Vec<u64> = Vec::new();
    for archive in store.archives() {
        for (name, bytes) in archive.files() {
            if !name.ends_with(".ADL") || done_files.iter().any(|d| d == name) {
                continue;
            }
            done_files.push(name.to_string());
            let Some((tracks, data)) = split_sound_file(VERSION, bytes) else {
                continue;
            };
            let stem = name.split('.').next().unwrap_or(name);
            let mut done: Vec<u16> = Vec::new();
            for (track, sound) in tracks.iter().enumerate() {
                let Some(sound) = *sound else { continue };
                if done.contains(&sound) {
                    continue;
                }
                done.push(sound);
                let rendered = lol_sound::render(VERSION, data, sound, RATE, LONGEST);
                // Silent ones (stop commands, programs that only set something up) are left out.
                if rendered.samples.iter().all(|s| s.unsigned_abs() < 64) {
                    continue;
                }
                let fingerprint = fingerprint(&rendered.samples);
                if heard.contains(&fingerprint) {
                    continue;
                }
                heard.push(fingerprint);
                let long = rendered.samples.len() as f32 / RATE as f32 > PIECE;
                let piece = rendered.repeats || (long && !rendered.endless);
                let dir = if piece { &music } else { &sounds };
                let path = dir.join(format!("{stem}_{track:03}.wav"));
                std::fs::write(&path, lol_sound::wav(&rendered.samples, RATE))
                    .map_err(|e| lol_data::Error::io(path.display(), e))?;
                if piece {
                    pieces += 1;
                } else {
                    effects += 1;
                }
            }
        }
    }
    println!(
        "Musik: {pieces} Stücke und {effects} Klänge der AdLib-Karte nach {}",
        music.display()
    );
    Ok(())
}

/// A hash of a sound's samples (FNV-1a), to know it when heard again.
fn fingerprint(samples: &[i16]) -> u64 {
    samples.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, s| {
        (h ^ (*s as u16 as u64)).wrapping_mul(0x0100_0000_01b3)
    })
}
