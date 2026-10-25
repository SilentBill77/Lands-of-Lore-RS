//! The CD as an image (ISO 9660), as GOG ships it (`GAME.DAT`): the game reads its files
//! straight from it, nothing needs unpacking.
//!
//! Sectors of 2048 bytes. Sector 16 holds the primary volume descriptor (`01 CD001`), whose
//! bytes 156–189 are the root directory's record. A directory is a run of records, none
//! crossing a sector's end (a zero length byte means: on to the next sector): its length
//! (u8), the extended attribute's length (u8), the first sector (u32 LE, then BE), the size
//! (u32 LE, then BE), the date (7 bytes), the flags (bit 1: a directory), …, the name's
//! length (byte 32) and the name (`GENERAL.PAK;1`; `00` and `01` for the directory itself and
//! its parent).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const SECTOR: u64 = 2048;
/// Deeper than any CD's directories go (and no endless loops on a broken image).
const MAX_DEPTH: usize = 8;

/// Where a file lies in the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoFile {
    pub offset: u64,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct IsoImage {
    pub path: PathBuf,
    pub volume: String,
    /// Upper-case path with `/` (`DATA/GER/GENERAL.PAK`) → where the file lies.
    files: BTreeMap<String, IsoFile>,
}

/// Whether a file is an ISO 9660 image (its volume descriptor where it belongs).
pub fn is_image(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 6];
    file.seek(SeekFrom::Start(16 * SECTOR)).is_ok()
        && file.read_exact(&mut magic).is_ok()
        && &magic == b"\x01CD001"
}

impl IsoImage {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = File::open(path).map_err(|e| Error::io(path.display(), e))?;
        Self::read_from(&mut file, path)
    }

    /// Reads the directories from any source of the image's bytes.
    pub fn read_from(source: &mut (impl Read + Seek), path: &Path) -> Result<Self> {
        let name = path.display().to_string();
        let bad = |message: &str| Error::format(&name, message);
        let mut descriptor = [0u8; SECTOR as usize];
        read_at(source, 16 * SECTOR, &mut descriptor).map_err(|e| Error::io(&name, e))?;
        if &descriptor[..6] != b"\x01CD001" {
            return Err(bad("kein ISO-9660-Abbild"));
        }
        let volume = String::from_utf8_lossy(&descriptor[40..72])
            .trim_matches(|c: char| c == '\0' || c.is_whitespace())
            .to_string();
        let root = Record::parse(&descriptor[156..190]).ok_or_else(|| bad("Wurzel fehlt"))?;
        let mut files = BTreeMap::new();
        let mut pending = vec![(String::new(), root, 0usize)];
        while let Some((prefix, dir, depth)) = pending.pop() {
            if depth > MAX_DEPTH {
                return Err(bad("Verzeichnisse zu tief verschachtelt"));
            }
            let mut bytes = vec![0u8; dir.size as usize];
            read_at(source, dir.sector * SECTOR, &mut bytes).map_err(|e| Error::io(&name, e))?;
            let mut pos = 0usize;
            while pos < bytes.len() {
                let len = bytes[pos] as usize;
                if len == 0 {
                    // On to the next sector.
                    pos = (pos / SECTOR as usize + 1) * SECTOR as usize;
                    continue;
                }
                let record = bytes
                    .get(pos..pos + len)
                    .and_then(Record::parse)
                    .ok_or_else(|| bad("beschädigter Verzeichniseintrag"))?;
                pos += len;
                if record.name.is_empty() {
                    continue;
                }
                let path = format!("{prefix}{}", record.name);
                if record.directory {
                    pending.push((format!("{path}/"), record, depth + 1));
                } else {
                    files.insert(
                        path,
                        IsoFile {
                            offset: record.sector * SECTOR,
                            size: record.size,
                        },
                    );
                }
            }
        }
        Ok(Self {
            path: path.to_path_buf(),
            volume,
            files,
        })
    }

    /// Every file's path (upper case, `/`, sorted) and size.
    pub fn files(&self) -> impl Iterator<Item = (&str, u64)> {
        self.files.iter().map(|(p, f)| (p.as_str(), f.size))
    }

    pub fn find(&self, path: &str) -> Option<IsoFile> {
        self.files
            .get(&path.replace('\\', "/").to_ascii_uppercase())
            .copied()
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let place = self
            .find(path)
            .ok_or_else(|| Error::NotFound(path.to_string()))?;
        let mut file = File::open(&self.path).map_err(|e| Error::io(self.path.display(), e))?;
        let mut bytes = vec![0u8; place.size as usize];
        read_at(&mut file, place.offset, &mut bytes)
            .map_err(|e| Error::io(format!("{} ({path})", self.path.display()), e))?;
        Ok(bytes)
    }
}

fn read_at(source: &mut (impl Read + Seek), offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
    source.seek(SeekFrom::Start(offset))?;
    source.read_exact(buf)
}

/// One directory record.
struct Record {
    sector: u64,
    size: u64,
    directory: bool,
    /// Upper case, without the version (`;1`) and a trailing dot; empty for `.` and `..`.
    name: String,
}

impl Record {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let u32le = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as u64;
        if bytes.len() < 34 {
            return None;
        }
        let name_len = bytes[32] as usize;
        let raw = bytes.get(33..33 + name_len)?;
        let name = if raw == [0] || raw == [1] {
            String::new()
        } else {
            let text: String = raw.iter().map(|&b| b as char).collect();
            let text = text.split(';').next().unwrap_or("");
            text.trim_end_matches('.').to_ascii_uppercase()
        };
        Some(Self {
            sector: u32le(2),
            size: u32le(10),
            directory: bytes[25] & 2 != 0,
            name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn record(name: &[u8], sector: u32, size: u32, directory: bool) -> Vec<u8> {
        let len = 33 + name.len() + (name.len() + 1) % 2;
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&sector.to_le_bytes());
        r[6..10].copy_from_slice(&sector.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if directory { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// An image with `DATA/GER/LANDS.EXE` and `README.GER` as the CD has them.
    fn image() -> Vec<u8> {
        let mut bytes = vec![0u8; 24 * SECTOR as usize];
        let at = |s: usize| s * SECTOR as usize;
        bytes[at(16)..at(16) + 6].copy_from_slice(b"\x01CD001");
        bytes[at(16) + 40..at(16) + 48].copy_from_slice(b"LOL_V102");
        let root = record(&[0], 18, SECTOR as u32, true);
        bytes[at(16) + 156..at(16) + 156 + root.len()].copy_from_slice(&root);
        let dirs = [
            (
                18,
                vec![
                    record(&[0], 18, 2048, true),
                    record(&[1], 18, 2048, true),
                    record(b"DATA", 19, 2048, true),
                    record(b"README.GER;1", 22, 5, false),
                ],
            ),
            (
                19,
                vec![record(&[0], 19, 2048, true), record(b"GER", 20, 2048, true)],
            ),
            (
                20,
                vec![
                    record(&[0], 20, 2048, true),
                    record(b"LANDS.EXE;1", 23, 3, false),
                ],
            ),
        ];
        for (sector, records) in dirs {
            let mut pos = at(sector);
            for r in records {
                bytes[pos..pos + r.len()].copy_from_slice(&r);
                pos += r.len();
            }
        }
        bytes[at(22)..at(22) + 5].copy_from_slice(b"Hallo");
        bytes[at(23)..at(23) + 3].copy_from_slice(b"MZx");
        bytes
    }

    #[test]
    fn lists_the_directories() {
        let iso = IsoImage::read_from(&mut Cursor::new(image()), Path::new("GAME.DAT")).unwrap();
        assert_eq!(iso.volume, "LOL_V102");
        let files: Vec<_> = iso.files().collect();
        assert_eq!(files, vec![("DATA/GER/LANDS.EXE", 3), ("README.GER", 5)]);
        let exe = iso.find(r"data\ger\lands.exe").unwrap();
        assert_eq!(exe.offset, 23 * SECTOR);
    }

    #[test]
    fn reads_a_file() {
        let path = std::env::temp_dir().join(format!("lolrs-iso-{}.dat", std::process::id()));
        std::fs::write(&path, image()).unwrap();
        assert!(is_image(&path));
        let iso = IsoImage::open(&path).unwrap();
        assert_eq!(iso.read("README.GER").unwrap(), b"Hallo");
        assert!(iso.read("FEHLT.PAK").is_err());
        std::fs::remove_file(&path).unwrap();
    }
}
