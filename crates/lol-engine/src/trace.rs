// Lands-of-Lore-RS, a fan-made engine for Lands of Lore: The Throne of Chaos.
// Copyright (C) 2026 The Lands-of-Lore-RS authors.
//
// This program is free software: you can redistribute it and/or modify it under the terms of
// the GNU General Public License as published by the Free Software Foundation, either version 3
// of the License, or (at your option) any later version. It is distributed WITHOUT ANY
// WARRANTY; see the GNU General Public License (the file LICENSE) for details.
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running a script without the game, only noting what it asks of it: for looking at the
//! levels' scripts (`lol-export skript LEVEL1.INI`).

use lol_data::emc::{Args, Host, Script, State};
use lol_data::tables::SCRIPT_FUNCTIONS;

/// One call of the game: its function's name and the first arguments (as numbers, and as the
/// script's strings where one fits).
#[derive(Debug, Clone)]
pub struct Call {
    pub id: u8,
    pub name: &'static str,
    pub args: Vec<i16>,
    pub strings: Vec<Option<String>>,
}

impl std::fmt::Display for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = if self.name.is_empty() {
            format!("#{}", self.id)
        } else {
            self.name.to_string()
        };
        let args: Vec<String> = self
            .args
            .iter()
            .zip(&self.strings)
            .map(|(a, s)| match s {
                Some(s) => format!("{a} \"{s}\""),
                None => a.to_string(),
            })
            .collect();
        write!(f, "{name}({})", args.join(", "))
    }
}

/// Notes every call; every call answers 0.
#[derive(Default)]
pub struct Tracer {
    pub calls: Vec<Call>,
}

impl Host for Tracer {
    fn sys_call(&mut self, id: u8, args: &Args) -> i16 {
        let count = 6.min(lol_data::emc::STACK_SIZE - 1 - args.state.sp);
        let values: Vec<i16> = (0..count).map(|i| args.pos(i)).collect();
        let texts = args.script.texts();
        let strings = values
            .iter()
            .map(|&v| {
                (v >= 0)
                    .then(|| texts.get(v as usize))
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
            .collect();
        self.calls.push(Call {
            id,
            name: SCRIPT_FUNCTIONS.get(id as usize).copied().unwrap_or(""),
            args: values,
            strings,
        });
        0
    }
}

/// The calls a function of a script makes, running it once.
pub fn trace(script: &Script, function: usize) -> Vec<Call> {
    let mut state = State::default();
    let mut tracer = Tracer::default();
    if script.start(&mut state, function) {
        script.run(&mut state, &mut tracer, 100_000);
    }
    tracer.calls
}
