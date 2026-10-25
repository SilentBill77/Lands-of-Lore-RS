// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Writing the sampled sounds out as WAV files (16 bit, mono, at their own rate):
//!
//! - `klaenge/allgemein/<ARCHIV>/NAME.VOC.wav`: the sounds in archives shared by all
//!   languages (`VOC.PAK` …), `klaenge/de|en|fr/<ARCHIV>/` those of a language's archives
//!   (the intro's voices, `INTROVOC.PAK`; English and French only where they differ);
//! - `sprache/<NN>/NAME.wav`: the speech, one folder per `NN.TLK`.

use std::path::Path;

use lol_data::Result;
use lol_data::assets::{Archive, AssetStore};
use lol_data::paths::{Installation, Language};
use lol_data::voc;

use crate::export::stem;

#[derive(Default)]
struct Counts {
    written: usize,
    failed: usize,
}

pub fn all(copy: &Installation, target: &Path) -> Result<()> {
    let german = AssetStore::open(copy, Language::German)?;
    let mut sounds = Counts::default();
    let folder = target.join("klaenge");
    for archive in german.archives() {
        let group = if archive.language.is_some() {
            Language::German.code()
        } else {
            "allgemein"
        };
        let dir = folder.join(group).join(stem(&archive.name));
        one_archive(archive, None, &dir, "VOC", &mut sounds)?;
    }
    for language in [Language::English, Language::French] {
        let store = AssetStore::open(copy, language)?;
        for archive in store.archives().iter().filter(|a| a.language.is_some()) {
            let dir = folder.join(language.code()).join(stem(&archive.name));
            one_archive(
                archive,
                german.archive(&archive.name),
                &dir,
                "VOC",
                &mut sounds,
            )?;
        }
    }
    report("Klänge", &sounds, &folder);

    let mut speech = Counts::default();
    let folder = target.join("sprache");
    for name in german.names_with("TLK") {
        let archive = german.open_archive(&name)?;
        one_archive(&archive, None, &folder.join(stem(&name)), "", &mut speech)?;
    }
    report("Sprache", &speech, &folder);
    Ok(())
}

fn report(what: &str, counts: &Counts, folder: &Path) {
    println!(
        "{what}: {} WAV nach {}{}",
        counts.written,
        folder.display(),
        if counts.failed > 0 {
            format!(", {} nicht lesbar", counts.failed)
        } else {
            String::new()
        }
    );
}

/// The archive's voice files (those with this extension; all with an empty one) as WAV.
fn one_archive(
    archive: &Archive,
    reference: Option<&Archive>,
    dir: &Path,
    extension: &str,
    counts: &mut Counts,
) -> Result<()> {
    for (name, data) in archive.files() {
        if !name.ends_with(&format!(".{extension}")) && !extension.is_empty() {
            continue;
        }
        if reference.is_some_and(|r| r.read(name) == Some(data)) {
            continue;
        }
        match voc::parse(name, data) {
            Ok(sound) => {
                let path = dir.join(format!("{name}.wav"));
                std::fs::create_dir_all(dir).map_err(|e| lol_data::Error::io(dir.display(), e))?;
                std::fs::write(&path, lol_sound::wav(&sound.samples, sound.rate))
                    .map_err(|e| lol_data::Error::io(path.display(), e))?;
                counts.written += 1;
            }
            Err(err) => {
                counts.failed += 1;
                eprintln!("  {}/{name}: {err}", archive.name);
            }
        }
    }
    Ok(())
}
