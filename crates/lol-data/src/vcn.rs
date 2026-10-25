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

//! A region's wall set (`VCN`: `KEEP.VCN`, `CIMMERIA.VCN` …): the 8×8 blocks the view is put
//! together from, and the lower half of the palette while the party is there (as ScummVM's
//! `LoLEngine::loadLevelGraphics` and `KyraRpgEngine::drawVcnBlocks`).
//!
//! Packed behind the header of a picture (`CPS`). Unpacked: the number of blocks (u16), a
//! shift per block (u8), a table of 128 colours, the palette's colours 0–127 (384 bytes, VGA),
//! then the blocks: 32 bytes each, two pixels a byte (the left one in the high half). A
//! pixel's colour is `table[nibble | shift]`.

use crate::cps::unpack;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct WallSet {
    pub shift: Vec<u8>,
    pub color_table: [u8; 128],
    /// Colours 0–127 of the palette, 6-bit VGA values.
    pub palette_low: Vec<u8>,
    pub blocks: Vec<[u8; 32]>,
}

impl WallSet {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let (data, _) = unpack(file, bytes)?;
        let short = || Error::format(file, "Wandsatz zu kurz");
        let count = u16::from_le_bytes([
            *data.first().ok_or_else(short)?,
            *data.get(1).ok_or_else(short)?,
        ]) as usize;
        let mut pos = 2;
        let mut take = |n: usize| -> Result<&[u8]> {
            let slice = data.get(pos..pos + n).ok_or_else(short)?;
            pos += n;
            Ok(slice)
        };
        let shift = take(count)?.to_vec();
        let color_table = take(128)?.try_into().expect("128 bytes");
        let palette_low = take(384)?.to_vec();
        let blocks = take(count * 32)?.as_chunks::<32>().0.to_vec();
        Ok(Self {
            shift,
            color_table,
            palette_low,
            blocks,
        })
    }

    /// The 64 colour indices of a block, row by row.
    pub fn block_pixels(&self, index: usize) -> [u8; 64] {
        let mut out = [0u8; 64];
        let shift = self.shift.get(index).copied().unwrap_or(0);
        if let Some(block) = self.blocks.get(index) {
            for (i, byte) in block.iter().enumerate() {
                let color = |nibble: u8| self.color_table[((nibble | shift) & 0x7f) as usize];
                out[2 * i] = color(byte >> 4);
                out[2 * i + 1] = color(byte & 0x0f);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_blocks_through_shift_and_table() {
        let mut data = vec![1, 0, 0x10];
        let mut table = [0u8; 128];
        table[0x12] = 200;
        table[0x13] = 201;
        data.extend_from_slice(&table);
        data.extend_from_slice(&[7; 384]);
        let mut block = [0u8; 32];
        block[0] = 0x23;
        data.extend_from_slice(&block);
        // Unpacked behind a picture's header.
        let mut bytes = vec![0, 0, 0, 0];
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&data);
        let size = (bytes.len() - 2) as u16;
        bytes[0..2].copy_from_slice(&size.to_le_bytes());
        let set = WallSet::parse("TEST.VCN", &bytes).unwrap();
        assert_eq!(set.blocks.len(), 1);
        let pixels = set.block_pixels(0);
        assert_eq!(&pixels[..2], &[200, 201]);
        assert_eq!(set.palette_low.len(), 384);
    }
}
