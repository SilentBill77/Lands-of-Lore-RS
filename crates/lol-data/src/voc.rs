//! Creative's voice files (`VOC`): the speech in the `TLK` archives, the sounds of
//! `VOC.PAK` and the intro's voices (`INTROVOC.PAK`).
//!
//! Header: `Creative Voice File` and `1A`, the header's size (u16, 26), version and check
//! (u16 each). Then blocks: a type (u8), the length (u24) and the body:
//! - 1: sound: the rate as `256 − 1 000 000 / rate` (u8), the coding (u8: 0 for 8-bit
//!   unsigned samples), the samples;
//! - 2: more samples at the same rate;
//! - 3: silence: its length less one (u16) and the rate;
//! - 9: sound in the newer form: the rate (u32), bits per sample (u8), channels (u8), the
//!   coding (u16: 0 8-bit unsigned, 4 16-bit signed), four bytes, the samples;
//! - 0 ends the file; the others (text, repeats, extended rates) carry no samples.

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    /// Samples a second.
    pub rate: u32,
    /// Mono, 16 bit.
    pub samples: Vec<i16>,
}

pub fn parse(file: &str, bytes: &[u8]) -> Result<Sound> {
    let bad = |message: String| Error::format(file, message);
    if bytes.len() < 26 || !bytes.starts_with(b"Creative Voice File\x1a") {
        return Err(bad("keine Creative-Voice-Datei".into()));
    }
    let header = u16::from_le_bytes([bytes[20], bytes[21]]) as usize;
    let mut pos = header;
    let mut rate = 0u32;
    let mut samples: Vec<i16> = Vec::new();
    let eight_bit = |data: &[u8], samples: &mut Vec<i16>| {
        samples.extend(data.iter().map(|&b| ((b as i16) - 128) << 8));
    };
    while pos < bytes.len() {
        let kind = bytes[pos];
        if kind == 0 {
            break;
        }
        let head = bytes
            .get(pos + 1..pos + 4)
            .ok_or_else(|| bad(format!("Block bei Byte {pos} ohne Länge")))?;
        let len = head[0] as usize | (head[1] as usize) << 8 | (head[2] as usize) << 16;
        let body = &bytes[(pos + 4).min(bytes.len())..(pos + 4 + len).min(bytes.len())];
        pos += 4 + len;
        match kind {
            1 => {
                let [divisor, coding, data @ ..] = body else {
                    return Err(bad("Klangblock zu kurz".into()));
                };
                if *coding != 0 {
                    return Err(bad(format!("Kodierung {coding} nicht unterstützt")));
                }
                rate = 1_000_000 / (256 - *divisor as u32);
                eight_bit(data, &mut samples);
            }
            2 => eight_bit(body, &mut samples),
            3 if body.len() >= 3 => {
                let length = u16::from_le_bytes([body[0], body[1]]) as usize + 1;
                if rate == 0 {
                    rate = 1_000_000 / (256 - body[2] as u32);
                }
                samples.extend(std::iter::repeat_n(0, length));
            }
            9 if body.len() >= 12 => {
                rate = u32::from_le_bytes(body[0..4].try_into().unwrap());
                let (bits, channels) = (body[4], body[5]);
                let coding = u16::from_le_bytes([body[6], body[7]]);
                let data = &body[12..];
                match (coding, bits, channels) {
                    (0, 8, 1) => eight_bit(data, &mut samples),
                    (4, 16, 1) => samples.extend(
                        data.as_chunks::<2>()
                            .0
                            .iter()
                            .map(|c| i16::from_le_bytes(*c)),
                    ),
                    _ => {
                        return Err(bad(format!(
                            "Kodierung {coding} mit {bits} Bit, {channels} Kanälen nicht unterstützt"
                        )));
                    }
                }
            }
            _ => {}
        }
    }
    if rate == 0 {
        return Err(bad("keine Klangdaten".into()));
    }
    Ok(Sound { rate, samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_eight_bit_samples() {
        let mut bytes = b"Creative Voice File\x1a".to_vec();
        bytes.extend_from_slice(&[26, 0, 0x0a, 0x01, 0x29, 0x11]);
        // 256 − 1 000 000 / 10 000 = 156; two samples, then two more, then the end.
        bytes.extend_from_slice(&[1, 4, 0, 0, 156, 0, 128, 255]);
        bytes.extend_from_slice(&[2, 2, 0, 0, 0, 129]);
        bytes.push(0);
        let sound = parse("TEST.VOC", &bytes).unwrap();
        assert_eq!(sound.rate, 10_000);
        assert_eq!(sound.samples, vec![0, 127 << 8, -128 << 8, 1 << 8]);
    }
}
