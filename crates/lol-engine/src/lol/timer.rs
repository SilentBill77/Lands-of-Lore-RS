//! ScummVM's `TimerManager`: timers counting in ticks, each running a function of the game
//! when due; paused singly while the game shows something else (`pauseSingleTimer`).

#[derive(Debug, Clone, Copy)]
pub struct TimerEntry {
    pub id: u8,
    pub countdown: i32,
    /// Bit 0 enabled, bit 1 paused.
    pub enabled: u8,
    pub last_update: i64,
    pub next_run: u32,
    pub pause_start: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Timers {
    pub list: Vec<TimerEntry>,
    pub next_run: u32,
}

impl Timers {
    pub fn add(&mut self, id: u8, countdown: i32, enabled: bool) {
        if self.list.iter().any(|t| t.id == id) {
            return;
        }
        self.list.push(TimerEntry {
            id,
            countdown,
            enabled: enabled as u8,
            last_update: 0,
            next_run: 0,
            pause_start: 0,
        });
    }

    fn get_mut(&mut self, id: u8) -> Option<&mut TimerEntry> {
        self.list.iter_mut().find(|t| t.id == id)
    }

    pub fn reset_next_run(&mut self) {
        self.next_run = 0;
    }

    pub fn set_countdown(&mut self, id: u8, countdown: i32, now: u32, tick: u32) {
        let mut next = None;
        if let Some(t) = self.get_mut(id) {
            t.countdown = countdown;
            if countdown >= 0 {
                t.last_update = now as i64;
                t.next_run = now + countdown as u32 * tick;
                if t.enabled & 2 != 0 {
                    t.pause_start = now;
                }
                next = Some(t.next_run);
            }
        }
        if let Some(n) = next {
            self.next_run = self.next_run.min(n);
        }
    }

    pub fn set_delay(&mut self, id: u8, countdown: i32) {
        if let Some(t) = self.get_mut(id) {
            t.countdown = countdown;
        }
    }

    pub fn get_delay(&self, id: u8) -> i32 {
        self.list
            .iter()
            .find(|t| t.id == id)
            .map_or(-1, |t| t.countdown)
    }

    pub fn set_next_run(&mut self, id: u8, next_run: u32, now: u32) {
        if let Some(t) = self.get_mut(id) {
            if t.enabled & 2 != 0 {
                t.pause_start = now;
            }
            t.next_run = next_run;
        }
    }

    pub fn get_next_run(&self, id: u8) -> u32 {
        self.list
            .iter()
            .find(|t| t.id == id)
            .map_or(u32::MAX, |t| t.next_run)
    }

    pub fn pause_single(&mut self, id: u8, pause: bool, now: u32) {
        let mut reset = false;
        if let Some(t) = self.get_mut(id) {
            if pause {
                t.pause_start = now;
                t.enabled |= 2;
            } else if t.enabled & 2 != 0 {
                let elapsed = now.wrapping_sub(t.pause_start);
                t.enabled &= !2;
                t.last_update += elapsed as i64;
                t.next_run = t.next_run.wrapping_add(elapsed);
                t.pause_start = 0;
                reset = true;
            }
        }
        if reset {
            self.reset_next_run();
        }
    }

    pub fn is_enabled(&self, id: u8) -> bool {
        self.list.iter().any(|t| t.id == id && t.enabled & 1 != 0)
    }

    pub fn enable(&mut self, id: u8) {
        if let Some(t) = self.get_mut(id) {
            t.enabled |= 1;
        }
    }

    pub fn disable(&mut self, id: u8) {
        if let Some(t) = self.get_mut(id) {
            t.enabled &= !1;
        }
    }

    /// The timers due now (`update` before calling their functions); the caller runs each and
    /// then calls [`Timers::ran`].
    pub fn due(&mut self, now: u32) -> Vec<u8> {
        if now < self.next_run {
            return Vec::new();
        }
        self.next_run = self.next_run.wrapping_add(99_999).max(now + 99_999);
        let mut due = Vec::new();
        for t in &self.list {
            if t.enabled == 1 && t.countdown >= 0 && t.next_run <= now {
                due.push(t.id);
            }
        }
        due
    }

    /// After a timer's function ran.
    pub fn ran(&mut self, id: u8, now: u32, tick: u32) {
        if let Some(t) = self.get_mut(id) {
            t.last_update = now as i64;
            t.next_run = now + t.countdown.max(0) as u32 * tick;
        }
        self.recompute_next();
    }

    /// The earliest run of the enabled timers.
    pub fn recompute_next(&mut self) {
        let mut next = self.next_run;
        for t in &self.list {
            if t.enabled == 1 && t.countdown >= 0 {
                next = next.min(t.next_run);
            }
        }
        self.next_run = next;
    }
}
