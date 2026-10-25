// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Locating an installed copy of Lands of Lore: the CD version, either as the image GOG ships
//! (`GAME.DAT`, ISO 9660) or as the CD's files in a folder (one holding `DATA\GENERAL.PAK`).
//!
//! In this order: the environment variable [`LOL_ENV`] (a folder or the image itself); where
//! GOG's installer noted the game in the registry; next to the program and in the working
//! folder; GOG and other usual places on every drive (folders whose name mentions the game).

use std::env;
use std::path::{Path, PathBuf};

use crate::iso;

/// Environment variable naming the copy's folder or image (only that one is used then).
pub const LOL_ENV: &str = "LOL_DATA";

/// The languages of the CD, each with its folder below `DATA`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Language {
    German,
    English,
    French,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::German, Language::English, Language::French];

    /// The folder below `DATA` (and the extension of a level's texts: `LEVEL01.GER`).
    pub fn folder(self) -> &'static str {
        match self {
            Language::German => "GER",
            Language::English => "ENG",
            Language::French => "FRE",
        }
    }

    /// `de`, `en`, `fr`.
    pub fn code(self) -> &'static str {
        match self {
            Language::German => "de",
            Language::English => "en",
            Language::French => "fr",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Language::German => "Deutsch",
            Language::English => "Englisch",
            Language::French => "Französisch",
        }
    }

    /// From `de`, `GER`, `deutsch` … (case does not matter).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.to_ascii_lowercase();
        Self::ALL.into_iter().find(|l| {
            text == l.code() || text == l.folder().to_ascii_lowercase() || {
                let label = l.label().to_lowercase();
                text == label || (text.len() >= 3 && label.starts_with(&text))
            }
        })
    }
}

/// What holds the CD's files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Medium {
    /// An image of the CD (`GAME.DAT`).
    Image(PathBuf),
    /// The CD's files in a folder (the folder holding `DATA`).
    Folder(PathBuf),
}

/// An installed copy: its folder and where the CD's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    pub dir: PathBuf,
    pub medium: Medium,
}

impl Installation {
    /// The folder's name (`Lands Of Lore`).
    pub fn name(&self) -> String {
        self.dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.dir.display().to_string())
    }
}

/// The copy a folder (or an image file) stands for: an image `GAME.DAT` in it, the CD's
/// `DATA` folder in it, or the image itself.
pub fn installation_in(path: &Path) -> Option<Installation> {
    if path.is_file() {
        return iso::is_image(path).then(|| Installation {
            dir: path.parent().unwrap_or(Path::new(".")).to_path_buf(),
            medium: Medium::Image(path.to_path_buf()),
        });
    }
    let image = path.join("GAME.DAT");
    if image.is_file() && iso::is_image(&image) {
        return Some(Installation {
            dir: path.to_path_buf(),
            medium: Medium::Image(image),
        });
    }
    path.join(r"DATA\GENERAL.PAK")
        .is_file()
        .then(|| Installation {
            dir: path.to_path_buf(),
            medium: Medium::Folder(path.to_path_buf()),
        })
}

/// The copy to use: with the environment variable set only that one, else the first found.
pub fn find() -> Option<Installation> {
    if let Some(path) = env::var_os(LOL_ENV) {
        return installation_in(Path::new(&path));
    }
    candidates().iter().find_map(|dir| installation_in(dir))
}

/// Places to look, most likely first.
fn candidates() -> Vec<PathBuf> {
    let mut dirs = gog_registry();
    if let Ok(exe) = env::current_exe()
        && let Some(folder) = exe.parent()
    {
        dirs.push(folder.to_path_buf());
    }
    if let Ok(cwd) = env::current_dir() {
        dirs.push(cwd);
    }
    for drive in drives() {
        for parent in [
            "",
            "GOG Games",
            r"Program Files (x86)\GOG Galaxy\Games",
            r"Program Files (x86)\GOG.com",
            r"Program Files\GOG Galaxy\Games",
            "Games",
            "Spiele",
        ] {
            dirs.extend(lore_folders(&drive.join(parent)));
        }
    }
    dirs
}

/// GOG's product number of Lands of Lore: The Throne of Chaos.
const GOG_ID: &str = "1207662513";

/// Where GOG's installer put the game (`HKLM\SOFTWARE\…\GOG.com\Games\<id>`, value `PATH`),
/// read with Windows' own `reg` tool.
fn gog_registry() -> Vec<PathBuf> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    for key in [
        r"HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\",
        r"HKLM\SOFTWARE\GOG.com\Games\",
    ] {
        let mut reg = std::process::Command::new("reg");
        reg.args(["query", &format!("{key}{GOG_ID}"), "/v", "PATH"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up.
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            reg.creation_flags(CREATE_NO_WINDOW);
        }
        let Ok(out) = reg.output() else {
            continue;
        };
        let text = String::from_utf8_lossy(&out.stdout);
        if let Some(path) = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("PATH"))
            .and_then(|rest| rest.trim().strip_prefix("REG_SZ"))
        {
            dirs.push(PathBuf::from(path.trim()));
        }
    }
    dirs
}

/// Subfolders whose name mentions the game (`Lands Of Lore`, `LOL1`, …).
fn lore_folders(parent: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| lore_name(&e.file_name().to_string_lossy()))
        .map(|e| e.path())
        .collect();
    found.sort();
    found
}

fn lore_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    (name.contains("lands of lore") || name.starts_with("lol")) && !name.contains("-rs")
}

/// Drive roots that exist (Windows); the file system root elsewhere.
fn drives() -> Vec<PathBuf> {
    if cfg!(windows) {
        (b'C'..=b'Z')
            .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
            .filter(|root| root.is_dir())
            .collect()
    } else {
        vec![PathBuf::from("/")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_language_names() {
        assert_eq!(Language::parse("de"), Some(Language::German));
        assert_eq!(Language::parse("GER"), Some(Language::German));
        assert_eq!(Language::parse("englisch"), Some(Language::English));
        assert_eq!(Language::parse("fra"), Some(Language::French));
        assert_eq!(Language::parse("x"), None);
    }

    #[test]
    fn knows_the_game_s_folders() {
        assert!(lore_name("Lands Of Lore"));
        assert!(lore_name("LOL1"));
        assert!(!lore_name("Lands-of-Lore-RS"));
        assert!(!lore_name("Eye of the Beholder"));
    }

    #[test]
    fn finds_the_cd_s_files_in_a_folder() {
        let root = env::temp_dir().join(format!("lolrs-pfad-{}", std::process::id()));
        std::fs::create_dir_all(root.join("DATA")).unwrap();
        assert!(installation_in(&root).is_none());
        std::fs::write(root.join(r"DATA\GENERAL.PAK"), b"").unwrap();
        let copy = installation_in(&root).unwrap();
        assert_eq!(copy.medium, Medium::Folder(root.clone()));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
