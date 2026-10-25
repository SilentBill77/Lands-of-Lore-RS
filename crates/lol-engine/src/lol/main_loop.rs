//! The game from start to end (ScummVM's `LoLEngine::init`, `go`, `startup`, `startupNew`,
//! `runLoop`, `update`): the prologue, the files of the game, a new game or a loaded one, then
//! the loop that runs the timers, the input and the view.

use lol_data::tables::INGAME_SFX_FILES;

use super::{Lol, TICK};
use crate::screen::{FontId, Pal, generate_overlay};
use crate::shape;

/// The settings the game keeps (`writeSettings`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    pub monster_difficulty: i32,
    pub smooth_scrolling: bool,
    pub floating_cursors: bool,
    /// 0 text, 1 voice, 2 both.
    pub config_voice: i32,
    pub music_volume: u8,
    pub sfx_volume: u8,
    pub speech_volume: u8,
}

impl Lol {
    /// `init`: what the engine sets up before the game.
    pub fn init(&mut self) {
        self.ingame_sound_list = INGAME_SFX_FILES.iter().map(|s| s.to_string()).collect();
        self.txt_init();
        self.menu_init();
        self.init_legend_data();
        self.setup_timers();
        self.screen.set_screen_dim(0);
        for it in self.items_in_play.iter_mut() {
            it.shp_cur_frame_flg |= 0x8000;
        }
    }

    /// `go`: the whole game; returns when it ends.
    pub fn go(&mut self) {
        let mut action = -1;
        // Test runs: the end at once (`LOL_OUTRO=hero`).
        if let Some(c) = std::env::var("LOL_OUTRO")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
        {
            self.res.load_pak("GENERAL.PAK");
            self.pre_init();
            self.show_outro(c, true);
            return;
        }
        if let Some(c) = self.quick_start {
            // Test runs: a new game with this hero, without intro and title.
            self.res.load_pak("GENERAL.PAK");
            self.setup_prologue_data(true);
            self.pre_init();
            self.char_selection = c;
            self.setup_prologue_data(false);
            action = 0;
        } else if self.game_to_load == -1 {
            action = self.process_prologue();
            if action == -1 {
                return;
            }
        }
        self.res.load_file_list("FILEDATA.FDT");
        if self.game_to_load != -1 && action != 3 {
            self.pre_init();
            self.screen.set_font(FontId::F9);
        }
        if self.should_quit() {
            return;
        }
        self.tim = super::tim::TimInterp::new(true);
        self.startup();
        if action == 0 {
            self.startup_new();
            // Test runs: the lamp, the magic atlas and the compass at once (`LOL_GEAR`).
            if std::env::var_os("LOL_GEAR").is_some() {
                self.flags_table[31] |= 0x58;
                self.lamp_oil_status = 100;
                self.gui_draw_play_field();
            }
            // Test runs: another place to begin (`LOL_START=level,block,direction`; a
            // negative block -n is the n-th open block of the level without objects).
            if let Some((l, mut b, d)) = std::env::var("LOL_START").ok().and_then(|v| {
                let n: Vec<i32> = v.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                (n.len() == 3).then(|| (n[0], n[1], n[2]))
            }) {
                self.current_block = b.max(0) as u16;
                self.current_direction = d as u16;
                let (x, y) = self.calc_coordinates(b.max(0) as u16, 0x80, 0x80);
                self.party_pos_x = x;
                self.party_pos_y = y;
                self.load_level(l);
                if b < 0 {
                    let open: Vec<usize> = (0..1024)
                        .filter(|&i| {
                            let p = &self.level_block_properties[i];
                            p.walls == [0; 4] && p.assigned_objects == 0
                        })
                        .collect();
                    if !open.is_empty() {
                        b = open[(-b as usize - 1) % open.len()] as i32;
                    }
                    eprintln!("start: level {l} block {b}");
                }
                self.current_block = b as u16;
                self.current_direction = d as u16;
                let (x, y) = self.calc_coordinates(b as u16, 0x80, 0x80);
                self.party_pos_x = x;
                self.party_pos_y = y;
            }
        } else if self.game_to_load != -1 {
            let slot = self.game_to_load;
            self.load_game_state(slot);
            self.game_to_load = -1;
        }
        self.screen.fade_flag = 3;
        self.scene_update_required = true;
        self.enable_sys_timer(1);
        self.run_loop();
    }

    /// `startup`.
    pub fn startup(&mut self) {
        self.screen.clear_page(0);
        let mut pal = self.screen.palette(0).clone();
        if let Some(d) = self.res.file_data("PLAYFLD.CPS") {
            self.screen.load_bitmap(&d, 3, Some(&mut pal));
        }
        *self.screen.palette_mut(0) = pal.clone();
        self.screen.copy_palette(1, 0);
        pal.fill(0, 1, 0x3F);
        pal.fill(2, 126, 0x3F);
        pal.fill(192, 4, 0x3F);
        self.screen.palette_overlay1 = generate_overlay(&pal, 1, 96, 254);
        self.screen.palette_overlay2 = generate_overlay(&pal, 144, 65, 254);
        self.screen.copy_palette(0, 1);
        *self.screen.palette_mut(1) = Pal::default();
        *self.screen.palette_mut(2) = Pal::default();
        self.load_item_icon_shapes();
        let c = self.item_icon_shapes.get(0x85).cloned().unwrap_or_default();
        self.screen.set_mouse_cursor(0, 0, &c);
        self.item_shapes = self.load_shape_file("ITEMSHP.SHP");
        self.thrown_shapes = self.load_shape_file("THROWN.SHP");
        self.effect_shapes = self.load_shape_file("ICE.SHP");
        self.fireball_shapes = self.load_shape_file("FIREBALL.SHP");
        self.heal_shapes = self.load_shape_file("HEAL.SHP");
        self.heali_shapes = self.load_shape_file("HEALI.SHP");
        for it in self.items_in_play.iter_mut() {
            *it = Default::default();
            it.shp_cur_frame_flg |= 0x8000;
        }
        self.run_init_script("ONETIME.INF", 0);
        self.item_script = self.load_script("ITEM.INF");
        self.load_supp_files_flag = 1;
        self.snd_load_sound_file_named("LORESFX");
        self.set_mouse_cursor_to_item_in_hand();
    }

    /// `startupNew`.
    pub fn startup_new(&mut self) {
        self.selected_spell = 0;
        self.compass_step = 0;
        self.compass_direction = -1;
        self.compass_direction_index = -1;
        self.last_mouse_region = -1;
        self.current_level = 1;
        self.give_credits(41, false);
        self.inventory[0] = self.make_item(216, 0, 0);
        self.inventory[1] = self.make_item(217, 0, 0);
        self.inventory[2] = self.make_item(218, 0, 0);
        self.available_spells[0] = 0;
        self.setup_screen_dims();
        self.global_script_vars2 = [0x100; 8];
        const SELECT_IDS: [i32; 4] = [-9, -1, -8, -5];
        let sel = self.char_selection.clamp(0, 3) as usize;
        self.add_character(SELECT_IDS[sel]);
        self.gui_enable_default_playfield_buttons();
        let lvl = self.current_level;
        self.load_level(lvl);
        self.screen.show_mouse();
    }

    /// `runLoop`.
    pub fn run_loop(&mut self) {
        self.enable_sys_timer(2);
        self.flags_table[73] |= 0x08;
        while !self.should_quit() {
            if self.game_to_load != -1 {
                let slot = self.game_to_load;
                self.load_game_state(slot);
                self.game_to_load = -1;
            }
            if self.next_script_func != 0 {
                let f = self.next_script_func;
                self.run_level_script(f, 2);
                self.next_script_func = 0;
            }
            self.timer_update();
            self.check_floating_pointer_regions();
            self.gui_update_input();
            self.update();
            self.update_play_timer();
            if self.scene_update_required {
                self.gui_draw_scene(0);
            } else {
                self.snd_update_environmental_sfx(0);
            }
            if self.party_damage_flags != -1 {
                self.check_for_party_death();
                self.party_damage_flags = -1;
            }
            self.delay(TICK, false);
        }
    }

    /// `update`: what runs whenever the game waits.
    pub fn update(&mut self) {
        self.update_sequence_background_animations();
        if self.update_char_num != -1 && self.millis() > self.update_portrait_next {
            self.update_portrait_speech_anim();
        }
        if self.flags_table[31] & 0x08 != 0 || self.update_flags & 4 == 0 {
            self.update_lamp_status();
        }
        if self.flags_table[31] & 0x40 != 0
            && self.update_flags & 4 == 0
            && (self.compass_direction == -1
                || ((self.current_direction as i32) << 6) != self.compass_direction
                || self.compass_step != 0)
        {
            self.update_compass();
        }
        self.snd_update_character_speech();
        self.fade_text_step();
        self.update_input();
        self.screen.update_screen();
    }

    pub fn update_sequence_background_animations(&mut self) {
        if self.update_flags & 8 != 0 {
            return;
        }
        for i in 0..6 {
            self.anim_update(i);
        }
    }

    /// `createTransparencyTables`: the CD version reads them with the level (`LEVELnn.TLC`).
    pub fn create_transparency_tables(&mut self) {}

    pub fn set_language_index(&mut self, index: usize) {
        self.lang_index = index.min(2);
        self.lang = match self.lang_index {
            1 => lol_data::paths::Language::French,
            2 => lol_data::paths::Language::German,
            _ => lol_data::paths::Language::English,
        };
        self.res.store.language = self.lang;
    }

    pub fn settings(&self) -> Settings {
        let m = self.screen.system.mixer();
        Settings {
            monster_difficulty: self.monster_difficulty,
            smooth_scrolling: self.smooth_scrolling,
            floating_cursors: self.floating_cursors,
            config_voice: self.config_voice,
            music_volume: m.music_volume,
            sfx_volume: m.sfx_volume,
            speech_volume: m.speech_volume,
        }
    }

    pub fn apply_settings(&mut self, s: Settings) {
        self.monster_difficulty = s.monster_difficulty.clamp(0, 2);
        self.smooth_scrolling = s.smooth_scrolling;
        self.floating_cursors = s.floating_cursors;
        self.config_voice = s.config_voice.clamp(0, 2);
        let mut m = self.screen.system.mixer();
        m.music_volume = s.music_volume;
        m.sfx_volume = s.sfx_volume;
        m.speech_volume = s.speech_volume;
        m.music_enabled = s.music_volume > 0;
        m.sfx_enabled = s.sfx_volume > 0;
        m.speech_enabled = s.speech_volume > 0;
    }

    /// `writeSettings`: handed to whoever keeps them (the window's settings file).
    pub fn write_settings(&mut self) {
        let s = self.settings();
        if let Some(f) = self.on_settings_changed.as_mut() {
            f(s);
        }
    }

    /// A figure of a file of figures, as `getPtrToShape` (for one-off uses).
    pub fn shape_of_file(&mut self, file: &str, index: usize) -> Vec<u8> {
        let Some(d) = self.res.file_data(file) else {
            return Vec::new();
        };
        self.screen.load_bitmap(&d, 3, None);
        shape::shape_copy(self.screen.page(3), index).unwrap_or_default()
    }
}
