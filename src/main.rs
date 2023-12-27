#![allow(dead_code)]
use database::episode::Episode;
use database::json_database::AnimeDatabaseData;
use database::Database;
use sdl2::image::ImageRWops;
use sdl2::keyboard;
use sdl2::rwops::RWops;
use sdl2::ttf::{Font, Sdl2TtfContext};
use sdl2::video::{Window, WindowContext};
use sdl2::{
    event::Event,
    image::LoadSurface,
    keyboard::Keycode,
    pixels::Color,
    rect::Rect,
    render::{Canvas, Texture, TextureCreator, TextureQuery},
    surface::Surface,
    url::open_url,
};
use std::collections::{btree_map::Entry, BTreeMap};
use std::rc::Rc;
use std::time::Duration;

mod database;

// Change this to where you keep your anime
const VIDEO_DIRECTORY: &[&str] = &["/home/bruh/Videos/not-anime"];
const DATABASE_PATH: &str = "./anime-cache.db";

const DEBUG_COLOR: u32 = 0xFF0000;

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;

const BACKGROUND_COLOR: u32 = 0x101010;

const LIBERATION_FONT: &[u8] =
    include_bytes!(r"../fonts/liberation-fonts-ttf-2.1.5/LiberationSans-Regular.ttf");

const PLAY_ICON: &[u8] = include_bytes!(r"../assets/play-icon.svg");

const CARD_WIDTH: u32 = 200;
const CARD_HEIGHT: u32 = 300;
const CARD_X_PAD_OUTER: i32 = 10;
const CARD_Y_PAD_OUTER: i32 = 10;
const CARD_X_PAD_INNER: i32 = 20;
const CARD_Y_PAD_INNER: i32 = 20;

const SCROLLBAR_COLOR: u32 = 0x2A2A2A;

//const TITLE_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
const TITLE_FONT: &str = r"LiberationSans-Regular.ttf";
const TITLE_FONT_PT: u16 = 16;
const TITLE_FONT_INFO: (&str, u16) = (TITLE_FONT, TITLE_FONT_PT);
const TITLE_FONT_COLOR: u32 = 0xABABAB;

//const PLAY_BUTTON_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
const PLAY_BUTTON_FONT: &str = r"LiberationSans-Regular.ttf";
const PLAY_BUTTON_FONT_PT: u16 = 16;
const PLAY_BUTTON_FONT_INFO: (&str, u16) = (PLAY_BUTTON_FONT, PLAY_BUTTON_FONT_PT);
const PLAY_BUTTON_FONT_COLOR: u32 = TITLE_FONT_COLOR;

const BACK_BUTTON_FONT_PT: u16 = 24;
const BACK_BUTTON_FONT: &str = TITLE_FONT;
const BACK_BUTTON_FONT_INFO: (&str, u16) = (BACK_BUTTON_FONT, BACK_BUTTON_FONT_PT);

const DESCRIPTION_X_PAD_OUTER: i32 = 10;
const DESCRIPTION_Y_PAD_OUTER: i32 = 10;
const DESCRIPTION_FONT: &str = TITLE_FONT;
const DESCRIPTION_FONT_PT: u16 = 16;
const DESCRIPTION_FONT_INFO: (&str, u16) = (DESCRIPTION_FONT, DESCRIPTION_FONT_PT);
const DESCRIPTION_FONT_COLOR: u32 = TITLE_FONT_COLOR;

const DIRECTORY_NAME_FONT_INFO: (&str, u16) = DESCRIPTION_FONT_INFO;
const DIRECTORY_NAME_FONT_COLOR: u32 = 0x404040;

const DEFAULT_BUTTON_FONT_PT: u16 = 24;
const DEFAULT_BUTTON_FONT: &str = TITLE_FONT;
const DEFAULT_BUTTON_FONT_INFO: (&str, u16) = (DEFAULT_BUTTON_FONT, DEFAULT_BUTTON_FONT_PT);

const H1_FONT_PT: u16 = 28;
const H1_FONT: &str = TITLE_FONT;
const H1_FONT_INFO: (&str, u16) = (H1_FONT, H1_FONT_PT);

const H2_FONT_PT: u16 = 20;
const H2_FONT: &str = TITLE_FONT;
const H2_FONT_INFO: (&str, u16) = (H2_FONT, H2_FONT_PT);

type FontInfo = (&'static str, u16);
type WidthRatio = u32;
type HeightRatio = u32;
type TextManagerKey = (String, FontInfo, u32, Option<u32>);

macro_rules! rect(
    ($x:expr, $y:expr, $w:expr, $h:expr) => {
        Rect::new($x as i32, $y as i32, $w as u32, $h as u32)
    }
);

// TODO: Quite a few of these booleans can be turned into a bitmap flag.
//
// Explore if this performance optimization is justifiable.
pub struct App<'a, 'b> {
    pub canvas: Canvas<Window>,
    pub screen: Screen,
    pub text_manager: TextManager<'a, 'b>,
    pub image_manager: TextureManager<'a>,
    pub running: bool,

    pub main_scroll: i32,
    pub main_selected: Option<usize>,
    pub main_keyboard_override: bool,

    pub episode_scroll: i32,

    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_moved: bool,
    pub mouse_clicked_left: bool,
    pub mouse_clicked_right: bool,
    pub resized: bool,
    pub keycode_map: BTreeMap<u32, bool>,
    pub keymod: keyboard::Mod,
}

#[derive(Debug, Clone)]
pub struct Style {
    pub fg_color: Color,
    pub bg_color: Color,
    pub fg_hover_color: Color,
    pub bg_hover_color: Color,
    pub font_info: FontInfo,
}

pub struct MostlyStatic {
    pub animes: Database,
}

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum Image {
    PlayIcon,
    FromPath(Box<str>),
}

pub struct TextureManager<'a> {
    texture_creator: &'a TextureCreator<WindowContext>,
    cache: BTreeMap<(Image, Option<(WidthRatio, HeightRatio)>), Rc<Texture<'a>>>,
}

pub struct FontManager<'a, 'b> {
    ttf_ctx: &'a Sdl2TtfContext,
    cache: BTreeMap<(String, u16), Rc<Font<'a, 'b>>>,
}

#[derive(PartialEq, Clone, Copy)]
pub struct OrdRect(Rect);

impl Eq for OrdRect {}
impl PartialOrd for OrdRect {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let a = self.0;
        let b = other.0;
        a.x()
            .partial_cmp(&b.x())
            .partial_cmp(&a.y().partial_cmp(&b.y()))
            .partial_cmp(&a.width().partial_cmp(&b.width()))
            .partial_cmp(&a.height().partial_cmp(&b.height()))
    }
}

impl Ord for OrdRect {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = self.0;
        let b = other.0;
        a.x()
            .cmp(&b.x())
            .cmp(&a.y().cmp(&b.y()))
            .cmp(&a.width().cmp(&b.width()))
            .cmp(&a.height().cmp(&b.height()))
    }
}

impl<'a, 'b> FontManager<'a, 'b> {
    pub fn new(ttf_ctx: &'a Sdl2TtfContext) -> Self {
        FontManager {
            ttf_ctx,
            cache: BTreeMap::new(),
        }
    }

    pub fn load(&mut self, (font, pt): (&str, u16)) -> Rc<Font> {
        match self.cache.entry((font.to_string(), pt)) {
            Entry::Occupied(v) => Rc::clone(v.get()),
            Entry::Vacant(v) => {
                let mut font = match font {
                    r"LiberationSans-Regular.ttf" => self
                        .ttf_ctx
                        .load_font_from_rwops(RWops::from_bytes(LIBERATION_FONT).unwrap(), pt)
                        .unwrap(),
                    _ => self.ttf_ctx.load_font(font, pt).unwrap(),
                };
                font.set_hinting(sdl2::ttf::Hinting::Normal);
                font.set_kerning(true);

                let font = Rc::new(font);
                v.insert(Rc::clone(&font));
                font
            }
        }
    }
}

pub struct TextManager<'a, 'b> {
    texture_creator: &'a TextureCreator<WindowContext>,
    font_manager: FontManager<'a, 'b>,
    cache: BTreeMap<TextManagerKey, Rc<Texture<'a>>>,
}

#[test]
fn hex_color_test_0() {
    let as_hex = hex_color(color_hex(0xDEADBE));
    println!("{:x}", as_hex);
    assert_eq!(0xDEADBE, as_hex);
}

#[test]
fn hex_color_test_1() {
    let as_color = color_hex(0xDEADBE);
    assert_eq!(0xDE, as_color.r);
    assert_eq!(0xAD, as_color.g);
    assert_eq!(0xBE, as_color.b);
}

pub fn hex_color(color: Color) -> u32 {
    ((color.r as u32) << 0x10) | ((color.g as u32) << 0x8) | ((color.b as u32) << 0x00)
}

impl<'a, 'b> TextManager<'a, 'b> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        font_manager: FontManager<'a, 'b>,
    ) -> Self {
        Self {
            texture_creator,
            font_manager,
            cache: BTreeMap::new(),
        }
    }

    fn text_size(&mut self, font_info: FontInfo, text: &str) -> (u32, u32) {
        self.font_manager.load(font_info).size_of(text).unwrap()
    }

    // TODO: Constant resizing of text can lead to memory leaks.
    //
    // A new texture is created for different wrap widths, which means that the textures of
    // previous text widths are no longer in use.
    //
    // A solution can be to have another map which is specifically for text widths of specific.
    // Extra memory indirect and datastructure bad though.
    //
    // Also, this leaks memory in a very specific location (when resizing window with wrappable
    // text), and not that much memory (about 4-5 MiB from 1920-700 width in description) so it's
    // kinda up in the air whether this is a real issue or not, and whether the proposed solution
    // will incur other performance issues.
    pub fn load(
        &mut self,
        text: &str,
        font_info: FontInfo,
        color: Color,
        wrap_width: Option<u32>,
    ) -> Rc<Texture<'a>> {
        match self
            .cache
            .entry((text.to_string(), font_info, hex_color(color), wrap_width))
        {
            Entry::Occupied(v) => Rc::clone(v.get()),
            Entry::Vacant(v) => {
                let texture_creator = self.texture_creator;
                let font = self.font_manager.load(font_info);
                let surface = match wrap_width {
                    Some(width) => font
                        .render(text.as_ref())
                        .blended_wrapped(color, width)
                        .unwrap(),
                    None => font.render(text.as_ref()).blended(color).unwrap(),
                };
                let texture = texture_creator
                    .create_texture_from_surface(surface)
                    .unwrap();
                let texture = Rc::new(texture);
                v.insert(Rc::clone(&texture));
                texture
            }
        }
    }
}

impl<'a> TextureManager<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        Self {
            texture_creator,
            cache: BTreeMap::new(),
        }
    }

    pub fn load(
        &mut self,
        image: Image,
        crop_pos: Option<(i32, i32)>,
        ratio: Option<(u32, u32)>,
    ) -> Rc<Texture<'a>> {
        // TODO: Anti-aliasing for images.
        //
        // I want images to be blurred bilinearly since they currently have little
        // pre-processing done prior to scaling down and bliting onto canvas.

        match self.cache.entry((image.clone(), ratio)) {
            Entry::Occupied(v) => Rc::clone(v.get()),
            Entry::Vacant(v) => {
                let raw_img = match image {
                    Image::PlayIcon => RWops::from_bytes(PLAY_ICON).unwrap().load().unwrap(),
                    Image::FromPath(path) => Surface::from_file(path.as_ref()).unwrap(),
                };
                match ratio {
                    Some((width_ratio, height_ratio)) => {
                        let (raw_width, raw_height) = raw_img.size();
                        let (width_scale, height_scale) = if (raw_width as f32 / raw_height as f32)
                            < (width_ratio as f32 / height_ratio as f32)
                        {
                            (
                                1.0,
                                height_ratio as f32 * raw_width as f32
                                    / width_ratio as f32
                                    / raw_height as f32,
                            )
                        } else {
                            (
                                width_ratio as f32 * raw_height as f32
                                    / height_ratio as f32
                                    / raw_width as f32,
                                1.0,
                            )
                        };
                        let crop = match crop_pos {
                            Some((x_crop, y_crop)) => {
                                rect!(
                                    x_crop,
                                    y_crop,
                                    raw_width as f32 * width_scale,
                                    raw_height as f32 * height_scale
                                )
                            }
                            None => Rect::from_center(
                                (raw_width as i32 / 2, raw_height as i32 / 2),
                                (raw_width as f32 * width_scale) as u32,
                                (raw_height as f32 * height_scale) as u32,
                            ),
                        };
                        assert!(crop.height() <= raw_height);
                        assert!(crop.width() <= raw_width);
                        let mut surface =
                            Surface::new(crop.width(), crop.height(), raw_img.pixel_format_enum())
                                .unwrap();
                        raw_img.blit(crop, &mut surface, None).unwrap();

                        surface
                            .set_blend_mode(sdl2::render::BlendMode::Blend)
                            .unwrap();
                        let texture = surface.as_texture(self.texture_creator).unwrap();
                        let texture = Rc::new(texture);
                        v.insert(Rc::clone(&texture));
                        texture
                    }
                    None => {
                        let texture = self
                            .texture_creator
                            .create_texture_from_surface(raw_img)
                            .unwrap();
                        let texture = Rc::new(texture);
                        v.insert(Rc::clone(&texture));
                        texture
                    }
                }
            }
        }
    }

    pub fn query_size(&mut self, image: Image) -> (u32, u32) {
        let TextureQuery { width, height, .. } = self.load(image, None, None).query();
        (width, height)
    }
}

#[derive(Clone, Debug)]
pub enum Screen {
    Main,
    SelectEpisode(Rc<database::Anime>),
}

impl Layout {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Layout {
            x,
            y,
            width,
            height,
        }
    }

    pub fn scroll_y(self, scroll_distance: i32) -> Self {
        Self {
            x: self.x,
            y: self.y + scroll_distance,
            width: self.width,
            height: self.height,
        }
    }

    pub fn split_grid_center(
        mut self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
        wrap_width: u32,
        n: usize,
    ) -> (usize, Vec<Self>) {
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        let max_width = (width as i32 + x_pad) * idx_wrap;
        self.x = (wrap_width as i32 - max_width) / 2;
        self.split_grid(width, height, x_pad, y_pad, wrap_width, n)
    }

    pub fn split_grid(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
        wrap_width: u32,
        n: usize,
    ) -> (usize, Vec<Self>) {
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        if idx_wrap == 0 {
            return (1, vec![self]);
        }
        (
            idx_wrap as usize,
            (0..=n)
                .map(|idx| Self {
                    x: self.x + (idx as i32 % idx_wrap * (width as i32 + x_pad)),
                    y: self.y + (height as i32 + y_pad) * (idx as i32 / idx_wrap),
                    width,
                    height,
                })
                .collect(),
        )
    }

    pub fn split_hori(self, top: u32, ratio: u32) -> (Self, Self) {
        assert!(top < ratio);
        let top_height = (self.height as f32 * (top as f32 / ratio as f32)) as u32;
        let top_layout = Self {
            x: self.x,
            y: self.y,
            width: self.width,
            height: top_height,
        };
        let bottom_layout = Self {
            x: self.x,
            y: self.y + top_height as i32,
            width: self.width,
            height: self.height - top_height,
        };
        (top_layout, bottom_layout)
    }

    pub fn split_vert(self, left: u32, ratio: u32) -> (Self, Self) {
        assert!(left < ratio);
        let left_width = (self.width as f32 * (left as f32 / ratio as f32)) as u32;
        let left_layout = Self {
            x: self.x,
            y: self.y,
            width: left_width,
            height: self.height,
        };
        let right_layout = Self {
            x: self.x + left_width as i32,
            y: self.y,
            width: self.width - left_width,
            height: self.height,
        };
        (left_layout, right_layout)
    }

    pub fn split_even_hori(self, height: u32, n: usize) -> Vec<Self> {
        (0..n)
            .map(|idx| Self {
                x: self.x,
                y: self.y + height as i32 * idx as i32,
                width: self.width,
                height,
            })
            .collect()
    }

    pub fn overlay_vert(self, top: u32, ratio: u32) -> (Self, Self) {
        assert!(top < ratio);
        let top_height = (self.height as f32 * (top as f32 / ratio as f32)) as u32;
        let top_layout = self;
        let bottom_layout = Self {
            x: self.x,
            y: self.y + top_height as i32,
            width: self.width,
            height: self.height - top_height,
        };
        (top_layout, bottom_layout)
    }

    pub fn pad_outer(self, pad_x: u32, pad_y: u32) -> Self {
        Self {
            x: self.x + pad_x as i32,
            y: self.y + pad_y as i32,
            width: self.width - 2 * pad_x,
            height: self.height - 2 * pad_y,
        }
    }

    pub fn pad_left(self, pad: u32) -> Self {
        Self {
            x: self.x + pad as i32,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn pad_right(self, pad: u32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            width: self.width - pad,
            height: self.height,
        }
    }

    pub fn pad_top(self, pad: u32) -> Self {
        Self {
            x: self.x,
            y: self.y + pad as i32,
            width: self.width,
            height: self.height,
        }
    }

    pub fn pad_bottom(self, pad: u32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height - pad,
        }
    }

    pub fn to_rect(self) -> Rect {
        rect!(self.x, self.y, self.width, self.height)
    }
}

fn rgb_hex(hex: u32) -> (u8, u8, u8) {
    let r = ((hex & 0xFF0000) >> 0x10) as u8;
    let g = ((hex & 0x00FF00) >> 0x08) as u8;
    let b = ((hex & 0x0000FF) >> 0x00) as u8;
    (r, g, b)
}

fn color_hex(hex: u32) -> Color {
    let (r, g, b) = rgb_hex(hex);
    Color::RGB(r, g, b)
}

fn color_hex_a(hex: u32, alpha: u8) -> Color {
    let (r, g, b) = rgb_hex(hex);
    Color::RGBA(r, g, b, alpha)
}

fn draw_image_clip(app: &mut App, image: Image, layout: Layout) {
    let texture = app
        .image_manager
        .load(image, None, Some((layout.width, layout.height)));
    let TextureQuery {
        width: mut image_width,
        height: mut image_height,
        ..
    } = texture.query();
    let scaling =
        if image_width as i32 - layout.width as i32 > image_height as i32 - layout.height as i32 {
            image_width as f32 / layout.width as f32
        } else {
            image_height as f32 / layout.height as f32
        };
    image_width = (image_width as f32 / scaling) as u32;
    image_height = (image_height as f32 / scaling) as u32;

    app.canvas
        .copy(
            &texture,
            None,
            Some(rect!(layout.x, layout.y, image_width, image_height)),
        )
        .unwrap();
}

fn draw_image_float(app: &mut App, image: Image, layout: Layout, padding: Option<(i32, i32)>) {
    let texture = app.image_manager.load(image, None, None);
    let TextureQuery {
        width: mut image_width,
        height: mut image_height,
        ..
    } = texture.query();
    let scaling =
        if image_width as i32 - layout.width as i32 > image_height as i32 - layout.height as i32 {
            image_width as f32 / layout.width as f32
        } else {
            image_height as f32 / layout.height as f32
        };
    image_width = (image_width as f32 / scaling) as u32;
    image_height = (image_height as f32 / scaling) as u32;

    let dest_rect = match padding {
        Some((pad_x, pad_y)) => rect!(
            layout.x + pad_x,
            layout.y + pad_y,
            image_width,
            image_height
        ),
        None => Rect::from_center(
            (
                layout.x + layout.width as i32 / 2,
                layout.y + layout.height as i32 / 2,
            ),
            image_width,
            image_height,
        ),
    };
    app.canvas.copy(&texture, None, Some(dest_rect)).unwrap();
}

fn draw_thumbnail(app: &mut App, anime: &database::Anime, layout: Layout) {
    match anime.thumbnail() {
        Some(path) => {
            //let path = app.get_string(path);
            draw_image_clip(app, Image::FromPath(path.clone().into_boxed_str()), layout);
        }
        None => {
            app.canvas.set_draw_color(color_hex(0x9A9A9A));
            app.canvas.fill_rect(layout.to_rect()).unwrap();
            draw_text_centered(
                app,
                DESCRIPTION_FONT_INFO,
                "No Thumbnail :<",
                color_hex(0x303030),
                layout.x + layout.width as i32 / 2,
                layout.y + layout.height as i32 / 2,
                None,
                None,
            );
        }
    }
}

fn draw_card(app: &mut App, anime: &mut database::Anime, idx: usize, layout: Layout) -> bool {
    // draw card background/border
    let mut selected = false;
    let title = anime.title();
    let card_bg_color = color_hex(0x1C1C1C);
    let card_fg_color = color_hex(TITLE_FONT_COLOR);
    let (text_width, text_height) = text_size(app, TITLE_FONT_INFO, &title);
    let (top_layout, text_layout) = layout.split_hori(layout.height - text_height, layout.height);
    let image_layout = layout;

    let title = if text_width > layout.width - 35 {
        format!("{}...", title.split_at(15).0)
    } else {
        title
    };

    // draw thumbnail
    draw_thumbnail(app, anime, image_layout);

    if (!app.main_keyboard_override && layout.to_rect().contains_point(app.mouse_points()))
        || (app.main_keyboard_override && app.main_selected.is_some_and(|i| i == idx))
    {
        selected = true;
        app.main_selected = Some(idx);
        app.canvas.set_draw_color(color_hex_a(0x303030, 0xAA));
        app.canvas.fill_rect(image_layout.to_rect()).unwrap();

        let play_button_pad_outer = 10;
        let (play_current_layout, rest) = top_layout.split_hori(1, 3);
        let (play_next_layout, _more_info_layout) = rest.split_hori(1, 2);
        let play_current_layout =
            play_current_layout.pad_outer(play_button_pad_outer, play_button_pad_outer);

        let play_next_layout =
            play_next_layout.pad_outer(play_button_pad_outer, play_button_pad_outer);

        let play_button_style = Style::new(color_hex(0x909090), color_hex(0x202020))
            .bg_hover_color(color_hex(0x404040))
            .font_info(PLAY_BUTTON_FONT_INFO);

        let mut play_button = false;
        let (current_ep, current_path) = anime.current_episode_path();
        if draw_button(
            app,
            // TODO: Explore string internment techniques for
            // these formating operations.
            //
            // Perhaps using an enum identifier and the episode,
            // you could save a hash of this string and clone
            // a reference to it rather than constantly
            // constructing the same copy of the string.
            //
            // https://en.wikipedia.org/wiki/String_interning
            &format!("Play Current: {}", current_ep),
            play_button_style.clone(),
            play_current_layout,
        ) {
            open_url(&current_path[0]).unwrap();
            anime.update_watched(current_ep).unwrap();
            app.main_scroll = 0;
            play_button = true;
        }

        if let Some((ep, path)) = anime.next_episode_path().unwrap() {
            if draw_button(
                app,
                &format!("Play Next: {}", ep),
                play_button_style.clone(),
                play_next_layout,
            ) {
                open_url(&path[0]).unwrap();
                anime.update_watched(ep).unwrap();
                app.main_scroll = 0;
                play_button = true;
            }
        }

        if !play_button && app.mouse_clicked_left {
            let anime = anime.clone();
            app.episode_scroll = 0;
            app.set_screen(Screen::SelectEpisode(Rc::new(anime)));
        }
    }

    // draw title background
    app.canvas.set_draw_color(card_bg_color);
    app.canvas.fill_rect(text_layout.to_rect()).unwrap();

    // draw title
    app.canvas.set_draw_color(card_fg_color);
    app.canvas.draw_rect(text_layout.to_rect()).unwrap();
    draw_text_centered(
        app,
        TITLE_FONT_INFO,
        title,
        card_fg_color,
        text_layout.x + text_layout.width as i32 / 2,
        text_layout.y + text_layout.height as i32 / 2,
        None,
        None,
    );
    selected
}

fn draw_text(
    app: &mut App,
    font_info: (&'static str, u16),
    text: impl AsRef<str>,
    color: Color,
    x: i32,
    y: i32,
    w: Option<u32>,
    h: Option<u32>,
) {
    let texture = app.text_manager.load(text.as_ref(), font_info, color, w);
    let TextureQuery { width, height, .. } = texture.query();
    if let Some(height) = h {
        let clip_rect = app.canvas.clip_rect().unwrap_or(rect!(x, y, width, height));
        app.canvas.set_clip_rect(clip_rect);
    }
    app.canvas
        .copy(&texture, None, Some(rect!(x, y, width, height)))
        .unwrap();
    if h.is_some() {
        app.canvas.set_clip_rect(None)
    };
}

fn draw_text_centered(
    app: &mut App,
    font_info: FontInfo,
    text: impl AsRef<str>,
    color: Color,
    x: i32,
    y: i32,
    w: Option<u32>,
    h: Option<u32>,
) {
    let (text_width, text_height) = text_size(app, font_info, text.as_ref());
    draw_text(
        app,
        font_info,
        text,
        color,
        x - text_width as i32 / 2,
        y - text_height as i32 / 2,
        w,
        h,
    );
}

fn text_size(app: &mut App, font_info: FontInfo, text: impl AsRef<str>) -> (u32, u32) {
    app.text_manager.text_size(font_info, text.as_ref())
}

fn draw_main(app: &mut App, mostly_static: &mut MostlyStatic) {
    let animes = &mut mostly_static.animes;
    let (window_width, window_height) = app.canvas.window().size();
    let (card_layouts, scrollbar_layout) =
        Layout::new(0, 0, window_width, window_height).split_vert(796, 800);

    // TODO: Cache expensive layouts
    let (cards_per_row, card_layouts) = card_layouts
        .pad_top(CARD_Y_PAD_OUTER as u32)
        .pad_bottom(CARD_Y_PAD_OUTER as u32)
        .scroll_y(app.main_scroll)
        .split_grid_center(
            CARD_WIDTH,
            CARD_HEIGHT,
            CARD_X_PAD_INNER,
            CARD_Y_PAD_INNER,
            card_layouts.width,
            animes.len() - 1,
        );

    // TODO: Extract into function
    //
    // Problem: Relies on state of several other components
    // (most promently the layout). I would like it to handle
    // events and know explicitly what it is doing with
    // components it is passing in.
    //
    // TODO: Clean up event handling.
    if app.keydown(Keycode::J) {
        if let Some(last) = card_layouts.last() {
            if last.y + last.height as i32 > window_height as i32 {
                app.main_scroll -= 40;
            }
        }
    } else if app.keydown(Keycode::K) {
        if let Some(first) = card_layouts.first() {
            if first.y < CARD_Y_PAD_OUTER as i32 {
                app.main_scroll += 40;
            }
        }
    } else if app.keydown(Keycode::Escape) {
        app.running = false;
    } else if app.keydown(Keycode::F) && app.keymod.contains(keyboard::Mod::LCTRLMOD) {
        app.main_keyboard_override = true;
        match &mut app.main_selected {
            Some(v) => *v = (*v + 1) % card_layouts.len(),
            None => app.main_selected = Some(0),
        }
    } else if app.keydown(Keycode::B) && app.keymod.contains(keyboard::Mod::LCTRLMOD) {
        app.main_keyboard_override = true;
        match &mut app.main_selected {
            Some(v) => *v = (*v - 1) % card_layouts.len(),
            None => app.main_selected = Some(0),
        }
    } else if app.keydown(Keycode::N) && app.keymod.contains(keyboard::Mod::LCTRLMOD) {
        app.main_keyboard_override = true;
        match &mut app.main_selected {
            Some(v) if *v + cards_per_row > card_layouts.len() => *v = 0,
            Some(v) => *v = (*v + cards_per_row) % card_layouts.len(),
            None => app.main_selected = Some(0),
        }
    } else if app.keydown(Keycode::P) && app.keymod.contains(keyboard::Mod::LCTRLMOD) {
        app.main_keyboard_override = true;
        match &mut app.main_selected {
            Some(v) if *v < cards_per_row => *v = card_layouts.len(),
            Some(v) => *v = (*v - cards_per_row) % card_layouts.len(),
            None => app.main_selected = Some(0),
        }
    } else if app.keydown(Keycode::Return) {
        if let Some(idx) = app.main_selected {
            let anime = Rc::new(animes.animes()[idx].clone());
            app.set_screen(Screen::SelectEpisode(anime));
        }
    }

    if app.resized {
        if let Some(last) = card_layouts.last() {
            if (last.y + last.height as i32) < window_height as i32 {
                app.main_scroll =
                    app.main_scroll - (last.y + last.height as i32 - window_height as i32);
            }
        }
    }

    let mut any = false;
    for (idx, (grid_space, anime)) in card_layouts
        .iter()
        .zip(animes.animes().iter_mut())
        .enumerate()
    {
        if grid_space.y + grid_space.height as i32 > 0 {
            if grid_space.y > window_height as i32 {
                break;
            }
            if draw_card(app, anime, idx, *grid_space) {
                any = true;
            }
        }
    }

    // Draw scrollbar
    if let Some(last) = card_layouts.last() {
        let scale = scrollbar_layout.height as f32 / (last.y + last.height as i32 - app.main_scroll) as f32;
        if scale < 1.0 {
            let height = (scrollbar_layout.height as f32 * scale) as u32;
            app.canvas.set_draw_color(color_hex(SCROLLBAR_COLOR));
            app.canvas
                .fill_rect(rect!(
                    scrollbar_layout.x,
                    scrollbar_layout.y + (-1.0 * app.main_scroll as f32 * scale) as i32,
                    scrollbar_layout.width,
                    height
                ))
                .unwrap();
        } else {
            app.main_scroll = 0;
        }
    }

    if !any {
        app.main_selected = None;
    }
}

impl Style {
    pub fn new(fg_color: Color, bg_color: Color) -> Self {
        Self {
            fg_color,
            bg_color,
            fg_hover_color: fg_color,
            bg_hover_color: bg_color,
            font_info: DEFAULT_BUTTON_FONT_INFO,
        }
    }

    pub fn fg_hover_color(mut self, fg_hover_color: Color) -> Self {
        self.fg_hover_color = fg_hover_color;
        self
    }

    pub fn bg_hover_color(mut self, bg_hover_color: Color) -> Self {
        self.bg_hover_color = bg_hover_color;
        self
    }

    pub fn font_info(mut self, font_info: FontInfo) -> Self {
        self.font_info = font_info;
        self
    }
}

fn draw_button(app: &mut App, text: &str, style: Style, layout: Layout) -> bool {
    let button_rect = layout.to_rect();
    let (text_width, _text_height) = text_size(app, TITLE_FONT_INFO, &text);
    let text = if text_width > layout.width {
        format!("{}...", text.split_at(15).0)
    } else {
        text.to_owned()
    };

    let (button_fg_color, button_bg_color) =
        if layout.to_rect().contains_point((app.mouse_x, app.mouse_y)) {
            (style.fg_hover_color, style.bg_hover_color)
        } else {
            (style.fg_color, style.bg_color)
        };
    app.canvas.set_draw_color(button_bg_color);
    app.canvas.fill_rect(button_rect).unwrap();

    draw_text_centered(
        app,
        style.font_info,
        text,
        button_fg_color,
        button_rect.x() + button_rect.width() as i32 / 2,
        button_rect.y() + button_rect.height() as i32 / 2,
        None,
        None,
    );

    app.mouse_clicked_left && layout.to_rect().contains_point(app.mouse_points())
}

fn draw_back_button(app: &mut App, screen: Screen, layout: Layout) {
    let style = Style::new(color_hex(0x9A9A9A), color_hex(0x2A2A2A))
        .bg_hover_color(color_hex(0x4A4A4A))
        .font_info(BACK_BUTTON_FONT_INFO);
    if draw_button(app, "Back", style, layout) {
        app.set_screen(screen.clone());
    }
}

fn draw_top_panel_with_metadata(
    app: &mut App,
    anime: &database::Anime,
    layout: Layout,
    metadata: &AnimeDatabaseData,
) {
    let (_, font_height) = app.text_manager.text_size(DIRECTORY_NAME_FONT_INFO, "L");
    let description_layout = match anime.thumbnail() {
        Some(thumbnail) => {
            let path = thumbnail.clone().into_boxed_str();
            let (image_width, image_height) = app
                .image_manager
                .query_size(Image::FromPath(path.clone()));
            let (image_layout, description_layout) =
                layout.split_vert(image_width * layout.height / image_height, layout.width);
            draw_image_float(app, Image::FromPath(path), image_layout, None);
            description_layout.pad_outer(10, 10)
        }
        None => layout,
    };
    let (title_layout, description_layout) = description_layout.split_hori(2, 7);
    let (title_layout, description_header_layout) = title_layout.split_hori(1, 2);
    let (description_layout, directory_name_layout) = description_layout.split_hori(description_layout.height - font_height, description_layout.height);
    draw_text(
        app,
        H1_FONT_INFO,
        metadata.title(),
        color_hex(DESCRIPTION_FONT_COLOR),
        title_layout.x,
        title_layout.y,
        Some(title_layout.width),
        Some(title_layout.height),
    );
    draw_text(
        app,
        H2_FONT_INFO,
        "Description",
        color_hex(DESCRIPTION_FONT_COLOR),
        description_header_layout.x,
        description_header_layout.y,
        Some(description_header_layout.width),
        Some(description_header_layout.height),
    );
    draw_text(
        app,
        DESCRIPTION_FONT_INFO,
        metadata.tags().join(", "),
        color_hex(DESCRIPTION_FONT_COLOR),
        description_layout.x,
        description_layout.y,
        Some(description_layout.width),
        Some(description_layout.height),
    );
    app.canvas.set_clip_rect(directory_name_layout.to_rect());
    draw_text_centered(
        app,
        DIRECTORY_NAME_FONT_INFO,
        anime.filename(),
        color_hex(DIRECTORY_NAME_FONT_COLOR),
        directory_name_layout.x + directory_name_layout.width as i32 / 2,
        directory_name_layout.y + directory_name_layout.height as i32 / 2,
        None,
        Some(directory_name_layout.height),
    );
    app.canvas.set_clip_rect(None);
}

fn draw_top_panel_anime_expand(app: &mut App, anime: &database::Anime, layout: Layout) {
    match anime.metadata() {
        Some(m) => draw_top_panel_with_metadata(app, anime, layout, m),
        None => {}
    }
}

fn draw_episode(
    app: &mut App,
    mostly_static: &mut MostlyStatic,
    anime: &database::Anime,
    text: &str,
    episode: &Episode,
    layout: Layout,
    clip_rect: Rect,
) {
    let (play_width, play_height) = app.image_manager.query_size(Image::PlayIcon);
    let (play_layout, ep_name_layout) = layout
        .pad_outer(0, 5)
        .pad_right(5)
        .split_vert(play_width * layout.height / play_height, layout.width);
    let ep_name_layout = ep_name_layout.pad_left(30);
    if layout.to_rect().contains_point(app.mouse_points())
        && clip_rect.contains_point(app.mouse_points())
    {
        app.canvas.set_draw_color(color_hex(0x4A4A4A));
        app.canvas.fill_rect(layout.to_rect()).unwrap();
        if app.mouse_clicked_left {
            let mutable_anime = mostly_static.animes.get_anime(anime.filename()).unwrap();
            let paths = anime.find_episode_path(&episode);
            mutable_anime.update_watched(episode.to_owned()).unwrap();
            let anime = Rc::new(mutable_anime.clone());
            app.set_screen(Screen::SelectEpisode(anime));
            open_url(&paths[0]).unwrap();
        }
    }
    draw_image_float(app, Image::PlayIcon, play_layout, Some((10, 0)));
    draw_text(
        app,
        BACK_BUTTON_FONT_INFO,
        text,
        color_hex(DESCRIPTION_FONT_COLOR),
        ep_name_layout.x,
        ep_name_layout.y,
        Some(ep_name_layout.width),
        None,
    );
    app.canvas.set_draw_color(color_hex(0x2A2A2A));
    app.canvas.draw_rect(layout.to_rect()).unwrap();
}

fn dbg_layout(app: &mut App, layout: Layout) {
    app.canvas.set_draw_color(Color::RED);
    app.canvas.draw_rect(layout.to_rect()).unwrap();
}

fn draw_episode_list(
    app: &mut App,
    mostly_static: &mut MostlyStatic,
    anime: &database::Anime,
    layout: Layout,
) {
    app.canvas.set_clip_rect(layout.to_rect());
    let episode_height = 70;
    let episode_count = anime.len() + 1 + anime.has_next_episode() as usize;
    let (layout, scrollbar_layout) = layout.split_vert(796, 800);
    let layouts = layout
        .scroll_y(app.episode_scroll)
        .split_even_hori(episode_height, episode_count);

    if app.keydown(Keycode::J) {
        if let Some(last) = layouts.last() {
            if last.y + last.height as i32 > layout.y + layout.height as i32 {
                app.episode_scroll -= 40;
            }
        }
    } else if app.keydown(Keycode::K) {
        if let Some(first) = layouts.first() {
            if first.y < layout.y {
                app.episode_scroll += 40;
            }
        }
    } else if app.keydown(Keycode::Escape) {
        app.set_screen(Screen::Main);
    }

    let mut layout_iter = layouts.iter();
    let current_ep = anime.current_episode();
    draw_episode(
        app,
        mostly_static,
        anime,
        &format!("Current: {current_ep}"),
        &current_ep,
        *layout_iter.next().unwrap(),
        layout.to_rect(),
    );

    if let Some(next_ep) = anime.next_episode() {
        draw_episode(
            app,
            mostly_static,
            anime,
            &format!("Next: {next_ep}"),
            &next_ep,
            *layout_iter.next().unwrap(),
            layout.to_rect(),
        );
    }

    for (episode_layout, (episode, _)) in layout_iter.zip(anime.episodes().iter()) {
        draw_episode(
            app,
            mostly_static,
            anime,
            &format!("{episode}"),
            episode,
            *episode_layout,
            layout.to_rect(),
        );
    }
    app.canvas.set_clip_rect(None);

    // Draw scrollbar
    let scale = scrollbar_layout.height as f32 / (episode_height as f32 * episode_count as f32);
    if scale < 1.0 {
        let height = (scrollbar_layout.height as f32 * scale) as u32;
        app.canvas.set_draw_color(color_hex(SCROLLBAR_COLOR));
        app.canvas
            .fill_rect(rect!(
            scrollbar_layout.x,
            scrollbar_layout.y + (-1.0 * app.episode_scroll as f32 * scale) as i32,
            scrollbar_layout.width,
            height
        ))
            .unwrap();
    } else {
        app.episode_scroll = 0;
    }
}

fn draw_anime_expand(app: &mut App, mostly_static: &mut MostlyStatic, anime: Rc<database::Anime>) {
    let (window_width, window_height) = app.canvas.window().size();
    let layout = Layout::new(
        DESCRIPTION_X_PAD_OUTER,
        DESCRIPTION_Y_PAD_OUTER,
        window_width - DESCRIPTION_X_PAD_OUTER as u32,
        window_height - DESCRIPTION_Y_PAD_OUTER as u32,
    );
    let (left_layout, right_layout) = layout.split_vert(1, 10);
    let (top_left_layout, _bottom_left_layout) = left_layout.split_hori(1, 11);

    let (top_description_layout, bottom_description_layout) = right_layout.split_hori(3, 7);
    // TODO: Think about abstracting scrolling type windows out
    // into a function or data structure
    let top_description_layout = top_description_layout.pad_bottom(10);
    draw_top_panel_anime_expand(app, &anime, top_description_layout);

    let (back_button_layout, _) = top_left_layout.split_hori(10, 11);
    draw_back_button(app, Screen::Main, back_button_layout.pad_right(10));

    draw_episode_list(app, mostly_static, &anime, bottom_description_layout);
}

fn draw(app: &mut App, mostly_static: &mut MostlyStatic) {
    match app.screen {
        Screen::Main => draw_main(app, mostly_static),
        Screen::SelectEpisode(ref anime) => draw_anime_expand(app, mostly_static, Rc::clone(anime)),
    }
}

impl MostlyStatic {
    pub fn new(database: Database) -> Self {
        Self { animes: database }
    }
}

impl<'a, 'b> App<'a, 'b> {
    pub fn new(
        canvas: Canvas<Window>,
        ttf_ctx: &'a Sdl2TtfContext,
        texture_creator: &'a TextureCreator<WindowContext>,
    ) -> Self {
        Self {
            canvas,
            screen: Screen::Main,
            text_manager: TextManager::new(&texture_creator, FontManager::new(ttf_ctx)),
            image_manager: TextureManager::new(&texture_creator),
            running: true,

            main_keyboard_override: false,
            main_scroll: 0,
            main_selected: None,

            episode_scroll: 0,

            mouse_x: 0,
            mouse_y: 0,
            mouse_moved: false,
            mouse_clicked_left: false,
            mouse_clicked_right: false,
            resized: false,

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
            .map(|v| *v)
            .unwrap_or(false)
    }

    pub fn reset_frame_state(&mut self) {
        if self.mouse_moved {
            self.main_keyboard_override = false;
        }

        self.keycode_map.clear();
        self.mouse_clicked_left = false;
        self.mouse_clicked_right = false;
        self.resized = false;
        self.mouse_moved = false;
    }
}

#[tokio::main]
async fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    video_subsystem.enable_screen_saver();

    let window = video_subsystem
        .window("TEST", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .resizable()
        .build()
        .unwrap();

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
    let mut app = App::new(canvas, &ttf_ctx, &texture_creator);
    let mut database =
        Database::new(DATABASE_PATH, VIDEO_DIRECTORY.to_owned()).unwrap();
    database.retrieve_images("thumbnails/").await.unwrap();
    let mut mostly_static = MostlyStatic::new(database);

    app.canvas.clear();
    app.canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
    'running: while app.running {
        if app.canvas.window().has_mouse_focus() {
            app.canvas.set_draw_color(color_hex(BACKGROUND_COLOR));
            app.canvas.clear();
            app.reset_frame_state()
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Q),
                    ..
                } => break 'running,
                Event::MouseButtonDown { .. } => {}
                Event::MouseButtonUp { mouse_btn: sdl2::mouse::MouseButton::Left, .. } => {
                    app.mouse_clicked_left = true;
                }
                Event::MouseButtonUp { mouse_btn: sdl2::mouse::MouseButton::Right, .. } => {
                    app.mouse_clicked_right = true;
                }
                Event::MouseMotion { x, y, .. } => {
                    app.mouse_moved = true;
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
                    app.resized = true;
                    app.mouse_x = 0;
                    app.mouse_y = 0;
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

    // Do not write to cache for development
    #[cfg(debug_assertions)]
    mostly_static.animes.write("./anime-cache.db").unwrap();
}
