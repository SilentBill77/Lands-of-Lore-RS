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

//! A region's decorations (`KEEP.DAT` with the figures of `KEEP.SHP`): what hangs on a wall,
//! stands in an archway or frames a door, as ScummVM's `loadLevelShpDat`.
//!
//! The number of decorations (u16), then 72 bytes each: for each of the ten places a
//! decoration can be seen from (near, far, to the side …) its figure in the `SHP` (u16, `FFFF`
//! none), whether it is placed through the program's table (u8, bit 0), and its x and y (i16
//! each); then the decoration drawn with it (u8, 0 none) and flags (u8: 1 drawn mirrored beside
//! itself, 2 and 4 mirrored, 8 only seen straight ahead).

use crate::error::{Error, Result};

pub const PLACES: usize = 10;
const ENTRY: usize = 72;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Decoration {
    pub shape_index: [u16; PLACES],
    pub scale_flag: [u8; PLACES],
    pub shape_x: [i16; PLACES],
    pub shape_y: [i16; PLACES],
    pub next: u8,
    pub flags: u8,
}

pub fn parse(file: &str, bytes: &[u8]) -> Result<Vec<Decoration>> {
    let count = bytes
        .get(0..2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]) as usize)
        .ok_or_else(|| Error::format(file, "zu kurz"))?;
    (0..count)
        .map(|i| {
            let e = bytes
                .get(2 + i * ENTRY..2 + (i + 1) * ENTRY)
                .ok_or_else(|| Error::format(file, format!("Verzierung {i} fehlt")))?;
            let word = |at: usize| u16::from_le_bytes([e[at], e[at + 1]]);
            Ok(Decoration {
                shape_index: std::array::from_fn(|k| word(2 * k)),
                scale_flag: std::array::from_fn(|k| e[20 + k]),
                shape_x: std::array::from_fn(|k| word(30 + 2 * k) as i16),
                shape_y: std::array::from_fn(|k| word(50 + 2 * k) as i16),
                next: e[70],
                flags: e[71],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_decoration() {
        let mut bytes = vec![1, 0];
        let mut entry = [0u8; ENTRY];
        entry[0..2].copy_from_slice(&7u16.to_le_bytes());
        entry[2..4].copy_from_slice(&0xffffu16.to_le_bytes());
        entry[20] = 1;
        entry[30..32].copy_from_slice(&(-3i16).to_le_bytes());
        entry[50..52].copy_from_slice(&12i16.to_le_bytes());
        entry[70] = 2;
        entry[71] = 8;
        bytes.extend_from_slice(&entry);
        let decorations = parse("KEEP.DAT", &bytes).unwrap();
        let d = decorations[0];
        assert_eq!((d.shape_index[0], d.shape_index[1]), (7, 0xffff));
        assert_eq!((d.scale_flag[0], d.shape_x[0], d.shape_y[0]), (1, -3, 12));
        assert_eq!((d.next, d.flags), (2, 8));
    }
}
