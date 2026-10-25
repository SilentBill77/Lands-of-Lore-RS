//! Lands-of-Lore-RS: Lands of Lore: The Throne of Chaos in Rust.
//!
//! The game (`lol-engine`) runs in a thread of its own as the original did, waiting as it
//! goes; it draws a 320×200 screen of colour indices with a palette. This program shows that
//! screen, stretched to 4:3 as under DOS, with the game's mouse pointer, passes on the keys and
//! the mouse, and plays the sound the game mixes. The switch to the coming HD graphics is
//! prepared (`renderer.rs`); F2 stays the game's key.

mod options;
mod renderer;
mod sound;

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key as LogicalKey, KeyboardInput};
use bevy::input::mouse::MouseButtonInput;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{CursorOptions, PrimaryWindow, WindowMode, WindowResolution};
use lol_data::assets::AssetStore;
use lol_data::paths;
use lol_engine::lol::Lol;
use lol_engine::lol::res::Res as GameRes;
use lol_engine::platform::{Event as GameEvent, HEIGHT, Key, Shared, System, WIDTH};
use lol_engine::screen::Screen;

use options::Options;

/// DOS showed the 320×200 screen on a 4:3 monitor: each pixel 1.2 times as tall as wide.
const TALL_PIXEL: f32 = 1.2;
/// The mixer's rate.
const RATE: u32 = 44_100;

#[derive(Resource)]
struct Game {
    shared: Arc<Shared>,
    /// The frame and pointer last shown.
    shown: (u64, (i32, i32)),
}

#[derive(Resource)]
struct ScreenImage(Handle<Image>);

#[derive(Component)]
struct OriginalScreen;

/// Which graphics are shown (the original's or, when ready, the HD ones).
#[derive(Resource)]
struct GraphicsMode(renderer::Renderers);

fn main() {
    let options = match Options::from_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let copy = match &options.data {
        Some(path) => paths::installation_in(path),
        None => paths::find(),
    };
    let Some(copy) = copy else {
        match &options.data {
            Some(path) => eprintln!(
                r"In {} liegt keine CD-Fassung von Lands of Lore (GAME.DAT oder DATA\GENERAL.PAK).",
                path.display()
            ),
            None => eprintln!(
                r#"Keine Installation von Lands of Lore gefunden. Bitte den Ordner angeben:
  lol-game --daten "D:\GOG Games\Lands Of Lore"
(oder die Umgebungsvariable {} setzen)."#,
                paths::LOL_ENV
            ),
        }
        std::process::exit(1);
    };
    println!(
        "Lands of Lore – {} ({})",
        copy.name(),
        options.language.label()
    );
    let shared = Shared::new(RATE);
    let data_dir = options::data_dir();
    let settings_path = data_dir.as_ref().map(|d| d.join("einstellungen.txt"));
    let settings = settings_path.as_ref().and_then(options::read_settings);
    {
        let shared = shared.clone();
        let language = options.language;
        let load = options.load;
        std::thread::Builder::new()
            .name("spiel".into())
            .stack_size(64 << 20)
            .spawn(move || {
                let store = match AssetStore::open(&copy, language) {
                    Ok(store) => store,
                    Err(err) => {
                        eprintln!("{}: {err}", copy.dir.display());
                        shared.quit.store(true, std::sync::atomic::Ordering::SeqCst);
                        return;
                    }
                };
                let screen = Screen::new(System::new(shared.clone()));
                let mut lol = Lol::new(screen, GameRes::new(store));
                lol.save_dir = data_dir.map(|d| d.join("spielstaende"));
                if let Some(s) = settings {
                    lol.apply_settings(s);
                }
                if let Some(path) = settings_path {
                    lol.on_settings_changed =
                        Some(Box::new(move |s| options::write_settings(&path, &s)));
                }
                lol.init();
                if let Some(slot) = load {
                    lol.game_to_load = slot;
                }
                lol.go();
                shared.quit.store(true, std::sync::atomic::Ordering::SeqCst);
            })
            .expect("game thread");
    }

    let mut window = Window {
        title: "Lands-of-Lore-RS".into(),
        resolution: WindowResolution::new(options.window_size.0, options.window_size.1),
        ..default()
    };
    if options.fullscreen {
        window.mode = WindowMode::BorderlessFullscreen(MonitorSelection::Current);
    }
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                })
                // The original's art is pixel art: never blur it.
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(sound::SoundPlugin(shared.mixer.clone()))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(options)
        .insert_resource(GraphicsMode(renderer::Renderers::new()))
        .insert_resource(Game {
            shared,
            shown: (u64::MAX, (-1, -1)),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (input, redraw, fit, quit).chain())
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let mut image = Image::new(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; WIDTH * HEIGHT * 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);
    commands.spawn(Camera2d);
    commands.spawn((Sprite::from_image(handle.clone()), OriginalScreen));
    commands.insert_resource(ScreenImage(handle));
    // The game draws its own pointer.
    cursor.visible = false;
}

/// The screen's rectangle in the window (the largest 4:3 picture, centred).
fn screen_rect(window: &Window) -> (Vec2, Vec2) {
    let (w, h) = (window.width(), window.height());
    let aspect = WIDTH as f32 / (HEIGHT as f32 * TALL_PIXEL);
    let size = if w / h > aspect {
        Vec2::new(h * aspect, h)
    } else {
        Vec2::new(w, w / aspect)
    };
    let origin = Vec2::new((w - size.x) / 2.0, (h - size.y) / 2.0);
    (origin, size)
}

/// The window's mouse position on the 320×200 screen.
fn screen_pos(window: &Window) -> Option<(i32, i32)> {
    let p = window.cursor_position()?;
    let (origin, size) = screen_rect(window);
    let x = ((p.x - origin.x) / size.x * WIDTH as f32).floor() as i32;
    let y = ((p.y - origin.y) / size.y * HEIGHT as f32).floor() as i32;
    Some((x.clamp(0, WIDTH as i32 - 1), y.clamp(0, HEIGHT as i32 - 1)))
}

fn key_of(code: KeyCode, logical: &LogicalKey) -> Option<Key> {
    Some(match code {
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Space => Key::Space,
        KeyCode::Tab => Key::Tab,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::F1 => Key::F(1),
        KeyCode::F2 => Key::F(2),
        KeyCode::F3 => Key::F(3),
        KeyCode::F4 => Key::F(4),
        KeyCode::F5 => Key::F(5),
        KeyCode::F6 => Key::F(6),
        KeyCode::Numpad0 => Key::Pad(0),
        KeyCode::Numpad1 => Key::Pad(1),
        KeyCode::Numpad2 => Key::Pad(2),
        KeyCode::Numpad3 => Key::Pad(3),
        KeyCode::Numpad4 => Key::Pad(4),
        KeyCode::Numpad5 => Key::Pad(5),
        KeyCode::Numpad6 => Key::Pad(6),
        KeyCode::Numpad7 => Key::Pad(7),
        KeyCode::Numpad8 => Key::Pad(8),
        KeyCode::Numpad9 => Key::Pad(9),
        _ => match logical {
            LogicalKey::Character(s) => {
                let c = s.chars().next()?;
                Key::Char(latin1(c.to_ascii_lowercase())?)
            }
            _ => return None,
        },
    })
}

fn latin1(c: char) -> Option<u8> {
    let v = c as u32;
    (v < 256).then_some(v as u8)
}

/// Keys and mouse to the game.
fn input(
    game: ResMut<Game>,
    mut typed: MessageReader<KeyboardInput>,
    mut buttons: MessageReader<MouseButtonInput>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let shared = game.shared.clone();
    if let Some((x, y)) = screen_pos(&window) {
        shared.set_mouse(x, y);
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    for k in typed.read() {
        if k.state != ButtonState::Pressed {
            continue;
        }
        // F2 is the game's own key (the second character's attack), so the HD graphics will
        // need a key of their own; the switch is prepared in `renderer.rs`.
        let Some(key) = key_of(k.key_code, &k.logical_key) else {
            continue;
        };
        let ascii = k
            .text
            .as_ref()
            .and_then(|t| t.chars().next())
            .and_then(latin1)
            .unwrap_or(0);
        shared.push_event(GameEvent::KeyDown {
            key,
            ascii,
            ctrl,
            alt,
            shift,
        });
    }
    let (x, y) = shared.mouse();
    for b in buttons.read() {
        let button = match b.button {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            _ => continue,
        };
        let event = match b.state {
            ButtonState::Pressed => GameEvent::MouseDown { button, x, y },
            ButtonState::Released => GameEvent::MouseUp { button, x, y },
        };
        shared.push_event(event);
    }
}

/// The game's screen into the texture when it changed (or the pointer moved).
fn redraw(
    mut game: ResMut<Game>,
    screen: Res<ScreenImage>,
    mut images: ResMut<Assets<Image>>,
    mut mode: ResMut<GraphicsMode>,
) {
    let mouse = game.shared.mouse();
    let frame = match game.shared.frame.lock() {
        Ok(f) => {
            if f.serial == game.shown.0 && game.shown.1 == mouse {
                return;
            }
            f.clone()
        }
        Err(_) => return,
    };
    game.shown = (frame.serial, mouse);
    let r = mode.0.active();
    let (w, h) = r.size();
    let rgba = r.render(&frame, mouse);
    if let Some(mut image) = images.get_mut(&screen.0) {
        if image.width() != w || image.height() != h {
            image.resize(Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            });
        }
        image.data = Some(rgba);
    }
}

fn fit(
    window: Single<&Window, With<PrimaryWindow>>,
    mut screens: Query<&mut Sprite, With<OriginalScreen>>,
) {
    let (_, size) = screen_rect(&window);
    for mut sprite in &mut screens {
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}

/// The window closes when the game ends; closing the window ends the game.
fn quit(
    game: Res<Game>,
    mut exit: MessageWriter<AppExit>,
    mut closed: MessageReader<bevy::window::WindowCloseRequested>,
) {
    if closed.read().next().is_some() {
        game.shared.push_event(GameEvent::Quit);
    }
    if game.shared.quit.load(std::sync::atomic::Ordering::SeqCst) {
        exit.write(AppExit::Success);
    }
}
