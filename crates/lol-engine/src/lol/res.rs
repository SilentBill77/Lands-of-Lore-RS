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

//! The game's files as ScummVM's `Resource` finds them: loose files first (the language's
//! folder, then `DATA`), then the archives in the order they were loaded (`loadPakFile`); the
//! game loads and unloads archives as it goes (the intro's, each level's speech). An archive
//! name stands for both its copies, the language's and the shared one (`STARTUP.PAK`).

use std::collections::HashMap;

use lol_data::assets::{Archive, AssetStore};

pub struct Res {
    pub store: AssetStore,
    /// Loaded archives in search order (upper case).
    loaded: Vec<String>,
    /// Speech archives read into memory while loaded.
    talk: HashMap<String, Archive>,
}

impl Res {
    pub fn new(store: AssetStore) -> Self {
        Self {
            store,
            loaded: Vec::new(),
            talk: HashMap::new(),
        }
    }

    pub fn language(&self) -> lol_data::paths::Language {
        self.store.language
    }

    /// `loadPakFile`.
    pub fn load_pak(&mut self, name: &str) -> bool {
        let name = name.to_ascii_uppercase();
        if self.loaded.contains(&name) {
            return true;
        }
        if name.ends_with(".TLK") {
            match self.store.open_archive(&name) {
                Ok(archive) => {
                    self.talk.insert(name.clone(), archive);
                }
                Err(_) => return false,
            }
        } else if self.store.archive(&name).is_none() {
            return false;
        }
        self.loaded.push(name);
        true
    }

    pub fn unload_pak(&mut self, name: &str) {
        let name = name.to_ascii_uppercase();
        self.loaded.retain(|n| *n != name);
        self.talk.remove(&name);
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.contains(&name.to_ascii_uppercase())
    }

    /// `loadFileList("FILEDATA.FDT")`: every archive the list names, in its order (entries of
    /// 20 bytes: the offset of the name, then 16 bytes).
    pub fn load_file_list(&mut self, list: &str) -> bool {
        let Some(data) = self.file_data(list) else {
            return false;
        };
        let mut pos = 0;
        while pos + 4 <= data.len() {
            let offset = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
            if offset == 0 {
                break;
            }
            let name: Vec<u8> = data
                .get(offset..offset + 12)
                .unwrap_or(&[])
                .iter()
                .take_while(|&&b| b != 0)
                .copied()
                .collect();
            let name = String::from_utf8_lossy(&name).to_ascii_uppercase();
            if name.ends_with(".PAK") {
                self.load_pak(&name);
            }
            pos += 20;
        }
        true
    }

    fn in_archives(&self, name: &str) -> Option<Vec<u8>> {
        let lang = Some(self.store.language);
        for archive in &self.loaded {
            if let Some(talk) = self.talk.get(archive) {
                if let Some(d) = talk.read(name) {
                    return Some(d.to_vec());
                }
                continue;
            }
            for language in [lang, None] {
                if let Some(a) = self.store.archive_in(archive, language)
                    && let Some(d) = a.read(name)
                {
                    return Some(d.to_vec());
                }
            }
        }
        None
    }

    /// `fileData`: a file by name.
    pub fn file_data(&self, name: &str) -> Option<Vec<u8>> {
        let upper = name.to_ascii_uppercase();
        if self.store.names().any(|n| n == upper)
            && let Ok(d) = self.store.read_loose(&upper)
        {
            return Some(d);
        }
        self.in_archives(&upper)
    }

    pub fn exists(&self, name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        if self.store.names().any(|n| n == upper) {
            return true;
        }
        let lang = Some(self.store.language);
        self.loaded.iter().any(|archive| {
            if let Some(talk) = self.talk.get(archive) {
                return talk.pak.find(&upper).is_some();
            }
            [lang, None].iter().any(|&l| {
                self.store
                    .archive_in(archive, l)
                    .is_some_and(|a| a.pak.find(&upper).is_some())
            })
        })
    }
}
