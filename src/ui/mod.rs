mod attach_flag_screen;
mod episode_screen;
pub mod layout;
mod login_screen;
mod main_screen;

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::ops::Sub;
use std::rc::Rc;

use crate::database;
use crate::database::episode::Episode;
use crate::database::AnimeMapIdx;
use crate::database::Database;
use crate::send_request;
use crate::App;
use crate::BindFlag;
use crate::ConnectionOverlayState;
use crate::Context;
use crate::HttpSender;
use crate::RequestKind;
use crate::SingleFlag;
use crate::CONNECTION_OVERLAY_TIMEOUT;
use crate::DEFAULT_VIDEO_PLAYER;
use anyhow::Context as _;
use anyhow::Result;
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::image::LoadSurface;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::BlendMode;
use sdl2::render::Canvas;
use sdl2::render::Texture;
use sdl2::render::TextureCreator;
use sdl2::render::TextureQuery;
use sdl2::surface::Surface;
use sdl2::ttf::Font;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::Window;
use sdl2::video::WindowContext;

use self::attach_flag_screen::draw_attach_flag;
use self::episode_screen::draw_anime_expand;
use self::episode_screen::DESCRIPTION_FONT_INFO;
use self::layout::Layout as _;
use self::login_screen::draw_login;
use self::main_screen::draw_main;
use self::main_screen::CARD_HEIGHT;
use self::main_screen::CARD_WIDTH;

use sdl2::image::ImageRWops;
use sdl2::rect::Rect;
use sdl2::rwops::RWops;

const DEBUG_COLOR: u32 = 0xFF0000;

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

pub const BACKGROUND_COLOR: u32 = 0x1A1B25;

const LIBERATION_FONT_BYTES: &[u8] =
    include_bytes!(r"../../fonts/liberation-fonts-ttf-2.1.5/LiberationSans-Regular.ttf");

const NOTO_FONT_BYTES: &[u8] = include_bytes!(r"../../fonts/NotoSansCJKjp-Regular.otf");

const PLAY_ICON: &str = "0";
const PLAY_ICON_IMAGE: &[u8] = include_bytes!(r"../../assets/play-icon.svg");

pub const MISSING_THUMBNAIL: &str = "1";
const MISSING_THUMBNAIL_IMAGE: &[u8] = include_bytes!(r"../../assets/missing-thumbnail.png");

pub const SCROLLBAR_COLOR: u32 = 0x2A2A2A;

const LIBERATION_FONT: &str = "0";
const NOTO_FONT: &str = "1";

//const TITLE_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
pub const TITLE_FONT: &str = LIBERATION_FONT;
pub const TITLE_FONT_PT: u16 = 19;
pub const TITLE_FONT_INFO: (&str, u16) = (TITLE_FONT, TITLE_FONT_PT);
pub const TITLE_FONT_COLOR: u32 = 0xCBCBCB;
pub const TITLE_HOVER_FONT_COLOR: u32 = 0x8F8F8F;

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
pub const INPUT_BOX_FONT_INFO: (&str, u16) = (INPUT_BOX_FONT, INPUT_BOX_FONT_PT);

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
type Layout = Rect;

#[macro_export]
macro_rules! rect {
    ($x:expr, $y:expr, $w:expr, $h:expr) => {
        Rect::new($x as i32, $y as i32, $w as u32, $h as u32)
    };
}

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

type ImageData = (String, TextureOptions);

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
                    NOTO_FONT => self
                        .ttf_ctx
                        .load_font_from_rwops(RWops::from_bytes(NOTO_FONT_BYTES).unwrap(), pt)
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
    pub texture_creator: &'a TextureCreator<WindowContext>,
    pub font_manager: FontManager<'a, 'b>,
    pub cache: BTreeMap<TextManagerKey, Rc<Texture<'a>>>,
}

#[test]
fn hex_color_test_0() {
    let as_hex = hex_color(color_hex(0xDEADBE));
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

    pub fn text_size(&mut self, font_info: FontInfo, text: &str) -> (u32, u32) {
        self.font_manager.load(font_info).size_of(text).unwrap()
    }

    pub fn font_height(&mut self, font_info: FontInfo) -> u32 {
        self.font_manager.load(font_info).height() as u32
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

#[derive(Debug, Clone, Copy, Default, Ord, PartialOrd, Eq, PartialEq)]
pub struct TextureOptions {
    crop_pos: Option<(i32, i32)>,
    ratio: Option<(u32, u32)>,
    rounded: Option<i16>,
    gradient: Option<i32>,
}

impl TextureOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn crop_pos(mut self, crop_pos: Option<(i32, i32)>) -> Self {
        self.crop_pos = crop_pos;
        self
    }

    pub fn ratio(mut self, ratio: Option<(u32, u32)>) -> Self {
        self.ratio = ratio;
        self
    }

    pub fn rounded(mut self, rounded: Option<i16>) -> Self {
        self.rounded = rounded;
        self
    }

    pub fn gradient(mut self, gradient: Option<i32>) -> Self {
        self.gradient = gradient;
        self
    }
}

fn texture_modify<'a>(
    canvas: &mut Canvas<Window>,
    texture_creator: &'a TextureCreator<WindowContext>,
    image_texture: Texture<'a>,
    options: TextureOptions,
) -> Texture<'a> {
    let TextureQuery { width, height, .. } = image_texture.query();
    let mut texture = texture_creator
        .create_texture_target(PixelFormatEnum::ARGB8888, width, height)
        .unwrap();
    texture.set_blend_mode(BlendMode::Blend);
    let mut intemediary_texture = texture_creator
        .create_texture_target(PixelFormatEnum::ARGB8888, width, height)
        .unwrap();
    canvas
        .with_texture_canvas(&mut intemediary_texture, |texture_canvas| {
            texture_canvas
                .copy(&image_texture, None, Rect::new(0, 0, width, height))
                .unwrap();
            if let Some(gradient) = options.gradient {
                texture_canvas.set_blend_mode(BlendMode::Blend);
                let height_offset = gradient as i32;
                texture_canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
                if height_offset > 0 {
                    texture_canvas
                        .fill_rect(Rect::new(
                            0,
                            height as i32 - height_offset,
                            width,
                            height_offset as u32,
                        ))
                        .unwrap();
                }

                for i in 0..255u8 {
                    let height_pos = -height_offset + height.saturating_sub(i as u32) as i32;
                    texture_canvas.set_draw_color(Color::RGBA(
                        0,
                        0,
                        0,
                        (255u8.saturating_sub(
                            (i as u32).saturating_mul(5).saturating_div(3).clamp(0, 255) as u8,
                        ))
                        .saturating_sub(0),
                    ));
                    texture_canvas
                        .fill_rect(Rect::new(0, height_pos, width, 1))
                        .unwrap();
                }
            }
        })
        .unwrap();
    let mut image_texture = intemediary_texture;
    image_texture.set_blend_mode(BlendMode::Blend);

    match options.rounded {
        Some(rad) => {
            let TextureQuery {
                width,
                height,
                format,
                ..
            } = texture.query();
            // Normalize rounded corners
            let rad = match options.ratio {
                Some((r_width, r_height)) => {
                    if (width as f32 / height as f32) < (r_width as f32 / r_height as f32) {
                        ((width as f32 / r_width as f32) * rad as f32).ceil() as i16
                    } else {
                        ((height as f32 / r_height as f32) * rad as f32).ceil() as i16
                    }
                }
                None => rad,
            };
            assert!(format.supports_alpha());
            canvas
                .with_texture_canvas(&mut texture, |texture_canvas| {
                    texture_canvas
                        .rounded_box(
                            0,
                            0,
                            width as i16,
                            height as i16,
                            rad,
                            Color::RGBA(0, 0, 0, 255),
                        )
                        .unwrap();
                    image_texture.set_blend_mode(BlendMode::Add);
                    texture_canvas
                        .copy(&image_texture, None, Rect::new(0, 0, width, height))
                        .unwrap();
                })
                .unwrap();
            texture
        }
        None => image_texture,
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
        canvas: &mut Canvas<Window>,
        path: impl AsRef<str>,
        options: TextureOptions,
    ) -> Result<Rc<Texture<'a>>> {
        // TODO: Anti-aliasing for images.
        //
        // I want images to be blurred bilinearly since they currently have little
        // pre-processing done prior to scaling down and bliting onto canvas.

        match self.cache.entry((path.as_ref().to_string(), options)) {
            Entry::Occupied(v) => Ok(Rc::clone(v.get())),
            Entry::Vacant(v) => {
                let raw_img = match path.as_ref() {
                    PLAY_ICON => RWops::from_bytes(PLAY_ICON_IMAGE)
                        .expect("Failed to load binary image")
                        .load()
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                    MISSING_THUMBNAIL => RWops::from_bytes(MISSING_THUMBNAIL_IMAGE)
                        .expect("Failed to load binary image")
                        .load()
                        .map_err(|e| anyhow::anyhow!("{e}"))?,
                    path => Surface::from_file(path)
                        .map_err(|e| anyhow::anyhow!("{e}"))
                        .with_context(|| "Could not load iamge")?,
                };

                let texture = match options.ratio {
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
                        let crop = match options.crop_pos {
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

                        let mut surface =
                            Surface::new(crop.width(), crop.height(), raw_img.pixel_format_enum())
                                .unwrap();
                        raw_img.blit(crop, &mut surface, None).unwrap();

                        surface
                            .set_blend_mode(sdl2::render::BlendMode::Blend)
                            .unwrap();
                        surface.as_texture(self.texture_creator).unwrap()
                    }
                    None => self
                        .texture_creator
                        .create_texture_from_surface(raw_img)
                        .unwrap(),
                };

                let texture = texture_modify(canvas, self.texture_creator, texture, options);
                let texture = Rc::new(texture);
                v.insert(Rc::clone(&texture));
                Ok(texture)
            }
        }
    }

    pub fn query_size(
        &mut self,
        canvas: &mut Canvas<Window>,
        path: impl AsRef<str>,
    ) -> Result<(u32, u32)> {
        let TextureQuery { width, height, .. } =
            self.load(canvas, path, TextureOptions::default())?.query();
        Ok((width, height))
    }
}

#[derive(Debug)]
pub enum Screen {
    Main,
    Login,
    SelectEpisode(AnimeMapIdx),
    AttachFlag(AnimeMapIdx),
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

pub fn update_anilist_watched(tx: &HttpSender, access_token: &str, anime: &mut database::Anime) {
    if let Some(media_id) = anime.anilist_id() {
        if let Episode::Numbered { episode, .. } = anime.current_episode() {
            let access_token = access_token.to_string();
            let ptr_id = anime.as_ptr_id();
            let access_token = access_token.to_string();
            dbg!(anime.title());
            send_request(
                tx,
                RequestKind::UpdateMedia {
                    access_token,
                    media_id,
                    episode,
                    ptr_id,
                },
            );
        }
    }
}

fn draw_image_clip(
    app: &mut App,
    path: impl AsRef<str>,
    layout: Layout,
    rounded: Option<i16>,
    gradient: Option<i32>,
) -> Result<()> {
    let texture = app.context.image_manager.load(
        &mut app.context.canvas,
        path,
        TextureOptions::new()
            .ratio(Some((layout.width(), layout.height())))
            .rounded(rounded)
            .gradient(gradient),
    )?;
    let TextureQuery {
        width: mut image_width,
        height: mut image_height,
        ..
    } = texture.query();

    let scaling = if image_width as i32 - layout.width() as i32
        > image_height as i32 - layout.height() as i32
    {
        image_width as f32 / layout.width() as f32
    } else {
        image_height as f32 / layout.height() as f32
    };
    image_width = (image_width as f32 / scaling) as u32;
    image_height = (image_height as f32 / scaling) as u32;

    app.context.canvas.set_blend_mode(BlendMode::Blend);
    app.context
        .canvas
        .copy(
            &texture,
            None,
            Some(rect!(layout.x, layout.y, image_width, image_height)),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

fn draw_image_float(
    context: &mut Context,
    path: impl AsRef<str>,
    layout: Layout,
    padding: Option<(i32, i32)>,
    rounded: Option<i16>,
    gradient: Option<i32>,
) -> Result<()> {
    context.canvas.set_blend_mode(BlendMode::Blend);
    let texture = context.image_manager.load(
        &mut context.canvas,
        path,
        TextureOptions::new().rounded(rounded).gradient(gradient),
    )?;
    let TextureQuery {
        width: mut image_width,
        height: mut image_height,
        ..
    } = texture.query();
    let scaling = if image_width as i32 - layout.width() as i32
        > image_height as i32 - layout.height() as i32
    {
        image_width as f32 / layout.width() as f32
    } else {
        image_height as f32 / layout.height() as f32
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
                layout.x + layout.width() as i32 / 2,
                layout.y + layout.height() as i32 / 2,
            ),
            image_width,
            image_height,
        ),
    };
    context.canvas.set_blend_mode(BlendMode::Blend);
    context
        .canvas
        .copy(&texture, None, Some(dest_rect))
        .unwrap();
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
fn draw_button(context: &mut Context, text: &str, style: Style, layout: Layout) -> bool {
    let button_id = context.create_id(layout);
    let button_rect = layout;
    let (text_width, _text_height) = text_size(&mut context.text_manager, style.font_info, text);
    let text = if text_width > layout.width() {
        format!("{}...", text.split_at(15).0)
    } else {
        text.to_owned()
    };

    let (button_fg_color, button_bg_color) = if context.state_id(button_id) {
        (style.fg_hover_color, style.bg_hover_color)
    } else {
        (style.fg_color, style.bg_color)
    };
    context.canvas.set_draw_color(button_bg_color);
    match style.round {
        Some(round) => {
            context
                .canvas
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
            context.canvas.fill_rect(button_rect).unwrap();
        }
    }

    draw_text_centered(
        &mut context.canvas,
        &mut context.text_manager,
        style.font_info,
        text,
        button_fg_color,
        button_rect.x() + button_rect.width() as i32 / 2,
        button_rect.y() + button_rect.height() as i32 / 2,
        None,
        None,
    );

    context.click_elem(button_id)
}

fn draw_back_button(app: &mut App, screen: Screen, layout: Layout) {
    let style = Style::new(color_hex(0x9A9A9A), color_hex(0x2A2A2A))
        .bg_hover_color(color_hex(0x4A4A4A))
        .font_info(BACK_BUTTON_FONT_INFO);
    if draw_button(&mut app.context, "Back", style, layout) {
        app.next_screen = Some(screen);
    }
}

pub fn draw_missing_thumbnail(app: &mut App, layout: Layout, rounded: Option<i16>) {
    let bg_color = color_hex(0x9A9A9A);
    if let Some(rounded) = rounded {
        let rect = layout;
        app.context
            .canvas
            .rounded_box(
                rect.left() as i16,
                rect.top() as i16,
                rect.right() as i16 - 1,
                rect.bottom() as i16 - 1,
                rounded,
                bg_color,
            )
            .unwrap();
    } else {
        app.context.canvas.set_draw_color(bg_color);
        app.context.canvas.fill_rect(layout).unwrap();
    }
    draw_text_centered(
        &mut app.context.canvas,
        &mut app.context.text_manager,
        DESCRIPTION_FONT_INFO,
        "No Thumbnail :<",
        color_hex(0x303030),
        layout.x + layout.width() as i32 / 2,
        layout.y + layout.height() as i32 / 2,
        None,
        None,
    );
}

fn dbg_layout(app: &mut App, layout: Layout) {
    app.context.canvas.set_draw_color(Color::RED);
    app.context.canvas.draw_rect(layout).unwrap();
}

fn draw_connection_overlay_connected(app: &mut App) {
    let (_, text_height) = app
        .context
        .text_manager
        .text_size(CONNECTION_FONT_INFO, "Connected");
    let (width, height) = app.context.canvas.window().size();
    let layout = Layout::new(0, (height - text_height) as i32, width, height);
    app.context.canvas.set_draw_color(color_hex(0x006600));
    app.context.canvas.fill_rect(layout).unwrap();
    draw_text_centered(
        &mut app.context.canvas,
        &mut app.context.text_manager,
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
        .context
        .text_manager
        .text_size(CONNECTION_FONT_INFO, "Disconnected");
    let (width, height) = app.context.canvas.window().size();
    let layout = Layout::new(0, (height - text_height) as i32, width, height);
    app.context.canvas.set_draw_color(color_hex(0x101010));
    app.context.canvas.fill_rect(layout).unwrap();
    draw_text_centered(
        &mut app.context.canvas,
        &mut app.context.text_manager,
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

    app.context.canvas.set_draw_color(color_hex(0x0B0B0B));
    app.context.canvas.fill_rect(layout).unwrap();

    // Draw login button
    let _layout = {
        let text = match app.connection_overlay.state {
            ConnectionOverlayState::Disconnected => "Login",
            ConnectionOverlayState::Connected => "Logout",
        };
        let (login_width, _) = app.context.text_manager.text_size(TOOLBAR_FONT_INFO, text);
        let login_width = login_width + toolbar_button_side_pad;
        let (layout, login_button_layout) =
            layout.split_vert(layout.width() - login_width, layout.width());
        if draw_button(
            &mut app.context,
            text,
            toolbar_button_style,
            login_button_layout,
        ) {
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
    let (window_width, window_height) = app.context.canvas.window().size();
    let (_, text_height) = app.context.text_manager.text_size(TOOLBAR_FONT_INFO, "W");
    if app.keyup(Keycode::LAlt) {
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
        Screen::Login => draw_login(app, layout),
        Screen::Main => draw_main(app, layout),
        Screen::SelectEpisode(idx) => {
            // Anime reference will never get changed while drawing frame
            draw_anime_expand(app, layout, *idx);
        }
        Screen::AttachFlag(idx) => {
            draw_attach_flag(app, layout, *idx);
        }
    }

    app.connection_overlay.timeout = app
        .connection_overlay
        .timeout
        .sub(app.frametime_frac())
        .max(0.0);
    if app.connection_overlay.timeout > 0.0 {
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
        match next_screen {
            Screen::AttachFlag(idx) => {
                let anime = app.database.get_idx(idx);
                let state = &mut app.attach_flag_state;
                state.video_player_textbox.cursor_location = 0;
                state.video_player_textbox.text.clear();
                state.single_flags.clear();
                state.regex_textbox.cursor_location = 0;
                state.regex_textbox.text.clear();
                state.bind_flags.clear();

                state.video_player_textbox.cursor_location = DEFAULT_VIDEO_PLAYER.len();
                state.video_player_textbox.text = anime
                    .video_player
                    .clone()
                    .unwrap_or(DEFAULT_VIDEO_PLAYER.to_string());
                state.regex_textbox.cursor_location = anime.pair_flags.video_regex.len();
                state.regex_textbox.text = anime.pair_flags.video_regex.clone();

                for flag in &anime.single_flags {
                    let mut single_flag = SingleFlag::default();
                    single_flag.switch.toggled = flag.enabled;
                    single_flag.textbox.cursor_location = flag.flag.len();
                    single_flag.textbox.text = flag.flag.clone();
                    state.single_flags.push(single_flag);
                }

                state.bind_flags_switch.toggled = anime.pair_flags.enabled;
                for flag in &anime.pair_flags.pair_flags {
                    let mut bind_flag = BindFlag::default();
                    bind_flag.switch.toggled = flag.enabled;
                    bind_flag.deliminator_switch.toggled = flag.use_deliminator;
                    bind_flag.search_path_textbox.cursor_location = flag.search_path.len();
                    bind_flag.search_path_textbox.text = flag.search_path.clone();
                    bind_flag.flag_textbox.cursor_location = flag.flag.len();
                    bind_flag.flag_textbox.text = flag.flag.clone();
                    bind_flag.regex_textbox.cursor_location = flag.regex.len();
                    bind_flag.regex_textbox.text = flag.regex.clone();
                    bind_flag.deliminator_textbox.cursor_location = flag.deliminator.len();
                    bind_flag.deliminator_textbox.text = flag.deliminator.clone();
                    state.bind_flags.push(bind_flag);
                }
            }
            _ => (),
        }
        for (rect, selected) in app.context.id_map.iter_mut().rev() {
            if *selected {
                *rect = Rect::new(0, 0, 0, 0);
            }
        }
        *screen = next_screen;
    }
}
