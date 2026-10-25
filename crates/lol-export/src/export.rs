//! Writing the pictures out, by archive, into `bilder/allgemein/<ARCHIV>/` (the files shared
//! by all languages) and `bilder/de|en|fr/<ARCHIV>/` (a language's own archives; English and
//! French only where a file differs from the German one). Named after the original file, as
//! the replacement graphics will be (`PLAYFLD.CPS.png`):
//!
//! - `NAME.CPS.png`: a picture (320×200);
//! - `NAME.SHP.png`: all figures of a file side by side, and each alone in `NAME.SHP/00.png` …;
//! - `NAME.WSA.png`: all frames of an animation in rows, and each alone in `NAME.WSA/000.png` …;
//! - `NAME.VCN.png`: the blocks of a wall set, 32 to a row;
//! - `NAME.PAL.png`, `NAME.COL.png`: a palette as 16×16 swatches.
//!
//! Colour 0 of figures is transparent. An animation drawn over a picture is shown over the
//! picture of its name (`HIST02.WSA` over `HIST02.CPS`), else colour 0 is transparent.
//!
//! The palette: a picture's or animation's own; else, in a wall set's archive, the game's with
//! that set's lower half (colours 0–127, which the walls bring); else that of the archive's
//! first picture with a palette; else the game's: the one of `PLAYFLD.CPS` (the colours of the
//! screen around the view) with the lower half of `KEEP.VCN` (the first level).

use std::path::Path;

use lol_data::Result;
use lol_data::assets::{Archive, AssetStore};
use lol_data::cps::Cps;
use lol_data::palette::Palette;
use lol_data::paths::{Installation, Language};
use lol_data::shp;
use lol_data::vcn::WallSet;
use lol_data::wsa::Wsa;

/// How wide sheets of figures and frames may grow (pixels).
const SHEET_WIDTH: usize = 1280;
/// Space between figures on a sheet.
const GAP: usize = 2;

#[derive(Default)]
struct Counts {
    files: usize,
    images: usize,
    failed: usize,
}

pub fn all(copy: &Installation, target: &Path) -> Result<()> {
    let german = AssetStore::open(copy, Language::German)?;
    let base = base_palette(&german)?;
    let mut counts = Counts::default();
    let pictures = target.join("bilder");
    for archive in german.archives() {
        let group = if archive.language.is_some() {
            Language::German.code()
        } else {
            "allgemein"
        };
        let folder = pictures.join(group).join(stem(&archive.name));
        one_archive(archive, None, &base, &folder, &mut counts)?;
    }
    for language in [Language::English, Language::French] {
        let store = AssetStore::open(copy, language)?;
        for archive in store.archives().iter().filter(|a| a.language.is_some()) {
            let folder = pictures.join(language.code()).join(stem(&archive.name));
            let reference = german.archive(&archive.name);
            one_archive(archive, reference, &base, &folder, &mut counts)?;
        }
    }
    println!(
        "Bilder: {} Dateien, {} PNG nach {}{}",
        counts.files,
        counts.images,
        pictures.display(),
        if counts.failed > 0 {
            format!(", {} nicht lesbar", counts.failed)
        } else {
            String::new()
        }
    );
    Ok(())
}

/// `L01` from `L01.PAK`.
pub fn stem(name: &str) -> &str {
    name.split('.').next().unwrap_or(name)
}

fn base_palette(store: &AssetStore) -> Result<Palette> {
    let field = Cps::parse("PLAYFLD.CPS", &store.read("PLAYFLD.CPS")?)?;
    let palette = field
        .palette
        .unwrap_or_else(|| Palette::from_vga(&[0; 768]));
    Ok(match store.read("KEEP.VCN") {
        Ok(bytes) => palette.with_low(&WallSet::parse("KEEP.VCN", &bytes)?.palette_low),
        Err(_) => palette,
    })
}

/// The palette for an archive's files without one of their own.
fn archive_palette(archive: &Archive, base: &Palette) -> Palette {
    let wall_set = archive
        .files()
        .find(|(n, _)| n.ends_with(".VCN"))
        .and_then(|(n, d)| WallSet::parse(n, d).ok());
    if let Some(set) = wall_set {
        return base.with_low(&set.palette_low);
    }
    archive
        .files()
        .filter(|(n, _)| n.ends_with(".CPS"))
        .find_map(|(n, d)| Cps::parse(n, d).ok()?.palette)
        .unwrap_or_else(|| base.clone())
}

fn one_archive(
    archive: &Archive,
    reference: Option<&Archive>,
    base: &Palette,
    folder: &Path,
    counts: &mut Counts,
) -> Result<()> {
    let palette = archive_palette(archive, base);
    for (name, data) in archive.files() {
        if reference.is_some_and(|r| r.read(name) == Some(data)) {
            continue;
        }
        let ext = name.rsplit('.').next().unwrap_or("");
        let written = match ext {
            "CPS" => picture(name, data, &palette, folder),
            "SHP" => figures(name, data, &palette, folder),
            "WSA" => animation(name, data, archive, &palette, folder),
            "VCN" => wall_blocks(name, data, base, folder),
            "PAL" | "COL" if data.len() == 768 => swatches(name, data, folder),
            _ => continue,
        };
        counts.files += 1;
        match written {
            Ok(n) => counts.images += n,
            Err(err) => {
                counts.failed += 1;
                eprintln!("  {}/{name}: {err}", archive.name);
            }
        }
    }
    Ok(())
}

fn picture(name: &str, data: &[u8], palette: &Palette, folder: &Path) -> Result<usize> {
    let cps = Cps::parse(name, data)?;
    let palette = cps.palette.as_ref().unwrap_or(palette);
    let rgba = palette.rgba(&cps.pixels, false);
    save(
        &folder.join(format!("{name}.png")),
        cps.width,
        cps.height,
        &rgba,
    )?;
    Ok(1)
}

fn figures(name: &str, data: &[u8], palette: &Palette, folder: &Path) -> Result<usize> {
    let shapes = shp::parse_file(name, data)?;
    let own = folder.join(name);
    let mut written = 0;
    let mut cells = Vec::new();
    for (i, shape) in shapes.iter().enumerate() {
        let Some(shape) = shape.as_ref().filter(|s| s.width > 0 && s.height > 0) else {
            continue;
        };
        let rgba = palette.rgba(&shape.colored(), true);
        save(
            &own.join(format!("{i:02}.png")),
            shape.width,
            shape.height,
            &rgba,
        )?;
        written += 1;
        cells.push((shape.width, shape.height, rgba));
    }
    if !cells.is_empty() {
        let (w, h, rgba) = sheet(&cells, GAP);
        save(&folder.join(format!("{name}.png")), w, h, &rgba)?;
        written += 1;
    }
    Ok(written)
}

fn animation(
    name: &str,
    data: &[u8],
    archive: &Archive,
    palette: &Palette,
    folder: &Path,
) -> Result<usize> {
    let wsa = Wsa::parse(name, data)?;
    // The picture of the same name (`HIST02.CPS` for `HIST02.WSA`), which it is drawn over.
    let picture = archive
        .read(&format!("{}.CPS", stem(name)))
        .and_then(|d| Cps::parse(name, d).ok());
    let palette = wsa
        .palette
        .as_ref()
        .or(picture.as_ref().and_then(|p| p.palette.as_ref()))
        .unwrap_or(palette);
    let ground = picture.as_ref().filter(|_| wsa.over_picture);
    let own = folder.join(name);
    let mut cells = Vec::new();
    for (i, frame) in wsa.frames.iter().enumerate() {
        let rgba = match &ground {
            Some(picture) => palette.rgba(&over(&wsa, frame, picture), false),
            None => palette.rgba(frame, wsa.over_picture),
        };
        save(
            &own.join(format!("{i:03}.png")),
            wsa.width,
            wsa.height,
            &rgba,
        )?;
        cells.push((wsa.width, wsa.height, rgba));
    }
    let (w, h, rgba) = sheet(&cells, 0);
    save(&folder.join(format!("{name}.png")), w, h, &rgba)?;
    Ok(cells.len() + 1)
}

/// A frame drawn over the picture beneath it (colour 0 shows the picture), cut to the frame.
fn over(wsa: &Wsa, frame: &[u8], picture: &Cps) -> Vec<u8> {
    let mut out = frame.to_vec();
    for (i, pixel) in out.iter_mut().enumerate() {
        if *pixel != 0 {
            continue;
        }
        let x = wsa.x as isize + (i % wsa.width) as isize;
        let y = wsa.y as isize + (i / wsa.width) as isize;
        if (0..picture.width as isize).contains(&x) && (0..picture.height as isize).contains(&y) {
            *pixel = picture.pixels[y as usize * picture.width + x as usize];
        }
    }
    out
}

fn wall_blocks(name: &str, data: &[u8], base: &Palette, folder: &Path) -> Result<usize> {
    let set = WallSet::parse(name, data)?;
    let palette = base.with_low(&set.palette_low);
    const PER_ROW: usize = 32;
    let rows = set.blocks.len().div_ceil(PER_ROW).max(1);
    let (w, h) = (PER_ROW * 8, rows * 8);
    let mut pixels = vec![0u8; w * h];
    for i in 0..set.blocks.len() {
        let (bx, by) = ((i % PER_ROW) * 8, (i / PER_ROW) * 8);
        for (p, color) in set.block_pixels(i).iter().enumerate() {
            pixels[(by + p / 8) * w + bx + p % 8] = *color;
        }
    }
    save(
        &folder.join(format!("{name}.png")),
        w,
        h,
        &palette.rgba(&pixels, false),
    )?;
    Ok(1)
}

fn swatches(name: &str, data: &[u8], folder: &Path) -> Result<usize> {
    let palette = Palette::parse(name, data)?;
    const CELL: usize = 8;
    let side = 16 * CELL;
    let pixels: Vec<u8> = (0..side * side)
        .map(|i| ((i / side / CELL) * 16 + (i % side) / CELL) as u8)
        .collect();
    save(
        &folder.join(format!("{name}.png")),
        side,
        side,
        &palette.rgba(&pixels, false),
    )?;
    Ok(1)
}

/// Images side by side in rows no wider than [`SHEET_WIDTH`], `gap` pixels apart, on a
/// transparent ground; each row as high as its highest image, the images on its top.
fn sheet(cells: &[(usize, usize, Vec<u8>)], gap: usize) -> (usize, usize, Vec<u8>) {
    let mut places = Vec::with_capacity(cells.len());
    let (mut x, mut y, mut row_height, mut width) = (0, 0, 0, 0);
    for (w, h, _) in cells {
        if x > 0 && x + w > SHEET_WIDTH {
            x = 0;
            y += row_height + gap;
            row_height = 0;
        }
        places.push((x, y));
        x += w + gap;
        width = width.max(x - gap);
        row_height = row_height.max(*h);
    }
    let height = y + row_height;
    let mut out = vec![0u8; width * height * 4];
    for ((w, h, rgba), (px, py)) in cells.iter().zip(places) {
        for row in 0..*h {
            let from = &rgba[row * w * 4..(row + 1) * w * 4];
            let at = ((py + row) * width + px) * 4;
            out[at..at + w * 4].copy_from_slice(from);
        }
    }
    (width, height, out)
}

pub fn save(path: &Path, width: usize, height: usize, rgba: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| lol_data::Error::io(dir.display(), e))?;
    }
    image::save_buffer(
        path,
        rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .map_err(|e| lol_data::Error::io(path.display(), std::io::Error::other(e.to_string())))
}
