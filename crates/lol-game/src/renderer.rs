// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How the game's screen becomes the picture in the window. The game always draws the
//! original 320×200 screen of colour indices; a renderer turns it into pixels. Today there is
//! the original look; HD graphics (redrawn art in the same places) will come as a second
//! renderer. F2 belongs to the game (the second character's attack), so switching will get a
//! key the game does not use.

use lol_engine::platform::{Frame, HEIGHT, WIDTH};

/// One way of showing the screen.
pub trait Renderer: Send + Sync {
    #[allow(dead_code)] // for the switch to come
    fn name(&self) -> &'static str;
    /// The picture's size in pixels.
    fn size(&self) -> (u32, u32);
    /// The picture (RGBA) of a frame with the pointer at `mouse`.
    fn render(&mut self, frame: &Frame, mouse: (i32, i32)) -> Vec<u8>;
}

/// The original's look: the screen as it is, the pointer drawn by the game's figure.
pub struct Original;

impl Renderer for Original {
    fn name(&self) -> &'static str {
        "Original"
    }

    fn size(&self) -> (u32, u32) {
        (WIDTH as u32, HEIGHT as u32)
    }

    fn render(&mut self, frame: &Frame, mouse: (i32, i32)) -> Vec<u8> {
        frame.rgba(Some(mouse))
    }
}

/// The renderers there are and the one in use.
pub struct Renderers {
    pub list: Vec<Box<dyn Renderer>>,
    pub current: usize,
}

impl Renderers {
    pub fn new() -> Self {
        Self {
            list: vec![Box::new(Original)],
            current: 0,
        }
    }

    /// The next renderer; false when there is only the original.
    #[allow(dead_code)] // for the switch to come
    pub fn switch(&mut self) -> bool {
        if self.list.len() < 2 {
            return false;
        }
        self.current = (self.current + 1) % self.list.len();
        true
    }

    pub fn active(&mut self) -> &mut dyn Renderer {
        self.list[self.current].as_mut()
    }
}
