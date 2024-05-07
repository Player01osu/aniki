use sdl2::gfx::primitives::DrawRenderer;
use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use sdl2::{
    keyboard::{self, Keycode},
    url::open_url,
};

use crate::database::json_database::AnimeDatabaseData;
use crate::{
    database,
    ui::{color_hex, draw_text, BACK_BUTTON_FONT_INFO},
    App,
};
use crate::{rect, register_scroll, textbox, Format};

use super::layout::Layout as _;
use super::{
    color_hex_a, draw_button, draw_image_clip, draw_missing_thumbnail, draw_text_centered, text_size, update_anilist_watched, Screen, Style, TextureOptions, INPUT_BOX_FONT_INFO, MISSING_THUMBNAIL, PLAY_BUTTON_FONT_INFO, TITLE_FONT_COLOR, TITLE_FONT_INFO
};

pub const CARD_RAD: i16 = 10;
pub const CARD_WIDTH: u32 = 200;
pub const CARD_HEIGHT: u32 = 300;
const CARD_X_PAD_OUTER: i32 = 13;
const CARD_Y_PAD_OUTER: i32 = 13;
const CARD_X_PAD_INNER: i32 = 25;
const CARD_Y_PAD_INNER: i32 = 25;

type Layout = Rect;

// TODO: Clean up event handling.
fn handle_main_events(app: &mut App) {
    if app.keydown(Keycode::Escape) {
        app.running = false;
    } else if app.keydown(Keycode::F) && app.context.keymod.contains(keyboard::Mod::LCTRLMOD) {
        // TODO: Select forward
    } else if app.keydown(Keycode::B) && app.context.keymod.contains(keyboard::Mod::LCTRLMOD) {
        // TODO: Select backwards
    } else if app.keydown(Keycode::N) && app.context.keymod.contains(keyboard::Mod::LCTRLMOD) {
        // TODO: Select down
    } else if app.keydown(Keycode::P) && app.context.keymod.contains(keyboard::Mod::LCTRLMOD) {
        // TODO: Select up
    } else if app.keydown(Keycode::Return) {
        if let Some(idx) = app.main_state.selected {
            // Should exist
            let idx = app.database.cache_idx_to_map_idx(idx);
            app.next_screen = Some(Screen::SelectEpisode(idx));
        }
    }
}

fn handle_main_search_events(app: &mut App) {
    if app.keydown(Keycode::Escape) {
        app.main_state.search_anime = None;
        app.main_state.alias_anime = None;
        app.context.input_util.stop();
    }
}

fn draw_main_anime_search(app: &mut App, layout: Layout, search_id: u32) {
    let outer_bounds_id = app.context.create_id(app.context.window_rect());
    let _inner_bounds_id = app.context.create_id(layout);
    let layout_y = layout.y();
    let (_, text_height) = app.context.text_manager.text_size(BACK_BUTTON_FONT_INFO, "");
    let textbox_state = &mut app.title_popup_state.textbox;

    app.context.canvas.set_draw_color(color_hex(0x303030));
    app.context.canvas.fill_rect(layout).unwrap();

    // TODO: Draw rect with border size
    app.context.canvas.set_draw_color(color_hex(0x101010));
    app.context.canvas.fill_rect(layout).unwrap();

    let mut layout = layout.pad_top(10).pad_bottom(5);
    textbox(&mut app.context, textbox_state, true, 10, &mut layout);
    let scroll = &mut app.title_popup_state.scroll;
    register_scroll(&mut app.context, scroll, &mut layout);
    let anime = &mut app.database.animes()[search_id as usize];
    let textbox = &mut app.title_popup_state.textbox;

    let options = {
        let search_previous: &mut Option<(String, Box<[*const AnimeDatabaseData]>)> =
            unsafe { &mut *(&mut app.main_state.search_previous as *mut _) };
        match search_previous {
            Some(prev) if prev.0 == textbox.text => &prev.1,
            _ => {
                let animes = app.database.fuzzy_find_anime(&textbox.text);
                let animes_cast: Box<[*const _]> = animes
                    .iter()
                    .map(|v| (*v) as *const _)
                    .collect();
                let (_, search) =
                    search_previous.insert((textbox.text.clone(), animes_cast.clone()));
                search
            }
        }
    };
    let option_layout = layout;
    let option_layouts = option_layout.split_even_hori(text_height + 20);

    app.title_popup_state.scroll.max_scroll = 0;
    for (layout, option) in option_layouts.into_iter().zip(options.into_iter()) {
        app.title_popup_state.scroll.max_scroll = layout.y() as i32 - layout_y;
        let layout = layout.scroll_y(app.title_popup_state.scroll.scroll);
        let option = unsafe { &**option };
        let option_id = app.context.create_id(layout);
        app.context.canvas.set_clip_rect(option_layout);
        let (left, right) = draw_option(app, option_id, &option.title());
        if left {
            anime.set_metadata(Some((*option).clone()));
            app.database.retrieve_images(&app.thumbnail_path).unwrap();
            app.main_state.search_anime = None;
            app.context.input_util.stop();
            app.context.canvas.set_clip_rect(None);
            return;
        }
        if right {
            let title = &option.title;
            app.title_popup_state.textbox.text = title.to_owned();
            app.title_popup_state.textbox.cursor_location = title.len();
        }
    }

    if app.context.click_elem(outer_bounds_id) {
        app.main_state.search_anime = None;
        app.main_state.alias_anime = None;
        app.context.input_util.stop();
    }
    app.context.canvas.set_clip_rect(None);
}

fn draw_main_anime_alias(app: &mut App, layout: Layout, alias_id: u32) {
    let outer_bounds_id = app.context.create_id(app.context.window_rect());
    let _inner_bounds_id = app.context.create_id(layout);
    let layout_y = layout.y();
    let anime = &mut app.database.animes()[alias_id as usize];
    let text_height = app
        .context
        .text_manager
        .font_height(BACK_BUTTON_FONT_INFO);

    app.context.canvas.set_draw_color(color_hex(0x303030));
    app.context.canvas.fill_rect(layout).unwrap();

    // TODO: Draw rect with border size
    app.context.canvas.set_draw_color(color_hex(0x101010));
    app.context.canvas.fill_rect(layout).unwrap();

    let mut layout = layout.pad_top(10);
    let textbox_state = &mut app.alias_popup_state.textbox;
    textbox(&mut app.context, textbox_state, true, 10, &mut layout);
    let scroll = &mut app.alias_popup_state.scroll;
    register_scroll(&mut app.context, scroll, &mut layout);
    let option_layout = layout;
    let options: Box<[&str]> = {
        if anime.title() == anime.filename() {
            [unsafe { &*(anime.title() as *const _) }].into()
        } else {
            unsafe {
                [
                    &*(anime.title() as *const _),
                    &*(anime.filename() as *const _),
                ]
            }
            .into()
        }
    };
    let height = option_layout.height() - 5;
    let (_, option_layout) = option_layout.split_hori(option_layout.height() - height, height);
    let option_layouts = option_layout.split_even_hori(text_height + 20);

    for (layout, option) in option_layouts.into_iter().zip(options.into_iter()) {
        let option_id = app.context.create_id(layout);
        debug_assert!(outer_bounds_id < option_id);
        app.alias_popup_state.scroll.max_scroll = layout.bottom() - layout_y;
        let (left, right) = draw_option(app, option_id, option);
        if left {
            anime.set_alias(option.to_string());
            app.main_state.alias_anime = None;
            app.context.input_util.stop();
        }
        if right {
            app.alias_popup_state.textbox.text = option.to_string();
            app.alias_popup_state.textbox.cursor_location = option.len();
        }
    }

    if app.keydown(Keycode::Return) {
        app.main_state.alias_anime = None;
        app.context.input_util.stop();
    }

    if app.context.click_elem(outer_bounds_id) {
        app.main_state.search_anime = None;
        app.main_state.alias_anime = None;
        app.context.input_util.stop();
    }
}

fn draw_option(app: &mut App, option_id: usize, option: &str) -> (bool, bool) {
    let layout = app.context.rect_id(option_id);
    let font_info = INPUT_BOX_FONT_INFO;
    if app.context.state_id(option_id) {
        app.context.canvas.set_draw_color(color_hex(0x505050));
        app.context.canvas.fill_rect(layout).unwrap();
    }

    let (text_width, text_height) = app.context.text_manager.text_size(font_info, option);

    let side_pad = 5;
    if text_width > layout.width() - side_pad {
        let layout = layout.pad_left(side_pad as i32).pad_right(side_pad as i32);
        app.context.canvas.set_clip_rect(layout);
        draw_text(
            &mut app.context.canvas,
            &mut app.context.text_manager,
            font_info,
            option,
            color_hex(0xa0a0a0),
            layout.x,
            layout.y + (layout.height() as i32 - text_height as i32) / 2,
            None,
            None,
        );
        app.context.canvas.set_clip_rect(None);
    } else {
        draw_text_centered(
            &mut app.context.canvas,
            &mut app.context.text_manager,
            font_info,
            option,
            color_hex(0xa0a0a0),
            layout.x + layout.width() as i32 / 2,
            layout.y + layout.height() as i32 / 2,
            None,
            None,
        );
    }

    (app.context.click_elem(option_id), app.context.click_elem_right(option_id))
}

pub fn draw_main(app: &mut App, layout: Layout) {
    let (window_width, window_height) = app.context.canvas.window().size();
    let mut card_layouts = layout;
    let scroll = &mut app.main_state.scroll;
    register_scroll(&mut app.context, scroll, &mut card_layouts);

    let (_cards_per_row, card_layouts) = card_layouts
        .pad_top(CARD_Y_PAD_OUTER)
        .pad_bottom(CARD_Y_PAD_OUTER)
        .scroll_y(app.main_state.scroll.scroll)
        .split_grid_center(CARD_WIDTH, CARD_HEIGHT, CARD_X_PAD_INNER, CARD_Y_PAD_INNER);
    let card_layouts = card_layouts.take(app.database.len()).collect::<Vec<_>>();

    if app.main_state.search_anime.is_none() && app.main_state.alias_anime.is_none() {
        handle_main_events(app);
    } else {
        handle_main_search_events(app);
    }

    if let Some(last) = card_layouts.last() {
        let scroll = &mut app.main_state.scroll;
        scroll.max_scroll = last.bottom() - scroll.scroll;
    }

    let anime_list = app.database.animes();
    let mut any = false;
    for (idx, (grid_space, anime)) in card_layouts.iter().zip(anime_list.iter_mut()).enumerate() {
        if grid_space.y + grid_space.height() as i32 > 0 {
            if grid_space.y > window_height as i32 {
                break;
            }
            if draw_card(app, anime, idx, *grid_space) {
                any = true;
            }
        }
    }

    if !any {
        app.main_state.selected = None;
    }

    // Draw search
    if let Some(search_id) = app.main_state.search_anime {
        let width = window_width * 4 / 5;
        let height = window_height * 4 / 5;
        let x = (window_width - width) / 2;
        let y = (window_height - height) / 2;
        let float_layout = Layout::new(x as i32, y as i32, width, height);

        draw_main_anime_search(app, float_layout, search_id);
    }

    // Draw alias
    if let Some(alias_id) = app.main_state.alias_anime {
        //let (_, text_height) = app.text_manager.text_size(BACK_BUTTON_FONT_INFO, "");
        let width = window_width * 2 / 5;
        let height = 300;
        let x = (window_width - width) / 2;
        let y = (window_height - height) / 2;
        let float_layout = Layout::new(x as i32, y as i32, width, height);
        draw_main_anime_alias(app, float_layout, alias_id);
    }
}

pub fn draw_gradient(app: &mut App, layout: Layout, rounded: Option<i16>, gradient: Option<i32>) {
    let texture = app
        .context
        .image_manager
        .load(
            &mut app.context.canvas,
            MISSING_THUMBNAIL,
            TextureOptions::new()
                .ratio(Some((layout.width(), layout.height())))
                .rounded(rounded)
                .gradient(gradient),
        )
        .unwrap();

    app.context.canvas.set_blend_mode(BlendMode::Blend);
    app.context.canvas.copy(texture.as_ref(), None, layout).unwrap();
}

fn draw_thumbnail(app: &mut App, anime: &database::Anime, layout: Layout) {
    let gradient = Some(0);
    let rad = Some(CARD_RAD);
    if let Some(path) = anime.thumbnail() {
        if draw_image_clip(app, path, layout, rad, gradient).is_ok() {
            return;
        }
    }
    draw_missing_thumbnail(app, layout, rad);
    draw_gradient(app, layout, rad, gradient);
}

fn is_card_selected(app: &mut App, layout: Layout, idx: usize) -> bool {
    ((!app.main_state.keyboard_override && layout.contains_point(app.mouse_points()))
        || (app.main_state.keyboard_override && app.main_state.selected.is_some_and(|i| i == idx)))
        && (app.main_state.search_anime.is_none() && app.main_state.alias_anime.is_none())
}

fn draw_card_extra_menu(
    app: &mut App,
    anime: &mut database::Anime,
    mut layout: Layout,
    idx: usize,
) -> bool {
    app.context.canvas.set_clip_rect(layout);
    let scroll = &mut app.main_state.extra_menu_scroll;
    register_scroll(&mut app.context, scroll, &mut layout);
    let mut clicked = false;
    let menu_button_pad_outer = 10;
    let card_height = layout.height() * 2 / 7;
    let scroll_distance = scroll.scroll;
    let mut card_layouts = rect!(layout.x(), layout.y(), layout.width(), card_height)
        .scroll_y(scroll_distance)
        .split_even_hori(card_height)
        .map(|layout| layout.pad_outer(menu_button_pad_outer, menu_button_pad_outer));

    let alias_title_layout = card_layouts.next().unwrap();
    let change_title_layout = card_layouts.next().unwrap();
    let change_image_layout = card_layouts.next().unwrap();
    let attach_flag_layout = card_layouts.next().unwrap();

    scroll.max_scroll = attach_flag_layout.bottom() - scroll_distance - layout.y();

    let menu_button_style = Style::new(color_hex(0x909090), color_hex(0x202020))
        .bg_hover_color(color_hex(0x404040))
        .font_info(PLAY_BUTTON_FONT_INFO);

    if draw_button(
        app,
        "Change title",
        menu_button_style.clone(),
        change_title_layout,
    ) {
        clicked = true;
        app.title_popup_state.textbox.text.clear();
        app.main_state.search_anime = Some(idx as u32);
    }

    if draw_button(
        app,
        "Alias title",
        menu_button_style.clone(),
        alias_title_layout,
    ) {
        clicked = true;
        app.alias_popup_state.textbox.text = anime.display_title().to_owned();
        app.alias_popup_state.textbox.cursor_location = anime.display_title().len();
        app.main_state.alias_anime = Some(idx as u32);
    }

    if draw_button(
        app,
        "Change image",
        menu_button_style.clone(),
        change_image_layout,
    ) {
        clicked = true;
        let new_path = native_dialog::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "gif", "svg"])
            .show_open_single_file()
            .expect("Failed to open native file picker");
        if let Some(new_path) = new_path {
            anime.set_thumbnail(Some(new_path.to_string_lossy().to_string()));
        }
    }

    if draw_button(
        app,
        "Attach Flags",
        menu_button_style.clone(),
        attach_flag_layout,
    ) {
        clicked = true;
        eprintln!("Attach flags clicked!");
    }
    app.context.canvas.set_clip_rect(None);
    clicked
}

fn draw_card_hover_menu(app: &mut App, anime: &mut database::Anime, layout: Layout) -> bool {
    let mut clicked = false;
    let play_button_pad_outer = 10;
    let (play_current_layout, rest) = layout.split_hori(1, 3);
    let (play_next_layout, _more_info_layout) = rest.split_hori(1, 2);
    let play_current_layout =
        play_current_layout.pad_outer(play_button_pad_outer, play_button_pad_outer);

    let play_next_layout = play_next_layout.pad_outer(play_button_pad_outer, play_button_pad_outer);

    let play_button_style = Style::new(color_hex(0x909090), color_hex(0x202020))
        .bg_hover_color(color_hex(0x404040))
        .font_info(PLAY_BUTTON_FONT_INFO);

    let (current_ep, current_path) = anime.current_episode_path();

    if draw_button(
        app,
        &format!("Play Current: {}", current_ep),
        play_button_style.clone(),
        play_current_layout,
    ) {
        clicked = true;
        open_url(&current_path[0]).unwrap();
        anime.update_watched(current_ep.clone()).unwrap();

        if let Some(access_token) = app.database.anilist_access_token() {
            update_anilist_watched(&app.http_tx, access_token, anime);
        }

        app.main_state.scroll.scroll = 0;
    }

    if let Some((ep, path)) = anime.next_episode_path().unwrap() {
        if draw_button(
            app,
            &format!("Play Next: {}", ep),
            play_button_style.clone(),
            play_next_layout,
        ) {
            clicked = true;
            open_url(&path[0]).unwrap();
            anime.update_watched(ep).unwrap();
            app.main_state.scroll.scroll = 0;

            if let Some(access_token) = app.database.anilist_access_token() {
                update_anilist_watched(&app.http_tx, access_token, anime);
            }
        }
    }
    clicked
}

fn draw_card(app: &mut App, anime: &mut database::Anime, idx: usize, layout: Layout) -> bool {
    let card_id = app.context.create_id(layout);
    app.main_state.selectable.insert(card_id);
    // draw card background/border
    let mut selected = false;
    //let card_bg_color = color_hex(0x1C1C1C);
    let card_fg_color = color_hex(TITLE_FONT_COLOR);
    let (text_width, text_height) = {
        let title = anime.display_title();
        text_size(&mut app.context.text_manager, TITLE_FONT_INFO, title)
    };
    let (_top_layout, text_layout) =
        layout.split_hori(layout.height() - text_height - 19, layout.height());
    let top_layout = layout;
    let image_layout = layout;

    // draw thumbnail
    draw_thumbnail(app, anime, image_layout);

    if app.context.click_elem(card_id) {
        let idx = app.database.cache_idx_to_map_idx(idx);
        app.episode_state.episode_scroll.scroll = 0;
        app.main_state.alias_anime = None;
        app.main_state.search_anime = None;
        app.next_screen = Some(Screen::SelectEpisode(idx));
    }

    if app.context.click_elem_right(card_id) {
        // Toggle extra menu
        app.main_state.extra_menu_scroll.scroll = 0;
        match app.main_state.extra_menu_id {
            Some(_) => app.main_state.extra_menu_id = None,
            None => app.main_state.extra_menu_id = Some(app.context.id as u32),
        }
    }

    // draw title background
    let f = {
        || {
            if text_width > layout.width() - 35 {
                let mut title = anime.display_title().to_string();

                while text_size(
                    &mut app.context.text_manager,
                    TITLE_FONT_INFO,
                    format!("{title}..."),
                )
                .0 > layout.width() - 35
                {
                    title.pop();
                }

                format!("{title}...")
            } else {
                anime.display_title().to_string()
            }
        }
    };

    let string_manager = &mut app.context.string_manager;
    let title = string_manager.load(anime.display_title().as_ptr(), Format::Truncate, f);

    // draw title
    app.context.canvas.set_draw_color(card_fg_color);
    draw_text_centered(
        &mut app.context.canvas,
        &mut app.context.text_manager,
        TITLE_FONT_INFO,
        title,
        card_fg_color,
        text_layout.x + text_layout.width() as i32 / 2,
        text_layout.y + text_layout.height() as i32 / 2,
        None,
        None,
    );

    if layout.contains_point(app.mouse_points())
        && app.main_state.search_anime.is_none()
        && app.main_state.alias_anime.is_none()
    {
        app.context.canvas.set_blend_mode(BlendMode::Blend);
        selected = true;
        let image_hover_color = color_hex_a(0x1010108B);
        let image_rect = image_layout;
        app.context.canvas
            .rounded_box(
                image_rect.left() as i16,
                image_rect.top() as i16,
                image_rect.right() as i16,
                image_rect.bottom() as i16,
                CARD_RAD,
                image_hover_color,
            )
            .unwrap();

        if app
            .main_state
            .extra_menu_id
            .is_some_and(|id| id == app.context.id as u32)
        {
            draw_card_extra_menu(app, anime, top_layout, idx)
        } else {
            draw_card_hover_menu(app, anime, top_layout)
        };
    } else if app
        .main_state
        .extra_menu_id
        .is_some_and(|id| id == app.context.id as u32)
    {
        app.main_state.extra_menu_id = None;
    }
    selected
}
