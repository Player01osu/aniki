use sdl2::gfx::primitives::DrawRenderer;
use sdl2::keyboard::Mod;
use sdl2::rect::Rect;
use sdl2::{keyboard::Keycode, url::open_url};

use crate::anilist_serde::{MediaList, Viewer};
use crate::{rect, send_request, App, HttpData, LoginProgress, HttpMutex};

use super::episode_screen::DESCRIPTION_FONT_INFO;
use super::layout::Layout;
use super::{
    color_hex, draw_button, draw_input_box, draw_text, draw_text_centered, text_size,
    Screen, Style, DEFAULT_BUTTON_FONT, H1_FONT_INFO, H2_FONT_INFO, INPUT_BOX_FONT_INFO,
};

pub fn get_anilist_media_list(mutex: &HttpMutex, user_id: u64, access_token: &str) {
    let anime_list_query = include_str!("anime_list.gql");
    let json = serde_json::json!({"query": anime_list_query, "variables": {"id": 15125, "uid": user_id}});
    let request = reqwest::Client::new()
        .post("https://graphql.anilist.co")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string());
    send_request(&mutex, request, |res| async move {
        Ok(HttpData::MediaList(MediaList::deserialize_json(
            &res.bytes().await?,
        )?))
    });
}

pub fn send_login(mutex: &HttpMutex, access_token: &str) {
    let mutation = r#"query { Viewer { id } }"#;
    let json = serde_json::json!({"query": mutation, "variables": {"id": 15125}});
    let request = reqwest::Client::new()
        .post("https://graphql.anilist.co")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(json.to_string());
    let access_token = access_token.to_string();
    send_request(mutex, request, |res| async move {
        Ok(HttpData::Viewer(
            Viewer::deserialize_json(&res.bytes().await?)?,
            access_token.to_string(),
        ))
    });
}

pub fn draw_login(app: &mut App) {
    if let Some(cred) = app.database.anilist_cred() {
        get_anilist_media_list(&app.mutex, cred.user_id(), cred.access_token());
        app.next_screen = Some(Screen::Main);
        return;
    }

    if app.keymod.contains(Mod::LCTRLMOD) && app.keydown(Keycode::Escape) {
        app.database.skip_login_set(true);
        app.next_screen = Some(Screen::Main);
    } else if app.keydown(Keycode::Escape) {
        app.next_screen = Some(Screen::Main);
    }

    let (window_width, window_height) = app.canvas.window().size();
    let layout = Rect::new(0, 0, window_width, window_height);
    let (header_layout, rest) = layout.split_hori(1, 4);
    let (header_layout, link_layout) = header_layout.split_hori(1, 2);

    // draw anilist header
    draw_text_centered(
        &mut app.canvas,
        &mut app.text_manager,
        H1_FONT_INFO,
        "AniList Login",
        color_hex(0x909090),
        header_layout.x + header_layout.width() as i32 / 2,
        header_layout.y + header_layout.height() as i32 / 2,
        None,
        None,
    );

    // draw clickable link to `https://anilist.co/api/v2/oauth/authorize?client_id=15365&response_type=token`
    let link_str = "Click here to get access token";
    let (link_width, link_height) = app.text_manager.text_size(H2_FONT_INFO, link_str);
    let (link_layout, _) = link_layout.split_hori(link_height, link_layout.height());
    let x = link_layout.x + (link_layout.width() - link_width) as i32 / 2;
    let y = link_layout.y;
    let link_rect = rect!(x, y, link_width, link_height);
    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        H2_FONT_INFO,
        link_str,
        color_hex(0x707070),
        x,
        y,
        None,
        None,
    );

    let link_id = app.create_id(link_rect);
    if app.click_elem(link_id) {
        open_url("https://anilist.co/api/v2/oauth/authorize?client_id=15365&response_type=token")
            .unwrap();
    }

    // draw `access token` input field
    let (_, input_text_height) = app.text_manager.text_size(INPUT_BOX_FONT_INFO, "W");
    let (input_box_layout, rest) = rest.split_hori(input_text_height * 2, rest.height());

    // Pad 130 pixels on left and right
    let (_, input_box_layout) = input_box_layout.split_vert(130, input_box_layout.width());
    let (input_box_layout, _) =
        input_box_layout.split_vert(input_box_layout.width() - 130, input_box_layout.width());

    let (input_box_title_layout, input_box_layout) = input_box_layout.split_hori(1, 2);

    draw_text(
        &mut app.canvas,
        &mut app.text_manager,
        INPUT_BOX_FONT_INFO,
        "Access Token:",
        color_hex(0x7C7C7C),
        input_box_title_layout.x,
        input_box_title_layout.y,
        None,
        None,
    );

    let input_box_submit = draw_input_box(
        app,
        input_box_layout.x,
        input_box_layout.y,
        input_box_layout.width(),
    );

    // draw submit button
    let button_width_pad = 28;
    let button_height = 42;
    let button_font_info = (DEFAULT_BUTTON_FONT, 18);
    let (submit_button_text_width, _) = app.text_manager.text_size(button_font_info, "Submit");
    let (skip_button_text_width, _) = app.text_manager.text_size(button_font_info, "Skip Login");
    let (_, rest) = rest.split_hori(10, rest.height());
    let (button_layout, _rest) = rest.split_hori(button_height, rest.height());

    // Pad 130 pixels on left and right
    let (_, button_layout) = button_layout.split_vert(130, button_layout.width());
    let (button_layout, _) =
        button_layout.split_vert(button_layout.width() - 130, button_layout.width());

    let (submit_button_layout, skip_button_layout) = button_layout.split_vert(
        submit_button_text_width + button_width_pad,
        button_layout.width(),
    );
    let (_, skip_button_layout) = skip_button_layout.split_vert(
        skip_button_layout.width() - (skip_button_text_width + button_width_pad),
        skip_button_layout.width(),
    );

    let submit_button_style = Style::new(color_hex(0xDDDDDD), color_hex(0x009000))
        .bg_hover_color(color_hex(0x00AB00))
        .font_info(button_font_info);

    let skip_button_style = Style::new(color_hex(0x909090), color_hex(0x222222))
        .bg_hover_color(color_hex(0x444444))
        .font_info(button_font_info);
    if draw_button(app, "Submit", submit_button_style, submit_button_layout) || input_box_submit {
        app.login_progress = LoginProgress::Started;
        let access_token = &app.text_input;
        send_login(&app.mutex, access_token);
    }

    // draw skip login button
    if draw_button(app, "Skip Login", skip_button_style, skip_button_layout) {
        app.database.skip_login_set(true);
        app.next_screen = Some(Screen::Main);
    }

    match app.login_progress {
        LoginProgress::None => (),
        // TODO: Timeout if wait too long
        LoginProgress::Started => {
            let (width, height) = app.canvas.window().size();
            let rect = Rect::from_center((width as i32 / 2, height as i32 / 2), 200, 100);
            app.canvas
                .rounded_box(
                    rect.left() as i16,
                    rect.top() as i16,
                    rect.right() as i16,
                    rect.bottom() as i16,
                    6,
                    color_hex(0x202032),
                )
                .unwrap();

            draw_text_centered(
                &mut app.canvas,
                &mut app.text_manager,
                DESCRIPTION_FONT_INFO,
                "One second please...",
                color_hex(0x909090),
                width as i32 / 2,
                height as i32 / 2,
                None,
                None,
            );
        }
        LoginProgress::Failed => {
            let font_info = DESCRIPTION_FONT_INFO;
            let text = "Incorrect token; Try again!";
            let (text_width, _text_height) = text_size(&mut app.text_manager, font_info, text);
            draw_text(
                &mut app.canvas,
                &mut app.text_manager,
                font_info,
                text,
                color_hex(0xA04040),
                input_box_title_layout.x + input_box_title_layout.width() as i32 - text_width as i32,
                input_box_title_layout.y,
                None,
                None,
            );
        }
    }
}
