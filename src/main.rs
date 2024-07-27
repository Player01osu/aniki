#![allow(unreachable_code)]
#![allow(dead_code)]

mod anilist_serde;
mod config;
mod database;
mod http;
mod ui;

use config::Config;
use database::episode::Episode;
use database::json_database::AnimeDatabaseData;
use database::{Anime, Database};
use http::{HttpData, HttpSender};
use lexopt::prelude::*;
use regex::Regex;
use sdl2::clipboard::ClipboardUtil;
use sdl2::keyboard::TextInputUtil;
use sdl2::keyboard::{self, Mod};
use sdl2::mouse::MouseButton;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Texture, TextureQuery};
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::{Window, WindowContext};
use sdl2::EventPump;
use sdl2::{
    event::Event,
    keyboard::Keycode,
    render::{Canvas, TextureCreator},
};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::ops::Sub;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;
use ui::Screen;
use ui::TextManager;
use ui::TextureManager;
use ui::WINDOW_HEIGHT;
use ui::WINDOW_WIDTH;
use ui::{color_hex, draw, BACKGROUND_COLOR};
use ui::{update_anilist_watched, FontManager};

use crate::http::{get_anilist_media_list, poll_http, send_login, send_request, RequestKind};
use crate::ui::layout::Layout;
use crate::ui::{INPUT_BOX_FONT_INFO, SCROLLBAR_COLOR};

const MOUSE_MOVED: u8      = 1 << 0;
const MOUSE_LEFT_UP: u8    = 1 << 1;
const MOUSE_LEFT_DOWN: u8  = 1 << 2;
const MOUSE_RIGHT_UP: u8   = 1 << 3;
const MOUSE_RIGHT_DOWN: u8 = 1 << 4;
const RESIZED: u8          = 1 << 5;
const ID_UPDATED: u8       = 1 << 6;

pub const CONNECTION_OVERLAY_TIMEOUT: f32 = 170.0;
pub const DEFAULT_VIDEO_PLAYER: &str = "mpv";

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Format {
    Truncate,
    CurrentMain,
    CurrentEpisode,
    NextMain,
    NextEpisode,
    Episode(u8),
}

pub struct StringManager {
    map: Vec<(*const u8, Format, String)>,
}

impl StringManager {
    pub fn new() -> Self {
        Self { map: vec![] }
    }

    pub fn load<'a, 'b>(
        &'a mut self,
        ptr: *const u8,
        format: Format,
        f: impl FnOnce() -> String,
    ) -> &'b str {
        match self
            .map
            .iter()
            .find(|(ptr_a, format_a, _)| *ptr_a == ptr && *format_a == format)
        {
            Some((_, _, s)) => unsafe { &*(s.as_str() as *const _) },
            None => {
                let s = f();
                self.map.push((ptr, format, s));
                unsafe { &*(self.map[self.map.len() - 1].2.as_str() as *const _) }
            }
        }
    }
}

#[derive(Debug)]
pub struct ConnectionOverlay {
    timeout: f32,
    state: ConnectionOverlayState,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionOverlayState {
    Connected,
    Disconnected,
}

#[derive(Debug, Default)]
pub struct SingleFlag {
    switch: Switch,
    textbox: Textbox,
}

#[derive(Debug, Default)]
pub struct BindFlag {
    switch: Switch,
    search_path_textbox: Textbox,
    flag_textbox: Textbox,
    deliminator_switch: Switch,
    deliminator_textbox: Textbox,
    regex_textbox: Textbox,
}

#[derive(Debug, Default)]
pub struct AttachFlagState {
    video_player_switch: Switch,
    video_player_textbox: Textbox,
    single_flags: Vec<SingleFlag>,
    bind_flags_switch: Switch,
    regex_textbox: Textbox,
    bind_flags: Vec<BindFlag>,
    scroll: Scroll,
    selectable: BTreeSet<usize>,
}

#[derive(Debug, Default)]
pub struct EpisodeState {
    episode_scroll: Scroll,
    selectable: BTreeSet<usize>,
}

#[derive(Debug, Default)]
pub struct AliasPopupState {
    selectable: BTreeSet<usize>,
    scroll: Scroll,
    textbox: Textbox,
}

#[derive(Debug, Default)]
pub struct TitlePopupState {
    selectable: BTreeSet<usize>,
    scroll: Scroll,
    textbox: Textbox,
}

#[derive(Debug, Default)]
pub struct LoginState {
    selectable: BTreeSet<usize>,
    textbox: Textbox,
}

#[derive(Debug, Default)]
pub struct MainState {
    pub selectable: BTreeSet<usize>,
    pub scroll: Scroll,
    pub extra_menu_scroll: Scroll,
    pub selected: Option<usize>,
    pub extra_menu_id: Option<u32>,
    pub keyboard_override: bool,
    pub search_anime: Option<u32>,
    pub alias_anime: Option<u32>,
    pub search_previous: Option<(String, Box<[*const AnimeDatabaseData]>)>,
}

#[derive(Debug, Clone, Default)]
pub struct Scroll {
    pub id: usize,
    pub scroll: i32,
    pub max_scroll: i32,
}

#[derive(Debug, Clone, Default)]
struct Textbox {
    id: usize,
    text: String,
    history: Vec<String>,
    history_time: f32,
    cursor_location: usize,
    view_offset: i32,
}

#[derive(Debug, Clone, Default)]
struct Switch {
    id: usize,
    toggled: bool,
}

impl Scroll {
    pub fn new() -> Self {
        Self {
            id: 0,
            scroll: 0,
            max_scroll: 0,
        }
    }
}

pub struct Context<'a, 'b> {
    pub canvas: Canvas<Window>,
    pub clipboard: ClipboardUtil,
    pub input_util: TextInputUtil,

    pub text_manager: TextManager<'a, 'b>,
    pub image_manager: TextureManager<'a>,
    pub string_manager: StringManager,

    /// bitfield for:
    /// 1  => mouse_moved
    /// 2  => mouse_left_up
    /// 4  => mouse_left_down
    /// 8  => mouse_right_up
    /// 16 => mouse_right_down
    /// 32 => resized
    /// 64 => id_updated
    state_flag: u8,

    event_pump: EventPump,

    weights: ScrollWeights,

    id: usize,
    id_map: Vec<(Rect, bool)>,
    id_scroll_map: Vec<usize>,
    scroll_id: Option<usize>,
    click_id: Option<usize>,
    click_id_right: Option<usize>,
    textbox_id: Option<usize>,

    pub text: String,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_scroll_x: f32,
    pub mouse_scroll_y: f32,
    pub mouse_scroll_y_accel: f32,
    pub keyset: HashSet<Keycode>,
    pub keyset_up: HashSet<Keycode>,
    pub keymod: keyboard::Mod,
}

pub struct ScreenStates {}

pub struct Scheduler {}

pub struct App<'a, 'b> {
    pub context: Context<'a, 'b>,
    pub next_screen: Option<Screen>,
    screen: Screen,

    pub thumbnail_path: String,
    pub database: Database<'a>,
    pub running: bool,
    pub show_toolbar: bool,
    pub frametime: std::time::Duration,

    pub http_rx: mpsc::Receiver<anyhow::Result<HttpData>>,
    pub http_tx: HttpSender,

    pub connection_overlay: ConnectionOverlay,
    pub login_progress: LoginProgress,

    pub main_state: MainState,
    pub episode_state: EpisodeState,
    pub login_state: LoginState,
    pub attach_flag_state: AttachFlagState,

    pub alias_popup_state: AliasPopupState,
    pub title_popup_state: TitlePopupState,
}

//pub fn update_watched(app: &mut App, anime: &mut Anime, ep: &Episode) {
pub fn update_watched(
    tx: &HttpSender,
    access_token: Option<String>,
    anime: &mut Anime,
    ep: &Episode,
) {
    anime.update_watched(ep.clone()).unwrap();
    if let Some(access_token) = access_token {
        update_anilist_watched(tx, &access_token, anime);
    }
}

fn get_search_path(s: &str, anime_paths: &[String]) -> Option<String> {
    if s.chars().next() == Some('/') {
        Some(s.to_string())
    } else {
        for p in anime_paths {
            let path = format!("{p}/{s}");
            if Path::new(&path).is_dir() {
                return Some(path);
            }
        }
        return None;
    }
}

fn get_captures(s: &str, regex: &Regex) -> Option<usize> {
    regex.captures(&s)?.get(1)?.as_str().parse().ok()
}

fn dir_pairing(path: &str, regex: &Regex) -> Vec<(usize, String)> {
    const MAX_DEPTH: usize = 5;
    let mut files = vec![];
    let mut stack = vec![];
    let mut depth = 0;
    stack.push(path.to_string());
    while let Some(path) = stack.pop() {
        depth += 1;
        if depth > MAX_DEPTH {
            break;
        }
        if let Ok(v) = std::fs::read_dir(path) {
            for f in v.filter_map(|v| v.ok()) {
                let s = o_to_str!(f.file_name());
                if let Some(n) = get_captures(&s, regex) {
                    files.push((n, o_to_str!(f.path())));
                }
                if let Ok(ftype) = f.file_type() {
                    if ftype.is_dir() {
                        stack.push(o_to_str!(f.path()));
                    }
                }
            }
        }
    }
    files
}

fn get_video_args(chosen_path: &str, anime: &Anime) -> Vec<String> {
    let mut args = vec![chosen_path.to_string()];
    for flag in &anime.single_flags {
        args.push(flag.flag.clone());
    }

    if anime.pair_flags.enabled {
        let video_regex = match Regex::new(&anime.pair_flags.video_regex) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("failed to create regex:{e}");
                return args;
            }
        };
        let videos = anime
            .episodes()
            .iter()
            .map(|(_, v)| o_to_str!(Path::new(&v[0]).file_name().unwrap()).to_string())
            .filter_map(|s| {
                Some((
                    video_regex
                        .captures(&s)?
                        .get(1)?
                        .as_str()
                        .parse::<usize>()
                        .ok()?,
                    s,
                ))
            })
            .collect::<Vec<(usize, String)>>();
        dbg!(&videos);
        let filename = Path::new(chosen_path).file_name().unwrap();
        for flag in anime.pair_flags.pair_flags.iter().filter(|v| v.enabled) {
            dbg!(&flag);
            let others = {
                let regex = match Regex::new(&flag.regex) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("failed to create regex:{}:{e}", &flag.regex);
                        continue;
                    }
                };
                let path = match get_search_path(&flag.search_path, &anime.paths()) {
                    Some(v) => v,
                    None => {
                        eprintln!("failed to find search path:{}", &flag.search_path);
                        continue;
                    }
                };
                dbg!(dir_pairing(&path, &regex))
            };

            for (n, path) in others {
                if let Some((_, video_path)) = videos.iter().find(|(n1, _)| *n1 == n) {
                    dbg!(&video_path);
                    if filename == Path::new(video_path).file_name().unwrap() {
                        let deliminator = if flag.use_deliminator {
                            flag.deliminator.clone()
                        } else {
                            " ".to_string()
                        };
                        let arg = dbg!(format!("{}{deliminator}{}", flag.flag, path));
                        args.push(arg);
                    }
                }
            }
        }
    }

    return dbg!(args);
}

fn open_video(path: &str, anime: &Anime) {
    let video_player = anime
        .video_player
        .clone()
        .unwrap_or(DEFAULT_VIDEO_PLAYER.to_string());
    let args = get_video_args(path, anime);
    tokio::task::spawn(async move {
        Command::new(&video_player).args(&args).spawn().unwrap().wait().unwrap();
    });
}

fn textbox(
    context: &mut Context,
    textbox_state: &mut Textbox,
    label: Option<&str>,
    enabled: bool,
    sidepad: i32,
    region: &mut Rect,
) -> bool {
    context.input_util.start();
    let font_info = INPUT_BOX_FONT_INFO;
    let text_color = color_hex(0xB0B0B0);
    let height = context.text_manager.font_height(font_info);
    let (label_region, new_region) = match label {
        Some(_) => region.split_hori(height + 8, region.height()),
        None => (rect!(0, 0, 0, 0), *region),
    };
    let (text_border_region, new_region) = new_region.split_hori(height + 12, region.height());
    let text_border_region = text_border_region.pad_left(sidepad).pad_right(sidepad);
    let text_region = Rect::from_center(
        text_border_region.center(),
        text_border_region.width() - 10,
        height,
    );

    if let Some(label) = label {
        let label_texture = context
            .text_manager
            .load(label, font_info, text_color, None);
        let TextureQuery {
            width: label_width,
            height: label_height,
            ..
        } = label_texture.query();
        let label_rect = rect!(
            text_border_region.x(),
            label_region.y(),
            label_width,
            label_height
        );
        context
            .canvas
            .copy(&label_texture, None, label_rect)
            .unwrap();
    }

    *region = new_region.pad_top(6);
    textbox_state.id = context.create_id(text_border_region);
    if context.click_elem(textbox_state.id) {
        context.textbox_id = Some(textbox_state.id);
        context.text.clear();
    } else if false && context.mouse_left_up() && context.click_id != Some(textbox_state.id) {
        // TODO: This breaks selecting textboxes with multiple textboxes.
        //
        // Because the click id for the next thing is not the same, it sets textbox_id
        // even though the previous textbox was clicked
        context.textbox_id = None;
    }

    context.canvas.set_draw_color(color_hex(0x909090));
    context.canvas.draw_rect(text_border_region).unwrap();

    let mut cursor_offset = 0;
    if !textbox_state.text.is_empty() {
        let font_texture =
            context
                .text_manager
                .load(&textbox_state.text, font_info, text_color, None);
        let TextureQuery { width, height, .. } = font_texture.query();
        debug_assert!(textbox_state.cursor_location <= textbox_state.text.len());
        cursor_offset = context
            .text_manager
            .text_size(
                font_info,
                &textbox_state.text[0..textbox_state.cursor_location],
            )
            .0 as i32;
        if cursor_offset < -textbox_state.view_offset {
            textbox_state.view_offset += -textbox_state.view_offset - cursor_offset;
        } else if cursor_offset > -textbox_state.view_offset + text_region.width() as i32 {
            textbox_state.view_offset -=
                cursor_offset + textbox_state.view_offset - text_region.width() as i32;
        }
        let text_rect = rect!(
            text_region.x + textbox_state.view_offset,
            text_region.y,
            width,
            height
        );
        context.canvas.set_clip_rect(text_region);
        context.canvas.copy(&font_texture, None, text_rect).unwrap();
        context.canvas.set_clip_rect(None);
    }

    fn is_skip_char(c: char) -> bool {
        !c.is_ascii_alphanumeric()
    }

    if !enabled {
        context
            .canvas
            .set_blend_mode(sdl2::render::BlendMode::Blend);
        context
            .canvas
            .set_draw_color(Color::RGBA(0x10, 0x10, 0x10, 0xA0));
        context.canvas.fill_rect(text_border_region).unwrap();
    } else if context.textbox_id == Some(textbox_state.id) {
        use sdl2::keyboard::Scancode::*;
        //use Keycode::*;
        let key = |k| context.keyset.contains(&Keycode::from_scancode(k).unwrap());
        let kmod = |m| context.keymod.contains(m);
        let cursor_rect = rect!(
            text_region.x + textbox_state.view_offset + cursor_offset,
            text_region.y,
            1,
            height
        );
        textbox_state.cursor_location = textbox_state.cursor_location.min(textbox_state.text.len());
        context.canvas.set_draw_color(text_color);
        context.canvas.fill_rect(cursor_rect).unwrap();
        for c in context.text.drain(..) {
            if !kmod(Mod::LCTRLMOD) && !kmod(Mod::LALTMOD) {
                textbox_state.text.insert(textbox_state.cursor_location, c);
                textbox_state.cursor_location += 1;
            }
        }
        if kmod(Mod::LCTRLMOD) && key(Z) {
            // TODO
        } else if (kmod(Mod::LCTRLMOD) && key(Left)) || (kmod(Mod::LALTMOD) && key(B)) {
            let bytes = textbox_state.text.as_bytes();
            if textbox_state.cursor_location > 0 {
                while textbox_state.cursor_location > 0
                    && is_skip_char(bytes[textbox_state.cursor_location - 1] as char)
                {
                    textbox_state.cursor_location -= 1;
                }

                while textbox_state.cursor_location > 0
                    && !is_skip_char(bytes[textbox_state.cursor_location - 1] as char)
                {
                    textbox_state.cursor_location -= 1;
                }
            }
        } else if kmod(Mod::LCTRLMOD) && key(Right) || (kmod(Mod::LALTMOD) && key(F)) {
            let bytes = textbox_state.text.as_bytes();
            let len = textbox_state.text.len();
            while textbox_state.cursor_location < len
                && is_skip_char(bytes[textbox_state.cursor_location] as char)
            {
                textbox_state.cursor_location += 1;
            }

            while textbox_state.cursor_location < len
                && !is_skip_char(bytes[textbox_state.cursor_location] as char)
            {
                textbox_state.cursor_location += 1;
            }

            while textbox_state.cursor_location < len
                && !(bytes[textbox_state.cursor_location] as char).is_whitespace()
                && !(bytes[textbox_state.cursor_location] as char).is_ascii_alphanumeric()
            {
                textbox_state.cursor_location += 1;
            }
        } else if kmod(Mod::LCTRLMOD) && (key(Backspace) || key(W)) {
            let bytes = textbox_state.text.as_bytes();
            if textbox_state.cursor_location > 0 {
                let end = textbox_state.cursor_location;
                while textbox_state.cursor_location > 0
                    && is_skip_char(bytes[textbox_state.cursor_location - 1] as char)
                {
                    textbox_state.cursor_location -= 1;
                }

                while textbox_state.cursor_location > 0
                    && !is_skip_char(bytes[textbox_state.cursor_location - 1] as char)
                {
                    textbox_state.cursor_location -= 1;
                }
                let end_range = end - textbox_state.cursor_location;
                let (lhs, rhs) = textbox_state.text.split_at(textbox_state.cursor_location);
                let mut new_text = lhs.to_string();
                new_text.push_str(&rhs[end_range..]);
                textbox_state.text = new_text;
            }
        } else if key(End) || (kmod(Mod::LCTRLMOD) && key(E)) {
            textbox_state.cursor_location = textbox_state.text.len();
        } else if key(Home) || (kmod(Mod::LCTRLMOD) && key(A)) {
            textbox_state.cursor_location = 0;
        } else if key(Backspace) {
            if textbox_state.cursor_location > 0 {
                textbox_state.text.remove(textbox_state.cursor_location - 1);
                textbox_state.cursor_location -= 1;
            }
        } else if kmod(Mod::LCTRLMOD) && key(U) {
            let (_, new_text) = textbox_state.text.split_at(textbox_state.cursor_location);
            textbox_state.text = new_text.to_string();
            textbox_state.cursor_location = 0;
        } else if key(Left) || (kmod(Mod::LCTRLMOD) && key(B)) {
            if textbox_state.cursor_location > 0 {
                textbox_state.cursor_location -= 1;
            }
        } else if key(Right) || (kmod(Mod::LCTRLMOD) && key(F)) {
            if textbox_state.cursor_location < textbox_state.text.len() {
                textbox_state.cursor_location += 1;
            }
        } else if kmod(Mod::LCTRLMOD) && key(V) {
            match context.clipboard.clipboard_text() {
                Ok(s) => {
                    textbox_state.text.push_str(&s);
                    textbox_state.cursor_location += s.len();
                }
                Err(e) => {
                    dbg!(e);
                }
            };
        }

        return key(Return);
    }

    false
}

impl<'a> App<'a, '_> {
    pub fn new(
        canvas: Canvas<Window>,
        clipboard: ClipboardUtil,
        database: Database<'a>,
        input_util: TextInputUtil,
        ttf_ctx: &'a Sdl2TtfContext,
        texture_creator: &'a TextureCreator<WindowContext>,
        thumbnail_path: String,
        event_pump: EventPump,
    ) -> Self {
        let (http_tx, http_rx) = mpsc::channel();

        Self {
            context: Context::new(
                canvas,
                clipboard,
                input_util,
                ttf_ctx,
                texture_creator,
                event_pump,
            ),
            database,
            next_screen: None,
            screen: Screen::Main,
            frametime: std::time::Duration::default(),

            running: true,
            thumbnail_path,

            show_toolbar: false,

            http_tx,
            http_rx,

            login_progress: LoginProgress::None,
            connection_overlay: ConnectionOverlay {
                timeout: CONNECTION_OVERLAY_TIMEOUT,
                state: ConnectionOverlayState::Disconnected,
            },

            main_state: MainState::default(),
            episode_state: EpisodeState::default(),
            login_state: LoginState::default(),
            alias_popup_state: AliasPopupState::default(),
            title_popup_state: TitlePopupState::default(),
            attach_flag_state: AttachFlagState::default(),
        }
    }

    pub fn mouse_points(&self) -> (i32, i32) {
        self.context.mouse_points()
    }

    pub fn keydown(&self, keycode: Keycode) -> bool {
        self.context.keydown(keycode)
    }

    pub fn keyup(&self, keycode: Keycode) -> bool {
        self.context.keyup(keycode)
    }

    pub fn frametime_frac(&self) -> f32 {
        // TODO: Timing still needs to be fixed.
        //
        // Specifically, as it relates to scrolling, the timing should be scaled so
        ((self.frametime.as_micros() as f64 / 16.0) / 1200.0) as f32
    }

    pub fn reset_frame_state(&mut self) {
        self.context.mouse_scroll_y_accel = self.context.mouse_scroll_y_accel
            //* self.frametime_frac()
            / self.context.weights.decel_decel;
        self.context.mouse_scroll_y =
            self.context.mouse_scroll_y * self.context.mouse_scroll_y_accel * 0.1
                / self.context.weights.decel;

        if self.context.mouse_left_up() {
            self.context.click_id = None;
            self.context.state_flag &= !(MOUSE_LEFT_UP | MOUSE_LEFT_DOWN);
        }

        if self.context.mouse_right_up() {
            self.context.click_id = None;
            self.context.state_flag &= !(MOUSE_RIGHT_UP | MOUSE_RIGHT_DOWN);
        }

        self.context.mouse_update_id();
        self.context.scroll_update_id();
        self.context.id = 0;

        self.context.keyset.clear();
        self.context.keyset_up.clear();

        self.context.state_flag &= !(MOUSE_MOVED | RESIZED | ID_UPDATED);
    }

    fn swap_screen(&mut self) {
        if let Some(next_screen) = self.next_screen.take() {
            self.screen = next_screen;
            self.context
                .id_map
                .resize(0, (Rect::new(0, 0, 0, 0), false));
        }
    }
}

fn switch(
    context: &mut Context,
    switch_state: &mut Switch,
    label: &str,
    region: &mut Rect,
) -> bool {
    const SIDE_PAD: i32 = 34;
    let height = context.text_manager.font_height(INPUT_BOX_FONT_INFO);
    let (switch_region, new_region) = region.split_hori(height as u32 + 12, region.height());
    *region = new_region;
    let switch_size = 70;
    let (label_region, switchable_region) =
        switch_region.split_vert(switch_region.width() - switch_size, switch_region.width());
    let switchable_region = switchable_region
        //.pad_right(SIDE_PAD)
        .pad_top(8)
        .pad_bottom(16);
    let switchable_region = rect!(
        switchable_region.x() - SIDE_PAD,
        switchable_region.y(),
        switchable_region.width(),
        switchable_region.height()
    );
    switch_state.id = context.create_id(switchable_region);

    let label_region = label_region.pad_left(SIDE_PAD);

    let label_texture =
        context
            .text_manager
            .load(label, INPUT_BOX_FONT_INFO, color_hex(0xB0B0B0), None);
    let TextureQuery { width, height, .. } = label_texture.query();
    let label_center = label_region.center();
    let label_rect = rect!(
        label_region.x,
        label_center.y() - height as i32 / 2,
        width,
        height
    );
    context
        .canvas
        .copy(&label_texture, None, label_rect)
        .unwrap();

    if context.click_elem(switch_state.id) {
        switch_state.toggled = !switch_state.toggled;
    }

    if switch_state.toggled {
        let (_slider, head) =
            switchable_region.split_vert(switchable_region.width() - 25, switchable_region.width());
        context.canvas.set_draw_color(color_hex(0x6B549C));
        context.canvas.fill_rect(switchable_region).unwrap();
        context.canvas.set_draw_color(color_hex(0x304A6C));
        context.canvas.fill_rect(head).unwrap();
    } else {
        let (head, _slider) = switchable_region.split_vert(25, switchable_region.width());
        context.canvas.set_draw_color(color_hex(0x707070));
        context.canvas.fill_rect(switchable_region).unwrap();
        context.canvas.set_draw_color(color_hex(0xB0B0B0));
        context.canvas.fill_rect(head).unwrap();
    }
    switch_state.toggled
}

/// Registers area to be scrollable and draws scrollbar
fn register_scroll(context: &mut Context, scroll: &mut Scroll, region: &mut Rect) {
    scroll.id = context.create_id(*region);
    context.id_scroll_map.push(scroll.id);

    if scroll.max_scroll as u32 >= region.height() {
        const PAD_TOP: u32 = 0;
        let (new_region, scroll_layout) = region.split_vert(796, 800);
        let bar_height = scroll_layout.height() * scroll_layout.height() / scroll.max_scroll as u32;
        let bar_scroll = scroll.scroll * scroll_layout.height() as i32 / scroll.max_scroll;
        let bar_rect = rect!(
            scroll_layout.x(),
            scroll_layout.y() - bar_scroll + PAD_TOP as i32 / 2,
            scroll_layout.width(),
            bar_height - PAD_TOP
        );
        context.canvas.set_draw_color(color_hex(SCROLLBAR_COLOR));
        context.canvas.fill_rect(bar_rect).unwrap();
        *region = new_region;
    }

    if context.scroll_id == Some(scroll.id) {
        if context.keyset.contains(&Keycode::J)
            && scroll.scroll - region.height() as i32 >= -scroll.max_scroll
        {
            scroll.scroll -= 6;
        }

        if context.keyset.contains(&Keycode::K) && scroll.scroll <= 0 {
            scroll.scroll += 6;
        }

        scroll.scroll = scroll
            .scroll
            .saturating_add(context.mouse_scroll_y as i32)
            .clamp(-scroll.max_scroll, 0);
    }

    scroll.scroll = scroll
        .scroll
        .max(-scroll.max_scroll + region.height() as i32);
    scroll.scroll = scroll.scroll.min(0);
}

impl<'a> Context<'a, '_> {
    fn new(
        canvas: Canvas<Window>,
        clipboard: ClipboardUtil,
        input_util: TextInputUtil,
        ttf_ctx: &'a Sdl2TtfContext,
        texture_creator: &'a TextureCreator<WindowContext>,
        event_pump: EventPump,
    ) -> Self {
        Self {
            canvas,
            clipboard,
            input_util,
            text_manager: TextManager::new(texture_creator, FontManager::new(ttf_ctx)),
            image_manager: TextureManager::new(texture_creator),
            string_manager: StringManager::new(),
            event_pump,

            weights: ScrollWeights {
                accel: 10.990031,
                accel_accel: 1.9800003,
                decel_decel: 3.119999,
                decel: 1.3700006,
            },

            state_flag: 0,

            mouse_x: 0,
            mouse_y: 0,
            mouse_scroll_x: 0.0,
            mouse_scroll_y: 0.0,
            mouse_scroll_y_accel: 0.0,

            id: 0,
            id_map: vec![(Rect::new(0, 0, 0, 0), false); 16],
            id_scroll_map: vec![],
            scroll_id: None,
            click_id: None,
            click_id_right: None,
            textbox_id: None,

            text: String::new(),
            keyset: HashSet::new(),
            keyset_up: HashSet::new(),
            keymod: keyboard::Mod::NOMOD,
        }
    }

    pub fn mouse_points(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }

    pub fn keydown(&self, keycode: Keycode) -> bool {
        self.keyset.contains(&keycode)
    }

    pub fn keyup(&self, keycode: Keycode) -> bool {
        self.keyset_up.contains(&keycode)
    }

    pub const fn mouse_moved(&self) -> bool {
        self.state_flag & MOUSE_MOVED != 0
    }

    pub const fn mouse_left_up(&self) -> bool {
        self.state_flag & MOUSE_LEFT_UP != 0
    }

    pub const fn mouse_left_down(&self) -> bool {
        self.state_flag & MOUSE_LEFT_DOWN != 0
    }

    pub const fn mouse_right_up(&self) -> bool {
        self.state_flag & MOUSE_RIGHT_UP != 0
    }

    pub const fn mouse_right_down(&self) -> bool {
        self.state_flag & MOUSE_RIGHT_DOWN != 0
    }

    pub const fn resized(&self) -> bool {
        self.state_flag & RESIZED != 0
    }

    pub const fn id_updated(&self) -> bool {
        self.state_flag & ID_UPDATED != 0
    }

    fn poll_event(&mut self) -> Option<bool> {
        for event in self.event_pump.poll_iter() {
            self.state_flag |= event.is_mouse() as u8;

            match event {
                Event::Quit { .. } => return None,
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    self.state_flag |= 4;
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    self.state_flag |= 16;
                }

                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    self.state_flag |= 2;
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    self.state_flag |= 8;
                }
                Event::MouseWheel { precise_y, .. } => {
                    if true || self.mouse_scroll_y.abs() <= 100.0 {
                        self.mouse_scroll_y_accel += self.weights.accel_accel * 0.32;
                        self.mouse_scroll_y += precise_y.signum()
                            * scroll_func((precise_y * self.weights.accel).abs());
                    }
                    self.mouse_scroll_y_accel = self.mouse_scroll_y_accel.signum()
                        * (100.0f32).min(self.mouse_scroll_y_accel.abs());
                    self.mouse_scroll_y =
                        self.mouse_scroll_y.signum() * (100.0f32).min(self.mouse_scroll_y.abs());
                    //self.mouse_scroll_x += precise_x * 8.3 * app.frametime_frac();
                }
                Event::MouseMotion { x, y, .. } => {
                    self.mouse_x = x;
                    self.mouse_y = y;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    self.keyset.insert(keycode);
                    self.keymod = keymod;
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    self.keyset_up.insert(keycode);
                    self.keymod = keymod;
                }
                Event::Window {
                    win_event: sdl2::event::WindowEvent::Resized(_, _),
                    ..
                } => {
                    self.state_flag |= 32;
                    self.mouse_x = 0;
                    self.mouse_y = 0;
                }
                Event::TextInput { text, .. } => {
                    self.text = text;
                }
                _ => {}
            }
            return Some(true);
        }

        Some(false)
    }

    fn scroll_update_id(&mut self) {
        if self.mouse_moved() {
            let mouse_point = self.mouse_point();
            for id in self.id_scroll_map.iter().rev() {
                let region = self.rect_id(*id);
                if region.contains_point(mouse_point) {
                    self.scroll_id = Some(*id);
                    return;
                }
            }
            self.scroll_id = None;
        }
        self.id_scroll_map.clear();
    }

    fn mouse_update_id(&mut self) {
        if self.mouse_moved() {
            let mouse_point = self.mouse_point();
            self.reset_id();
            for (region, select) in self.id_map.iter_mut().rev() {
                if region.contains_point(mouse_point) {
                    *select = true;
                    return;
                }
            }
        }
    }

    fn select_id(&mut self, id: usize) {
        self.id_map[id].1 = true;
    }

    fn create_id(&mut self, region: Rect) -> usize {
        let id = self.id;
        if id >= self.id_map.len() {
            self.id_map.push((Rect::new(0, 0, 0, 0), false));
        }
        self.id_map[id].0 = region;
        self.id += 1;
        return id;
    }

    fn state_id(&self, id: usize) -> bool {
        self.id_map[id].1
    }

    fn register_click(&mut self, id: usize) {
        if self.mouse_left_down() && self.state_id(id) && self.click_id.is_none() {
            self.click_id = Some(id);
        }
    }

    fn register_click_right(&mut self, id: usize) {
        if self.mouse_right_down() && self.state_id(id) && self.click_id.is_none() {
            self.click_id_right = Some(id);
        }
    }

    fn check_click(&self, id: usize) -> bool {
        self.click_id == Some(id) && self.state_id(id) && self.mouse_left_up()
    }

    fn check_click_right(&self, id: usize) -> bool {
        self.click_id_right == Some(id) && self.state_id(id) && self.mouse_right_up()
    }

    fn check_return(&self, id: usize) -> bool {
        self.keyset_up.contains(&Keycode::Return) && self.state_id(id)
    }

    fn click_elem(&mut self, id: usize) -> bool {
        self.register_click(id);
        self.check_click(id) || self.check_return(id)
    }

    fn click_elem_right(&mut self, id: usize) -> bool {
        self.register_click_right(id);
        self.check_click_right(id)
    }

    fn mouse_region(&self, region: Rect) -> bool {
        region.contains_point(self.mouse_point())
    }

    fn rect_id(&self, id: usize) -> Rect {
        self.id_map[id].0
    }

    fn mouse_point(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }

    fn reset_id(&mut self) {
        self.id_map.truncate(self.id);
        for (_, selected) in self.id_map.iter_mut() {
            *selected = false;
        }
    }

    fn window_rect(&self) -> Rect {
        let (width, height) = self.canvas.window().size();
        rect!(0, 0, width, height)
    }
}

fn lock_file() -> anyhow::Result<()> {
    match fs::read_to_string("/tmp/aniki.lock") {
        Ok(v) if Path::new(&format!("/proc/{v}")).exists() => {
            anyhow::bail!("Lock file exists! PID:{v}")
        }
        _ => {
            fs::write("/tmp/aniki.lock", std::process::id().to_string())?;
            Ok(())
        }
    }
}

fn release_lock_file() -> anyhow::Result<()> {
    if std::path::Path::new("/tmp/aniki.lock").exists() {
        fs::remove_file("/tmp/aniki.lock")?;
    }

    Ok(())
}

pub enum LoginProgress {
    None,
    Started,
    Failed,
}

#[derive(Debug)]
struct ScrollWeights {
    accel: f32,
    accel_accel: f32,
    decel_decel: f32,
    decel: f32,
}

fn scroll_func(x: f32) -> f32 {
    x
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lock_file()?;
    let cfg = Config::parse_cfg();

    let mut avg_time = [0.0; 60];
    let mut frame_num = 0;
    let mut show_fps_time = std::time::Instant::now();
    let mut prev_time = std::time::Instant::now();

    let mut show_fps = false;
    let mut force_vsync = false;
    let mut args_parser = lexopt::Parser::from_env();

    while let Some(arg) = args_parser.next()? {
        match arg {
            Short('f') | Long("show-fps") => {
                show_fps = true;
            }
            Short('F') | Long("force-vsync") => {
                force_vsync = true;
            }
            _ => {
                anyhow::Result::Err(arg.unexpected())?;
            }
        }
    }

    let database_path = cfg.database_path().to_string_lossy();
    let thumbnail_path = cfg.thumbnail_path().to_string_lossy();
    let video_paths = cfg
        .video_paths()
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect();

    let sdl_context = sdl2::init().map_err(|e| anyhow::anyhow!(e))?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "1");
    sdl2::hint::set("SDL_RENDER_BATCHING", "1");
    let video_subsystem = sdl_context.video().map_err(|e| anyhow::anyhow!(e))?;
    video_subsystem.enable_screen_saver();

    let window = video_subsystem
        .window("Aniki", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .build()?;

    let clipboard = video_subsystem.clipboard();
    let input_util = video_subsystem.text_input();
    input_util.stop();

    let mut canvas = if force_vsync {
        window.into_canvas().present_vsync().accelerated().build()?
    } else {
        window.into_canvas().accelerated().build()?
    };
    canvas.window_mut().set_minimum_size(700, 500)?;
    canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
    let texture_creator = canvas.texture_creator();
    let ttf_ctx = sdl2::ttf::init()?;
    // TODO: Run this asynchronously and poll in draw loop
    let mut database = Database::new(database_path, video_paths)?;
    database.retrieve_images(&thumbnail_path)?;

    let mut screen = {
        if !database.skip_login() && database.anilist_cred().is_none() {
            Screen::Login
        } else {
            Screen::Main
        }
    };

    let event_pump = sdl_context.event_pump().map_err(|e| anyhow::anyhow!(e))?;

    let mut app = App::new(
        canvas,
        clipboard,
        database,
        input_util,
        &ttf_ctx,
        &texture_creator,
        thumbnail_path.to_string(),
        event_pump,
    );

    if let Some(cred) = app.database.anilist_cred() {
        send_login(&app.http_tx, cred.access_token());
        get_anilist_media_list(&app.http_tx, cred.user_id(), cred.access_token());
    }

    enum CanvasTexture<'a> {
        Cached(Texture<'a>),
        Wait(f32),
    }

    const IDLE_TIME: f32 = 10.0;
    let mut canvas_texture = CanvasTexture::Wait(IDLE_TIME);

    app.context.canvas.clear();
    app.context.canvas.present();
    'running: while app.running {
        // TODO: Id needs to get reset even when the window is not in focus
        if true
            || app.context.canvas.window().has_input_focus()
            || app.context.canvas.window().has_mouse_focus()
        {
            app.reset_frame_state()
        }

        match app.context.poll_event() {
            Some(true) => {
                canvas_texture = CanvasTexture::Wait(IDLE_TIME);
            }
            None => break 'running,
            _ => (),
        }

        match canvas_texture {
            CanvasTexture::Cached(ref texture) => {
                app.context.canvas.copy(texture, None, None).unwrap();
            }
            CanvasTexture::Wait(ref mut t) => {
                *t = t.sub(app.frametime_frac()).max(0.0);
                poll_http(&mut app);

                app.context
                    .canvas
                    .set_draw_color(color_hex(BACKGROUND_COLOR));
                app.context.canvas.clear();

                draw(&mut app, &mut screen);
                if *t <= 0.0 && app.connection_overlay.timeout <= 0.0 {
                    let (width, height) = app.context.canvas.window().size();
                    let pixel_format = app.context.canvas.default_pixel_format();
                    let pitch = pixel_format.byte_size_per_pixel() * width as usize;
                    let pixels = app
                        .context
                        .canvas
                        .read_pixels(app.context.window_rect(), pixel_format)
                        .unwrap();
                    let mut texture = texture_creator
                        .create_texture_static(pixel_format, width, height)
                        .unwrap();
                    texture.update(None, &pixels, pitch).unwrap();
                    canvas_texture = CanvasTexture::Cached(texture);
                }
            }
        }
        app.context.canvas.present();

        if show_fps {
            let time = prev_time.elapsed().as_secs_f64();
            avg_time[frame_num] = time;
            if show_fps_time.elapsed().as_secs() == 1 {
                show_fps_time = std::time::Instant::now();
                let avg = avg_time.iter().sum::<f64>() / avg_time.len() as f64;
                let avg_fps = 1.0 / avg;
                let fps = 1.0 / time;
                println!("DEBUG:avg:{}, current:{}", avg_fps, fps);
            }

            frame_num = (frame_num + 1) % avg_time.len();
        }

        if false {
            let mut scroll_change = 0.01;
            if app.keydown(Keycode::A) {
                if app.context.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.context.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.context.weights.accel += scroll_change;
                dbg!(&app.context.weights);
            }
            if app.keydown(Keycode::S) {
                if app.context.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.context.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.context.weights.accel_accel += scroll_change;
                dbg!(&app.context.weights);
            }
            if app.keydown(Keycode::D) {
                if app.context.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.context.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.context.weights.decel += scroll_change;
                dbg!(&app.context.weights);
            }
            if app.keydown(Keycode::F) {
                if app.context.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.context.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.context.weights.decel_decel += scroll_change;
                dbg!(&app.context.weights);
            }
        }

        app.frametime = prev_time.elapsed();
        prev_time = std::time::Instant::now();

        if matches!(canvas_texture, CanvasTexture::Cached(_)) {
            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
        }
        if !(app.context.canvas.window().has_input_focus()
            || app.context.canvas.window().has_mouse_focus())
        {
            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
        }
    }

    release_lock_file()?;

    // Do not write to cache while developing
    #[cfg(debug_assertions)]
    if true {
        return Ok(());
    }

    app.database.write(cfg.database_path())?;

    Ok(())
}
