// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lists the files of an archive of the game: `paklist <archive> [filter]`.

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let copy = paths::find().expect("installation");
    let store = AssetStore::open(&copy, Language::German).expect("store");
    let filter = args
        .get(2)
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let archive = match store.archive(&args[1]) {
        Some(a) => a.clone(),
        None => store.open_archive(&args[1]).expect("archive"),
    };
    for (name, data) in archive.files() {
        if name.contains(&filter) {
            println!("{name} {}", data.len());
        }
    }
}
