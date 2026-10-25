//! What the game needs of the computer it runs on, as ScummVM's `OSystem`: the picture on the
//! monitor (320×200 colour indices, the palette, the mouse pointer), the keyboard and mouse,
//! the clock, and the sound card ([`crate::audio`]).
//!
//! The game runs in a thread of its own and waits as the original did; the window (Bevy) shows
//! what the game last put on the monitor and passes on the events. Both share [`Shared`]. For
//! test runs without a window, time can be virtual: waiting then takes no time at all.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::audio::Mixer;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 200;

/// Keys the game knows besides letters and digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Char(u8),
    Escape,
    Enter,
    Backspace,
    Space,
    Tab,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    F(u8),
    /// The number pad's keys 0–9.
    Pad(u8),
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    KeyDown {
        key: Key,
        /// The character typed, if any (with shift applied).
        ascii: u8,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    MouseMove {
        x: i32,
        y: i32,
    },
    /// Button 0 left, 1 right.
    MouseDown {
        button: u8,
        x: i32,
        y: i32,
    },
    MouseUp {
        button: u8,
        x: i32,
        y: i32,
    },
    Quit,
}

/// The mouse pointer: its picture (colour indices, `key` transparent) and hot spot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub key: u8,
    pub hot_x: i32,
    pub hot_y: i32,
}

/// What the monitor shows.
#[derive(Debug, Clone)]
pub struct Frame {
    pub pixels: Vec<u8>,
    /// 256 colours, 8 bits each.
    pub palette: [[u8; 3]; 256],
    pub cursor: Option<Cursor>,
    pub cursor_visible: bool,
    /// The picture shifted (`setShakePos`).
    pub shake: (i32, i32),
    /// Counts each change, for the window to notice.
    pub serial: u64,
}

impl Frame {
    /// The picture as RGBA, with the mouse pointer drawn at `mouse` if asked.
    pub fn rgba(&self, mouse: Option<(i32, i32)>) -> Vec<u8> {
        let mut out = Vec::with_capacity(WIDTH * HEIGHT * 4);
        let (sx, sy) = self.shake;
        for y in 0..HEIGHT as i32 {
            for x in 0..WIDTH as i32 {
                let (px, py) = (x - sx, y - sy);
                let c = if (0..WIDTH as i32).contains(&px) && (0..HEIGHT as i32).contains(&py) {
                    self.pixels[py as usize * WIDTH + px as usize]
                } else {
                    0
                };
                let [r, g, b] = self.palette[c as usize];
                out.extend_from_slice(&[r, g, b, 255]);
            }
        }
        if let (Some((mx, my)), Some(cursor), true) = (mouse, &self.cursor, self.cursor_visible) {
            for cy in 0..cursor.height {
                for cx in 0..cursor.width {
                    let c = cursor.pixels[cy * cursor.width + cx];
                    if c == cursor.key {
                        continue;
                    }
                    let x = mx - cursor.hot_x + cx as i32;
                    let y = my - cursor.hot_y + cy as i32;
                    if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
                        let at = (y as usize * WIDTH + x as usize) * 4;
                        let [r, g, b] = self.palette[c as usize];
                        out[at..at + 3].copy_from_slice(&[r, g, b]);
                    }
                }
            }
        }
        out
    }
}

/// What the game thread and the window share.
pub struct Shared {
    pub frame: Mutex<Frame>,
    pub events: Mutex<VecDeque<Event>>,
    /// The mouse's place on the 320×200 screen (x, y in 16 bits each).
    pub mouse: AtomicU32,
    pub quit: AtomicBool,
    pub mixer: Arc<Mutex<Mixer>>,
}

impl Shared {
    pub fn new(rate: u32) -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(Frame {
                pixels: vec![0; WIDTH * HEIGHT],
                palette: [[0; 3]; 256],
                cursor: None,
                cursor_visible: false,
                shake: (0, 0),
                serial: 0,
            }),
            events: Mutex::new(VecDeque::new()),
            mouse: AtomicU32::new(0),
            quit: AtomicBool::new(false),
            mixer: Arc::new(Mutex::new(Mixer::new(rate))),
        })
    }

    /// The window passes on an event.
    pub fn push_event(&self, event: Event) {
        match event {
            Event::MouseMove { x, y }
            | Event::MouseDown { x, y, .. }
            | Event::MouseUp { x, y, .. } => {
                self.set_mouse(x, y);
            }
            Event::Quit => self.quit.store(true, Ordering::SeqCst),
            _ => {}
        }
        if let Ok(mut events) = self.events.lock() {
            events.push_back(event);
        }
    }

    pub fn set_mouse(&self, x: i32, y: i32) {
        let x = x.clamp(0, WIDTH as i32 - 1) as u32;
        let y = y.clamp(0, HEIGHT as i32 - 1) as u32;
        self.mouse.store(x | y << 16, Ordering::SeqCst);
    }

    pub fn mouse(&self) -> (i32, i32) {
        let v = self.mouse.load(Ordering::SeqCst);
        ((v & 0xffff) as i32, (v >> 16) as i32)
    }
}

/// What a test run does while the game waits (virtual time): it sees the time and the
/// monitor and may pass on input.
pub type Hook = Box<dyn FnMut(u32, &Shared) + Send>;

/// The game's side of the computer.
pub struct System {
    pub shared: Arc<Shared>,
    start: Instant,
    /// Virtual milliseconds (test runs): waiting advances them instead of sleeping.
    virtual_ms: Cell<Option<u64>>,
    hook: RefCell<Option<Hook>>,
    /// Calls of `millis` since the game last waited. Some loops of the original wait for
    /// input without ever waiting for time (the CPU's own speed paced them): every so many
    /// calls a millisecond passes, so they neither hang a test run nor hog a core.
    polls: Cell<u64>,
}

/// Calls of `millis` without waiting after which a millisecond passes.
const POLLS_PER_MS: u64 = 2000;

impl System {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            start: Instant::now(),
            virtual_ms: Cell::new(None),
            hook: RefCell::new(None),
            polls: Cell::new(0),
        }
    }

    /// A system whose clock only advances when the game waits.
    pub fn with_virtual_time(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            start: Instant::now(),
            virtual_ms: Cell::new(Some(0)),
            hook: RefCell::new(None),
            polls: Cell::new(0),
        }
    }

    /// A test run's driver, called whenever virtual time passes.
    pub fn set_hook(&mut self, hook: Hook) {
        *self.hook.get_mut() = Some(hook);
    }

    pub fn millis(&self) -> u32 {
        self.polled(1);
        match self.virtual_ms.get() {
            Some(ms) => ms as u32,
            None => self.start.elapsed().as_millis() as u32,
        }
    }

    pub fn delay_millis(&mut self, ms: u32) {
        self.polls.set(0);
        self.pass(ms);
    }

    /// Counts calls that wait for something without waiting for time.
    fn polled(&self, weight: u64) {
        let before = self.polls.get();
        let n = before + weight;
        self.polls.set(n);
        if n / POLLS_PER_MS != before / POLLS_PER_MS {
            self.pass(1);
        }
    }

    fn pass(&self, ms: u32) {
        match self.virtual_ms.get() {
            Some(v) => {
                let now = v + ms as u64;
                self.virtual_ms.set(Some(now));
                if let Ok(mut mixer) = self.shared.mixer.lock() {
                    mixer.advance_virtual(ms);
                }
                if let Ok(mut hook) = self.hook.try_borrow_mut()
                    && let Some(h) = hook.as_mut()
                {
                    h(now as u32, &self.shared);
                }
            }
            None => std::thread::sleep(Duration::from_millis(ms as u64)),
        }
    }

    pub fn should_quit(&self) -> bool {
        self.shared.quit.load(Ordering::SeqCst)
    }

    pub fn quit(&self) {
        self.shared.quit.store(true, Ordering::SeqCst);
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        // Asking for input weighs more than asking for the time (a loop that only waits for
        // a click lets time pass sooner).
        self.polled(50);
        self.shared.events.lock().ok()?.pop_front()
    }

    pub fn mouse_pos(&self) -> (i32, i32) {
        self.shared.mouse()
    }

    pub fn warp_mouse(&self, x: i32, y: i32) {
        self.shared.set_mouse(x, y);
    }

    /// Shows a picture on the monitor.
    pub fn copy_to_screen(&self, pixels: &[u8]) {
        if let Ok(mut frame) = self.shared.frame.lock() {
            frame.pixels.copy_from_slice(&pixels[..WIDTH * HEIGHT]);
            frame.serial += 1;
        }
    }

    /// Sets colours from 6-bit VGA values, `first` onward.
    pub fn set_palette(&self, vga: &[u8], first: usize) {
        if let Ok(mut frame) = self.shared.frame.lock() {
            for (i, rgb) in vga.as_chunks::<3>().0.iter().enumerate() {
                if let Some(entry) = frame.palette.get_mut(first + i) {
                    let w = |v: u8| ((v as u32 & 0x3f) * 255 / 63) as u8;
                    *entry = [w(rgb[0]), w(rgb[1]), w(rgb[2])];
                }
            }
            frame.serial += 1;
        }
    }

    pub fn set_cursor(&self, cursor: Option<Cursor>) {
        if let Ok(mut frame) = self.shared.frame.lock() {
            frame.cursor = cursor;
            frame.serial += 1;
        }
    }

    pub fn show_cursor(&self, visible: bool) {
        if let Ok(mut frame) = self.shared.frame.lock() {
            frame.cursor_visible = visible;
            frame.serial += 1;
        }
    }

    pub fn set_shake(&self, x: i32, y: i32) {
        if let Ok(mut frame) = self.shared.frame.lock() {
            frame.shake = (x, y);
            frame.serial += 1;
        }
    }

    pub fn mixer(&self) -> std::sync::MutexGuard<'_, Mixer> {
        self.shared.mixer.lock().unwrap_or_else(|e| e.into_inner())
    }
}
