// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Plays the game without a window in virtual time with random input, to find crashes and
//! hangs: `fuzz <seed> <millis> [<out-dir>]`. The place to begin comes from `LOL_START` (see
//! `main_loop.rs`), `LOL_QUICK` picks the hero. With an out-dir, pictures every 5 seconds.

use std::path::PathBuf;

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_engine::lol::Lol;
use lol_engine::lol::res::Res;
use lol_engine::platform::{Event, Key, Shared, System};
use lol_engine::screen::Screen;

/// A small xorshift; the run is the same for the same seed.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

fn key(k: Key) -> Event {
    let ascii = match k {
        Key::Char(c) => c,
        _ => 0,
    };
    Event::KeyDown {
        key: k,
        ascii,
        ctrl: false,
        alt: false,
        shift: false,
    }
}

/// One random thing a player might do.
fn random_input(d: &mut Dice, sh: &Shared) {
    let click = |sh: &Shared, x: i32, y: i32, b: u8| {
        sh.push_event(Event::MouseMove { x, y });
        sh.push_event(Event::MouseDown { button: b, x, y });
        sh.push_event(Event::MouseUp { button: b, x, y });
    };
    match d.below(100) {
        // Walking and turning most of the time.
        0..=29 => sh.push_event(key(Key::Up)),
        30..=34 => sh.push_event(key(Key::Down)),
        35..=39 => sh.push_event(key(Key::Left)),
        40..=44 => sh.push_event(key(Key::Right)),
        45..=51 => sh.push_event(key(Key::Home)),
        52..=58 => sh.push_event(key(Key::PageUp)),
        // Attacks.
        59..=66 => sh.push_event(key(Key::F(1 + d.below(3) as u8))),
        // Into the scene (doors, switches, items, monsters).
        67..=79 => click(sh, 112 + d.below(176) as i32, d.below(120) as i32, 0),
        // The inventory bar and the portraits.
        80..=85 => click(sh, d.below(320) as i32, 140 + d.below(60) as i32, 0),
        86..=87 => click(sh, d.below(320) as i32, 140 + d.below(60) as i32, 1),
        // Anywhere (dialogue buttons, menus, the more button).
        88..=93 => click(sh, d.below(320) as i32, d.below(200) as i32, 0),
        94 => sh.push_event(key(Key::Char(b'm'))),
        95 => sh.push_event(key(Key::Escape)),
        96 => sh.push_event(key(Key::Space)),
        97 => sh.push_event(key(Key::Enter)),
        _ => sh.push_event(key(Key::Char(b'c'))),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let until: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(120_000);
    let out = args.get(3).map(PathBuf::from);
    if let Some(o) = &out {
        std::fs::create_dir_all(o).unwrap();
    }
    let copy = paths::find().expect("no installation found");
    let store = AssetStore::open(&copy, Language::German).expect("installation readable");
    let shared = Shared::new(22050);
    let mut system = System::with_virtual_time(shared.clone());
    let mut dice = Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    // The game takes some seconds to reach the play field.
    let mut next = 14_000;
    let mut next_pic = 0;
    let pics = out.clone();
    system.set_hook(Box::new(move |ms, sh| {
        if ms >= until {
            sh.push_event(Event::Quit);
            return;
        }
        if ms >= next {
            if std::env::var_os("FUZZ_LOG").is_some() {
                eprintln!("t={ms}");
            }
            random_input(&mut dice, sh);
            next = ms + 150 + dice.below(900);
        }
        if let Some(o) = &pics
            && ms >= next_pic
        {
            next_pic = ms + 5000;
            let frame = sh.frame.lock().unwrap().clone();
            let rgba = frame.rgba(Some(sh.mouse()));
            if let Some(img) = image::RgbaImage::from_raw(320, 200, rgba) {
                let _ = img.save(o.join(format!("{ms:08}.png")));
            }
        }
    }));
    let mut lol = Lol::new(Screen::new(system), Res::new(store));
    lol.init();
    lol.quick_start = Some(
        std::env::var("LOL_QUICK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
    );
    lol.save_dir = Some(std::env::temp_dir().join(format!("lol-rs-fuzz-{seed}")));
    lol.go();
    eprintln!(
        "ended at {} on level {} block {}",
        lol.millis(),
        lol.current_level,
        lol.current_block
    );
}
