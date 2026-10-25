// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Readers for the data files of Lands of Lore: The Throne of Chaos (Westwood, 1993), read
//! from the player's own copy (the CD version, as GOG sells it). See `docs/FORMATE.md`.

pub mod assets;
pub mod compression;
pub mod cps;
pub mod decor;
pub mod emc;
pub mod error;
pub mod iso;
pub mod level;
pub mod pak;
pub mod palette;
pub mod paths;
pub mod shp;
pub mod tables;
pub mod vcn;
pub mod view;
pub mod voc;
pub mod wsa;

pub use error::{Error, Result};
