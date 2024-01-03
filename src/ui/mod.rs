use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::rc::Rc;

use sdl2::image::LoadSurface;
use sdl2::pixels::Color;
use anyhow::Context;
use anyhow::Result;
use sdl2::render::Texture;
use sdl2::render::TextureCreator;
use sdl2::render::TextureQuery;
use sdl2::surface::Surface;
use sdl2::ttf::Font;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::WindowContext;
use crate::App;
use crate::database;
use crate::database::Database;

use self::episode_screen::draw_anime_expand;
use self::main_screen::draw_main;

use sdl2::image::ImageRWops;
use sdl2::rwops::RWops;
use sdl2::rect::Rect;

mod episode_screen;

mod main_screen;

const DEBUG_COLOR: u32 = 0xFF0000;

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

pub const BACKGROUND_COLOR: u32 = 0x1B1B1B;

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

//const PLAY_BUTTON_FONT: &'static str = r"./fonts/OpenSans/OpenSans-VariableFont_wdth,wght.ttf";
const PLAY_BUTTON_FONT: &str = LIBERATION_FONT;
const PLAY_BUTTON_FONT_PT: u16 = 16;
const PLAY_BUTTON_FONT_INFO: (&str, u16) = (PLAY_BUTTON_FONT, PLAY_BUTTON_FONT_PT);
const PLAY_BUTTON_FONT_COLOR: u32 = TITLE_FONT_COLOR;

const BACK_BUTTON_FONT_PT: u16 = 24;
const BACK_BUTTON_FONT: &str = TITLE_FONT;
const BACK_BUTTON_FONT_INFO: (&str, u16) = (BACK_BUTTON_FONT, BACK_BUTTON_FONT_PT);

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

#[derive(Clone, Debug)]
pub enum Screen {
    Main,
    SelectEpisode(Box<str>),
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
        n: usize,
    ) -> (usize, Vec<Self>) {
        self.width += x_pad as u32;
        let wrap_width = self.width;
        let idx_wrap = (wrap_width as i32 - self.x) / (width as i32 + x_pad);
        let max_width = (width as i32 + x_pad) * idx_wrap;
        self.x = (wrap_width as i32 - max_width) / 2;
        self.split_grid(width, height, x_pad, y_pad, n)
    }

    pub fn split_grid(
        self,
        width: u32,
        height: u32,
        x_pad: i32,
        y_pad: i32,
        n: usize,
    ) -> (usize, Vec<Self>) {
        let wrap_width = self.width;
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
            width: (self.width as i32 - pad) as u32,
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

// TODO: Add filler image if image not found
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

// TODO: Add filler image if image not found
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

/// Returns whether the button has been clicked
fn draw_button(app: &mut App, text: &str, style: Style, layout: Layout) -> bool {
    let button_rect = layout.to_rect();
    let (text_width, _text_height) = text_size(app, TITLE_FONT_INFO, text);
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
        app.set_screen(screen.clone());
    }
}


fn dbg_layout(app: &mut App, layout: Layout) {
    app.canvas.set_draw_color(Color::RED);
    app.canvas.draw_rect(layout.to_rect()).unwrap();
}


pub fn draw(app: &mut App, mostly_static: &mut MostlyStatic) {
    match app.screen {
        Screen::Main => draw_main(app, mostly_static),
        Screen::SelectEpisode(ref filename) => {
            // Anime reference will never get changed while drawing frame
            let anime: &database::Anime = unsafe {
                let ptr = mostly_static.animes.get_anime(filename).unwrap();
                std::mem::transmute(ptr)
            };
            draw_anime_expand(app, mostly_static, anime);
        }
    }
}

