use sdl2::rect::Rect;
use sdl2::{keyboard::Keycode, url::open_url};

use crate::{database, get_scroll, Format, StringManager};
use crate::database::episode::Episode;
use crate::database::json_database::AnimeDatabaseData;
use crate::{
    ui::{color_hex, draw_text, BACK_BUTTON_FONT_INFO},
    App,
};

use super::layout::Layout;
use super::{
    draw_back_button, draw_image_float, draw_missing_thumbnail, draw_text_centered,
    update_anilist_watched, Screen, H1_FONT_INFO, H2_FONT_INFO, PLAY_ICON,
    THUMBNAIL_MISSING_SIZE, TITLE_FONT, TITLE_FONT_COLOR,
};

pub const DESCRIPTION_X_PAD_OUTER: u32 = 10;
pub const DESCRIPTION_Y_PAD_OUTER: u32 = 10;
pub const DESCRIPTION_FONT: &str = TITLE_FONT;
pub const DESCRIPTION_FONT_PT: u16 = 16;
pub const DESCRIPTION_FONT_INFO: (&str, u16) = (DESCRIPTION_FONT, DESCRIPTION_FONT_PT);
pub const DESCRIPTION_FONT_COLOR: u32 = TITLE_FONT_COLOR;

pub const DIRECTORY_NAME_FONT_INFO: (&str, u16) = DESCRIPTION_FONT_INFO;
pub const DIRECTORY_NAME_FONT_COLOR: u32 = 0x404040;

const THUMBNAIL_RAD: i16 = 6;

fn draw_episode_list(app: &mut App, anime: &database::Anime, mut layout: Rect) {
    app.canvas.set_clip_rect(layout);
    let string_manager = unsafe { &mut *(&mut app.string_manager as *mut StringManager) };
    let episode_height = 70;
    let episode_count = { anime.len() + 1 + anime.has_next_episode() as usize };
    let scroll = get_scroll(&mut app.episode_state.episode_scroll);
    app.register_scroll(scroll, &mut layout);
    let layouts = layout
        .scroll_y(app.episode_state.episode_scroll.scroll)
        .split_even_hori(episode_height)
        .take(episode_count)
        .collect::<Box<[Rect]>>();
    if let Some(last) = layouts.last() {
        let max_height = last.bottom() - scroll.scroll - layout.y();
        app.episode_state.episode_scroll.max_scroll = max_height;
    }

    if app.keydown(Keycode::Escape) {
        app.next_screen = Some(Screen::Main);
    }

    let mut layout_iter = layouts.iter();
    let current_ep = anime.current_episode();
    draw_episode(
        app,
        anime,
        &format!("Current: {current_ep}"),
        current_ep.clone(),
        *layout_iter.next().unwrap(),
        layout,
    );

    let next_ep = anime.next_episode();
    if let Some(next_ep) = next_ep {
        draw_episode(
            app,
            anime,
            &format!("Next: {next_ep}"),
            next_ep,
            *layout_iter.next().unwrap(),
            layout,
        );
    }

    let episode_map = anime.episodes();
    for (idx, (episode_layout, (episode, _))) in layout_iter.zip(episode_map).enumerate() {
        let episode_str = string_manager.load(
            anime.filename().as_ptr(),
            Format::Episode(idx as u8),
            || format!("{episode}"),
        );
        draw_episode(
            app,
            anime,
            episode_str,
            episode.to_owned(),
            *episode_layout,
            layout,
        );
    }
    app.canvas.set_clip_rect(None);
}

pub fn draw_anime_expand(app: &mut App, layout: Rect, anime: &database::Anime) {
    let layout = layout.pad_outer(DESCRIPTION_X_PAD_OUTER, DESCRIPTION_Y_PAD_OUTER);
    let (left_layout, right_layout) = layout.split_vert(1, 10);
    let (top_left_layout, _bottom_left_layout) = left_layout.split_hori(1, 11);
    let (top_description_layout, bottom_description_layout) = right_layout.split_hori(3, 7);
    let top_description_layout = top_description_layout.pad_bottom(10);
    let (back_button_layout, _) = top_left_layout.split_hori(10, 11);

    draw_top_panel_anime_expand(app, anime, top_description_layout);
    draw_back_button(app, Screen::Main, back_button_layout.pad_right(5));
    draw_episode_list(app, anime, bottom_description_layout);
}

fn draw_top_panel_with_metadata(
    app: &mut App,
    anime: &database::Anime,
    layout: Rect,
    metadata: &AnimeDatabaseData,
) {
    let (_, font_height) = app.text_manager.text_size(DIRECTORY_NAME_FONT_INFO, "L");
    let description_layout = layout;
    let (title_layout, description_layout) = description_layout.split_hori(2, 7);
    let (title_layout, description_header_layout) = title_layout.split_hori(1, 2);
    let (description_layout, directory_name_layout) = description_layout.split_hori(
        description_layout.height() - font_height,
        description_layout.height(),
    );
    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        H1_FONT_INFO,
        anime.display_title(),
        color_hex(DESCRIPTION_FONT_COLOR),
        title_layout.x,
        title_layout.y,
        Some(title_layout.width()),
        Some(title_layout.height()),
    );
    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        H2_FONT_INFO,
        "Description",
        color_hex(DESCRIPTION_FONT_COLOR),
        description_header_layout.x,
        description_header_layout.y,
        Some(description_header_layout.width()),
        Some(description_header_layout.height()),
    );
    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        DESCRIPTION_FONT_INFO,
        metadata.tags().join(", "),
        color_hex(DESCRIPTION_FONT_COLOR),
        description_layout.x,
        description_layout.y,
        Some(description_layout.width()),
        Some(description_layout.height()),
    );
    app.canvas.set_clip_rect(directory_name_layout);
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        DIRECTORY_NAME_FONT_INFO,
        anime.filename(),
        color_hex(DIRECTORY_NAME_FONT_COLOR),
        directory_name_layout.x + directory_name_layout.width() as i32 / 2,
        directory_name_layout.y + directory_name_layout.height() as i32 / 2,
        None,
        Some(directory_name_layout.height()),
    );
    app.canvas.set_clip_rect(None);
}

fn draw_top_panel_anime_expand(app: &mut App, anime: &database::Anime, layout: Rect) {
    let description_layout = match anime.thumbnail() {
        Some(thumbnail) => {
            if let Ok((image_width, image_height)) = app.image_manager.query_size(&mut app.canvas, thumbnail) {
                let (image_layout, description_layout) =
                    layout.split_vert(image_width * layout.height() / image_height, layout.width());
                let _ = draw_image_float(app, thumbnail, image_layout, None, Some(THUMBNAIL_RAD), None);
                description_layout.pad_outer(10, 10)
            } else {
                let (image_width, image_height) = THUMBNAIL_MISSING_SIZE;
                let (image_layout, description_layout) =
                    layout.split_vert(image_width * layout.height() / image_height, layout.width());
                draw_missing_thumbnail(app, image_layout, None);
                description_layout.pad_outer(10, 10)
            }
        }
        None => {
            let (image_width, image_height) = THUMBNAIL_MISSING_SIZE;
            let (image_layout, description_layout) =
                layout.split_vert(image_width * layout.height() / image_height, layout.width());
            draw_missing_thumbnail(app, image_layout, None);
            description_layout.pad_outer(10, 10)
        }
    };

    match anime.metadata() {
        Some(m) => draw_top_panel_with_metadata(app, anime, description_layout, m),
        None => {}
    }
}

fn draw_episode(
    app: &mut App,
    anime: &database::Anime,
    text: &str,
    episode: Episode,
    layout: Rect,
    _clip_rect: Rect,
) {
    let (play_width, play_height) = app
        .image_manager
        .query_size(&mut app.canvas, PLAY_ICON)
        .expect("Failed to load image");
    let (play_layout, ep_name_layout) = layout
        .pad_outer(0, 5)
        .pad_right(5)
        .split_vert(play_width * layout.height() / play_height, layout.width());
    let ep_name_layout = ep_name_layout.pad_left(30);
    let id = app.create_id(layout);
    app.episode_state.selectable.insert(id);

    if app.state_id(id) {
        app.canvas.set_draw_color(color_hex(0x4A4A4A));
        app.canvas.fill_rect(layout).unwrap();
    }
    if app.click_elem(id) {
        let anime = app.database.get_anime(anime.filename()).unwrap();
        anime.update_watched(episode.to_owned()).unwrap();
        let paths = anime.find_episode_path(&episode);
        app.episode_state.episode_scroll.scroll = 0;
        open_url(&paths[0]).unwrap();

        if let Some(access_token) = app.database.anilist_access_token() {
            update_anilist_watched(&app.http_tx, &access_token, anime);
        }
    }
    let _ = draw_image_float(app, PLAY_ICON, play_layout, Some((10, 0)), None, None);
    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        BACK_BUTTON_FONT_INFO,
        text,
        color_hex(DESCRIPTION_FONT_COLOR),
        ep_name_layout.x,
        ep_name_layout.y,
        Some(ep_name_layout.width()),
        None,
    );
    app.canvas.set_draw_color(color_hex(0x2A2A2A));
    app.canvas.draw_rect(layout).unwrap();
}
