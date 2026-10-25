//! One way to the game's files for one language. The CD keeps them in `DATA` (shared by all
//! languages: levels, scenes, wall sets, speech) and `DATA\GER`, `DATA\ENG`, `DATA\FRE` (the
//! program, texts, intro and finale); a language's file goes before a shared one of the same
//! name (`GENERAL.PAK`). Nearly all files lie in PAK archives, which are read once when the
//! store opens; the speech (`*.TLK`, some 250 MB) only when asked for.
//!
//! Names are matched without regard to case. Several archives hold files of the same name
//! (each level its own palette); [`AssetStore::read`] gives the first, [`Archive::read`] the
//! one of a given archive.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::iso::IsoImage;
use crate::pak::Pak;
use crate::paths::{Installation, Language, Medium};

/// An archive read into memory.
#[derive(Debug, Clone)]
pub struct Archive {
    /// `L01.PAK`, `GENERAL.PAK` …
    pub name: String,
    /// The language whose folder holds it (`None`: shared by all).
    pub language: Option<Language>,
    pub pak: Pak,
    bytes: Vec<u8>,
}

impl Archive {
    pub fn parse(name: &str, language: Option<Language>, bytes: Vec<u8>) -> Result<Self> {
        let pak = Pak::parse(name, &bytes)?;
        Ok(Self {
            name: name.to_string(),
            language,
            pak,
            bytes,
        })
    }

    pub fn read(&self, name: &str) -> Option<&[u8]> {
        let entry = self.pak.find(name)?;
        self.pak.data(entry, &self.bytes).ok()
    }

    /// Every file of the archive, in its order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.pak.entries.iter().filter_map(|e| {
            let data = self.pak.data(e, &self.bytes).ok()?;
            Some((e.name.as_str(), data))
        })
    }
}

#[derive(Debug, Clone)]
enum Source {
    Image(IsoImage),
    Folder(PathBuf),
}

#[derive(Debug, Clone)]
pub struct AssetStore {
    pub installation: Installation,
    pub language: Language,
    source: Source,
    /// Upper-case name → path on the CD (`DATA/GER/LANDS.EXE`).
    loose: BTreeMap<String, String>,
    /// The language's archives first, then the shared ones (by name).
    archives: Vec<Archive>,
}

impl AssetStore {
    pub fn open(installation: &Installation, language: Language) -> Result<Self> {
        let source = match &installation.medium {
            Medium::Image(path) => Source::Image(IsoImage::open(path)?),
            Medium::Folder(dir) => Source::Folder(dir.clone()),
        };
        let shared = list(&source, "DATA")?;
        let own = list(&source, &format!("DATA/{}", language.folder()))?;
        if shared.is_empty() && own.is_empty() {
            return Err(Error::NotFound(format!(
                "DATA auf {}",
                installation.dir.display()
            )));
        }
        let mut loose = BTreeMap::new();
        for path in shared.into_iter().chain(own.iter().cloned()) {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            loose.insert(name, path);
        }
        let mut store = Self {
            installation: installation.clone(),
            language,
            source,
            loose,
            archives: Vec::new(),
        };
        let mut archives = Vec::new();
        for (name, path) in &store.loose {
            if !name.ends_with(".PAK") {
                continue;
            }
            let own = own.contains(path);
            let archive = Archive::parse(name, own.then_some(language), store.read_path(path)?)?;
            archives.push(archive);
        }
        archives.sort_by_key(|a| (a.language.is_none(), a.name.clone()));
        store.archives = archives;
        Ok(store)
    }

    /// The loose files on the CD for this language (upper case, sorted): the archives, the
    /// speech, the programs …
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.loose.keys().map(String::as_str)
    }

    /// The loose files with this extension (`"TLK"`, without the dot).
    pub fn names_with(&self, extension: &str) -> Vec<String> {
        let suffix = format!(".{}", extension.to_ascii_uppercase());
        self.names()
            .filter(|n| n.ends_with(&suffix))
            .map(str::to_string)
            .collect()
    }

    pub fn archives(&self) -> &[Archive] {
        &self.archives
    }

    pub fn archive(&self, name: &str) -> Option<&Archive> {
        self.archives
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
    }

    /// A loose file of the CD (an archive's bytes as they are, the speech, the program).
    pub fn read_loose(&self, name: &str) -> Result<Vec<u8>> {
        let path = self
            .loose
            .get(&name.to_ascii_uppercase())
            .ok_or_else(|| Error::NotFound(name.to_string()))?;
        self.read_path(path)
    }

    /// A loose archive read when needed (the speech: `00.TLK` …).
    pub fn open_archive(&self, name: &str) -> Result<Archive> {
        let bytes = self.read_loose(name)?;
        Archive::parse(&name.to_ascii_uppercase(), None, bytes)
    }

    /// A file by name: loose, else from the first archive holding it.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        if self.loose.contains_key(&name.to_ascii_uppercase()) {
            return self.read_loose(name);
        }
        self.archives
            .iter()
            .find_map(|a| a.read(name))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| Error::NotFound(name.to_string()))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.loose.contains_key(&name.to_ascii_uppercase())
            || self.archives.iter().any(|a| a.pak.find(name).is_some())
    }

    fn read_path(&self, path: &str) -> Result<Vec<u8>> {
        match &self.source {
            Source::Image(iso) => iso.read(path),
            Source::Folder(dir) => {
                let file = dir.join(path.replace('/', "\\"));
                std::fs::read(&file).map_err(|e| Error::io(file.display(), e))
            }
        }
    }
}

/// The files directly in a folder of the CD (`DATA`, `DATA/GER`), as paths with `/`.
fn list(source: &Source, folder: &str) -> Result<Vec<String>> {
    let prefix = format!("{folder}/");
    match source {
        Source::Image(iso) => Ok(iso
            .files()
            .map(|(path, _)| path)
            .filter(|p| {
                p.strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/'))
            })
            .map(str::to_string)
            .collect()),
        Source::Folder(dir) => {
            let full = dir.join(folder.replace('/', "\\"));
            let Ok(entries) = std::fs::read_dir(&full) else {
                return Ok(Vec::new());
            };
            let mut found = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|e| Error::io(full.display(), e))?;
                if entry.file_type().is_ok_and(|t| t.is_file()) {
                    let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
                    found.push(format!("{prefix}{name}"));
                }
            }
            found.sort();
            Ok(found)
        }
    }
}
