// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs the game without a window in virtual time, driven by a script of timed input, and
//! saves pictures of the screen: `headless <script> <out-dir>`.
//!
//! Script lines (`#` comments): `<ms> key <name>`, `<ms> char <c>`, `<ms> click <x> <y>`,
//! `<ms> rclick <x> <y>`, `<ms> move <x> <y>`, `<ms> snap <name>`, `<ms> every <ms>` (a
//! picture at that interval from then on), `<ms> quit`. Times are virtual milliseconds.

use std::path::{Path, PathBuf};

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_engine::lol::Lol;
use lol_engine::lol::res::Res;
use lol_engine::platform::{Event, Key, Shared, System};
use lol_engine::screen::Screen;

#[derive(Debug, Clone)]
enum Action {
    Key(Key),
    Char(u8),
    Click(i32, i32, u8),
    Move(i32, i32),
    Snap(String),
    Every(u32),
    Quit,
}

fn key(name: &str) -> Key {
    match name {
        "esc" | "escape" => Key::Escape,
        "enter" | "return" => Key::Enter,
        "space" => Key::Space,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "home" => Key::Home,
        "end" => Key::End,
        "pgup" => Key::PageUp,
        "pgdn" => Key::PageDown,
        "back" => Key::Backspace,
        s if s.len() == 1 => Key::Char(s.as_bytes()[0]),
        s if s.starts_with('f') => Key::F(s[1..].parse().unwrap_or(1)),
        _ => Key::Other(0),
    }
}

fn parse(script: &str) -> Vec<(u32, Action)> {
    let mut out = Vec::new();
    for line in script.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let w: Vec<&str> = line.split_whitespace().collect();
        let t: u32 = w[0].parse().expect("time");
        let n = |i: usize| w.get(i).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
        let a = match w.get(1).copied().unwrap_or("") {
            "key" => Action::Key(key(w.get(2).copied().unwrap_or(""))),
            "char" => Action::Char(w.get(2).map_or(b' ', |s| s.as_bytes()[0])),
            "click" => Action::Click(n(2), n(3), 0),
            "rclick" => Action::Click(n(2), n(3), 1),
            "move" => Action::Move(n(2), n(3)),
            "snap" => Action::Snap(w.get(2).copied().unwrap_or("snap").to_string()),
            "every" => Action::Every(n(2) as u32),
            "quit" => Action::Quit,
            other => panic!("unknown action {other}"),
        };
        out.push((t, a));
    }
    out.sort_by_key(|(t, _)| *t);
    out
}

fn save(shared: &Shared, dir: &Path, name: &str) {
    let frame = shared.frame.lock().unwrap().clone();
    let (mx, my) = shared.mouse();
    let rgba = frame.rgba(Some((mx, my)));
    let img = image::RgbaImage::from_raw(320, 200, rgba).unwrap();
    let big = image::imageops::resize(&img, 640, 400, image::imageops::FilterType::Nearest);
    let _ = big.save(dir.join(format!("{name}.png")));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let script = std::fs::read_to_string(&args[1]).expect("script");
    let dir = PathBuf::from(&args[2]);
    let save_dir = dir.join("saves");
    std::fs::create_dir_all(&dir).unwrap();
    let lang = args
        .get(3)
        .and_then(|s| Language::parse(s))
        .unwrap_or(Language::German);
    let copy = paths::find().expect("no installation found");
    let store = AssetStore::open(&copy, lang).expect("installation readable");
    let shared = Shared::new(22050);
    let mut system = System::with_virtual_time(shared.clone());
    let mut actions = parse(&script);
    actions.reverse();
    let mut every: Option<(u32, u32)> = None;
    let mut last_log = 0;
    system.set_hook(Box::new(move |ms, sh| {
        while actions.last().is_some_and(|(t, _)| *t <= ms) {
            let (_, a) = actions.pop().unwrap();
            match a {
                Action::Key(k) => sh.push_event(Event::KeyDown {
                    key: k,
                    ascii: 0,
                    ctrl: false,
                    alt: false,
                    shift: false,
                }),
                Action::Char(c) => sh.push_event(Event::KeyDown {
                    key: Key::Char(c),
                    ascii: c,
                    ctrl: false,
                    alt: false,
                    shift: false,
                }),
                Action::Click(x, y, b) => {
                    sh.push_event(Event::MouseMove { x, y });
                    sh.push_event(Event::MouseDown { button: b, x, y });
                    sh.push_event(Event::MouseUp { button: b, x, y });
                }
                Action::Move(x, y) => sh.push_event(Event::MouseMove { x, y }),
                Action::Snap(n) => save(sh, &dir, &format!("{ms:08}-{n}")),
                Action::Every(i) => every = Some((i, ms)),
                Action::Quit => sh.push_event(Event::Quit),
            }
        }
        if let Some((i, next)) = every.as_mut()
            && ms >= *next
        {
            save(sh, &dir, &format!("{ms:08}"));
            *next = ms + *i;
        }
        if ms >= last_log + 10_000 {
            last_log = ms;
            eprintln!("t={ms}");
        }
    }));
    let screen = Screen::new(system);
    let res = Res::new(store);
    let mut lol = Lol::new(screen, res);
    lol.init();
    lol.quick_start = std::env::var("LOL_QUICK").ok().and_then(|v| v.parse().ok());
    lol.save_dir = Some(save_dir);
    if let Ok(slot) = std::env::var("LOL_LOAD") {
        lol.game_to_load = slot.parse().unwrap_or(-1);
    }
    lol.go();
    eprintln!(
        "ended at {} on level {} block {}",
        lol.millis(),
        lol.current_level,
        lol.current_block
    );
}
