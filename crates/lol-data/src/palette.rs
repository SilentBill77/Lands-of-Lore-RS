//! Palettes (`COL`, `PAL`): 256 colours of three bytes, each 0–63 as the VGA takes them.

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// 8-bit RGB per colour.
    pub colors: [[u8; 3]; 256],
    /// The 6-bit values as the VGA took them.
    pub vga: [[u8; 3]; 256],
}

impl Palette {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 768 {
            return Err(Error::format(
                file,
                format!("Palette zu kurz ({} statt 768 Byte)", bytes.len()),
            ));
        }
        Ok(Self::from_vga(&bytes[..768]))
    }

    /// From 768 bytes of 6-bit VGA values (higher bits are ignored).
    pub fn from_vga(bytes: &[u8]) -> Self {
        let mut colors = [[0u8; 3]; 256];
        let mut vga = [[0u8; 3]; 256];
        for ((color, six), rgb) in colors
            .iter_mut()
            .zip(vga.iter_mut())
            .zip(bytes.as_chunks::<3>().0)
        {
            for ((c, s), v) in color.iter_mut().zip(six.iter_mut()).zip(rgb) {
                *s = v & 0x3f;
                *c = ((*s as u16 * 255 + 31) / 63) as u8;
            }
        }
        Self { colors, vga }
    }

    /// This palette with its first colours from 6-bit VGA values (a wall set's 0–127).
    pub fn with_low(&self, vga: &[u8]) -> Self {
        let mut all = self.vga.as_flattened().to_vec();
        let n = vga.len().min(all.len());
        all[..n].copy_from_slice(&vga[..n]);
        Self::from_vga(&all)
    }

    /// Pixels of colour indices as RGBA, index 0 transparent if asked (the game's figures
    /// leave it out).
    pub fn rgba(&self, pixels: &[u8], zero_transparent: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(pixels.len() * 4);
        for &p in pixels {
            let [r, g, b] = self.colors[p as usize];
            let a = if zero_transparent && p == 0 { 0 } else { 255 };
            out.extend_from_slice(&[r, g, b, a]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widens_six_bit_values() {
        let mut bytes = vec![0u8; 768];
        bytes[3..6].copy_from_slice(&[63, 32, 0]);
        let palette = Palette::parse("TEST.PAL", &bytes).unwrap();
        assert_eq!(palette.colors[1], [255, 130, 0]);
        assert_eq!(
            palette.rgba(&[1, 0], true),
            vec![255, 130, 0, 255, 0, 0, 0, 0]
        );
    }
}
