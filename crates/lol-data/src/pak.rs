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

//! Westwood's PAK archives, in which Lands of Lore keeps nearly all its files: one per level
//! (`L01.PAK` …), per scene (`O01A.PAK` …), per region's wall set (`CIMMERIA.PAK` …), the
//! intro and finale, and the speech (`00.TLK` … `29.TLK`, archives of the same kind).
//!
//! The archive begins with its table: for each file the offset of its data (u32) and its name
//! (NUL-terminated). The first offset is where the table ends; a file runs to the next one's
//! offset. The table closes with the archive's size as a last offset and an empty name (Lands
//! of Lore) or with four bytes of no meaning just before the first file's data (Eye of the
//! Beholder); as ScummVM's `ResLoaderPak`. A file `LINKLIST` (`SCVM` …) names further names
//! for files of the archive.

use crate::error::{Error, Reader, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakEntry {
    pub name: String,
    pub offset: usize,
    pub size: usize,
}

/// The table of a PAK archive (the archive's bytes stay with the caller).
#[derive(Debug, Clone)]
pub struct Pak {
    pub entries: Vec<PakEntry>,
}

impl Pak {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(file, bytes);
        let len = bytes.len();
        let mut start = reader.u32()? as usize;
        let first = start;
        let mut entries = Vec::new();
        loop {
            let pos = reader.pos();
            if start < pos || start > len {
                return Err(reader.error(format!("ungültiger Eintrag bei Byte {pos}")));
            }
            let name = reader.cstr()?;
            if name.is_empty() {
                if entries.is_empty() {
                    return Err(reader.error("leeres Inhaltsverzeichnis"));
                }
                break;
            }
            let mut end = if reader.remaining() >= 4 {
                reader.u32()? as usize
            } else {
                0
            };
            if end == 0 || reader.pos() == first {
                end = len;
            }
            if end < start || end > len {
                return Err(reader.error(format!("{name}: Daten vor denen der vorigen Datei")));
            }
            if end != start {
                entries.push(PakEntry {
                    name,
                    offset: start,
                    size: end - start,
                });
            }
            if end == len {
                break;
            }
            start = end;
        }
        let mut pak = Self { entries };
        pak.add_links(file, bytes)?;
        Ok(pak)
    }

    /// The names a `LINKLIST` gives to files of the archive.
    fn add_links(&mut self, file: &str, bytes: &[u8]) -> Result<()> {
        let Some(list) = self.find("LINKLIST").cloned() else {
            return Ok(());
        };
        let data = self.data(&list, bytes)?;
        if data.len() < 8 || &data[..4] != b"SCVM" {
            return Err(Error::format(file, "LINKLIST ohne Kennung SCVM"));
        }
        let be32 = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let mut reader = Reader::new(file, data);
        reader.seek(4);
        let links = be32(reader.take(4)?);
        for _ in 0..links {
            let target = reader.cstr()?;
            let sources = be32(reader.take(4)?);
            let entry = self
                .find(&target)
                .cloned()
                .ok_or_else(|| Error::format(file, format!("LINKLIST: {target} fehlt")))?;
            for _ in 0..sources {
                let name = reader.cstr()?;
                self.entries.push(PakEntry {
                    name,
                    ..entry.clone()
                });
            }
        }
        Ok(())
    }

    /// The first entry of that name (case does not matter).
    pub fn find(&self, name: &str) -> Option<&PakEntry> {
        self.entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// The bytes of an entry within the archive's bytes.
    pub fn data<'a>(&self, entry: &PakEntry, bytes: &'a [u8]) -> Result<&'a [u8]> {
        bytes
            .get(entry.offset..entry.offset + entry.size)
            .ok_or_else(|| Error::format(&entry.name, "liegt außerhalb des Archivs"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An archive as Lands of Lore's are made: the size as the last offset, an empty name and
    /// padding.
    fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
        let table: usize = files.iter().map(|(n, _)| 4 + n.len() + 1).sum::<usize>() + 4 + 5;
        let total = table + files.iter().map(|(_, b)| b.len()).sum::<usize>();
        let mut out = Vec::new();
        let mut offset = table;
        for (name, body) in files {
            out.extend_from_slice(&(offset as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.push(0);
            offset += body.len();
        }
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&[0; 5]);
        for (_, body) in files {
            out.extend_from_slice(body);
        }
        out
    }

    #[test]
    fn reads_table_and_data() {
        let bytes = archive(&[("GUARD.SHP", b"abc"), ("LEVEL1.CMZ", b"defgh")]);
        let pak = Pak::parse("L01.PAK", &bytes).unwrap();
        assert_eq!(pak.entries.len(), 2);
        let maze = pak.find("level1.cmz").unwrap();
        assert_eq!(maze.size, 5);
        assert_eq!(pak.data(maze, &bytes).unwrap(), b"defgh");
        assert_eq!(pak.data(&pak.entries[0], &bytes).unwrap(), b"abc");
    }

    #[test]
    fn reads_the_older_table_end() {
        // Eye of the Beholder: four bytes of no meaning just before the data.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(4u32 + 6 + 4).to_le_bytes());
        bytes.extend_from_slice(b"A.CPS\0");
        bytes.extend_from_slice(&[0x0c, 0x00, 0x10, 0x04]);
        bytes.extend_from_slice(b"xyz");
        let pak = Pak::parse("EOB.PAK", &bytes).unwrap();
        assert_eq!(pak.entries.len(), 1);
        assert_eq!(pak.data(&pak.entries[0], &bytes).unwrap(), b"xyz");
    }

    #[test]
    fn rejects_offsets_outside() {
        let mut bytes = archive(&[("A.CPS", b"x")]);
        bytes[0] = 0xff;
        assert!(Pak::parse("TEST.PAK", &bytes).is_err());
    }
}
