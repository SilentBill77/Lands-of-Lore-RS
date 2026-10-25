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

//! Figures (`SHP`): monsters, items, the party's faces, buttons and mouse pointers, several
//! to a file (as ScummVM's `Screen_v2::getPtrToShape` and `Screen::drawShape`).
//!
//! The file begins with the number of figures (u16) and the offset of each (u32, counted from
//! byte 2). A figure: flags (u16; bit 0: a table of colours follows, bit 1: not packed, bit 2:
//! the table's length is given), height (u8), width (u16), the height again (u8), the
//! figure's size in the file (u16), the size of its lines unpacked (u16); then the table's
//! length (u8, else 16) and the table; then the lines, packed with LCW unless bit 1 is set.
//! Each line: a byte per pixel, where `00 n` leaves `n` pixels transparent. With a table, the
//! pixels are its places (a figure of 16 colours, drawn in the colours the table gives).

use crate::compression::lcw;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    pub width: usize,
    pub height: usize,
    /// The pixels as stored, 0 transparent: colour indices, or places in `table`.
    pub pixels: Vec<u8>,
    /// The figure's own table of colours (the original draws through it unless given another).
    pub table: Option<Vec<u8>>,
}

impl Shape {
    /// The pixels as colour indices (through the figure's own table), 0 transparent.
    pub fn colored(&self) -> Vec<u8> {
        match &self.table {
            Some(table) => self
                .pixels
                .iter()
                .map(|&p| match p {
                    0 => 0,
                    p => table.get(p as usize).copied().unwrap_or(p),
                })
                .collect(),
            None => self.pixels.clone(),
        }
    }
}

/// The figures of a file (`None` where a place is empty). Most files are packed as a whole
/// behind the header of a picture (`CPS`; the game loads them as one).
pub fn parse_file(file: &str, bytes: &[u8]) -> Result<Vec<Option<Shape>>> {
    let packed = bytes.len() >= 10
        && u16::from_le_bytes([bytes[0], bytes[1]]) as usize + 2 == bytes.len()
        && matches!(u16::from_le_bytes([bytes[2], bytes[3]]), 0 | 3 | 4);
    if packed {
        let (unpacked, _) = crate::cps::unpack(file, bytes)?;
        return parse_table(file, &unpacked);
    }
    parse_table(file, bytes)
}

fn parse_table(file: &str, bytes: &[u8]) -> Result<Vec<Option<Shape>>> {
    let bad = |message: String| Error::format(file, message);
    if bytes.len() < 2 {
        return Err(bad("zu kurz".into()));
    }
    let count = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
    let table_end = 2 + 4 * count;
    if count == 0 || table_end > bytes.len() {
        return Err(bad(format!("{count} Figuren passen nicht in die Datei")));
    }
    (0..count)
        .map(|i| {
            let at = 2 + 4 * i;
            let offset = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            if offset == 0 {
                return Ok(None);
            }
            let data = bytes
                .get(offset + 2..)
                .filter(|d| !d.is_empty())
                .ok_or_else(|| bad(format!("Figur {i} liegt außerhalb")))?;
            parse(&format!("{file}#{i}"), data).map(Some)
        })
        .collect()
}

/// One figure from its header on.
pub fn parse(file: &str, bytes: &[u8]) -> Result<Shape> {
    let bad = |message: &str| Error::format(file, message);
    if bytes.len() < 10 {
        return Err(bad("Kopf zu kurz"));
    }
    let word = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    let flags = word(0);
    let height = bytes[2] as usize;
    let width = word(3);
    let unpacked = word(8);
    let mut pos = 10;
    let colors = if flags & 4 != 0 {
        let colors = *bytes.get(pos).ok_or_else(|| bad("Kopf zu kurz"))? as usize;
        pos += 1;
        colors
    } else {
        16
    };
    let table = if flags & 1 != 0 {
        let table = bytes
            .get(pos..pos + colors)
            .ok_or_else(|| bad("Farbtabelle zu kurz"))?;
        pos += colors;
        Some(table)
    } else {
        None
    };
    let body = &bytes[pos..];
    let lines = if flags & 2 == 0 {
        lcw(file, body, unpacked)?
    } else {
        body.to_vec()
    };
    let mut pixels = vec![0u8; width * height];
    let mut src = 0;
    let byte = |i: usize| {
        lines
            .get(i)
            .copied()
            .ok_or_else(|| bad("Zeilen enden vorzeitig"))
    };
    for row in pixels.chunks_mut(width.max(1)).take(height) {
        let mut x = 0;
        while x < width {
            let c = byte(src)?;
            src += 1;
            if c == 0 {
                x += byte(src)? as usize;
                src += 1;
            } else {
                if let Some(pixel) = row.get_mut(x) {
                    *pixel = c;
                }
                x += 1;
            }
        }
    }
    Ok(Shape {
        width,
        height,
        pixels,
        table: table.map(<[u8]>::to_vec),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file of one figure, unpacked, 3×2: `5 _ 6` / `_ _ 7`.
    fn file(flags: u16, table: &[u8]) -> Vec<u8> {
        let lines = [5, 0, 1, 6, 0, 2, 7];
        let mut shape = Vec::new();
        shape.extend_from_slice(&(flags | 2).to_le_bytes());
        shape.push(2);
        shape.extend_from_slice(&3u16.to_le_bytes());
        shape.push(2);
        shape.extend_from_slice(&0u16.to_le_bytes());
        shape.extend_from_slice(&(lines.len() as u16).to_le_bytes());
        shape.extend_from_slice(table);
        shape.extend_from_slice(&lines);
        let mut out = vec![1, 0];
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&shape);
        out
    }

    #[test]
    fn reads_a_figure() {
        let shapes = parse_file("TEST.SHP", &file(0, &[])).unwrap();
        let shape = shapes[0].as_ref().unwrap();
        assert_eq!((shape.width, shape.height), (3, 2));
        assert_eq!(shape.pixels, vec![5, 0, 6, 0, 0, 7]);
    }

    #[test]
    fn colours_through_the_table() {
        let mut table = [0u8; 16];
        table[5] = 50;
        table[6] = 60;
        table[7] = 70;
        let shapes = parse_file("TEST.SHP", &file(1, &table)).unwrap();
        let shape = shapes[0].as_ref().unwrap();
        assert_eq!(shape.pixels, vec![5, 0, 6, 0, 0, 7]);
        assert_eq!(shape.colored(), vec![50, 0, 60, 0, 0, 70]);
    }
}
