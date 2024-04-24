#![allow(unreachable_code)]
#![allow(dead_code)]
use anilist_serde::{MediaEntry, MediaList, Viewer};
use anyhow::bail;
use config::Config;
use database::json_database::AnimeDatabaseData;
use database::{AniListCred, Database};
use reqwest::RequestBuilder;
use sdl2::clipboard::ClipboardUtil;
use sdl2::keyboard;
use sdl2::keyboard::TextInputUtil;
use sdl2::mouse::MouseButton;
use sdl2::rect::Rect;
use sdl2::render::Texture;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::{Window, WindowContext};
use sdl2::{
    event::Event,
    keyboard::Keycode,
    render::{Canvas, TextureCreator},
};
use std::collections::{BTreeSet, HashSet};
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

const MOUSE_CLICK_LEFT: u8 = 0x01;
const MOUSE_CLICK_RIGHT: u8 = 0x02;
const MOUSE_MOVED: u8 = 0x04;
const RESIZED: u8 = 0x08;
pub const CONNECTION_OVERLAY_TIMEOUT: u16 = 210;

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

#[derive(Debug)]
pub struct EpisodeState {
    episode_scroll: Scroll,
    selectable: BTreeSet<usize>,
}

#[derive(Debug)]
pub struct AliasPopupState {
    selectable: BTreeSet<usize>,
}

#[derive(Debug)]
pub struct TitlePopupState {
    selectable: BTreeSet<usize>,
    scroll: Scroll,
}

pub struct MainState {
    pub selectable: BTreeSet<usize>,
    pub scroll: Scroll,
    pub selected: Option<usize>,
    pub extra_menu_id: Option<u32>,
    pub keyboard_override: bool,
    pub search_anime: Option<u32>,
    pub alias_anime: Option<u32>,
    pub search_previous: Option<(String, Box<[*const AnimeDatabaseData]>)>,
}

impl Scroll {
    pub fn new() -> Self {
        Self { id: 0, scroll: 0 }
    }
}

#[derive(Debug, Clone)]
pub struct Scroll {
    pub id: usize,
    pub scroll: i32,
}

pub struct App<'a, 'b> {
    pub canvas: Canvas<Window>,
    pub clipboard: ClipboardUtil,
    pub next_screen: Option<Screen>,
    screen: Screen,
    pub input_util: TextInputUtil,
    pub text_manager: TextManager<'a, 'b>,
    pub image_manager: TextureManager<'a>,
    pub string_manager: StringManager,
    pub thumbnail_path: String,
    pub database: Database<'a>,
    pub running: bool,
    pub show_toolbar: bool,
    pub frametime: std::time::Duration,

    pub mutex: HttpMutex,

    pub connection_overlay: ConnectionOverlay,
    pub login_progress: LoginProgress,

    pub main_state: MainState,
    pub episode_state: EpisodeState,

    pub alias_popup_state: AliasPopupState,
    pub title_popup_state: TitlePopupState,
    // bitfield for:
    //   mouse_moved
    //   mouse_clicked_left
    //   mouse_clicked_right
    //   resized
    //state_flag: u8,

    mouse_left_up: bool,
    mouse_left_down: bool,
    mouse_right_up: bool,
    mouse_right_down: bool,
    mouse_moved: bool,
    resized: bool,

    weights: ScrollWeights,

    id: usize,
    id_map: Vec<(Rect, bool)>,
    id_updated: bool,
    click_id: Option<usize>,
    click_id_right: Option<usize>,

    pub text_input: String,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_scroll_x: f32,
    pub mouse_scroll_y: f32,
    pub mouse_scroll_y_accel: f32,
    pub keyset: HashSet<Keycode>,
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
            screen: Screen::Main,
            text_manager: TextManager::new(texture_creator, FontManager::new(ttf_ctx)),
            image_manager: TextureManager::new(texture_creator),
            string_manager: StringManager::new(),
            frametime: std::time::Duration::default(),

            running: true,
            thumbnail_path,

            show_toolbar: false,

            mutex: Arc::new(Mutex::new(vec![])),

            login_progress: LoginProgress::None,
            connection_overlay: ConnectionOverlay {
                timeout: CONNECTION_OVERLAY_TIMEOUT,
                state: ConnectionOverlayState::Disconnected,
            },

            main_state: MainState {
                scroll: Scroll::new(),
                selectable: BTreeSet::new(),
                selected: None,
                extra_menu_id: None,
                keyboard_override: false,
                search_anime: None,
                alias_anime: None,
                search_previous: None,
            },

            episode_state: EpisodeState {
                episode_scroll: Scroll::new(),
                selectable: BTreeSet::new(),
            },

            alias_popup_state: AliasPopupState {
                selectable: BTreeSet::new(),
            },

            title_popup_state: TitlePopupState {
                selectable: BTreeSet::new(),
                scroll: Scroll::new(),
            },

            //state_flag: 0,
            mouse_left_up: false,
            mouse_left_down: false,
            mouse_right_up: false,
            mouse_right_down: false,
            mouse_moved: false,

            weights: ScrollWeights {
                accel: 10.990031,
                accel_accel: 1.9800003,
                deccel_deccel: 3.119999,
                deccel: 1.3700006,
            },

            resized: false,

            mouse_x: 0,
            mouse_y: 0,
            mouse_scroll_x: 0.0,
            mouse_scroll_y: 0.0,
            mouse_scroll_y_accel: 0.0,

            id: 0,
            id_map: vec![(Rect::new(0, 0, 0, 0), false); 16],
            id_updated: false,
            click_id: None,
            click_id_right: None,

            text_input: String::new(),
            keyset: HashSet::new(),
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
        self.keyset.contains(&keycode)
    }

    pub fn frametime_frac(&self) -> f32 {
        (self.frametime.as_micros() / 16) as f32 / 1200.0
    }

    pub fn reset_frame_state(&mut self) {
        self.mouse_scroll_y_accel = self.mouse_scroll_y_accel * self.frametime_frac() / self.weights.deccel_deccel;
        self.mouse_scroll_y = self.mouse_scroll_y * self.mouse_scroll_y_accel * self.frametime_frac() / self.weights.deccel;

        if self.mouse_left_up {
            self.mouse_left_down = false;
            self.mouse_left_up = false;
            self.click_id = None;
        }

        if self.mouse_right_up {
            self.mouse_right_down = false;
            self.mouse_right_up = false;
            self.click_id = None;
        }

        self.mouse_update_id();
        self.id = 0;

        self.keyset.clear();

        self.mouse_moved = false;
        self.resized = false;
    }

    fn swap_screen(&mut self) {
        if let Some(next_screen) = self.next_screen.take() {
            self.screen = next_screen;
            self.id_map.resize(0, (Rect::new(0, 0, 0, 0), false));
        }
    }

    fn mouse_update_id(&mut self) {
        if self.mouse_moved {
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
        if self.mouse_left_down && self.state_id(id) && self.click_id.is_none() {
            self.click_id = Some(id);
        }
    }

    fn register_click_right(&mut self, id: usize) {
        if self.mouse_right_down && self.state_id(id) && self.click_id.is_none() {
            self.click_id_right = Some(id);
        }
    }

    fn check_click(&self, id: usize) -> bool {
        self.click_id == Some(id) && self.state_id(id) && self.mouse_left_up
    }

    fn check_click_right(&self, id: usize) -> bool {
        self.click_id_right == Some(id) && self.state_id(id) && self.mouse_right_up
    }

    fn check_return(&self, id: usize) -> bool {
        self.keyset.contains(&Keycode::Return) && self.state_id(id)
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
        self.id_updated = false;
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

#[derive(Debug)]
struct ScrollWeights {
    accel: f32,
    accel_accel: f32,
    deccel_deccel: f32,
    deccel: f32,
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

fn scroll_func(x: f32) -> f32 {
    x
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    lock_file()?;
    let cfg = Config::parse_cfg();
    let mut args = std::env::args();
    let program_name = args.next().unwrap_or(String::from("aniki"));

    let mut avg_time = [0.0; 60];
    let mut frame_num = 0;
    let mut show_fps_time = std::time::Instant::now();
    let mut prev_time = std::time::Instant::now();
    let mut show_fps = false;

    for arg in args {
        match arg.as_str() {
            "-f" | "--show-fps" => {
                show_fps = true;
            }
            _ => {
                bail!("{program_name}:unknown argument:{arg}");
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

    let mut canvas = if show_fps {
        window.into_canvas().accelerated().build()?
    } else {
        window.into_canvas().present_vsync().accelerated().build()?
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
        // TODO: Id needs to get reset even when the window is not in focus
        if true || app.canvas.window().has_input_focus() || app.canvas.window().has_mouse_focus() {
            app.reset_frame_state()
        }

        for event in event_pump.poll_iter() {
            canvas_texture = CanvasTexture::Wait(idle_time);
            app.mouse_moved = event.is_mouse();

            match event {
                Event::Quit { .. } => break 'running,
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    app.mouse_left_down = true;
                }
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    app.mouse_right_down = true;
                }

                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    ..
                } => {
                    app.mouse_left_up = true;
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Right,
                    ..
                } => {
                    app.mouse_right_up = true;
                }
                Event::MouseWheel {
                    precise_x,
                    precise_y,
                    ..
                } => {
                    if app.mouse_scroll_y.abs() <= 80.0 {
                        app.mouse_scroll_y_accel += app.weights.accel_accel * app.frametime_frac();
                        app.mouse_scroll_y += precise_y.signum() * scroll_func((precise_y * app.weights.accel).abs()) * app.frametime_frac();
                    }
                    app.mouse_scroll_x += precise_x * 8.3 * app.frametime_frac();
                }
                Event::MouseMotion { x, y, .. } => {
                    app.mouse_x = x;
                    app.mouse_y = y;
                }
                Event::KeyDown {
                    keycode: Some(keycode),
                    keymod,
                    ..
                } => {
                    app.keyset.insert(keycode);
                    app.keymod = keymod;
                }
                Event::Window {
                    win_event: sdl2::event::WindowEvent::Resized(_, _),
                    ..
                } => {
                    app.resized = true;
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
                *t = t.saturating_sub(1);
                poll_http(&mut app);

                app.canvas.set_draw_color(color_hex(BACKGROUND_COLOR));
                app.canvas.clear();

                draw(&mut app, &mut screen);
                if *t <= 0 && app.connection_overlay.timeout <= 0 {
                    let (width, height) = app.canvas.window().size();
                    let pixel_format = app.canvas.default_pixel_format();
                    let pitch = pixel_format.byte_size_per_pixel() * width as usize;
                    let pixels = app.canvas.read_pixels(app.window_rect(), pixel_format).unwrap();
                    let mut texture = texture_creator
                        .create_texture_static(pixel_format, width, height)
                        .unwrap();
                    texture.update(None, &pixels, pitch).unwrap();
                    canvas_texture = CanvasTexture::Cached(texture);
                }
            }
        }
        app.canvas.present();

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
                if app.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.weights.accel += scroll_change;
                dbg!(&app.weights);
            }
            if app.keydown(Keycode::S) {
                if app.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.weights.accel_accel += scroll_change;
                dbg!(&app.weights);
            }
            if app.keydown(Keycode::D) {
                if app.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.weights.deccel += scroll_change;
                dbg!(&app.weights);
            }
            if app.keydown(Keycode::F) {
                if app.keymod.contains(sdl2::keyboard::Mod::LCTRLMOD) {
                    scroll_change *= -1.0;
                }
                if app.keymod.contains(sdl2::keyboard::Mod::LSHIFTMOD) {
                    scroll_change *= 10.0;
                }
                app.weights.deccel_deccel += scroll_change;
                dbg!(&app.weights);
            }
        }

        app.frametime = prev_time.elapsed();
        prev_time = std::time::Instant::now();
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
