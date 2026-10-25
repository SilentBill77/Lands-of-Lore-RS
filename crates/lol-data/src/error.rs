// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Ein-/Ausgabefehler bei {path}: {source}")]
    Io { path: String, source: io::Error },
    #[error("Datei nicht gefunden: {0}")]
    NotFound(String),
    #[error("{file}: {message}")]
    Format { file: String, message: String },
}

impl Error {
    pub fn format(file: impl Into<String>, message: impl Into<String>) -> Self {
        Error::Format {
            file: file.into(),
            message: message.into(),
        }
    }

    pub fn io(path: impl std::fmt::Display, source: io::Error) -> Self {
        Error::Io {
            path: path.to_string(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Little-endian reader over a byte slice that reports truncation as a format error.
pub(crate) struct Reader<'a> {
    file: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(file: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            file,
            bytes,
            pos: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| {
                Error::format(
                    self.file,
                    format!("Datei endet vorzeitig bei Byte {} (+{n})", self.pos),
                )
            })?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    pub fn u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Result<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A NUL-terminated string (Latin-1), without its NUL.
    pub fn cstr(&mut self) -> Result<String> {
        let rest = &self.bytes[self.pos.min(self.bytes.len())..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| self.error(format!("Zeichenkette ohne Ende bei Byte {}", self.pos)))?;
        let text = rest[..len].iter().map(|&b| b as char).collect();
        self.pos += len + 1;
        Ok(text)
    }

    pub fn error(&self, message: impl Into<String>) -> Error {
        Error::format(self.file, message)
    }
}
