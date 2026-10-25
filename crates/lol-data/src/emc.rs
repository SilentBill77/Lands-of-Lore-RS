//! Westwood's scripts (`EMC2`: the levels' `LEVELn.INI` and `LEVELn.INF`, and others) and the
//! machine that runs them, as ScummVM's `EMCInterpreter` (Kyra engine).
//!
//! The file is an IFF `FORM` of type `EMC2` with the chunks `TEXT` (the strings: offsets as
//! u16 BE, then the NUL-terminated texts), `ORDR` (per function the offset of its code in
//! `DATA`, u16 BE, `FFFF` none) and `DATA` (the code, u16 BE words).
//!
//! A word of code: bit 15 set: jump to its lower 15 bits; else the opcode in bits 8–12, and
//! the parameter in the low byte (bit 14, signed), in the next word (bit 13) or none. The
//! machine has a stack of 100 values (growing down), a base pointer, 30 registers and a return
//! value. The game's functions (`sysCall`) read their arguments from the stack's top
//! ([`Args::pos`]); a string argument is an index into `TEXT` ([`Args::string`]).

use crate::error::{Error, Result};

pub const STACK_SIZE: usize = 100;
const STACK_LAST: usize = STACK_SIZE - 1;

#[derive(Debug, Clone)]
pub struct Script {
    pub name: String,
    text: Vec<u8>,
    ordr: Vec<u16>,
    data: Vec<u16>,
}

/// A running script: where it is and its stack.
#[derive(Debug, Clone)]
pub struct State {
    pub ip: Option<usize>,
    pub ret: i16,
    pub sp: usize,
    pub bp: isize,
    pub regs: [i16; 30],
    pub stack: [i16; STACK_SIZE],
}

impl Default for State {
    /// As `EMCInterpreter::init`.
    fn default() -> Self {
        Self {
            ip: None,
            ret: 0,
            sp: STACK_LAST,
            bp: STACK_SIZE as isize + 1,
            regs: [0; 30],
            stack: [0; STACK_SIZE],
        }
    }
}

/// What a script's call of the game sees: its arguments.
pub struct Args<'a> {
    pub script: &'a Script,
    pub state: &'a State,
}

impl Args<'_> {
    /// The argument `i` (0 the first), as `stackPos`.
    pub fn pos(&self, i: usize) -> i16 {
        self.state
            .stack
            .get(self.state.sp + i)
            .copied()
            .unwrap_or(0)
    }

    /// The argument `i` as a string of the script's `TEXT`, as `stackPosString`.
    pub fn string(&self, i: usize) -> &str {
        self.script.text(self.pos(i) as u16 as usize)
    }
}

/// The game, as the script calls it.
pub trait Host {
    /// Function `id` of the game's table; returns the script's new return value.
    fn sys_call(&mut self, id: u8, args: &Args) -> i16;
}

impl Script {
    pub fn parse(file: &str, bytes: &[u8]) -> Result<Self> {
        let bad = |message: &str| Error::format(file, message);
        if bytes.len() < 12 || &bytes[0..4] != b"FORM" || &bytes[8..12] != b"EMC2" {
            return Err(bad("kein EMC2-Skript"));
        }
        let (mut text, mut ordr, mut data) = (Vec::new(), None, None);
        let mut pos = 12;
        while pos + 8 <= bytes.len() {
            let id = &bytes[pos..pos + 4];
            let size = u32::from_be_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
            let body = bytes
                .get(pos + 8..pos + 8 + size)
                .ok_or_else(|| bad("Abschnitt reicht über das Dateiende"))?;
            let words = || -> Vec<u16> {
                body.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|w| u16::from_be_bytes(*w))
                    .collect()
            };
            match id {
                b"TEXT" => text = body.to_vec(),
                b"ORDR" => ordr = Some(words()),
                b"DATA" => data = Some(words()),
                _ => {}
            }
            pos += 8 + size + (size & 1);
        }
        Ok(Self {
            name: file.to_string(),
            text,
            ordr: ordr.ok_or_else(|| bad("ORDR fehlt"))?,
            data: data.ok_or_else(|| bad("DATA fehlt"))?,
        })
    }

    /// The string of `TEXT` with this index (empty if there is none).
    pub fn text(&self, index: usize) -> &str {
        let at = self
            .text
            .get(2 * index..2 * index + 2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]) as usize);
        let Some(rest) = at.and_then(|at| self.text.get(at..)) else {
            return "";
        };
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        std::str::from_utf8(&rest[..end]).unwrap_or("")
    }

    /// Every string of `TEXT`.
    pub fn texts(&self) -> Vec<&str> {
        let count = self
            .text
            .get(0..2)
            .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]) as usize / 2);
        (0..count).map(|i| self.text(i)).collect()
    }

    /// Whether the script has this function.
    pub fn has_function(&self, function: usize) -> bool {
        self.ordr.get(function).is_some_and(|&o| o != 0xffff)
    }

    /// Readies `state` to run a function, as `init` and `start` (false if there is none).
    pub fn start(&self, state: &mut State, function: usize) -> bool {
        *state = State::default();
        match self.ordr.get(function) {
            Some(&offset) if offset != 0xffff && (offset as usize + 1) < self.data.len() => {
                state.ip = Some(offset as usize + 1);
                true
            }
            _ => false,
        }
    }

    /// The word before where a started function begins (the levels' scripts keep there which
    /// events run the function: `runLevelScriptCustom` checks it against its flags).
    pub fn entry_flags(&self, state: &State) -> u16 {
        state
            .ip
            .and_then(|ip| ip.checked_sub(1))
            .and_then(|i| self.data.get(i))
            .copied()
            .unwrap_or(0)
    }

    /// Runs until the function ends, at most `max_steps` words (a guard against endless
    /// loops). Returns the steps taken.
    pub fn run(&self, state: &mut State, host: &mut dyn Host, max_steps: usize) -> usize {
        let mut steps = 0;
        while state.ip.is_some() && steps < max_steps {
            self.step(state, host);
            steps += 1;
        }
        steps
    }

    /// One word of code, as `EMCInterpreter::run`. A broken script stops (`ip` none).
    pub fn step(&self, state: &mut State, host: &mut dyn Host) {
        let Some(ip) = state.ip else { return };
        let Some(&code) = self.data.get(ip) else {
            state.ip = None;
            return;
        };
        let mut next = ip + 1;
        let (opcode, parameter) = if code & 0x8000 != 0 {
            (0, (code & 0x7fff) as i16)
        } else if code & 0x4000 != 0 {
            ((code >> 8) & 0x1f, code as u8 as i8 as i16)
        } else if code & 0x2000 != 0 {
            let value = self.data.get(next).copied().unwrap_or(0) as i16;
            next += 1;
            ((code >> 8) & 0x1f, value)
        } else {
            ((code >> 8) & 0x1f, 0)
        };
        state.ip = Some(next);
        if self.execute(state, host, opcode as u8, parameter).is_none() {
            state.ip = None;
        }
    }

    /// One instruction; `None` stops the script (as ScummVM's setting `ip` to null, and on
    /// stack accesses out of range).
    fn execute(
        &self,
        state: &mut State,
        host: &mut dyn Host,
        opcode: u8,
        parameter: i16,
    ) -> Option<()> {
        fn at(index: isize) -> Option<usize> {
            (0..STACK_SIZE as isize)
                .contains(&index)
                .then_some(index as usize)
        }
        let push = |state: &mut State, value: i16| -> Option<()> {
            state.sp = state.sp.checked_sub(1)?;
            state.stack[state.sp] = value;
            Some(())
        };
        let pop = |state: &mut State| -> Option<i16> {
            let value = *state.stack.get(state.sp)?;
            state.sp += 1;
            Some(value)
        };
        let p = parameter as isize;
        match opcode {
            // jmp
            0 => state.ip = Some(parameter as u16 as usize),
            // setRetValue
            1 => state.ret = parameter,
            // pushRetOrPos
            2 => match parameter {
                0 => push(state, state.ret)?,
                1 => {
                    let back = state.ip? as i16;
                    push(state, back)?;
                    push(state, state.bp as i16)?;
                    state.bp = state.sp as isize + 2;
                }
                _ => return None,
            },
            // push
            3 | 4 => push(state, parameter)?,
            // pushReg
            5 => push(state, *state.regs.get(parameter as usize)?)?,
            // pushBPNeg, pushBPAdd
            6 => push(state, state.stack[at(state.bp - (p + 2))?])?,
            7 => push(state, state.stack[at(state.bp + p - 1)?])?,
            // popRetOrPos
            8 => match parameter {
                0 => state.ret = pop(state)?,
                1 => {
                    if state.sp >= STACK_LAST {
                        return None;
                    }
                    state.bp = pop(state)? as isize;
                    state.ip = Some(pop(state)? as u16 as usize);
                }
                _ => return None,
            },
            // popReg
            9 => {
                let value = pop(state)?;
                *state.regs.get_mut(parameter as usize)? = value;
            }
            // popBPNeg, popBPAdd
            10 => {
                let value = pop(state)?;
                state.stack[at(state.bp - (p + 2))?] = value;
            }
            11 => {
                let value = pop(state)?;
                state.stack[at(state.bp + p - 1)?] = value;
            }
            // addSP, subSP
            12 => state.sp = at(state.sp as isize + p)?,
            13 => state.sp = at(state.sp as isize - p)?,
            // sysCall
            14 => {
                let args = Args {
                    script: self,
                    state,
                };
                state.ret = host.sys_call(parameter as u8, &args);
            }
            // ifNotJmp
            15 => {
                if pop(state)? == 0 {
                    state.ip = Some((parameter as u16 & 0x7fff) as usize);
                }
            }
            // negate
            16 => {
                let value = *state.stack.get(state.sp)?;
                state.stack[state.sp] = match parameter {
                    0 => (value == 0) as i16,
                    1 => value.wrapping_neg(),
                    2 => !value,
                    _ => return None,
                };
            }
            // eval
            17 => {
                let a = pop(state)?;
                let b = pop(state)?;
                let value = match parameter {
                    0 => (b != 0 && a != 0) as i16,
                    1 => (b != 0 || a != 0) as i16,
                    2 => (a == b) as i16,
                    3 => (a != b) as i16,
                    4 => (a > b) as i16,
                    5 => (a >= b) as i16,
                    6 => (a < b) as i16,
                    7 => (a <= b) as i16,
                    8 => a.wrapping_add(b),
                    9 => b.wrapping_sub(a),
                    10 => a.wrapping_mul(b),
                    11 => b.checked_div(a)?,
                    12 => b.wrapping_shr(a as u32),
                    13 => b.wrapping_shl(a as u32),
                    14 => a & b,
                    15 => a | b,
                    16 => b.checked_rem(a)?,
                    17 => a ^ b,
                    _ => return None,
                };
                push(state, value)?;
            }
            // setRetAndJmp
            18 => {
                if state.sp >= STACK_LAST {
                    return None;
                }
                state.ret = pop(state)?;
                let target = pop(state)? as u16 as usize;
                state.stack[STACK_LAST] = 0;
                state.ip = Some(target);
            }
            _ => return None,
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script file of the given strings and one function of code.
    fn file(texts: &[&str], code: &[u16]) -> Vec<u8> {
        let mut text = Vec::new();
        let mut offset = 2 * texts.len();
        for t in texts {
            text.extend_from_slice(&(offset as u16).to_be_bytes());
            offset += t.len() + 1;
        }
        for t in texts {
            text.extend_from_slice(t.as_bytes());
            text.push(0);
        }
        let mut data = vec![0u16]; // the word before the function (its flags)
        data.extend_from_slice(code);
        let mut out = b"FORM\0\0\0\0EMC2".to_vec();
        for (id, body) in [
            (b"TEXT", text),
            (b"ORDR", 0u16.to_be_bytes().to_vec()),
            (
                b"DATA",
                data.iter()
                    .flat_map(|w| w.to_be_bytes())
                    .collect::<Vec<u8>>(),
            ),
        ] {
            out.extend_from_slice(id);
            out.extend_from_slice(&(body.len() as u32).to_be_bytes());
            out.extend_from_slice(&body);
            if body.len() & 1 == 1 {
                out.push(0);
            }
        }
        out
    }

    struct Recorder(Vec<(u8, String, i16)>);

    impl Host for Recorder {
        fn sys_call(&mut self, id: u8, args: &Args) -> i16 {
            self.0.push((id, args.string(0).to_string(), args.pos(1)));
            7
        }
    }

    #[test]
    fn calls_the_game_with_its_arguments() {
        // push 5; push 1 (the string "KEEP"); sysCall 9; addSP 2; popRetOrPos 1 (end).
        let code = [0x4305, 0x4301, 0x4e09, 0x4c02, 0x4801];
        let script = Script::parse("TEST.INI", &file(&["LEVEL1", "KEEP"], &code)).unwrap();
        assert_eq!(script.texts(), vec!["LEVEL1", "KEEP"]);
        let mut state = State::default();
        assert!(script.start(&mut state, 0));
        let mut host = Recorder(Vec::new());
        script.run(&mut state, &mut host, 100);
        assert_eq!(host.0, vec![(9, "KEEP".to_string(), 5)]);
        assert_eq!(state.ret, 7);
        assert!(state.ip.is_none());
    }

    #[test]
    fn computes_and_branches() {
        // push 2; push 3; eval 10 (2·3); popReg 0; pushReg 0; push 6; eval 2 (==);
        // ifNotJmp to the end; setRetValue 1; end.
        let code = [
            0x4302, 0x4303, 0x510a, 0x4900, 0x4500, 0x4306, 0x5102, 0x2f00, 0x000b, 0x4101, 0x4801,
        ];
        let script = Script::parse("TEST.INF", &file(&[], &code)).unwrap();
        let mut state = State::default();
        script.start(&mut state, 0);
        script.run(&mut state, &mut Recorder(Vec::new()), 100);
        assert_eq!(state.regs[0], 6);
        assert_eq!(state.ret, 1);
    }
}
