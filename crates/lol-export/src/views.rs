// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Views of each level as the original draws them, the whole screen, into `ansichten/`: level 1
//! from where a new game begins, every level from a few open blocks (`LEVEL01_0272_O.png`:
//! level, block, direction). For checking the view against the original.

use std::path::Path;

use lol_data::Result;
use lol_data::assets::AssetStore;
use lol_data::level::{BLOCKS, step};
use lol_data::paths::{Installation, Language};
use lol_engine::game::{Game, HEIGHT, START, WIDTH};

use crate::export::save;

/// Views per level besides the start.
const PER_LEVEL: usize = 4;
const DIRECTIONS: [&str; 4] = ["N", "O", "S", "W"];

pub fn all(copy: &Installation, target: &Path) -> Result<()> {
    let store = AssetStore::open(copy, Language::German)?;
    let folder = target.join("ansichten");
    let (level, block, direction) = START;
    let mut game = Game::new(store, level, block, direction)?;
    let mut written = 0;
    for index in 1..=29 {
        if let Err(err) = game.change_level(index) {
            eprintln!("  Ebene {index}: {err}");
            continue;
        }
        if !game.level.unhandled.is_empty() && index == 1 {
            let names: Vec<&str> = game.level.unhandled.iter().copied().collect();
            println!("  Noch nicht umgesetzt (Init-Skript): {}", names.join(", "));
        }
        let mut places = Vec::new();
        if index == level {
            places.push((block, direction));
        }
        places.extend(open_places(&game));
        for (block, direction) in places {
            game.block = block;
            game.direction = direction;
            let screen = game.render();
            let name = format!("LEVEL{index:02}_{block:04}_{}.png", DIRECTIONS[direction]);
            save(&folder.join(name), WIDTH, HEIGHT, &screen.rgba())?;
            written += 1;
        }
    }
    println!("Ansichten: {written} nach {}", folder.display());
    Ok(())
}

/// A few blocks without walls, spread over the map, each looking where a wall stands close.
fn open_places(game: &Game) -> Vec<(usize, usize)> {
    let maze = &game.level.maze;
    let open: Vec<usize> = (0..BLOCKS)
        .filter(|&b| maze.walls[b].iter().all(|&w| w == 0))
        .collect();
    if open.is_empty() {
        return Vec::new();
    }
    (0..PER_LEVEL)
        .map(|i| open[i * open.len() / PER_LEVEL])
        .map(|block| {
            let direction = (0..4)
                .find(|&d| {
                    let ahead = step(step(block, d), d);
                    maze.walls[ahead].iter().any(|&w| w != 0)
                })
                .unwrap_or(0);
            (block, direction)
        })
        .collect()
}
