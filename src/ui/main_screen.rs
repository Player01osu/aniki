use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use sdl2::{
    keyboard::{self, Keycode},
    url::open_url,
};

use crate::database::json_database::AnimeDatabaseData;
use crate::{
    database,
    rect,
    ui::{color_hex, draw_text, BACK_BUTTON_FONT_INFO},
    App,
};
use crate::Format;

use super::{
    color_hex_a, draw_button, draw_image_clip, draw_missing_thumbnail, draw_text_centered,
    text_size, Layout, Screen, Style, PLAY_BUTTON_FONT_INFO, SCROLLBAR_COLOR,
    TITLE_FONT_COLOR, TITLE_FONT_INFO, draw_input_box, INPUT_BOX_FONT_INFO
};

pub const CARD_WIDTH: u32 = 200;
pub const CARD_HEIGHT: u32 = 300;
const CARD_X_PAD_OUTER: i32 = 10;
const CARD_Y_PAD_OUTER: i32 = 10;
const CARD_X_PAD_INNER: i32 = 20;
const CARD_Y_PAD_INNER: i32 = 20;

// TODO: Clean up event handling.
fn handle_main_events(
    app: &mut App,
    window_height: u32,
    card_layouts: &[Layout],
    cards_per_row: usize,
) {
    if app.main_search_anime.is_none() {
        if app.keydown(Keycode::J) {
            if let Some(last) = card_layouts.last() {
                if last.y + last.height as i32 > window_height as i32 {
                    app.main_scroll -= 40;
                }
            }
        } else if app.keydown(Keycode::K) {
            if let Some(first) = card_layouts.first() {
                if first.y < CARD_Y_PAD_OUTER {
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
                // Should exist
                let anime = unsafe { app.database.animes().get_unchecked(idx) };
                app.next_screen = Some(Screen::SelectEpisode(*anime));
            }
        }
    }
}

fn handle_main_search_events(app: &mut App) {
    if app.keydown(Keycode::Escape) {
        app.main_search_anime = None;
        app.main_alias_anime = None;
        app.input_util.stop();
    }
}

fn draw_main_anime_search(
    app: &mut App,
    layout: Layout,
    search_id: u32,
) {
    let (_, text_height) = app.text_manager.text_size(BACK_BUTTON_FONT_INFO, "");
    let anime = &mut app.database.animes()[search_id as usize];
    let options = {
        let search_previous: &mut Option<(String, Box<[*const AnimeDatabaseData]>)> =
            unsafe { &mut *(&mut app.main_search_previous as *mut _) };
        match search_previous {
            Some(prev) if prev.0 == app.text_input => &prev.1,
            _ => {
                let animes = app.database.fuzzy_find_anime(&app.text_input);
                let animes_cast: Box<[*const AnimeDatabaseData]> = animes
                    .iter()
                    .map(|v| (*v) as *const AnimeDatabaseData)
                    .collect();
                let (_, search) =
                    search_previous.insert((app.text_input.clone(), animes_cast.clone()));
                search
            }
        }
    };
    let (search_layout, option_layout) = layout.split_hori(text_height + 20, layout.height);
    let option_layouts = option_layout.split_even_hori(text_height + 20);

    app.canvas.set_draw_color(color_hex(0x303030));
    app.canvas.fill_rect(layout.to_rect()).unwrap();

    // TODO: Draw rect with border size
    app.canvas.set_draw_color(color_hex(0x101010));
    app.canvas.fill_rect(layout.to_rect()).unwrap();

    for (layout, option) in option_layouts.into_iter().zip(options.into_iter()) {
        let option = unsafe { &**option };
        if draw_option(app, layout, &option.title()) {
            anime.set_metadata(Some((*option).clone()));
            app
                .database
                .retrieve_images(&app.thumbnail_path)
                .unwrap();
            app.main_search_anime = None;
            app.input_util.stop();
            return;
        }
    }

    let search_width = search_layout.width * 8 / 9;
    let search_x = search_layout.x + ((layout.width - search_width) as i32 / 2);
    let search_y = search_layout.y + 10;
    draw_input_box(app, search_x, search_y, search_width);

    if app.mouse_clicked_left() && !layout.to_rect().contains_point(app.mouse_points()) {
        app.mouse_clicked_left_unset();
        app.main_search_anime = None;
        app.main_alias_anime = None;
        app.text_input.clear();
    }
}

fn draw_main_anime_alias(
    app: &mut App,
    layout: Layout,
    alias_id: u32,
) {
    let anime = &mut app.database.animes()[alias_id as usize];
    let (_, text_height) = app
        .text_manager
        .text_size(BACK_BUTTON_FONT_INFO, &app.text_input);
    let (search_layout, option_layout) = layout.split_hori(text_height + 40, layout.height);
    let search_width = search_layout.width * 8 / 9;
    let search_x = search_layout.x + ((search_layout.width - search_width) as i32 / 2);
    let search_y = search_layout.y + (search_layout.height as i32 - text_height as i32) / 2;
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
    let height = option_layout.height - 5;
    let (_, option_layout) = option_layout.split_hori(option_layout.height - height, height);
    let option_layouts = option_layout.split_even_hori(text_height + 20);

    app.canvas.set_draw_color(color_hex(0x303030));
    app.canvas.fill_rect(layout.to_rect()).unwrap();

    // TODO: Draw rect with border size
    app.canvas.set_draw_color(color_hex(0x101010));
    app.canvas.fill_rect(layout.to_rect()).unwrap();

    draw_input_box(app, search_x, search_y, search_width);

    for (layout, option) in option_layouts.into_iter().zip(options.into_iter()) {
        if draw_option(app, layout, option) {
            anime.set_alias(option.to_string());
            app.main_alias_anime = None;
            app.text_input.clear();
            app.input_util.stop();
        }
    }

    if app.keydown(Keycode::Return) {
        anime.set_alias(app.text_input.clone());
        app.main_alias_anime = None;
        app.text_input.clear();
        app.input_util.stop();
    }

    if app.mouse_clicked_left() && !layout.to_rect().contains_point(app.mouse_points()) {
        app.mouse_clicked_left_unset();
        app.main_search_anime = None;
        app.main_alias_anime = None;
    }
}

fn draw_option(app: &mut App, layout: Layout, option: &str) -> bool {
    let font_info = INPUT_BOX_FONT_INFO;
    if layout.to_rect().contains_point(app.mouse_points()) {
        app.canvas.set_draw_color(color_hex(0x505050));
        app.canvas.fill_rect(layout.to_rect()).unwrap();

        if app.mouse_clicked_left() {
            return true;
        }
    }
    let (text_width, text_height) = app.text_manager.text_size(font_info, option);

    let side_pad = 5;
    if text_width > layout.width - side_pad {
        let layout = layout.pad_left(side_pad as i32).pad_right(side_pad as i32);
        app.canvas.set_clip_rect(layout.to_rect());
        draw_text(
            &mut app.canvas,
            &mut app.text_manager,
            font_info,
            option,
            color_hex(0xa0a0a0),
            layout.x,
            layout.y + (layout.height as i32 - text_height as i32) / 2,
            None,
            None,
        );
        app.canvas.set_clip_rect(None);
    } else {
        draw_text_centered(
            &mut app.canvas,
            &mut app.text_manager,
            font_info,
            option,
            color_hex(0xa0a0a0),
            layout.x + layout.width as i32 / 2,
            layout.y + layout.height as i32 / 2,
            None,
            None,
        );
    }

    false
}

pub fn draw_main(app: &mut App) {
    let (window_width, window_height) = app.canvas.window().size();
    let (card_layouts, scrollbar_layout) =
        Layout::new(0, 0, window_width, window_height).split_vert(796, 800);

    let (cards_per_row, card_layouts) = card_layouts
        .pad_top(CARD_Y_PAD_OUTER)
        .pad_bottom(CARD_Y_PAD_OUTER)
        .scroll_y(app.main_scroll)
        .split_grid_center(
            CARD_WIDTH,
            CARD_HEIGHT,
            CARD_X_PAD_INNER,
            CARD_Y_PAD_INNER,
        );
    let card_layouts = card_layouts.take(app.database.len()).collect::<Vec<_>>();

    if app.main_search_anime.is_none() && app.main_alias_anime.is_none() {
        handle_main_events(app, window_height, &card_layouts, cards_per_row);
    } else {
        handle_main_search_events(app);
    }
    if app.resized() {
        if let Some(last) = card_layouts.last() {
            if (last.y + last.height as i32) < window_height as i32 {
                app.main_scroll -= last.y + last.height as i32 - window_height as i32;
            }
        }
    }

    let anime_list = app.database.animes();
    let mut any = false;
    for (idx, (grid_space, anime)) in card_layouts.iter().zip(anime_list.iter_mut()).enumerate() {
        if grid_space.y + grid_space.height as i32 > 0 {
            if grid_space.y > window_height as i32 {
                break;
            }
            app.id += 1;
            if draw_card(app, anime, idx, *grid_space) {
                any = true;
            }
        }
    }

    // Draw scrollbar
    if let Some(last) = card_layouts.last() {
        let scale =
            scrollbar_layout.height as f32 / (last.y + last.height as i32 - app.main_scroll) as f32;
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

    // Draw search
    if let Some(search_id) = app.main_search_anime {
        let width = window_width * 4 / 5;
        let height = window_height * 4 / 5;
        let x = (window_width - width) / 2;
        let y = (window_height - height) / 2;
        let float_layout = Layout::new(x as i32, y as i32, width, height);

        draw_main_anime_search(app, float_layout, search_id);
    }

    // Draw alias
    if let Some(alias_id) = app.main_alias_anime {
        //let (_, text_height) = app.text_manager.text_size(BACK_BUTTON_FONT_INFO, "");
        let width = window_width * 2 / 5;
        let height = 300;
        let x = (window_width - width) / 2;
        let y = (window_height - height) / 2;
        let float_layout = Layout::new(x as i32, y as i32, width, height);
        draw_main_anime_alias(app, float_layout, alias_id);
    }
}

fn draw_thumbnail(app: &mut App, anime: &database::Anime, layout: Layout) {
    if let Some(path) = anime.thumbnail() {
        if draw_image_clip(app, path, layout).is_ok() {
            return;
        }
    }
    draw_missing_thumbnail(app, layout);
}

fn is_card_selected(app: &mut App, layout: Layout, idx: usize) -> bool {
    ((!app.main_keyboard_override && layout.to_rect().contains_point(app.mouse_points()))
        || (app.main_keyboard_override && app.main_selected.is_some_and(|i| i == idx)))
        && (app.main_search_anime.is_none() && app.main_alias_anime.is_none())
}

fn draw_card_extra_menu(
    app: &mut App,
    anime: &mut database::Anime,
    layout: Layout,
    idx: usize,
) -> bool {
    let mut clicked = false;
    let menu_button_pad_outer = 10;
    let (change_title_layout, rest) = layout.split_hori(1, 3);
    let (alias_title_layout, change_image_layout) = rest.split_hori(1, 2);
    let change_title_layout =
        change_title_layout.pad_outer(menu_button_pad_outer, menu_button_pad_outer);
    let alias_title_layout =
        alias_title_layout.pad_outer(menu_button_pad_outer, menu_button_pad_outer);
    let change_image_layout =
        change_image_layout.pad_outer(menu_button_pad_outer, menu_button_pad_outer);
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
        app.text_input.clear();
        app.main_search_anime = Some(idx as u32);
    }

    if draw_button(
        app,
        "Alias title",
        menu_button_style.clone(),
        alias_title_layout,
    ) {
        clicked = true;
        app.text_input = anime.display_title().to_owned();
        app.main_alias_anime = Some(idx as u32);
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
        clicked = true;
        open_url(&current_path[0]).unwrap();
        anime.update_watched(current_ep).unwrap();
        app.main_scroll = 0;
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
            app.main_scroll = 0;
        }
    }
    clicked
}

fn draw_card(app: &mut App, anime: &mut database::Anime, idx: usize, layout: Layout) -> bool {
    // draw card background/border
    let mut selected = false;
    let card_bg_color = color_hex(0x1C1C1C);
    let card_fg_color = color_hex(TITLE_FONT_COLOR);
    let (text_width, text_height) = {
        let title = anime.display_title();
        text_size(&mut app.text_manager, TITLE_FONT_INFO, title)
    };
    let (top_layout, text_layout) = layout.split_hori(layout.height - text_height, layout.height);
    let image_layout = layout;

    // draw thumbnail
    draw_thumbnail(app, anime, image_layout);

    if is_card_selected(app, layout, idx) {
        app.canvas.set_blend_mode(BlendMode::Blend);
        selected = true;
        app.main_selected = Some(idx);
        app.canvas.set_draw_color(color_hex_a(0x303030AA));
        app.canvas.fill_rect(image_layout.to_rect()).unwrap();

        if app.main_extra_menu_id.is_some_and(|id| id == app.id) {
            draw_card_extra_menu(app, anime, top_layout, idx)
        } else {
            draw_card_hover_menu(app, anime, top_layout)
        };
        if app.mouse_clicked_right() {
            // Toggle extra menu
            match app.main_extra_menu_id {
                Some(_) => app.main_extra_menu_id = None,
                None => app.main_extra_menu_id = Some(app.id),
            }
        }

        if app.mouse_clicked_left() {
            app.episode_scroll = 0;
            app.main_alias_anime = None;
            app.main_search_anime = None;
            app.next_screen = Some(Screen::SelectEpisode(anime));
        }
    } else if app.main_extra_menu_id.is_some_and(|id| id == app.id) {
        app.main_extra_menu_id = None;
    }

    // draw title background
    app.canvas.set_draw_color(card_bg_color);
    app.canvas.fill_rect(text_layout.to_rect()).unwrap();

    let f = {
        || {
            if text_width > layout.width - 35 {
                let mut title = anime.display_title().to_string();

                while text_size(
                    &mut app.text_manager,
                    TITLE_FONT_INFO,
                    format!("{title}..."),
                )
                .0 > layout.width - 35
                {
                    title.pop();
                }

                format!("{title}...")
            } else {
                anime.display_title().to_string()
            }
        }
    };

    let string_manager = &mut app.string_manager;
    let title = string_manager.load(anime.display_title().as_ptr(), Format::Truncate, f);

    // draw title
    app.canvas.set_draw_color(card_fg_color);
    app.canvas.draw_rect(text_layout.to_rect()).unwrap();
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
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
