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

//! A level's map and walls (as ScummVM's `loadBlockProperties`, `loadLevelWallData` and
//! `loadLevelGraphics`):
//!
//! - `LEVELn.CMZ`: the map, 32×32 blocks, packed behind a picture's header. Unpacked: six bytes
//!   (bytes 4–5: the size of a block's entry), then per block its entry, whose first four bytes
//!   are the wall numbers on its four sides (north, east, south, west).
//! - `LEVELn.WLL`: what the wall numbers mean: first the index of the level's decorations in the
//!   program's lists ([`crate::tables::LEVEL_SHP_LIST`], `…DAT_LIST`), then 12 bytes per wall
//!   number: the number (u16), its kind in the wall set (`VMP`, 0 none), its decoration (i16:
//!   above 0 one of the level's decorations, else a byte), its special type, flags (bit 3: no
//!   wall that hides what lies behind) and its sign on the map, each in two bytes.
//! - `NAME.VMP`: how each kind of wall is put together from the wall set's blocks (`VCN`):
//!   packed behind a picture's header, the number of entries (u16), then the entries (u16):
//!   330 for ceiling and floor (22×15), then 431 per kind ([`crate::view`]). An entry is a
//!   block's number; bit 14 mirrors it, bit 15 lays it over the ceiling and floor.

use crate::cps::unpack;
use crate::error::{Error, Result};

/// Blocks a side.
pub const SIZE: usize = 32;
pub const BLOCKS: usize = SIZE * SIZE;

/// The level's map: per block its four walls (north, east, south, west).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Maze {
    pub walls: Vec<[u8; 4]>,
}

impl Maze {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let (data, _) = unpack(file, bytes)?;
        let len = data
            .get(4..6)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .filter(|&l| l >= 4)
            .ok_or_else(|| Error::format(file, "Kopf der Karte fehlt"))?;
        let walls = (0..BLOCKS)
            .map(|i| {
                let at = 6 + i * len;
                data.get(at..at + 4)
                    .map(|w| [w[0], w[1], w[2], w[3]])
                    .ok_or_else(|| Error::format(file, format!("Karte endet bei Block {i}")))
            })
            .collect::<Result<_>>()?;
        Ok(Self { walls })
    }
}

/// The block one step from `block` in `direction` (0 north, 1 east, 2 south, 3 west), the map
/// wrapping round (as `calcNewBlockPosition`).
pub fn step(block: usize, direction: usize) -> usize {
    const OFFSET: [isize; 4] = [-(SIZE as isize), 1, SIZE as isize, -1];
    (block as isize + OFFSET[direction & 3]).rem_euclid(BLOCKS as isize) as usize
}

/// What the wall numbers of a level mean.
#[derive(Debug, Clone)]
pub struct WallData {
    /// Index into the program's lists of decoration files.
    pub decorations_file: u16,
    pub vmp_map: [u8; 256],
    /// The decoration as the file gives it (above 0: one of the level's decorations).
    pub shape: [i16; 256],
    pub special: [u8; 256],
    pub flags: [u8; 256],
    pub automap: [u8; 256],
}

impl WallData {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            return Err(Error::format(file, "zu kurz"));
        }
        let word = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let mut data = Self {
            decorations_file: word(0),
            vmp_map: [0; 256],
            shape: [0; 256],
            special: [0; 256],
            flags: [0; 256],
            automap: [0; 256],
        };
        for entry in bytes[2..].as_chunks::<12>().0 {
            let wall = u16::from_le_bytes([entry[0], entry[1]]) as usize & 0xff;
            data.vmp_map[wall] = entry[2];
            data.shape[wall] = i16::from_le_bytes([entry[4], entry[5]]);
            data.special[wall] = entry[6];
            data.flags[wall] = entry[8];
            data.automap[wall] = entry[10];
        }
        Ok(data)
    }

    /// Whether a wall lets one see what lies behind it (none, or flag 8: an open archway, a
    /// door in its frame …).
    pub fn see_through(&self, wall: u8) -> bool {
        self.flags[wall as usize] & 8 != 0
    }

    /// A wall that hides what lies behind it (`hasWall`).
    pub fn has_wall(&self, wall: u8) -> bool {
        wall != 0 && !self.see_through(wall)
    }
}

/// How the wall set's kinds of wall are put together.
#[derive(Debug, Clone)]
pub struct Vmp {
    pub entries: Vec<u16>,
}

impl Vmp {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let (data, _) = unpack(file, bytes)?;
        let count = data
            .get(0..2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
            .ok_or_else(|| Error::format(file, "zu kurz"))?;
        let entries = data[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .take(count)
            .map(|w| u16::from_le_bytes(*w))
            .collect();
        Ok(Self { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes behind a picture's header, unpacked.
    fn headed(data: &[u8]) -> Vec<u8> {
        let mut out = vec![0, 0, 0, 0];
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(data);
        let size = (out.len() - 2) as u16;
        out[0..2].copy_from_slice(&size.to_le_bytes());
        out
    }

    #[test]
    fn reads_the_map() {
        let mut data = vec![0, 0, 0, 0, 6, 0];
        for i in 0..BLOCKS {
            data.extend_from_slice(&[i as u8, 1, 2, 3, 0xaa, 0xbb]);
        }
        let maze = Maze::parse("LEVEL1.CMZ", &headed(&data)).unwrap();
        assert_eq!(maze.walls[5], [5, 1, 2, 3]);
        assert_eq!(maze.walls.len(), BLOCKS);
    }

    #[test]
    fn steps_wrap_round() {
        assert_eq!(step(33, 0), 1);
        assert_eq!(step(33, 1), 34);
        assert_eq!(step(0, 0), BLOCKS - SIZE);
        assert_eq!(step(0, 3), BLOCKS - 1);
    }

    #[test]
    fn reads_the_walls() {
        let mut bytes = vec![2, 0];
        bytes.extend_from_slice(&[7, 0, 3, 0, 0xfe, 0xff, 1, 0, 8, 0, 17, 0]);
        let walls = WallData::parse("LEVEL1.WLL", &bytes).unwrap();
        assert_eq!(walls.decorations_file, 2);
        assert_eq!(walls.vmp_map[7], 3);
        assert_eq!(walls.shape[7], -2);
        assert!(walls.see_through(7));
        assert!(!walls.has_wall(7));
        assert_eq!(walls.automap[7], 17);
    }

    #[test]
    fn reads_the_wall_kinds() {
        let mut data = 3u16.to_le_bytes().to_vec();
        for e in [1u16, 0x4002, 0x8003] {
            data.extend_from_slice(&e.to_le_bytes());
        }
        let vmp = Vmp::parse("KEEP.VMP", &headed(&data)).unwrap();
        assert_eq!(vmp.entries, vec![1, 0x4002, 0x8003]);
    }
}
