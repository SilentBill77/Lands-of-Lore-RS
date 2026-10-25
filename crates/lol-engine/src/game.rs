// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The game's state and the screen as the original draws it: the play field (`PLAYFLD.CPS`)
//! with the view of the level in its window (176×120 at x 112).

use lol_data::Result;
use lol_data::assets::AssetStore;
use lol_data::cps::Cps;
use lol_data::level::step;
use lol_data::palette::Palette;
use lol_data::view::{self, Scene};

use crate::level::Level;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;

/// Where a new game begins: level 1, the block where the king's audience (`KING1.TIM`) leaves
/// the party, looking east.
pub const START: (usize, usize, usize) = (1, 272, 1);

/// A picture of the whole screen: colour indices and the palette to show them in.
#[derive(Debug, Clone)]
pub struct Screen {
    pub pixels: Vec<u8>,
    pub palette: Palette,
}

impl Screen {
    pub fn rgba(&self) -> Vec<u8> {
        self.palette.rgba(&self.pixels, false)
    }
}

pub struct Game {
    pub store: AssetStore,
    pub level: Level,
    /// The party's block and the direction it looks in (0 north, 1 east, 2 south, 3 west).
    pub block: usize,
    pub direction: usize,
    playfield: Cps,
    base: Palette,
}

impl Game {
    pub fn new(store: AssetStore, level: usize, block: usize, direction: usize) -> Result<Self> {
        let playfield = Cps::parse("PLAYFLD.CPS", &store.read("PLAYFLD.CPS")?)?;
        let base = playfield
            .palette
            .clone()
            .unwrap_or_else(|| Palette::from_vga(&[0; 768]));
        let level = Level::load(&store, level, &base)?;
        Ok(Self {
            store,
            level,
            block,
            direction: direction & 3,
            playfield,
            base,
        })
    }

    /// A new game, where the original's begins.
    pub fn new_game(store: AssetStore) -> Result<Self> {
        let (level, block, direction) = START;
        Self::new(store, level, block, direction)
    }

    /// Changes to another level, the party keeping its place (for trying levels out).
    pub fn change_level(&mut self, index: usize) -> Result<()> {
        self.level = Level::load(&self.store, index, &self.base)?;
        Ok(())
    }

    pub fn turn(&mut self, right: bool) {
        self.direction = (self.direction + if right { 1 } else { 3 }) & 3;
    }

    /// One step: 0 ahead, 1 right, 2 back, 3 left of where the party looks. False if a wall
    /// is in the way (as `checkBlockPassability`: the wall of the block ahead facing the party,
    /// flag 1).
    pub fn walk(&mut self, relative: usize) -> bool {
        let direction = (self.direction + relative) & 3;
        let next = step(self.block, direction);
        let facing = self.level.maze.walls[next][direction ^ 2];
        if self.level.walls.flags[facing as usize] & 1 != 0 {
            return false;
        }
        self.block = next;
        true
    }

    /// The view of the level from where the party stands (176×120).
    pub fn view(&self) -> Vec<u8> {
        let level = &self.level;
        Scene::compose(
            &level.maze,
            &level.walls,
            &level.vmp,
            self.block,
            self.direction,
        )
        .render(&level.wall_set)
    }

    /// The whole screen as the original shows it.
    pub fn render(&self) -> Screen {
        let mut pixels = self.playfield.pixels.clone();
        pixels.resize(WIDTH * HEIGHT, 0);
        let picture = self.view();
        for row in 0..view::HEIGHT {
            let at = (view::SCREEN_Y + row) * WIDTH + view::SCREEN_X;
            pixels[at..at + view::WIDTH]
                .copy_from_slice(&picture[row * view::WIDTH..(row + 1) * view::WIDTH]);
        }
        crate::scene::draw_scene_shapes(&mut pixels, &self.level, self.block, self.direction);
        Screen {
            pixels,
            palette: self.level.palette.clone(),
        }
    }
}
