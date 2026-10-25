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

//! Animations (`WSA`): the intro's and finale's scenes, the faces that speak in
//! conversations, spells, the map … (as ScummVM's `WSAMovie_v2`).
//!
//! Header: the number of frames (u16), where it is drawn (x, y: i16), width and height (u16),
//! the size of the delta buffer (u16), flags (u16; bit 0: a palette follows the offsets). Then
//! the offsets of the frames' data (u32, one per frame and two more: the step from the last
//! frame back to the first, `0` if there is none); the first `0` means the animation has no
//! first frame of its own (it is drawn over a picture already on the screen). Then the
//! palette (768 bytes) and the frames: each LCW-packed into the delta buffer, then laid over
//! the one before as an XOR delta ([`crate::compression::xor_delta`]).

use crate::compression::{lcw, xor_delta};
use crate::error::{Error, Reader, Result};
use crate::palette::Palette;

#[derive(Debug, Clone)]
pub struct Wsa {
    pub x: i16,
    pub y: i16,
    pub width: usize,
    pub height: usize,
    /// Every frame, whole (`width × height` colour indices).
    pub frames: Vec<Vec<u8>>,
    pub palette: Option<Palette>,
    /// Without a first frame of its own: colour 0 shows what lies below.
    pub over_picture: bool,
}

impl Wsa {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(file, bytes);
        let frames = (r.u16()? & 0x7fff) as usize;
        let x = r.u16()? as i16;
        let y = r.u16()? as i16;
        let width = r.u16()? as usize;
        let height = r.u16()? as usize;
        let delta_size = r.u16()? as usize;
        let flags = r.u16()?;
        if frames == 0 || width == 0 || height == 0 {
            return Err(r.error(format!("{frames} Bilder zu {width}×{height}")));
        }
        let offsets = (0..frames + 2)
            .map(|_| r.u32().map(|o| o as usize))
            .collect::<Result<Vec<_>>>()?;
        let over_picture = offsets[0] == 0;
        let base = if over_picture { offsets[1] } else { offsets[0] };
        let palette = if flags & 1 != 0 {
            Some(Palette::parse(file, r.take(768)?)?)
        } else {
            None
        };
        let data = &bytes[r.pos()..];
        let frame_data = |i: usize| -> Result<&[u8]> {
            let at = offsets[i]
                .checked_sub(base)
                .ok_or_else(|| Error::format(file, format!("Bild {i} vor den Daten")))?;
            data.get(at..)
                .ok_or_else(|| Error::format(file, format!("Bild {i} hinter dem Dateiende")))
        };
        let mut screen = vec![0u8; width * height];
        let mut out = Vec::with_capacity(frames);
        for i in 0..frames {
            if i > 0 || !over_picture {
                let delta = lcw(file, frame_data(i)?, delta_size)?;
                xor_delta(&format!("{file}, Bild {i}"), &delta, &mut screen)?;
            }
            out.push(screen.clone());
        }
        Ok(Self {
            x,
            y,
            width,
            height,
            frames: out,
            palette,
            over_picture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two frames of 2×1: `[3, 4]`, then the second pixel XOR 1: `[3, 5]`.
    fn file() -> Vec<u8> {
        // Each frame's delta, LCW-packed as literal bytes: "02 a b" "80 00 00".
        let first = [0x86, 0x02, 3, 4, 0x80, 0, 0, 0x80];
        let second = [0x86, 0x81, 0x01, 0x01, 0x80, 0, 0, 0x80];
        let header = 14 + 4 * 4;
        let mut out = Vec::new();
        for v in [2u16, 10, 20, 2, 1, 16, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let offsets = [header, header + first.len(), 0, 0];
        for o in offsets {
            out.extend_from_slice(&(o as u32).to_le_bytes());
        }
        out.extend_from_slice(&first);
        out.extend_from_slice(&second);
        out
    }

    #[test]
    fn plays_the_frames() {
        let wsa = Wsa::parse("TEST.WSA", &file()).unwrap();
        assert_eq!((wsa.x, wsa.y, wsa.width, wsa.height), (10, 20, 2, 1));
        assert_eq!(wsa.frames, vec![vec![3, 4], vec![3, 5]]);
        assert!(!wsa.over_picture);
    }
}
