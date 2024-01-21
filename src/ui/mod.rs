use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::CONNECTION_OVERLAY_TIMEOUT;
use crate::anilist_serde::MediaEntry;
use crate::database;
use crate::database::episode::Episode;
use crate::database::Database;
use crate::send_request;
use crate::App;
use crate::ConnectionOverlayState;
use crate::HttpData;
use crate::HttpMutex;
use anyhow::Context;
use anyhow::Result;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::image::LoadSurface;
use sdl2::keyboard;
use sdl2::keyboard::Keycode;
use sdl2::keyboard::Mod;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::render::Texture;
use sdl2::render::TextureCreator;
use sdl2::render::TextureQuery;
use sdl2::surface::Surface;
use sdl2::ttf::Font;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::Window;
use sdl2::video::WindowContext;

use self::episode_screen::draw_anime_expand;
use self::episode_screen::DESCRIPTION_FONT_INFO;
use self::login_screen::draw_login;
use self::main_screen::draw_main;
use self::main_screen::CARD_HEIGHT;
use self::main_screen::CARD_WIDTH;

use sdl2::image::ImageRWops;
use sdl2::rect::Rect;
use sdl2::rwops::RWops;

pub mod episode_screen;
pub mod login_screen;
pub mod main_screen;

const DEBUG_COLOR: u32 = 0xFF0000;

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

pub const BACKGROUND_COLOR: u32 = 0x1A1B25;

const LIBERATION_FONT_BYTES: &[u8] =
    include_bytes!(r"../../fonts/liberation-fonts-ttf-2.1.5/LiberationSans-Regular.ttf");

const PLAY_ICON: &str = "0";
const PLAY_ICON_IMAGE: &[u8] = include_bytes!(r"../../assets/play-icon.svg");

const SCROLLBAR_COLOR: u32 = 0x2A2A2A;

const LIBERATION_FONT: &str = "0";

//const TITLE_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
pub const TITLE_FONT: &str = LIBERATION_FONT;
pub const TITLE_FONT_PT: u16 = 16;
pub const TITLE_FONT_INFO: (&str, u16) = (TITLE_FONT, TITLE_FONT_PT);
pub const TITLE_FONT_COLOR: u32 = 0xABABAB;

pub const CONNECTION_FONT: &str = TITLE_FONT;
pub const CONNECTION_FONT_PT: u16 = 14;
pub const CONNECTION_FONT_INFO: (&str, u16) = (CONNECTION_FONT, CONNECTION_FONT_PT);

pub const TOOLBAR_FONT: &str = TITLE_FONT;
pub const TOOLBAR_FONT_PT: u16 = 14;
pub const TOOLBAR_FONT_INFO: (&str, u16) = (TOOLBAR_FONT, TOOLBAR_FONT_PT);
pub const TOOLBAR_FONT_COLOR: u32 = 0xABABAB;

pub const THUMBNAIL_MISSING_SIZE: (u32, u32) = (CARD_WIDTH, CARD_HEIGHT);

//const PLAY_BUTTON_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
const PLAY_BUTTON_FONT: &str = LIBERATION_FONT;
const PLAY_BUTTON_FONT_PT: u16 = 16;
const PLAY_BUTTON_FONT_INFO: (&str, u16) = (PLAY_BUTTON_FONT, PLAY_BUTTON_FONT_PT);
const PLAY_BUTTON_FONT_COLOR: u32 = TITLE_FONT_COLOR;

const BACK_BUTTON_FONT_PT: u16 = 24;
const BACK_BUTTON_FONT: &str = TITLE_FONT;
const BACK_BUTTON_FONT_INFO: (&str, u16) = (BACK_BUTTON_FONT, BACK_BUTTON_FONT_PT);

const INPUT_BOX_FONT_PT: u16 = 24;
const INPUT_BOX_FONT: &str = TITLE_FONT;
const INPUT_BOX_FONT_INFO: (&str, u16) = (INPUT_BOX_FONT, INPUT_BOX_FONT_PT);

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

#[macro_export]
macro_rules! rect(
    ($x:expr, $y:expr, $w:expr, $h:expr) => {
        Rect::new($x as i32, $y as i32, $w as u32, $h as u32)
    }
);

#[derive(Debug, Clone)]
pub struct Style {
    pub fg_color: Color,
    pub bg_color: Color,
    pub fg_hover_color: Color,
    pub bg_hover_color: Color,
    pub font_info: FontInfo,
    pub round: Option<i16>,
}

pub struct MostlyStatic<'a> {
    pub animes: Database<'a>,
}

#[derive(Debug, Clone, Copy)]
pub struct Layout {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

type ImageData = (String, Option<(WidthRatio, HeightRatio)>);

pub struct TextureManager<'a> {
    texture_creator: &'a TextureCreator<WindowContext>,
    cache: BTreeMap<ImageData, Rc<Texture<'a>>>,
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

impl<'a> MostlyStatic<'a> {
    pub fn new(database: Database<'a>) -> Self {
        Self { animes: database }
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
                    LIBERATION_FONT => self
                        .ttf_ctx
                        .load_font_from_rwops(RWops::from_bytes(LIBERATION_FONT_BYTES).unwrap(), pt)
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
    ((color.r as u32) << 0x10) | ((color.g as u32) << 0x8) | (color.b as u32)
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
        path: impl AsRef<str>,
        crop_pos: Option<(i32, i32)>,
        ratio: Option<(u32, u32)>,
    ) -> Result<Rc<Texture<'a>>> {
        // TODO: Anti-aliasing for images.
        //
        // I want images to be blurred bilinearly since they currently have little
        // pre-processing done prior to scaling down and bliting onto canvas.

        match self.cache.entry((path.as_ref().to_string(), ratio)) {
            Entry::Occupied(v) => Ok(Rc::clone(v.get())),
            Entry::Vacant(v) => {
                let raw_img = match path.as_ref() {
                    PLAY_ICON => RWops::from_bytes(PLAY_ICON_IMAGE)
                        .expect("Failed to load binary image")
                        .load()
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                    path => Surface::from_file(path)
                        .map_err(|e| anyhow::anyhow!("{e}"))
                        .with_context(|| "Could not load iamge")?,
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
                        Ok(texture)
                    }
                    None => {
                        let texture = self
                            .texture_creator
                            .create_texture_from_surface(raw_img)
                            .unwrap();
                        let texture = Rc::new(texture);
                        v.insert(Rc::clone(&texture));
                        Ok(texture)
                    }
                }
            }
        }
    }

    pub fn query_size(&mut self, path: impl AsRef<str>) -> Result<(u32, u32)> {
        let TextureQuery { width, height, .. } = self.load(path, None, None)?.query();
        Ok((width, height))
    }
}

#[derive(Debug)]
pub enum Screen {
    Main,
    Login,
    SelectEpisode(*const database::Anime),
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
    ) -> (usize, impl Iterator<Item = Self>) {
        self.width += x_pad as u32;
        let wrap_width = self.width;
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        let max_width = (width as i32 + x_pad) * idx_wrap;
        self.x = (wrap_width as i32 - max_width) / 2;
        self.split_grid(width, height, x_pad, y_pad)
    }

    pub fn split_grid(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
    ) -> (usize, impl Iterator<Item = Self>) {
        let wrap_width = self.width;
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        (
            idx_wrap as usize,
            (0..).map(move |idx| Self {
                x: self.x + (idx as i32 % idx_wrap * (width as i32 + x_pad)),
                y: self.y + (height as i32 + y_pad) * (idx as i32 / idx_wrap),
                width,
                height,
            }),
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

    pub fn split_even_hori(self, height: u32) -> impl Iterator<Item = Layout> {
        (0..).map(move |idx| Self {
            x: self.x,
            y: self.y + height as i32 * idx as i32,
            width: self.width,
            height,
        })
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

    pub fn pad_left(self, pad: i32) -> Self {
        Self {
            x: self.x + pad,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn pad_right(self, pad: i32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            width: (self.width as i32 - pad * 2) as u32,
            height: self.height,
        }
    }

    pub fn pad_top(self, pad: i32) -> Self {
        Self {
            x: self.x,
            y: self.y + pad,
            width: self.width,
            height: self.height,
        }
    }

    pub fn pad_bottom(self, pad: i32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            width: self.width,
            height: (self.height as i32 - pad) as u32,
        }
    }

    pub fn to_rect(self) -> Rect {
        rect!(self.x, self.y, self.width, self.height)
    }
}

fn rgb_hex(hex: u32) -> (u8, u8, u8) {
    let r = ((hex & 0xFF0000) >> 0x10) as u8;
    let g = ((hex & 0x00FF00) >> 0x08) as u8;
    let b = (hex & 0x0000FF) as u8;
    (r, g, b)
}

pub fn color_hex(hex: u32) -> Color {
    let (r, g, b) = rgb_hex(hex);
    Color::RGB(r, g, b)
}

pub fn color_hex_a(hex: u32) -> Color {
    let r = ((hex & 0xFF000000) >> 0x18) as u8;
    let g = ((hex & 0x00FF0000) >> 0x10) as u8;
    let b = ((hex & 0x0000FF00) >> 0x08) as u8;
    let a = (hex & 0x000000FF) as u8;
    Color::RGBA(r, g, b, a)
}

#[test]
fn color_hex_a_test_0() {
    assert_eq!(color_hex_a(0xDEADBEEF), Color::RGBA(0xDE, 0xAD, 0xBE, 0xEF));
}

pub fn update_anilist_watched(mutex: &HttpMutex, access_token: &str, anime: &mut database::Anime) {
    if let Some(media_id) = anime.anilist_id() {
        if let Episode::Numbered { episode, .. } = anime.current_episode() {
            let anime_list_query = include_str!("update_anilist_media.gql");
            let json = serde_json::json!({"query": anime_list_query, "variables": {"id": 15125, "mediaId": media_id, "episode": episode}});
            let request = reqwest::Client::new()
                .post("https://graphql.anilist.co")
                .header("Authorization", format!("Bearer {access_token}"))
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .body(json.to_string());
            // TODO: Handle error
            let path = anime.path().to_string();
            send_request(mutex, request, |res| async move {
                anyhow::Ok(HttpData::UpdateMedia(path, MediaEntry::deserialize_json(&res.bytes().await?)?))
            });
        }
    }
}

pub fn draw_input_box(app: &mut App, x: i32, y: i32, width: u32) -> bool {
    let font_info = INPUT_BOX_FONT_INFO;
    let pad_side = 5;
    let pad_height = 2;
    let (text_width, text_height) = app.text_manager.text_size(font_info, &app.text_input);
    let layout = Layout::new(x, y, width, text_height);
    let text_shift_x = text_width.saturating_sub(width - pad_side as u32 * 2);
    let text_layout = Layout::new(
        layout.x + pad_side - text_shift_x as i32,
        y,
        width,
        text_height,
    );

    app.input_util.start();

    // Draw box
    app.canvas.set_draw_color(color_hex(0x909090));
    app.canvas.draw_rect(layout.to_rect()).unwrap();

    // Draw cursor
    app.canvas.set_draw_color(color_hex(0xB0B0B0));
    app.canvas
        .fill_rect(rect!(
            text_layout.x + text_width as i32,
            text_layout.y + pad_height,
            1,
            text_layout.height - (pad_height as u32 * 2)
        ))
        .unwrap();

    if !app.text_input.is_empty() {
        let input: &str = unsafe { &*(app.text_input.as_str() as *const _) };
        app.canvas.set_clip_rect(layout.to_rect());
        draw_text(
            &mut app.canvas,
            &mut app.text_manager,
            font_info,
            input,
            color_hex(0x909090),
            text_layout.x,
            text_layout.y,
            None,
            None,
        );
        app.canvas.set_clip_rect(None);
    }

    if app.keydown(Keycode::Backspace) {
        app.text_input.pop();
    } else if app.keymod.contains(Mod::LCTRLMOD) && app.keydown(Keycode::V) {
        match app.clipboard.clipboard_text() {
            Ok(s) => {
                app.text_input.push_str(&s);
            }
            Err(e) => {
                dbg!(e);
            }
        };
    } else if app.keydown(Keycode::Return) {
        return true;
    }
    false
}

fn draw_image_clip(app: &mut App, path: impl AsRef<str>, layout: Layout) -> Result<()> {
    let texture = app
        .image_manager
        .load(path, None, Some((layout.width, layout.height)))?;
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
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

fn draw_image_float(
    app: &mut App,
    path: impl AsRef<str>,
    layout: Layout,
    padding: Option<(i32, i32)>,
) -> Result<()> {
    let texture = app.image_manager.load(path, None, None)?;
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
    Ok(())
}

fn draw_text(
    canvas: &mut Canvas<Window>,
    text_manager: &mut TextManager,
    font_info: (&'static str, u16),
    text: impl AsRef<str>,
    color: Color,
    x: i32,
    y: i32,
    w: Option<u32>,
    h: Option<u32>,
) {
    if text.as_ref().is_empty() {
        return;
    }
    let texture = text_manager.load(text.as_ref(), font_info, color, w);
    let TextureQuery { width, height, .. } = texture.query();
    if let Some(height) = h {
        let clip_rect = canvas.clip_rect().unwrap_or(rect!(x, y, width, height));
        canvas.set_clip_rect(clip_rect);
    }
    canvas
        .copy(&texture, None, Some(rect!(x, y, width, height)))
        .unwrap();
    if h.is_some() {
        canvas.set_clip_rect(None)
    };
}

fn draw_text_centered(
    canvas: &mut Canvas<Window>,
    text_manager: &mut TextManager,
    font_info: FontInfo,
    text: impl AsRef<str>,
    color: Color,
    x: i32,
    y: i32,
    w: Option<u32>,
    h: Option<u32>,
) {
    let (text_width, text_height) = text_size(text_manager, font_info, text.as_ref());
    draw_text(
        canvas,
        text_manager,
        font_info,
        text,
        color,
        x - text_width as i32 / 2,
        y - text_height as i32 / 2,
        w,
        h,
    );
}

fn text_size(
    text_manager: &mut TextManager,
    font_info: FontInfo,
    text: impl AsRef<str>,
) -> (u32, u32) {
    text_manager.text_size(font_info, text.as_ref())
}

impl Style {
    pub fn new(fg_color: Color, bg_color: Color) -> Self {
        Self {
            fg_color,
            bg_color,
            fg_hover_color: fg_color,
            bg_hover_color: bg_color,
            font_info: DEFAULT_BUTTON_FONT_INFO,
            round: Some(7),
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

    pub fn round(mut self, round: Option<i16>) -> Self {
        self.round = round;
        self
    }
}

/// Returns whether the button has been clicked
fn draw_button(app: &mut App, text: &str, style: Style, layout: Layout) -> bool {
    let button_rect = layout.to_rect();
    let (text_width, _text_height) = text_size(&mut app.text_manager, style.font_info, text);
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
    match style.round {
        Some(round) => {
            app.canvas
                .rounded_box(
                    button_rect.left() as i16,
                    button_rect.top() as i16,
                    button_rect.right() as i16,
                    button_rect.bottom() as i16,
                    round,
                    button_bg_color,
                )
                .unwrap();
        }
        None => {
            app.canvas.fill_rect(button_rect).unwrap();
        }
    }

    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        style.font_info,
        text,
        button_fg_color,
        button_rect.x() + button_rect.width() as i32 / 2,
        button_rect.y() + button_rect.height() as i32 / 2,
        None,
        None,
    );

    let clicked = app.mouse_clicked_left();
    let in_bounds = layout.to_rect().contains_point(app.mouse_points());
    if clicked && in_bounds {
        app.mouse_clicked_left_unset();
        true
    } else {
        false
    }
}

fn draw_back_button(app: &mut App, screen: Screen, layout: Layout) {
    let style = Style::new(color_hex(0x9A9A9A), color_hex(0x2A2A2A))
        .bg_hover_color(color_hex(0x4A4A4A))
        .font_info(BACK_BUTTON_FONT_INFO);
    if draw_button(app, "Back", style, layout) {
        app.next_screen = Some(screen);
    }
}

pub fn draw_missing_thumbnail(app: &mut App, layout: Layout) {
    app.canvas.set_draw_color(color_hex(0x9A9A9A));
    app.canvas.fill_rect(layout.to_rect()).unwrap();
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        DESCRIPTION_FONT_INFO,
        "No Thumbnail :<",
        color_hex(0x303030),
        layout.x + layout.width as i32 / 2,
        layout.y + layout.height as i32 / 2,
        None,
        None,
    );
}

fn dbg_layout(app: &mut App, layout: Layout) {
    app.canvas.set_draw_color(Color::RED);
    app.canvas.draw_rect(layout.to_rect()).unwrap();
}

fn draw_connection_overlay_connected(app: &mut App) {
    let (_, text_height) = app
        .text_manager
        .text_size(CONNECTION_FONT_INFO, "Connected");
    let (width, height) = app.canvas.window().size();
    let layout = Layout::new(0, (height - text_height) as i32, width, height);
    app.canvas.set_draw_color(color_hex(0x006600));
    app.canvas.fill_rect(layout.to_rect()).unwrap();
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        CONNECTION_FONT_INFO,
        "Connected",
        color_hex(0xDADADA),
        (width / 2) as i32,
        (height - text_height / 2) as i32,
        None,
        None,
    );
}

fn draw_connection_overlay_disconnected(app: &mut App) {
    let (_, text_height) = app
        .text_manager
        .text_size(CONNECTION_FONT_INFO, "Disconnected");
    let (width, height) = app.canvas.window().size();
    let layout = Layout::new(0, (height - text_height) as i32, width, height);
    app.canvas.set_draw_color(color_hex(0x101010));
    app.canvas.fill_rect(layout.to_rect()).unwrap();
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        CONNECTION_FONT_INFO,
        "Disconnected",
        color_hex(0xDADADA),
        (width / 2) as i32,
        (height - text_height / 2) as i32,
        None,
        None,
    );
}

fn draw_toolbar(app: &mut App, layout: Layout) {
    let toolbar_button_side_pad = 25;
    let toolbar_button_style = Style::new(color_hex(0x909090), color_hex(0x202020))
        .bg_hover_color(color_hex(TOOLBAR_FONT_COLOR))
        .font_info(TOOLBAR_FONT_INFO)
        .round(None);

    app.canvas.set_draw_color(color_hex(0x0B0B0B));
    app.canvas.fill_rect(layout.to_rect()).unwrap();

    // Draw login button
    let layout = {
        let text = match app.connection_overlay.state {
            ConnectionOverlayState::Disconnected => "Login",
            ConnectionOverlayState::Connected => "Logout",
        };
        let (login_width, _) = app.text_manager.text_size(TOOLBAR_FONT_INFO, text);
        let login_width = login_width + toolbar_button_side_pad;
        let (layout, login_button_layout) =
            layout.split_vert(layout.width - login_width, layout.width);
        if draw_button(app, text, toolbar_button_style, login_button_layout) {
            match app.connection_overlay.state {
                ConnectionOverlayState::Disconnected => {
                    app.next_screen = Some(Screen::Login);
                    return;
                }
                ConnectionOverlayState::Connected => {
                    app.connection_overlay.state = ConnectionOverlayState::Disconnected;
                    app.connection_overlay.timeout = CONNECTION_OVERLAY_TIMEOUT;
                    app.database.anilist_clear();
                }
            }
        };
        layout
    };
}

pub fn draw<'frame>(app: &mut App, screen: &mut Screen) {
    let (window_width, window_height) = app.canvas.window().size();
    let (_, text_height) = app.text_manager.text_size(TOOLBAR_FONT_INFO, "W");
    if app.keycode_map.is_empty() && app.keymod.contains(keyboard::Mod::LALTMOD) {
        app.keymod.remove(keyboard::Mod::LALTMOD);
        app.show_toolbar = !app.show_toolbar;
    };

    let (toolbar_layout, layout) = if app.show_toolbar {
        let (toolbar_layout, layout) =
            Layout::new(0, 0, window_width, window_height).split_hori(text_height, window_height);
        (Some(toolbar_layout), layout)
    } else {
        (None, Layout::new(0, 0, window_width, window_height))
    };

    match screen {
        Screen::Login => draw_login(app),
        Screen::Main => draw_main(app, layout),
        Screen::SelectEpisode(anime) => {
            // Anime reference will never get changed while drawing frame
            let anime = unsafe { &**anime };
            draw_anime_expand(app, anime);
        }
    }

    if app.connection_overlay.timeout > 0 {
        app.connection_overlay.timeout -= 1;
        match app.connection_overlay.state {
            ConnectionOverlayState::Connected => {
                draw_connection_overlay_connected(app);
            }
            ConnectionOverlayState::Disconnected => {
                draw_connection_overlay_disconnected(app);
            }
        }
    }

    if let Some(toolbar_layout) = toolbar_layout {
        draw_toolbar(app, toolbar_layout);
    }

    if let Some(next_screen) = app.next_screen.take() {
        *screen = next_screen;
    }
}
