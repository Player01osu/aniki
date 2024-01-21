#![allow(unreachable_code)]
#![allow(dead_code)]
use anilist_serde::{MediaEntry, MediaList, Viewer};
use config::Config;
use database::json_database::AnimeDatabaseData;
use database::{AniListCred, Database};
use reqwest::RequestBuilder;
use sdl2::clipboard::ClipboardUtil;
use sdl2::keyboard;
use sdl2::keyboard::TextInputUtil;
use sdl2::rect::Rect;
use sdl2::render::Texture;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::{Window, WindowContext};
use sdl2::{
    event::Event,
    keyboard::Keycode,
    render::{Canvas, TextureCreator},
};
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ui::login_screen::{get_anilist_media_list, send_login};
use ui::Screen;
use ui::TextManager;
use ui::TextureManager;
use ui::WINDOW_HEIGHT;
use ui::WINDOW_WIDTH;
use ui::{color_hex, draw, BACKGROUND_COLOR};
use ui::{update_anilist_watched, FontManager};

mod anilist_serde;
mod config;
mod database;
mod ui;

const MOUSE_CLICK_LEFT: u8 = 0x00000001;
const MOUSE_CLICK_RIGHT: u8 = 0x00000002;
const MOUSE_MOVED: u8 = 0x00000004;
const RESIZED: u8 = 0x00000008;
pub const CONNECTION_OVERLAY_TIMEOUT: u16 = 210;

#[derive(Debug, Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Format {
    Truncate,
}

pub struct StringManager {
    map: Vec<(*const u8, Format, String)>,
}

impl StringManager {
    pub fn new() -> Self {
        Self { map: vec![] }
    }

    pub fn load(&mut self, ptr: *const u8, format: Format, f: impl FnOnce() -> String) -> &str {
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
    timeout: u16,
    state: ConnectionOverlayState,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionOverlayState {
    Connected,
    Disconnected,
}

pub struct App<'a, 'b> {
    pub canvas: Canvas<Window>,
    pub clipboard: ClipboardUtil,
    pub next_screen: Option<Screen>,
    pub input_util: TextInputUtil,
    pub text_manager: TextManager<'a, 'b>,
    pub image_manager: TextureManager<'a>,
    pub string_manager: StringManager,
    pub thumbnail_path: String,
    pub database: Database<'a>,
    pub running: bool,
    pub show_toolbar: bool,

    pub mutex: HttpMutex,

    pub id: u32,

    pub connection_overlay: ConnectionOverlay,
    pub login_progress: LoginProgress,

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
    pub mouse_scroll_x: f32,
    pub mouse_scroll_y: f32,
    pub mouse_scroll_y_accel: f32,
    pub keycode_map: BTreeMap<u32, bool>,
    pub keymod: keyboard::Mod,
}

impl<'a, 'b> App<'a, 'b> {
    pub fn new(
        canvas: Canvas<Window>,
        clipboard: ClipboardUtil,
        database: Database<'a>,
        input_util: TextInputUtil,
        ttf_ctx: &'a Sdl2TtfContext,
        texture_creator: &'a TextureCreator<WindowContext>,
        thumbnail_path: String,
    ) -> Self {
        Self {
            canvas,
            clipboard,
            database,
            input_util,
            next_screen: None,
            text_manager: TextManager::new(texture_creator, FontManager::new(ttf_ctx)),
            image_manager: TextureManager::new(texture_creator),
            string_manager: StringManager::new(),

            running: true,
            thumbnail_path,

            show_toolbar: false,

            id: 0,
            mutex: Arc::new(Mutex::new(vec![])),

            login_progress: LoginProgress::None,
            connection_overlay: ConnectionOverlay {
                timeout: CONNECTION_OVERLAY_TIMEOUT,
                state: ConnectionOverlayState::Disconnected,
            },

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
            mouse_scroll_x: 0.0,
            mouse_scroll_y: 0.0,
            mouse_scroll_y_accel: 0.0,

            text_input: String::new(),
            keycode_map: BTreeMap::new(),
            keymod: keyboard::Mod::NOMOD,
        }
    }

    pub fn get_string(&mut self, s: &str) -> Rc<str> {
        // TODO: intern this boy
        Rc::from(s)
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

        self.mouse_scroll_y_accel = self.mouse_scroll_y_accel / 1.9;
        self.mouse_scroll_y = self.mouse_scroll_y * self.mouse_scroll_y_accel * 3.0 / 5.0;

        self.keycode_map.clear();
        self.state_flag = 0;
        self.id = 0;
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

type HttpMutex = Arc<Mutex<Vec<HttpData>>>;

pub enum LoginProgress {
    None,
    Started,
    Failed,
}

fn sync_to_anilist(mutex: &HttpMutex, access_token: &str, animes: &mut [&mut database::Anime]) {
    for anime in animes {
        update_anilist_watched(mutex, access_token, *anime);
    }
}

fn poll_http(app: &mut App) {
    let mutex = &app.mutex;
    if let Ok(ref mut lock) = mutex.try_lock() {
        for data in lock.drain(..) {
            match data {
                HttpData::Viewer(viewer, access_token) => match viewer {
                    Viewer::Ok(id) => {
                        app.database
                            .anilist_cred_set(Some(AniListCred::new(id, access_token)));
                        app.login_progress = LoginProgress::None;
                        app.connection_overlay.state = ConnectionOverlayState::Connected;
                        app.connection_overlay.timeout = CONNECTION_OVERLAY_TIMEOUT;
                    }
                    Viewer::Err(_) => {
                        app.login_progress = LoginProgress::Failed;
                        app.text_input.clear();
                    }
                },
                HttpData::MediaList(media_list) => match media_list {
                    MediaList::Ok(collections) => {
                        for collection in collections.iter() {
                            let mut sync_newer = app.database.update_anilist_list(collection);

                            if let Some(access_token) = app.database.anilist_access_token() {
                                sync_to_anilist(mutex, access_token, &mut sync_newer);
                            }
                        }
                        app.database.update_cached();
                    }
                    MediaList::Err(_) => {
                        eprintln!("{}:{}:Oops", std::file!(), std::line!());
                    }
                },
                HttpData::UpdateMedia(path, entry) => {
                    let anime = app
                        .database
                        .animes()
                        .iter_mut()
                        .find(|v| v.path() == path)
                        .unwrap();
                    if entry.updated_at() > anime.last_watched() {
                        anime.set_last_watched(entry.updated_at());
                    }
                }
                HttpData::Debug(v) => {
                    dbg!(v);
                }
            }
        }
    }
}

pub fn send_request<Fut>(
    mutex: &HttpMutex,
    request: RequestBuilder,
    f: impl FnOnce(reqwest::Response) -> Fut + Send + 'static,
) where
    Fut: Future<Output = anyhow::Result<HttpData>> + Send,
{
    let mutex = Arc::clone(mutex);
    tokio::spawn(async move {
        let res = request.send().await?;
        let v = f(res).await?;
        let mut guard = mutex.lock().unwrap();
        guard.push(v);
        anyhow::Ok(())
    });
}

#[derive(Clone, Debug)]
pub enum HttpData {
    Viewer(Viewer, String),
    MediaList(MediaList),
    UpdateMedia(String /* path */, MediaEntry),
    Debug(String),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lock_file()?;
    let cfg = Config::parse_cfg();
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

    let mut canvas = window.into_canvas().present_vsync().accelerated().build()?;
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

    let mut app = App::new(
        canvas,
        clipboard,
        database,
        input_util,
        &ttf_ctx,
        &texture_creator,
        thumbnail_path.to_string(),
    );

    if let Some(cred) = app.database.anilist_cred() {
        send_login(&app.mutex, cred.access_token());
        get_anilist_media_list(&app.mutex, cred.user_id(), cred.access_token());
    }

    enum CanvasTexture<'a> {
        Cached(Texture<'a>),
        Wait(u32),
    }

    let idle_time = 100;
    let mut canvas_texture = CanvasTexture::Wait(idle_time);

    app.canvas.clear();
    app.canvas.present();
    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow::anyhow!(e))?;
    'running: while app.running {
        if app.canvas.window().has_input_focus() || app.canvas.window().has_mouse_focus() {
            app.reset_frame_state()
        }

        for event in event_pump.poll_iter() {
            canvas_texture = CanvasTexture::Wait(idle_time);
            match event {
                Event::Quit { .. } => break 'running,
                Event::MouseButtonDown { .. } => {}
                Event::MouseButtonUp {
                    mouse_btn: sdl2::mouse::MouseButton::Left,
                    ..
                } => {
                    app.mouse_click_left_true();
                }
                Event::MouseWheel {
                    precise_x,
                    precise_y,
                    ..
                } => {
                    app.mouse_scroll_y_accel += 0.3;
                    app.mouse_scroll_y += precise_y * 4.3;
                    app.mouse_scroll_x += precise_x * 8.3;
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

        match canvas_texture {
            CanvasTexture::Cached(ref texture) => {
                app.canvas.copy(texture, None, None).unwrap();
            }
            CanvasTexture::Wait(ref mut t) => {
                *t -= 1;
                poll_http(&mut app);
                app.canvas.set_draw_color(color_hex(BACKGROUND_COLOR));
                app.canvas.clear();
                draw(&mut app, &mut screen);

                if *t <= 0 {
                    let (width, height) = app.canvas.window().size();
                    let pixel_format = app.canvas.default_pixel_format();
                    let pitch = pixel_format.byte_size_per_pixel() * width as usize;
                    let rect = rect!(0, 0, width, height);

                    let pixels = app.canvas.read_pixels(rect, pixel_format).unwrap();
                    let mut texture = texture_creator
                        .create_texture_static(pixel_format, width, height)
                        .unwrap();
                    texture.update(None, &pixels, pitch).unwrap();
                    canvas_texture = CanvasTexture::Cached(texture);
                }
            }
        }
        app.canvas.present();
        if !(app.canvas.window().has_input_focus() || app.canvas.window().has_mouse_focus()) {
            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
        }
    }

    release_lock_file()?;

    // Do not write to cache while developing
    #[cfg(debug_assertions)]
    {
        return Ok(());
    }
    app.database.write(cfg.database_path())?;

    Ok(())
}
