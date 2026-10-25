//! Lands-of-Lore-RS: Lands of Lore in Rust. For now: walk the levels in the original look.
//!
//! The game (`lol-engine`) draws the screen as the original did, 320×200 colour indices; this
//! program shows it, stretched to 4:3 as under DOS, and passes on the keys.

mod options;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::window::{PrimaryWindow, WindowResolution};
use lol_data::assets::AssetStore;
use lol_data::paths;
use lol_engine::game::{self, Game, HEIGHT, WIDTH};

use options::Options;

/// DOS showed the 320×200 screen on a 4:3 monitor: each pixel 1.2 times as tall as wide.
const TALL_PIXEL: f32 = 1.2;

/// The game, kept outside Bevy's world of components.
#[derive(Resource)]
struct GameState {
    game: Game,
    /// The screen needs drawing anew.
    dirty: bool,
}

/// The texture the screen is shown in.
#[derive(Resource)]
struct ScreenImage(Handle<Image>);

#[derive(Component)]
struct OriginalScreen;

fn main() {
    let options = match Options::from_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let Some(copy) = paths::find() else {
        eprintln!(
            "Keine Installation von Lands of Lore gefunden (Umgebungsvariable {}).",
            paths::LOL_ENV
        );
        std::process::exit(1);
    };
    let game = AssetStore::open(&copy, options.language).and_then(|store| {
        let (level, block, direction) = game::START;
        let mut game = Game::new(
            store,
            options.level.unwrap_or(level),
            options.block.unwrap_or(block),
            options.direction.unwrap_or(direction),
        )?;
        for step in options.moves.chars() {
            apply_move(&mut game, step);
        }
        Ok(game)
    });
    let game = match game {
        Ok(game) => game,
        Err(err) => {
            eprintln!("{}: {err}", copy.dir.display());
            std::process::exit(1);
        }
    };
    println!(
        "Lands of Lore – {} ({}), Ebene {}",
        copy.name(),
        options.language.label(),
        game.level.index
    );

    // A screenshot run needs no window.
    if let Some(path) = &options.screenshot {
        let screen = game.render();
        let rgba = screen.rgba();
        if let Err(err) = image::save_buffer(
            path,
            &rgba,
            WIDTH as u32,
            HEIGHT as u32,
            image::ColorType::Rgba8,
        ) {
            eprintln!("{path}: {err}");
            std::process::exit(1);
        }
        println!("Bildschirm nach {path}");
        return;
    }

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Lands-of-Lore-RS".into(),
                        resolution: WindowResolution::new(
                            options.window_size.0,
                            options.window_size.1,
                        ),
                        ..default()
                    }),
                    ..default()
                })
                // The original's art is pixel art: never blur it.
                .set(ImagePlugin::default_nearest()),
        )
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(options)
        .insert_resource(GameState { game, dirty: true })
        .add_systems(Startup, setup)
        .add_systems(Update, (keys, redraw, fit).chain())
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
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
}

/// One of the moves of `--wege` or a key: `V` ahead, `Z` back, `A`/`D` aside, `L`/`R` turn.
fn apply_move(game: &mut Game, step: char) {
    match step {
        'V' => {
            game.walk(0);
        }
        'Z' => {
            game.walk(2);
        }
        'A' => {
            game.walk(3);
        }
        'D' => {
            game.walk(1);
        }
        'L' => game.turn(false),
        'R' => game.turn(true),
        _ => {}
    }
}

/// The keys as in the original (arrows and the number pad) and as in EOBRS (W A S D, Q E);
/// Page Up and Page Down change the level, for trying levels out.
fn keys(keys: Res<ButtonInput<KeyCode>>, mut state: ResMut<GameState>) {
    let moves: [(&[KeyCode], char); 6] = [
        (&[KeyCode::KeyW, KeyCode::ArrowUp, KeyCode::Numpad8], 'V'),
        (
            &[
                KeyCode::KeyS,
                KeyCode::ArrowDown,
                KeyCode::Numpad5,
                KeyCode::Numpad2,
            ],
            'Z',
        ),
        (&[KeyCode::KeyA, KeyCode::Numpad4], 'A'),
        (&[KeyCode::KeyD, KeyCode::Numpad6], 'D'),
        (&[KeyCode::KeyQ, KeyCode::ArrowLeft, KeyCode::Numpad7], 'L'),
        (&[KeyCode::KeyE, KeyCode::ArrowRight, KeyCode::Numpad9], 'R'),
    ];
    for (codes, step) in moves {
        if keys.any_just_pressed(codes.iter().copied()) {
            apply_move(&mut state.game, step);
            state.dirty = true;
        }
    }
    let change = if keys.just_pressed(KeyCode::PageUp) {
        Some(1)
    } else if keys.just_pressed(KeyCode::PageDown) {
        Some(-1)
    } else {
        None
    };
    if let Some(change) = change {
        let index = (state.game.level.index as i32 + change).clamp(1, 29) as usize;
        match state.game.change_level(index) {
            Ok(()) => println!("Ebene {index} ({})", state.game.level.graphics),
            Err(err) => eprintln!("Ebene {index}: {err}"),
        }
        state.dirty = true;
    }
}

/// Draws the screen anew into its texture when something changed.
fn redraw(
    mut state: ResMut<GameState>,
    screen: Res<ScreenImage>,
    mut images: ResMut<Assets<Image>>,
) {
    if !state.dirty {
        return;
    }
    state.dirty = false;
    let rgba = state.game.render().rgba();
    if let Some(mut image) = images.get_mut(&screen.0) {
        image.data = Some(rgba);
    }
}

/// The largest 4:3 picture the window holds, centred.
fn fit(
    window: Single<&Window, With<PrimaryWindow>>,
    mut screens: Query<&mut Sprite, With<OriginalScreen>>,
) {
    let (w, h) = (window.width(), window.height());
    let aspect = WIDTH as f32 / (HEIGHT as f32 * TALL_PIXEL);
    let size = if w / h > aspect {
        Vec2::new(h * aspect, h)
    } else {
        Vec2::new(w, w / aspect)
    };
    for mut sprite in &mut screens {
        if sprite.custom_size != Some(size) {
            sprite.custom_size = Some(size);
        }
    }
}
