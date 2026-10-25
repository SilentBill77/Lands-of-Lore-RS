// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Westwood's packed pictures (`CPS`): 320×200 pixels of colour indices, some with a palette
//! of their own, most shown with one from a `COL` file.
//!
//! Header: the file's size less two (u16), the compression (u16: 0 none, 3 RLE, 4 LCW), the
//! unpacked size (u32, 64 000), the size of the palette that follows (u16, 0 or 768).
//!
//! A picture without a header (a palette and the 64 000 pixels unpacked, as Eye of the
//! Beholder has some) is read as well.

use crate::compression::{lcw, rle};
use crate::error::{Error, Result};
use crate::palette::Palette;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;
/// A picture without a header: palette and pixels.
const RAW_SIZE: usize = 768 + WIDTH * HEIGHT;

#[derive(Debug, Clone)]
pub struct Cps {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub palette: Option<Palette>,
}

/// Unpacks a file with the CPS header (also used for `VCN`): its bytes and its own palette.
pub fn unpack(file: &str, bytes: &[u8]) -> Result<(Vec<u8>, Option<Palette>)> {
    if bytes.len() < 10 {
        return Err(Error::format(file, "zu kurz für den Kopf"));
    }
    let word = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    let compression = word(2);
    let size = word(4) | word(6) << 16;
    let palette_size = word(8);
    let start = 10 + palette_size;
    let body = bytes
        .get(start..)
        .ok_or_else(|| Error::format(file, "Palette reicht über das Dateiende"))?;
    let palette = (palette_size >= 768).then(|| Palette::from_vga(&bytes[10..10 + 768]));
    let data = match compression {
        0 => {
            let mut raw = body.to_vec();
            raw.resize(size, 0);
            raw
        }
        3 => rle(file, body, size)?,
        4 => lcw(file, body, size)?,
        other => {
            return Err(Error::format(
                file,
                format!("Kompression {other} nicht unterstützt"),
            ));
        }
    };
    Ok((data, palette))
}

impl Cps {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let headed = bytes.len() >= 2
            && u16::from_le_bytes([bytes[0], bytes[1]]) as usize + 2 == bytes.len();
        let (mut pixels, palette) = if !headed && bytes.len() == RAW_SIZE {
            (
                bytes[768..].to_vec(),
                Some(Palette::from_vga(&bytes[..768])),
            )
        } else {
            unpack(file, bytes)?
        };
        let height = pixels.len().div_ceil(WIDTH).max(1);
        pixels.resize(WIDTH * height, 0);
        Ok(Self {
            width: WIDTH,
            height,
            pixels,
            palette,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_packed_picture() {
        // 64 000 bytes of colour 7: one fill command.
        let mut bytes = vec![0, 0, 4, 0, 0x00, 0xfa, 0, 0, 0, 0];
        bytes.extend_from_slice(&[0xfe, 0x00, 0xfa, 7, 0x80]);
        let cps = Cps::parse("TEST.CPS", &bytes).unwrap();
        assert_eq!((cps.width, cps.height), (320, 200));
        assert!(cps.pixels.iter().all(|&p| p == 7));
        assert!(cps.palette.is_none());
    }

    #[test]
    fn reads_a_picture_without_header() {
        let mut bytes = vec![63u8; 768];
        bytes.extend(std::iter::repeat_n(5u8, WIDTH * HEIGHT));
        let cps = Cps::parse("RAW.CPS", &bytes).unwrap();
        assert_eq!(cps.pixels[0], 5);
        assert_eq!(cps.palette.unwrap().colors[5], [255, 255, 255]);
    }
}
