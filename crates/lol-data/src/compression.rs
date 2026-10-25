//! Westwood's LCW compression ("format 80"), with which the game packs its pictures (`CPS`),
//! figures (`SHP`) and the frames of its animations (`WSA`, together with the XOR deltas of
//! [`xor_delta`]).
//!
//! A stream of commands, each copying bytes to the output:
//! - `0ccc pppp pppppppp`: `ccc + 3` bytes from `p` bytes back in the output;
//! - `10cc cccc`: the next `c` bytes as they are (`c` = 0 ends the stream);
//! - `11cc cccc pppppppp pppppppp`: `c + 3` bytes from position `p` of the output;
//! - `FE cccc vv`: `c` times the byte `v`;
//! - `FF cccc pppp`: `c` bytes from position `p` of the output.

use crate::error::{Error, Result};

/// Unpacks an LCW stream into `size` bytes (a shorter stream leaves the rest zero).
pub fn lcw(file: &str, input: &[u8], size: usize) -> Result<Vec<u8>> {
    let bad = |what: &str, at: usize| Error::format(file, format!("LCW: {what} bei Byte {at}"));
    let mut out: Vec<u8> = Vec::with_capacity(size);
    let mut i = 0;
    let byte = |i: usize| input.get(i).copied().ok_or_else(|| bad("Daten enden", i));
    let word = |i: usize| -> Result<usize> { Ok(byte(i)? as usize | (byte(i + 1)? as usize) << 8) };
    // Copies byte by byte: source and target may overlap (a run repeating itself).
    let copy = |out: &mut Vec<u8>, from: usize, count: usize, at: usize| -> Result<()> {
        if from >= out.len() && count > 0 {
            return Err(bad("Verweis hinter das Ende", at));
        }
        for k in 0..count {
            let b = out[from + k];
            out.push(b);
        }
        Ok(())
    };
    while i < input.len() && out.len() < size {
        let at = i;
        let command = byte(i)?;
        i += 1;
        if command & 0x80 == 0 {
            let count = ((command >> 4) & 7) as usize + 3;
            let back = ((command & 0x0f) as usize) << 8 | byte(i)? as usize;
            i += 1;
            let from = out
                .len()
                .checked_sub(back)
                .filter(|_| back > 0)
                .ok_or_else(|| bad("Verweis vor den Anfang", at))?;
            copy(&mut out, from, count, at)?;
        } else if command & 0x40 == 0 {
            let count = (command & 0x3f) as usize;
            if count == 0 {
                break;
            }
            let literal = input
                .get(i..i + count)
                .ok_or_else(|| bad("Daten enden", i))?;
            out.extend_from_slice(literal);
            i += count;
        } else if command == 0xfe {
            let count = word(i)?;
            let value = byte(i + 2)?;
            i += 3;
            out.extend(std::iter::repeat_n(value, count));
        } else {
            let (count, from) = if command == 0xff {
                let pair = (word(i)?, word(i + 2)?);
                i += 4;
                pair
            } else {
                let pair = ((command & 0x3f) as usize + 3, word(i)?);
                i += 2;
                pair
            };
            copy(&mut out, from, count, at)?;
        }
    }
    out.resize(size, 0);
    Ok(out)
}

/// Unpacks Westwood's run lengths ("format 3"): a byte `c`
/// read as signed, then for `c` > 0 that many bytes as they are, for `c` < 0 the next byte
/// `-c` times, for `c` = 0 a count (u16, big-endian) and the byte to repeat.
pub fn rle(file: &str, input: &[u8], size: usize) -> Result<Vec<u8>> {
    let bad = |at: usize| Error::format(file, format!("RLE: Daten enden bei Byte {at}"));
    let mut out = Vec::with_capacity(size);
    let mut i = 0;
    while i < input.len() && out.len() < size {
        let code = input[i] as i8;
        i += 1;
        match code {
            0 => {
                let run = input.get(i..i + 3).ok_or_else(|| bad(i))?;
                let count = (run[0] as usize) << 8 | run[1] as usize;
                out.extend(std::iter::repeat_n(run[2], count));
                i += 3;
            }
            c if c < 0 => {
                let value = *input.get(i).ok_or_else(|| bad(i))?;
                out.extend(std::iter::repeat_n(value, c.unsigned_abs() as usize));
                i += 1;
            }
            c => {
                let count = c as usize;
                out.extend_from_slice(input.get(i..i + count).ok_or_else(|| bad(i))?);
                i += count;
            }
        }
    }
    out.resize(size, 0);
    Ok(out)
}

/// Applies an XOR delta ("format 40", the frames of a `WSA` after their LCW) to `target`:
/// a stream of commands, each XOR-ing bytes into the target or skipping over it:
/// - `00 cc vv`: `c` times the byte `v`;
/// - `0ccc cccc` (`c` > 0): the next `c` bytes;
/// - `1ccc cccc` (`c` > 0): skip `c` bytes;
/// - `80 cccc` (u16): `c` = 0 ends the stream; else `10cc …` skip `c`, `1000 … c` the next
///   `c` bytes, `1100 … c vv` `c` times the byte `v`.
pub fn xor_delta(file: &str, input: &[u8], target: &mut [u8]) -> Result<()> {
    let bad = |what: &str, at: usize| Error::format(file, format!("Delta: {what} bei Byte {at}"));
    let byte = |i: usize| input.get(i).copied().ok_or_else(|| bad("Daten enden", i));
    let mut i = 0;
    let mut at = 0usize;
    let fill = |target: &mut [u8], at: &mut usize, count: usize, value: u8, cmd: usize| {
        let place = target
            .get_mut(*at..*at + count)
            .ok_or_else(|| bad("Ziel überschritten", cmd))?;
        place.iter_mut().for_each(|t| *t ^= value);
        *at += count;
        Ok::<(), Error>(())
    };
    let copy = |target: &mut [u8], at: &mut usize, from: usize, count: usize, cmd: usize| {
        let bytes = input
            .get(from..from + count)
            .ok_or_else(|| bad("Daten enden", from))?;
        let place = target
            .get_mut(*at..*at + count)
            .ok_or_else(|| bad("Ziel überschritten", cmd))?;
        place.iter_mut().zip(bytes).for_each(|(t, b)| *t ^= b);
        *at += count;
        Ok::<(), Error>(())
    };
    loop {
        let cmd = i;
        let code = byte(i)?;
        i += 1;
        if code == 0 {
            let (count, value) = (byte(i)? as usize, byte(i + 1)?);
            i += 2;
            fill(target, &mut at, count, value, cmd)?;
        } else if code & 0x80 == 0 {
            let count = code as usize;
            copy(target, &mut at, i, count, cmd)?;
            i += count;
        } else if code != 0x80 {
            at += (code & 0x7f) as usize;
        } else {
            let sub = byte(i)? as usize | (byte(i + 1)? as usize) << 8;
            i += 2;
            if sub == 0 {
                return Ok(());
            } else if sub & 0x8000 == 0 {
                at += sub;
            } else if sub & 0x4000 == 0 {
                let count = sub & 0x3fff;
                copy(target, &mut at, i, count, cmd)?;
                i += count;
            } else {
                let value = byte(i)?;
                i += 1;
                fill(target, &mut at, sub & 0x3fff, value, cmd)?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_an_xor_delta() {
        let mut target = [1u8; 12];
        let stream = [
            0x02, 0x10, 0x20, // XOR the next two bytes
            0x81, // skip one
            0x00, 0x02, 0x03, // XOR 3 twice
            0x80, 0x02, 0x00, // skip two
            0x80, 0x02, 0xc0, 0x07, // XOR 7 twice
            0x80, 0x01, 0x80, 0xff, // XOR the next byte
            0x80, 0x00, 0x00, // end
        ];
        xor_delta("TEST", &stream, &mut target).unwrap();
        assert_eq!(target, [0x11, 0x21, 1, 2, 2, 1, 1, 6, 6, 0xfe, 1, 1]);
    }

    #[test]
    fn a_delta_stays_inside_its_target() {
        let mut target = [0u8; 2];
        assert!(xor_delta("TEST", &[0x00, 0x03, 0x01, 0x80, 0, 0], &mut target).is_err());
    }

    #[test]
    fn unpacks_run_lengths() {
        let stream = [2, b'a', b'b', 0xfd, b'c', 0, 0x00, 0x02, b'd'];
        assert_eq!(&rle("TEST", &stream, 8).unwrap(), b"abcccdd\0");
    }

    #[test]
    fn unpacks_every_command() {
        let stream = [
            0x83, b'a', b'b', b'c', // three literal bytes
            0x00, 0x03, // three bytes from three back: "abc"
            0xc0, 0x01, 0x00, // three bytes from position 1: "bca"
            0xfe, 0x02, 0x00, b'z', // "zz"
            0xff, 0x04, 0x00, 0x00, 0x00, // four bytes from position 0: "abca"
            0x80, // end
        ];
        let out = lcw("TEST", &stream, 17).unwrap();
        assert_eq!(&out, b"abcabcbcazzabca\0\0");
    }

    #[test]
    fn a_run_may_repeat_itself() {
        // One literal byte, then nine bytes from one back.
        let out = lcw("TEST", &[0x81, b'x', 0x60, 0x01, 0x80], 10).unwrap();
        assert_eq!(&out, b"xxxxxxxxxx");
    }

    #[test]
    fn rejects_references_before_the_start() {
        assert!(lcw("TEST", &[0x00, 0x05, 0x80], 8).is_err());
    }
}
