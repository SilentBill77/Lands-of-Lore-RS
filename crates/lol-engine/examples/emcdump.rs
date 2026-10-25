//! Lists a script of the game (`LEVEL1.INF` …) as its functions and words, the calls of the
//! game's functions by name: `emcdump <file> [function]`.

use lol_data::assets::AssetStore;
use lol_data::paths::{self, Language};
use lol_data::tables::SCRIPT_FUNCTIONS;
use lol_engine::lol::res::Res;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let name = &args[1];
    let raw: Option<(usize, usize)> = (args.get(2).map(|s| s.as_str()) == Some("raw"))
        .then(|| (args[3].parse().unwrap(), args[4].parse().unwrap()));
    let only: Option<usize> = args.get(2).and_then(|s| s.parse().ok());
    let copy = paths::find().expect("installation");
    let store = AssetStore::open(&copy, Language::German).expect("store");
    let mut res = Res::new(store);
    res.load_pak("GENERAL.PAK");
    res.load_pak("STARTUP.PAK");
    res.load_file_list("FILEDATA.FDT");
    let bytes = res.file_data(name).expect("file");
    // FORM/EMC2: TEXT, ORDR, DATA.
    let mut pos = 12;
    let (mut text, mut ordr, mut data) = (Vec::new(), Vec::new(), Vec::new());
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = &bytes[pos + 8..pos + 8 + size];
        let words = || {
            body.as_chunks::<2>()
                .0
                .iter()
                .map(|w| u16::from_be_bytes([w[0], w[1]]))
                .collect::<Vec<u16>>()
        };
        match id {
            b"TEXT" => text = body.to_vec(),
            b"ORDR" => ordr = words(),
            b"DATA" => data = words(),
            _ => {}
        }
        pos += 8 + size + (size & 1);
    }
    let txt = |i: usize| -> String {
        let at = text
            .get(2 * i..2 * i + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize);
        let Some(rest) = at.and_then(|a| text.get(a..)) else {
            return String::new();
        };
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        String::from_utf8_lossy(&rest[..end]).to_string()
    };
    let mut starts: Vec<(usize, usize)> = if let Some((a, _)) = raw {
        vec![(usize::MAX, a.wrapping_sub(1))]
    } else {
        ordr.iter()
            .enumerate()
            .filter(|(_, o)| **o != 0xFFFF)
            .map(|(i, o)| (i, *o as usize))
            .collect()
    };
    starts.sort_by_key(|s| s.1);
    for (f, start) in &starts {
        if only.is_some_and(|o| o != *f) {
            continue;
        }
        let end = match raw {
            Some((_, e)) => e,
            None => starts
                .iter()
                .map(|s| s.1)
                .filter(|&s| s > *start)
                .min()
                .unwrap_or(data.len()),
        };
        println!(
            "== Funktion {f} (0x{f:X}) bei {start}, Ereignisse {:#06x}",
            data.get(*start).copied().unwrap_or(0)
        );
        let mut ip = start.wrapping_add(1);
        let mut last_push: Vec<i16> = Vec::new();
        while ip < end {
            let code = data[ip];
            let at = ip;
            ip += 1;
            let (op, p) = if code & 0x8000 != 0 {
                (0u16, (code & 0x7fff) as i16)
            } else if code & 0x4000 != 0 {
                ((code >> 8) & 0x1f, code as u8 as i8 as i16)
            } else if code & 0x2000 != 0 {
                let v = data.get(ip).copied().unwrap_or(0) as i16;
                ip += 1;
                ((code >> 8) & 0x1f, v)
            } else {
                ((code >> 8) & 0x1f, 0)
            };
            let desc = match op {
                0 => format!("jmp {p}"),
                1 => format!("setRet {p}"),
                2 => format!("pushRetOrPos {p}"),
                3 | 4 => {
                    last_push.push(p);
                    format!("push {p}")
                }
                5 => format!("pushReg {p}"),
                6 => format!("pushBPNeg {p}"),
                7 => format!("pushBPAdd {p}"),
                8 => format!("popRetOrPos {p}"),
                9 => format!("popReg {p}"),
                10 => format!("popBPNeg {p}"),
                11 => format!("popBPAdd {p}"),
                12 => format!("addSP {p}"),
                13 => format!("subSP {p}"),
                14 => {
                    // The interpreter takes the parameter's low byte (`(uint8)_parameter`).
                    let n = SCRIPT_FUNCTIONS
                        .get(p as u8 as usize)
                        .copied()
                        .unwrap_or("?");
                    let strs: Vec<String> = last_push
                        .iter()
                        .rev()
                        .take(3)
                        .map(|&v| txt(v.max(0) as usize))
                        .filter(|s| !s.is_empty())
                        .collect();
                    last_push.clear();
                    format!(
                        "CALL {n} {}",
                        if strs.is_empty() {
                            String::new()
                        } else {
                            format!("{strs:?}")
                        }
                    )
                }
                15 => format!("ifNotJmp {p}"),
                16 => format!("negate {p}"),
                17 => format!("eval {p}"),
                18 => format!("setRetAndJmp {p}"),
                _ => format!("?? {op} {p}"),
            };
            println!("  {at:5}: {desc}");
        }
    }
}
