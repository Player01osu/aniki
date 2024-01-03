#![allow(unreachable_code)]
#![allow(dead_code)]
use config::Config;
use database::Database;
use database::json_database::AnimeDatabaseData;
use sdl2::keyboard;
use sdl2::keyboard::TextInputUtil;
use sdl2::rect::Rect;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::{Window, WindowContext};
use sdl2::{
    event::Event,
    keyboard::Keycode,
    render::{Canvas, TextureCreator},
};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;
use ui::FontManager;
use ui::MostlyStatic;
use ui::Screen;
use ui::TextManager;
use ui::TextureManager;
use ui::WINDOW_HEIGHT;
use ui::WINDOW_WIDTH;
use ui::{color_hex, draw, BACKGROUND_COLOR};

mod config;
mod database;
mod ui;

const MOUSE_CLICK_LEFT: u8 = 0x00000001;
const MOUSE_CLICK_RIGHT: u8 = 0x00000002;
const MOUSE_MOVED: u8 = 0x00000004;
const RESIZED: u8 = 0x00000008;

pub struct App<'a, 'b> {
    pub canvas: Canvas<Window>,
    pub screen: Screen,
    pub input_util: TextInputUtil,
    pub text_manager: TextManager<'a, 'b>,
    pub image_manager: TextureManager<'a>,
    pub thumbnail_path: String,
    pub running: bool,

    pub id: u32,

    pub main_scroll: i32,
    pub main_selected: Option<usize>,
    pub main_extra_menu_id: Option<u32>,
    pub main_keyboard_override: bool,
    pub main_search_anime: Option<u32>,
    pub main_alias_anime: Option<u32>,
    pub main_search_previous: Option<(String, Box<[*const AnimeDatabaseData]>)>,

    pub episode_scroll: i32,

    // bitfield for:
    //   mouse_moved
    //   mouse_clicked_left
    //   mouse_clicked_right
    //   resized
    state_flag: u8,

    pub text_input: String,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub keycode_map: BTreeMap<u32, bool>,
    pub keymod: keyboard::Mod,
}
impl<'a, 'b> App<'a, 'b> {
    pub fn new(
        canvas: Canvas<Window>,
        input_util: TextInputUtil,
        ttf_ctx: &'a Sdl2TtfContext,
        texture_creator: &'a TextureCreator<WindowContext>,
        thumbnail_path: String,
    ) -> Self {
        Self {
            canvas,
            input_util,
            screen: Screen::Main,
            text_manager: TextManager::new(texture_creator, FontManager::new(ttf_ctx)),
            image_manager: TextureManager::new(texture_creator),
            running: true,
            thumbnail_path,

            id: 0,

            main_scroll: 0,
            main_selected: None,
            main_extra_menu_id: None,
            main_keyboard_override: false,
            main_search_anime: None,
            main_alias_anime: None,
            main_search_previous: None,

            episode_scroll: 0,

            state_flag: 0,

            mouse_x: 0,
            mouse_y: 0,

            text_input: String::new(),
            keycode_map: BTreeMap::new(),
            keymod: keyboard::Mod::NOMOD,
        }
    }

    pub fn get_string(&mut self, s: &str) -> Rc<str> {
        // TODO: intern this boy
        Rc::from(s)
    }

    pub fn set_screen(&mut self, screen: Screen) {
        let (window_width, window_height) = self.canvas.window().size();
        self.screen = screen;
        self.canvas
            .set_viewport(rect!(0, 0, window_width, window_height));
    }

    pub fn mouse_points(&self) -> (i32, i32) {
        (self.mouse_x, self.mouse_y)
    }

    pub fn keydown(&self, keycode: Keycode) -> bool {
        self.keycode_map
            .get(&(keycode as u32))
            .copied()
            .unwrap_or(false)
    }

    pub fn mouse_click_left_true(&mut self) {
        self.state_flag |= MOUSE_CLICK_LEFT;
    }

    pub fn mouse_click_right_true(&mut self) {
        self.state_flag |= MOUSE_CLICK_RIGHT;
    }

    pub fn mouse_moved_true(&mut self) {
        self.state_flag |= MOUSE_MOVED;
    }

    pub fn resized_true(&mut self) {
        self.state_flag |= RESIZED;
    }

    pub fn mouse_clicked_left_unset(&mut self) {
        self.state_flag &= !MOUSE_CLICK_LEFT;
    }

    pub fn mouse_clicked_left(&self) -> bool {
        // This value can only be observed once
        self.state_flag & MOUSE_CLICK_LEFT != 0
    }

    pub fn mouse_clicked_right(&self) -> bool {
        self.state_flag & MOUSE_CLICK_RIGHT != 0
    }

    pub fn mouse_moved(&self) -> bool {
        self.state_flag & MOUSE_MOVED != 0
    }

    pub fn resized(&self) -> bool {
        self.state_flag & RESIZED != 0
    }

    pub fn reset_frame_state(&mut self) {
        if self.mouse_moved() {
            self.main_keyboard_override = false;
        }

        self.keycode_map.clear();
        self.state_flag = 0;
        self.id = 0;
    }
}

#[tokio::main]
async fn main() {
    let cfg = Config::parse_cfg();
    let database_path = cfg.database_path().to_string_lossy();
    let thumbnail_path = cfg.thumbnail_path().to_string_lossy();
    let video_paths = cfg
        .video_paths()
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    video_subsystem.enable_screen_saver();

    let window = video_subsystem
        .window("Aniki", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

    let input_util = video_subsystem.text_input();
    input_util.stop();

    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .accelerated()
        .build()
        .unwrap();
    canvas.window_mut().set_minimum_size(700, 500).unwrap();
    canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
    let texture_creator = canvas.texture_creator();
    let ttf_ctx = sdl2::ttf::init().unwrap();
    let mut app = App::new(
        canvas,
        input_util,
        &ttf_ctx,
        &texture_creator,
        thumbnail_path.to_string(),
    );

    // TODO: Run this asynchronously and poll in draw loop
    let mut database = Database::new(database_path, video_paths).unwrap();
    database.retrieve_images(&thumbnail_path).unwrap();
    let mut mostly_static = MostlyStatic::new(database);

    app.canvas.clear();
    app.canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    'running: while app.running {
        if app.canvas.window().has_mouse_focus() {
            app.reset_frame_state()
        }
        app.canvas.set_draw_color(color_hex(BACKGROUND_COLOR));
        app.canvas.clear();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::MouseButtonDown { .. } => {}
                Event::MouseButtonUp {
                    mouse_btn: sdl2::mouse::MouseButton::Left,
                    ..
                } => {
                    app.mouse_click_left_true();
                }
                Event::MouseButtonUp {
                    mouse_btn: sdl2::mouse::MouseButton::Right,
                    ..
                } => {
                    app.mouse_click_right_true();
                }
                Event::MouseMotion { x, y, .. } => {
                    app.mouse_moved_true();
                    app.mouse_x = x;
                    app.mouse_y = y;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    app.keycode_map.insert(keycode as u32, true);
                    app.keymod = keymod;
                }
                Event::KeyUp {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    app.keycode_map.insert(keycode as u32, false);
                    app.keymod = keymod;
                }
                Event::Window {
                    win_event: sdl2::event::WindowEvent::Resized(_, _),
                    ..
                } => {
                    app.resized_true();
                    app.mouse_x = 0;
                    app.mouse_y = 0;
                }
                Event::TextInput { text, .. } => {
                    app.text_input.push_str(&text);
                }
                _ => {}
            }
        }
        draw(&mut app, &mut mostly_static);
        app.canvas.present();
        if app.canvas.window().has_mouse_focus() {
            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
        } else {
            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 3));
        }
    }

    // Do not write to cache while developing
    #[cfg(debug_assertions)]
    {
        return;
    }
    mostly_static.animes.write(cfg.database_path()).unwrap();
}
