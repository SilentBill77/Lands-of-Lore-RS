// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The whole game in virtual time against the player's own copy (passes quietly without one):
//! a new game, the guard's greeting, a step, saving and loading.

use std::sync::{Arc, Mutex};

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_engine::lol::Lol;
use lol_engine::lol::res::Res;
use lol_engine::platform::{Event, Key, Shared, System};
use lol_engine::screen::Screen;

/// Input at virtual times, and what the screen showed then.
type Script = Vec<(u32, Option<Event>)>;

fn run(
    script: Script,
    quit_at: u32,
    save_dir: std::path::PathBuf,
) -> Option<(Lol, Arc<Mutex<Vec<u64>>>)> {
    let copy = paths::find()?;
    let store = AssetStore::open(&copy, Language::German).expect("installation readable");
    let shared = Shared::new(22050);
    let mut system = System::with_virtual_time(shared.clone());
    let mut script = script;
    script.reverse();
    let frames = Arc::new(Mutex::new(Vec::new()));
    let seen = frames.clone();
    system.set_hook(Box::new(move |ms, sh| {
        while script.last().is_some_and(|(t, _)| *t <= ms) {
            if let (_, Some(e)) = script.pop().unwrap() {
                sh.push_event(e);
            }
        }
        if ms >= quit_at {
            sh.push_event(Event::Quit);
        }
        if ms % 1000 < 20 {
            seen.lock().unwrap().push(sh.frame.lock().unwrap().serial);
        }
    }));
    let mut lol = Lol::new(Screen::new(system), Res::new(store));
    lol.init();
    lol.quick_start = Some(2);
    lol.save_dir = Some(save_dir);
    lol.go();
    Some((lol, frames))
}

fn key(k: Key) -> Option<Event> {
    Some(Event::KeyDown {
        key: k,
        ascii: 0,
        ctrl: false,
        alt: false,
        shift: false,
    })
}

#[test]
fn a_new_game_starts_at_gladstone_keep_and_walks() {
    let dir = std::env::temp_dir().join("lol-rs-test-walk");
    let Some((lol, frames)) = run(vec![(15_000, key(Key::Up))], 20_000, dir) else {
        eprintln!("Keine Installation gefunden, Test übersprungen.");
        return;
    };
    assert_eq!(lol.current_level, 1);
    // Block 557 facing north, one step ahead after the guard's greeting.
    assert_eq!(lol.current_block, 557 - 32);
    assert_eq!(lol.characters[0].name, b"Kieran");
    // The screen kept changing (the game ran).
    let f = frames.lock().unwrap();
    assert!(f.windows(2).filter(|w| w[0] != w[1]).count() > 5);
}

#[test]
fn saving_and_loading_keeps_the_place() {
    let dir = std::env::temp_dir().join("lol-rs-test-save");
    let _ = std::fs::remove_dir_all(&dir);
    let Some((mut lol, _)) = run(vec![], 14_000, dir.clone()) else {
        return;
    };
    assert!(lol.save_game_state(0, b"test"));
    let block = lol.current_block;
    lol.current_direction = (lol.current_direction + 1) & 3;
    let dir_after = lol.current_direction;
    assert_eq!(lol.savegame_list().len(), 1);
    assert!(lol.load_game_state(0));
    assert_eq!(lol.current_block, block);
    assert_ne!(lol.current_direction, dir_after);
    let _ = std::fs::remove_dir_all(&dir);
}
