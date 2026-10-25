//! The game, ported from ScummVM's Lands of Lore engine (`LoLEngine`, `KyraRpgEngine`,
//! `KyraEngine_v1`; GPL). One struct holds the game's state as the original's class does; its
//! functions are spread over the modules as ScummVM spreads them over files. The game runs in
//! a thread of its own and waits as the original did ([`Lol::delay`]).

// The original's loops over tables and counters are kept as they are, for comparison.
#![allow(clippy::needless_range_loop, clippy::explicit_counter_loop)]

pub mod automap;
pub mod chars;
pub mod fight;
pub mod gui;
pub mod items;
pub mod magic;
pub mod main_loop;
pub mod menu;
pub mod res;
pub mod save;
pub mod scene;
pub mod script;
pub mod seq;
pub mod sound;
pub mod sprites;
pub mod text;
pub mod tim;
pub mod timer;
pub mod timers;
pub mod types;
pub mod wsa;

use std::collections::VecDeque;
use std::rc::Rc;

use lol_data::emc::Script;
use lol_data::paths::Language;

use crate::platform::{Event, Key};
use crate::screen::Screen;
use crate::util::Rng;

use res::Res;
use timer::Timers;
use types::*;

/// Milliseconds a tick lasts (`_tickLength`, 60 ticks a second).
pub const TICK: u32 = 16;

/// An event as the original keeps it (`Event`: the event and whether it skips sequences).
#[derive(Debug, Clone, Copy)]
pub struct QueuedEvent {
    pub event: Event,
    pub caused_skip: bool,
}

pub struct Lol {
    pub screen: Screen,
    pub res: Res,
    pub rnd: Rng,
    pub lang: Language,
    /// 0 English, 1 French, 2 German (`_lang`).
    pub lang_index: usize,

    // input
    pub event_list: VecDeque<QueuedEvent>,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_click: u8,
    pub preserve_events: bool,
    pub button_list_changed: bool,

    // flags and timers
    pub flags_table: [u8; 100],
    pub timers: Timers,
    pub update_flags: i32,

    // options (`_configVoice`: 0 text, 1 voice, 2 both)
    pub config_voice: i32,
    pub monster_difficulty: i32,
    pub smooth_scrolling: bool,
    pub floating_cursors: bool,
    pub game_to_load: i32,

    // language files
    pub lands_file: Vec<u8>,
    pub level_lang_file: Vec<u8>,

    // shapes
    pub item_icon_shapes: Vec<Vec<u8>>,
    pub item_shapes: Vec<Vec<u8>>,
    pub game_shapes: Vec<Vec<u8>>,
    pub thrown_shapes: Vec<Vec<u8>>,
    pub effect_shapes: Vec<Vec<u8>>,
    pub fireball_shapes: Vec<Vec<u8>>,
    pub heal_shapes: Vec<Vec<u8>>,
    pub heali_shapes: Vec<Vec<u8>>,
    pub character_face_shapes: Vec<[Option<Vec<u8>>; 3]>,
    pub monster_shapes: Vec<Option<Vec<u8>>>,
    pub monster_palettes: Vec<Vec<u8>>,
    pub monster_decoration_shapes: Vec<Option<Vec<u8>>>,
    pub door_shapes: [Option<Vec<u8>>; 6],
    pub automap_shapes: Vec<Vec<u8>>,

    // characters
    pub characters: [Character; 4],
    pub active_chars_xpos: [u16; 3],
    pub selected_character: i32,
    pub update_char_num: i32,
    pub portrait_speech_anim_mode: i32,
    pub update_portrait_speech_anim_duration: i32,
    pub reset_portrait_after_speech_anim: i32,
    pub text_color_flag: i32,
    pub pal_update_timer: u32,
    pub update_portrait_next: u32,
    pub fade_text: bool,
    pub need_scene_restore: i32,
    pub char_stats_temp: [i32; 5],
    pub char_inventory_unk: i32,
    pub char_status_flags: [u16; 3],
    pub last_char_inventory: i32,
    pub char_selection: i32,

    // items
    pub items_in_play: Vec<LolItem>,
    pub item_properties: Vec<ItemProperty>,
    pub item_in_hand: Item,
    pub inventory: [Item; 48],
    pub inventory_cur_item: i32,
    pub credits: u16,
    pub money_column_height: [u8; 5],
    pub emc_last_item: i32,
    pub flying_objects: [FlyingObject; 8],
    pub item_script: Option<Rc<Script>>,

    // monsters
    pub monsters: Vec<Monster>,
    pub monster_properties: [MonsterProperty; 5],
    pub monster_anim_type: [u8; 3],
    pub monster_cur_block: u16,
    pub object_last_direction: i32,
    pub monster_step_counter: i32,
    pub monster_step_mode: i32,

    // level
    pub current_level: i32,
    pub current_block: u16,
    pub current_direction: u16,
    pub level_block_properties: Vec<LevelBlockProperty>,
    pub wll_vmp_map: [u8; 256],
    pub wll_shape_map: [i8; 256],
    pub special_wall_types: [u8; 256],
    pub wll_wall_flags: [u8; 256],
    pub wll_automap_data: [u8; 256],
    pub vmp: Vec<u16>,
    pub vcn_blocks: Vec<u8>,
    pub vcn_shift: Vec<u8>,
    pub vcn_col_table: [u8; 128],
    pub block_drawing_buffer: Vec<u16>,
    pub scene_window_buffer: Vec<u8>,
    pub visible_blocks: [usize; 18],
    pub lvl_shape_left_right: [i16; 36],
    pub lvl_shape_top: [i16; 18],
    pub lvl_shape_bottom: [i16; 18],
    pub scene_draw_var_down: usize,
    pub scene_draw_var_right: usize,
    pub scene_draw_var_left: usize,
    pub wll_process_flag: bool,
    pub open_door_state: [OpenDoorState; 3],
    pub scene_draw_page1: usize,
    pub scene_draw_page2: usize,
    pub scene_update_required: bool,
    pub scene_default_update: i32,
    pub last_block_data_file: String,
    pub last_override_pal_file: String,
    pub last_special_color: i32,
    pub last_special_color_weight: i32,
    pub has_temp_data_flags: u32,
    pub level_temp_data: Vec<Option<LevelTempData>>,
    pub level_decoration_data: Vec<LevelDecoration>,
    pub level_decoration_properties: Vec<LevelDecoration>,
    pub level_decoration_shapes: Vec<Option<Vec<u8>>>,
    pub decoration_count: u16,
    pub mapped_decorations_count: i32,
    pub lvl_shape_index: i32,
    pub lvl_shp_file: Option<Vec<u8>>,
    pub temp_buffer5120: Vec<u8>,
    pub script_data: Option<Rc<Script>>,
    pub suspend_script: bool,
    pub script_direction: u16,
    pub global_script_vars: [i16; 24],
    pub global_script_vars2: [u16; 8],
    pub next_script_func: i32,
    pub load_level_flag: i32,
    pub party_pos_x: u16,
    pub party_pos_y: u16,
    pub party_awake: bool,
    pub block_door: i32,
    pub special_gui_shape: Option<Vec<u8>>,
    pub special_gui_shape_x: u16,
    pub special_gui_shape_y: u16,
    pub special_gui_shape_mirror_flag: u16,
    pub transparency_table1: Vec<u8>,
    pub transparency_table2: Vec<u8>,
    pub shp_dm_x: i32,
    pub shp_dm_y: i32,
    pub dm_scale_w: u16,
    pub dm_scale_h: u16,
    pub shp_dm_x1: i16,
    pub shp_dm_x2: i16,
    pub smooth_scroll_mode_normal: i32,
    pub load_supp_files_flag: i32,

    // gui
    pub buttons: Vec<Button>,
    pub weapons_disabled: bool,
    pub last_button_shape: i32,
    pub button_press_timer: u32,
    pub compass_step: i32,
    pub compass_direction: i32,
    pub compass_direction_index: i32,
    pub compass_timer: u32,
    pub compass_broken: u8,
    pub drain_magic: u8,
    pub current_control_mode: i32,
    pub special_scene_flag: i32,
    pub active_magic_menu: i32,
    pub sub_menu_index: i32,
    pub scene_window_button: (i16, i16, u16, u16),
    pub seq_window: (i32, i32, i32, i32),
    pub seq_trigger: i32,
    pub sps_window: (i32, i32, i32, i32),
    pub floating_cursor_control: i32,
    pub current_floating_cursor: i32,
    pub last_mouse_region: i32,
    pub page_buffer1: Vec<u8>,
    pub page_buffer2: Vec<u8>,
    pub dialogue_field: bool,
    pub dialogue_button_string: [Vec<u8>; 3],
    pub dialogue_num_buttons: i32,
    pub dialogue_highlighted_button: i32,
    pub dialogue_button_x_offs: i32,
    pub dialogue_button_y_offs: i32,
    pub dialogue_button_pos_x: [u16; 3],
    pub dialogue_button_pos_y: [u8; 3],
    pub dialogue_button_width: u16,

    // lamp and brightness
    pub lamp_effect: i8,
    pub brightness: i32,
    pub lamp_oil_status: i32,
    pub lamp_status_timer: u32,
    pub lamp_status_suspended: bool,

    // magic
    pub available_spells: [i8; 8],
    pub selected_spell: i32,
    pub active_spell: ActiveSpell,
    pub swarm_spell_status: u8,
    pub heal_overlay: Vec<u8>,
    pub lightning_cur_sfx: i16,
    pub lightning_div: i16,
    pub lightning_first_sfx: i16,
    pub lightning_sfx_frame: i16,

    // fight
    pub party_damage_flags: i32,

    // sound
    pub last_speech_id: i32,
    pub last_speaker: i32,
    pub next_speech_id: i32,
    pub next_speaker: i32,
    pub last_sfx_track: i32,
    pub last_music_track: i32,
    pub cur_music_theme: i32,
    pub cur_music_file_index: i32,
    pub cur_music_file_ext: u8,
    pub cur_tlk_file: i32,
    pub speech_list: VecDeque<(Vec<i16>, u32)>,
    pub speech_handle: u32,
    pub active_voice_file_total_time: u32,
    pub env_sfx_use_queue: bool,
    pub env_sfx_num_tracks_in_queue: usize,
    pub env_sfx_queued_tracks: [u16; 10],
    pub env_sfx_queued_blocks: [u16; 10],
    pub environment_sfx: i32,
    pub environment_sfx_vol: i32,
    pub env_sfx_dist_threshold: i32,
    pub ingame_sound_list: Vec<String>,

    // automap
    pub current_map_level: u8,
    pub map_overlay: Vec<u8>,
    pub map_cursor_overlay: Vec<u8>,
    pub automap_top_left_x: u8,
    pub automap_top_left_y: u8,
    pub map_update_needed: bool,
    pub default_legend_data: Vec<MapLegendData>,

    // play time
    pub play_time_start: u32,
    pub play_time_total: u32,

    // scripts and text
    pub emc_stop: bool,
    pub script_character_cycle: i32,
    pub active_tim: Vec<Option<tim::Tim>>,
    pub seq_tim: Option<tim::Tim>,
    pub tim: tim::TimInterp,
    pub txt: text::TextState,
    pub visible_block_index: [u16; 18],

    // menus, sequences, saves
    pub gui_state: gui::GuiState,
    pub menu: menu::MenuState,
    pub selection: seq::SelectionState,
    pub save_dir: Option<std::path::PathBuf>,
    pub on_settings_changed: Option<Box<dyn FnMut(main_loop::Settings) + Send>>,
    /// Prints each call of the game's functions by the scripts (`LOL_TRACE`).
    pub trace_calls: bool,
    /// Test runs: a new game with this hero (0–3) at once.
    pub quick_start: Option<i32>,
}

/// A decoration of the level (`LevelDecorationProperty`).
#[derive(Debug, Clone, Copy, Default)]
pub struct LevelDecoration {
    pub shape_index: [u16; 10],
    pub scale_flag: [u8; 10],
    pub shape_x: [i16; 10],
    pub shape_y: [i16; 10],
    pub next: i8,
    pub flags: u8,
}

/// What a level keeps while the party is elsewhere (`LevelTempData`).
#[derive(Debug, Clone, Default)]
pub struct LevelTempData {
    pub walls_xor: Vec<u8>,
    pub flags: Vec<u16>,
    pub monsters: Vec<Monster>,
    pub flying_objects: Vec<FlyingObject>,
    pub monster_difficulty: u8,
}

impl Lol {
    pub fn new(screen: Screen, res: Res) -> Self {
        let lang = res.language();
        let lang_index = match lang {
            Language::English => 0,
            Language::French => 1,
            Language::German => 2,
        };
        Self {
            screen,
            res,
            rnd: Rng::new(0x1234_5678),
            lang,
            lang_index,
            event_list: VecDeque::new(),
            mouse_x: 0,
            mouse_y: 0,
            mouse_click: 0,
            preserve_events: false,
            button_list_changed: false,
            flags_table: [0; 100],
            timers: Timers::default(),
            update_flags: 0,
            config_voice: 2,
            monster_difficulty: 1,
            smooth_scrolling: true,
            floating_cursors: false,
            game_to_load: -1,
            lands_file: Vec::new(),
            level_lang_file: Vec::new(),
            item_icon_shapes: Vec::new(),
            item_shapes: Vec::new(),
            game_shapes: Vec::new(),
            thrown_shapes: Vec::new(),
            effect_shapes: Vec::new(),
            fireball_shapes: Vec::new(),
            heal_shapes: Vec::new(),
            heali_shapes: Vec::new(),
            character_face_shapes: vec![[None, None, None]; 40],
            monster_shapes: vec![None; 48],
            monster_palettes: vec![Vec::new(); 48],
            monster_decoration_shapes: vec![None; 576],
            door_shapes: Default::default(),
            automap_shapes: Vec::new(),
            characters: Default::default(),
            active_chars_xpos: [0; 3],
            selected_character: 0,
            update_char_num: -1,
            portrait_speech_anim_mode: 0,
            update_portrait_speech_anim_duration: 0,
            reset_portrait_after_speech_anim: 0,
            text_color_flag: 0,
            pal_update_timer: 0,
            update_portrait_next: 0,
            fade_text: false,
            need_scene_restore: 0,
            char_stats_temp: [0; 5],
            char_inventory_unk: 0,
            char_status_flags: [0; 3],
            last_char_inventory: -1,
            char_selection: -1,
            items_in_play: vec![LolItem::default(); 400],
            item_properties: Vec::new(),
            item_in_hand: 0,
            inventory: [0; 48],
            inventory_cur_item: 0,
            credits: 0,
            money_column_height: [0; 5],
            emc_last_item: -1,
            flying_objects: [FlyingObject::default(); 8],
            item_script: None,
            monsters: vec![Monster::default(); 30],
            monster_properties: [MonsterProperty::default(); 5],
            monster_anim_type: [0; 3],
            monster_cur_block: 0,
            object_last_direction: 0,
            monster_step_counter: 0,
            monster_step_mode: 0,
            current_level: 0,
            current_block: 0,
            current_direction: 0,
            level_block_properties: vec![LevelBlockProperty::default(); 1025],
            wll_vmp_map: [0; 256],
            wll_shape_map: [0; 256],
            special_wall_types: [0; 256],
            wll_wall_flags: [0; 256],
            wll_automap_data: [0; 256],
            vmp: Vec::new(),
            vcn_blocks: Vec::new(),
            vcn_shift: Vec::new(),
            vcn_col_table: std::array::from_fn(|i| (i & 0x0f) as u8),
            block_drawing_buffer: vec![0; 1320],
            scene_window_buffer: vec![0; 176 * 120],
            visible_blocks: [0; 18],
            lvl_shape_left_right: [0; 36],
            lvl_shape_top: [0; 18],
            lvl_shape_bottom: [0; 18],
            scene_draw_var_down: 0,
            scene_draw_var_right: 0,
            scene_draw_var_left: 0,
            wll_process_flag: false,
            open_door_state: [OpenDoorState::default(); 3],
            scene_draw_page1: 2,
            scene_draw_page2: 6,
            scene_update_required: false,
            scene_default_update: 0,
            last_block_data_file: String::new(),
            last_override_pal_file: String::new(),
            last_special_color: 0,
            last_special_color_weight: 0,
            has_temp_data_flags: 0,
            level_temp_data: vec![None; 29],
            level_decoration_data: Vec::new(),
            level_decoration_properties: vec![LevelDecoration::default(); 100],
            level_decoration_shapes: vec![None; 400],
            decoration_count: 0,
            mapped_decorations_count: 0,
            lvl_shape_index: 0,
            lvl_shp_file: None,
            temp_buffer5120: vec![0; 5120],
            script_data: None,
            suspend_script: false,
            script_direction: 0,
            global_script_vars: [0; 24],
            global_script_vars2: [0; 8],
            next_script_func: 0,
            load_level_flag: 0,
            party_pos_x: 0,
            party_pos_y: 0,
            party_awake: true,
            block_door: 0,
            special_gui_shape: None,
            special_gui_shape_x: 0,
            special_gui_shape_y: 0,
            special_gui_shape_mirror_flag: 0,
            transparency_table1: vec![0; 256],
            transparency_table2: vec![0; 5120],
            shp_dm_x: 0,
            shp_dm_y: 0,
            dm_scale_w: 0,
            dm_scale_h: 0,
            shp_dm_x1: 0,
            shp_dm_x2: 0,
            smooth_scroll_mode_normal: 1,
            load_supp_files_flag: 0,
            buttons: Vec::new(),
            weapons_disabled: false,
            last_button_shape: 0,
            button_press_timer: 0,
            compass_step: 0,
            compass_direction: -1,
            compass_direction_index: -1,
            compass_timer: 0,
            compass_broken: 0,
            drain_magic: 0,
            current_control_mode: 0,
            special_scene_flag: 0,
            active_magic_menu: -1,
            sub_menu_index: 0,
            scene_window_button: (0, 0, 0, 0),
            seq_window: (0, 0, 0, 0),
            seq_trigger: 0,
            sps_window: (0, 0, 0, 0),
            floating_cursor_control: 0,
            current_floating_cursor: 0,
            last_mouse_region: 0,
            page_buffer1: vec![0; 0xfa00],
            page_buffer2: vec![0; 0xfa00],
            dialogue_field: false,
            dialogue_button_string: Default::default(),
            dialogue_num_buttons: 0,
            dialogue_highlighted_button: 0,
            dialogue_button_x_offs: 0,
            dialogue_button_y_offs: 0,
            dialogue_button_pos_x: [0; 3],
            dialogue_button_pos_y: [0; 3],
            dialogue_button_width: 74,
            lamp_effect: 0,
            brightness: 0,
            lamp_oil_status: 0,
            lamp_status_timer: u32::MAX,
            lamp_status_suspended: false,
            available_spells: [-1; 8],
            selected_spell: 0,
            active_spell: ActiveSpell::default(),
            swarm_spell_status: 0,
            heal_overlay: Vec::new(),
            lightning_cur_sfx: -1,
            lightning_div: 0,
            lightning_first_sfx: 0,
            lightning_sfx_frame: 0,
            party_damage_flags: -1,
            last_speech_id: -1,
            last_speaker: -1,
            next_speech_id: -1,
            next_speaker: -1,
            last_sfx_track: -1,
            last_music_track: -1,
            cur_music_theme: -1,
            cur_music_file_index: -1,
            cur_music_file_ext: 0,
            cur_tlk_file: -1,
            speech_list: VecDeque::new(),
            speech_handle: 0,
            active_voice_file_total_time: 0,
            env_sfx_use_queue: false,
            env_sfx_num_tracks_in_queue: 0,
            env_sfx_queued_tracks: [0; 10],
            env_sfx_queued_blocks: [0; 10],
            environment_sfx: 0,
            environment_sfx_vol: 0,
            env_sfx_dist_threshold: 15,
            ingame_sound_list: Vec::new(),
            current_map_level: 0,
            map_overlay: vec![0; 256],
            map_cursor_overlay: vec![0; 256],
            automap_top_left_x: 0,
            automap_top_left_y: 0,
            map_update_needed: false,
            default_legend_data: Vec::new(),
            play_time_start: 0,
            play_time_total: 0,
            emc_stop: false,
            script_character_cycle: 0,
            active_tim: vec![None; 10],
            seq_tim: None,
            tim: tim::TimInterp::new(true),
            txt: text::TextState::default(),
            visible_block_index: [0; 18],
            gui_state: gui::GuiState::default(),
            menu: menu::MenuState::default(),
            selection: seq::SelectionState::default(),
            save_dir: None,
            on_settings_changed: None,
            trace_calls: std::env::var_os("LOL_TRACE").is_some(),
            quick_start: None,
        }
    }

    // ---- time

    pub fn millis(&self) -> u32 {
        self.screen.system.millis()
    }

    pub fn should_quit(&self) -> bool {
        self.screen.system.should_quit()
    }

    pub fn quit_game(&self) {
        self.screen.system.quit();
    }

    /// `KyraEngine_v1::delayUntil`.
    pub fn delay_until(&mut self, time: u32, do_update: bool) {
        let now = self.millis();
        if time > now {
            self.delay(time - now, do_update);
        }
    }

    pub fn delay_with_ticks(&mut self, ticks: i32) {
        self.delay((ticks.max(0) as u32) * TICK, false);
    }

    /// `rollDice`.
    pub fn roll_dice(&mut self, times: i32, pips: i32, inc: i32) -> i32 {
        if times <= 0 || pips <= 0 {
            return inc;
        }
        let mut res = 0;
        for _ in 0..times {
            res += self.rnd.range(1, pips);
        }
        res + inc
    }

    // ---- game flags

    pub fn set_game_flag(&mut self, flag: i32) -> i32 {
        if let Some(b) = self.flags_table.get_mut((flag >> 3) as usize) {
            *b |= 1 << (flag & 7);
        }
        1
    }

    pub fn query_game_flag(&self, flag: i32) -> i32 {
        self.flags_table
            .get((flag >> 3) as usize)
            .map_or(0, |b| ((b >> (flag & 7)) & 1) as i32)
    }

    pub fn reset_game_flag(&mut self, flag: i32) -> i32 {
        if let Some(b) = self.flags_table.get_mut((flag >> 3) as usize) {
            *b &= !(1 << (flag & 7));
        }
        0
    }

    pub fn speech_enabled(&self) -> bool {
        self.config_voice == 1 || self.config_voice == 2
    }

    pub fn text_enabled(&self) -> bool {
        self.config_voice == 0 || self.config_voice == 2
    }

    pub fn pos_within_rect(x: i32, y: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> bool {
        !(x < x1 || x > x2 || y < y1 || y > y2)
    }

    // ---- input

    /// `updateInput`: the platform's events into the event list.
    pub fn update_input(&mut self) {
        while let Some(event) = self.screen.system.poll_event() {
            match event {
                Event::KeyDown { key, ctrl, .. } => {
                    if ctrl && key == Key::Char(b'q') {
                        self.quit_game();
                        continue;
                    }
                    let skip = matches!(
                        key,
                        Key::Char(b'.')
                            | Key::Escape
                            | Key::Space
                            | Key::Enter
                            | Key::Up
                            | Key::Right
                            | Key::Down
                            | Key::Left
                    );
                    self.event_list.push_back(QueuedEvent {
                        event,
                        caused_skip: skip,
                    });
                }
                Event::MouseDown { .. } => self.event_list.push_back(QueuedEvent {
                    event,
                    caused_skip: true,
                }),
                Event::MouseUp { .. } => self.event_list.push_back(QueuedEvent {
                    event,
                    caused_skip: false,
                }),
                Event::Quit => self.quit_game(),
                Event::MouseMove { .. } => {}
            }
        }
        // The pointer moves on the monitor by itself (the window draws it).
    }

    pub fn remove_input_top(&mut self) {
        self.event_list.pop_front();
    }

    /// `skipFlag`: a key or click waits that skips a sequence.
    pub fn skip_flag(&self) -> bool {
        self.event_list.iter().any(|e| e.caused_skip)
    }

    pub fn reset_skip_flag(&mut self, remove: bool) {
        if let Some(i) = self.event_list.iter().position(|e| e.caused_skip) {
            if remove {
                self.event_list.remove(i);
            } else {
                self.event_list[i].caused_skip = false;
            }
        }
    }

    pub fn get_mouse_pos(&self) -> (i32, i32) {
        self.screen.system.mouse_pos()
    }

    pub fn set_mouse_pos(&mut self, x: i32, y: i32) {
        self.screen.system.warp_mouse(x, y);
    }

    /// The original's key codes (`_keyMap`, DOS values).
    pub fn key_code(key: Key) -> u16 {
        match key {
            Key::Space => 61,
            Key::Enter => 43,
            Key::Up | Key::Pad(8) => 96,
            Key::Right | Key::Pad(6) => 102,
            Key::Down | Key::Pad(2) => 98,
            Key::Pad(5) => 97,
            Key::Left | Key::Pad(4) => 92,
            Key::Home | Key::Pad(7) => 91,
            Key::PageUp | Key::Pad(9) => 101,
            Key::End | Key::Pad(1) => 93,
            Key::PageDown | Key::Pad(3) => 103,
            Key::F(n) if (1..=6).contains(&n) => 111 + n as u16,
            Key::Tab => 16,
            Key::Escape => 110,
            Key::Char(c) => match c.to_ascii_lowercase() {
                b'a' => 31,
                b'b' => 50,
                b'c' => 48,
                b'd' => 33,
                b'e' => 19,
                b'f' => 34,
                b'i' => 24,
                b'k' => 38,
                b'm' => 52,
                b'n' => 51,
                b'o' => 25,
                b'p' => 26,
                b'r' => 20,
                b's' => 32,
                b'w' => 18,
                b'y' => 22,
                b'z' => 46,
                b'1' => 2,
                b'2' => 3,
                b'3' => 4,
                b'4' => 5,
                b'5' => 6,
                b'6' => 7,
                b'7' => 8,
                b'/' => 55,
                b'-' => 12,
                b'+' => 13,
                b',' | b';' | b'<' => 53,
                b'.' | b':' | b'>' => 54,
                _ => 0,
            },
            _ => 0,
        }
    }

    pub const KEY_SPACE: u16 = 61;
    pub const KEY_RETURN: u16 = 43;
    pub const KEY_ESCAPE: u16 = 110;
    pub const KEY_UP: u16 = 96;
    pub const KEY_DOWN: u16 = 98;
    pub const KEY_LEFT: u16 = 92;
    pub const KEY_RIGHT: u16 = 102;
    pub const KEY_SLASH: u16 = 55;

    /// `checkInput`: the first event that means something, as the original's code (keys
    /// through the key map, 199/200 left button down/up, 201/202 right), passed to the button
    /// list if one is given. The event stays in the list (`removeInputTop` takes it).
    pub fn check_input(
        &mut self,
        list: Option<ButtonListId>,
        main_loop: bool,
        event_flag: i32,
    ) -> i32 {
        let _ = main_loop;
        self.update_input();
        let mut keys: i32 = 0;
        while let Some(&QueuedEvent { event, .. }) = self.event_list.front() {
            let mut break_loop = false;
            match event {
                // ScummVM's quick save/load (Ctrl/Alt + digit) and Ctrl+Q are left out: the
                // keys work as in the original.
                Event::KeyDown { key, shift, .. } => {
                    let k = Self::key_code(key) as i32;
                    keys = k;
                    if keys != 0 && shift {
                        keys |= 0x100;
                    }
                    if keys != 0 {
                        break_loop = true;
                    }
                }
                Event::MouseDown { button, x, y } | Event::MouseUp { button, x, y } => {
                    self.mouse_x = x;
                    self.mouse_y = y;
                    let down = matches!(event, Event::MouseDown { .. });
                    keys = match (button, down) {
                        (0, true) => 199,
                        (0, false) => 200 | 0x800,
                        (_, true) => 201,
                        (_, false) => 202 | 0x800,
                    };
                    break_loop = true;
                }
                _ => {}
            }
            if break_loop {
                break;
            }
            self.event_list.pop_front();
        }
        let (mx, my) = self.get_mouse_pos();
        if keys == 0 {
            self.mouse_x = mx;
            self.mouse_y = my;
        }
        match list {
            Some(id) => {
                self.process_button_list(id, if keys != 0 { keys | event_flag } else { 0 }, 0)
            }
            // GUI_LoL::processButtonList without a list.
            None => {
                if keys != 0 {
                    (keys | event_flag) & 0x7FFF
                } else {
                    0
                }
            }
        }
    }

    /// `KyraEngine_v1::delay` for Lands of Lore: waits in ticks, keeping the game updated or
    /// at least the input.
    pub fn delay(&mut self, millis: u32, do_update: bool) {
        let mut millis = millis;
        while millis > 0 && !self.should_quit() {
            if do_update {
                self.update();
            } else {
                self.update_input();
            }
            let step = millis.min(TICK);
            self.screen.system.delay_millis(step);
            millis -= step;
        }
    }
}

/// Which list of buttons `checkInput` passes events to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonListId {
    /// The play field's (`_activeButtons`).
    Active,
    /// The open menu's.
    Menu,
}
