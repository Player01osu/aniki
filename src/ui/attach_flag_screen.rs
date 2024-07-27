use sdl2::{keyboard::Keycode, rect::Rect};

use crate::{
    database::{AnimeMapIdx, PairFlag, SingleVideoPlayerFlag},
    rect, register_scroll, switch, textbox, App, BindFlag, SingleFlag,
};

use super::{
    color_hex, draw_back_button, draw_button, layout::Layout, Screen, Style, DEFAULT_BUTTON_FONT,
    PLAY_BUTTON_FONT_INFO,
};

fn save_flags(app: &mut App, idx: AnimeMapIdx) {
    let anime = app.database.get_mut_idx(idx);
    let state = &app.attach_flag_state;

    anime.video_player = Some(state.video_player_textbox.text.clone());

    anime.single_flags.clear();
    for single_flag in &state.single_flags {
        let flag = SingleVideoPlayerFlag {
            enabled: single_flag.switch.toggled,
            flag: single_flag.textbox.text.clone(),
        };
        anime.single_flags.push(flag);
    }

    anime.pair_flags.enabled = state.bind_flags_switch.toggled;
    anime.pair_flags.pair_flags.clear();
    anime.pair_flags.video_regex = state.regex_textbox.text.clone();
    for bind_flag in &state.bind_flags {
        let flag = PairFlag {
            enabled: bind_flag.switch.toggled,
            search_path: bind_flag.search_path_textbox.text.clone(),
            flag: bind_flag.flag_textbox.text.clone(),
            use_deliminator: bind_flag.deliminator_switch.toggled,
            deliminator: bind_flag.deliminator_textbox.text.clone(),
            regex: bind_flag.regex_textbox.text.clone(),
        };
        anime.pair_flags.pair_flags.push(flag);
    }
}

pub fn draw_attach_flag(app: &mut App, layout: Rect, idx: AnimeMapIdx) {
    let mut layout = layout;

    if app.keydown(Keycode::Escape) {
        app.next_screen = Some(Screen::Main);
    }

    register_scroll(
        &mut app.context,
        &mut app.attach_flag_state.scroll,
        &mut layout,
    );

    layout.set_height(u32::MAX);
    layout.set_y(app.attach_flag_state.scroll.scroll);

    let (mut back_button_layout, mut layout) = layout.split_vert(120, layout.width());
    back_button_layout.set_height(60);
    back_button_layout.offset(10, 10);
    draw_back_button(app, Screen::Main, back_button_layout);

    layout.offset(0, 15);
    let button_font_info = (DEFAULT_BUTTON_FONT, 18);
    let save_button_style = Style::new(color_hex(0xDDDDDD), color_hex(0x009000))
        .bg_hover_color(color_hex(0x00AB00))
        .font_info(button_font_info);
    let (mut save_layout, mut layout) = layout.split_hori(40, layout.height());
    save_layout.offset(25, 0);
    save_layout.set_width(100);

    if draw_button(&mut app.context, "Save", save_button_style, save_layout) {
        save_flags(app, idx);
    }

    layout.offset(0, 15);

    // Draw video player command
    let video_player_switch = switch(
        &mut app.context,
        &mut app.attach_flag_state.video_player_switch,
        "Video Player:",
        &mut layout,
    );

    textbox(
        &mut app.context,
        &mut app.attach_flag_state.video_player_textbox,
        None,
        video_player_switch,
        25,
        &mut layout,
    );

    // Draw single flag options
    let mut delete_idx = None;
    for (i, single_flag) in app.attach_flag_state.single_flags.iter_mut().enumerate() {
        let single_flag_switch = switch(
            &mut app.context,
            &mut single_flag.switch,
            "Enable flag",
            &mut layout,
        );

        textbox(
            &mut app.context,
            &mut single_flag.textbox,
            None,
            single_flag_switch,
            25,
            &mut layout,
        );

        let (mut delete_button_layout, new_layout) = layout.split_hori(40, layout.height());
        let delete_center = delete_button_layout.center();
        delete_button_layout.set_width(100);
        delete_button_layout.center_on(delete_center);
        let delete_button_style = Style::new(color_hex(0xB0B0B0), color_hex(0x901010))
            .bg_hover_color(color_hex(0xAB2020))
            .font_info(PLAY_BUTTON_FONT_INFO);
        if draw_button(
            &mut app.context,
            "Delete",
            delete_button_style,
            delete_button_layout,
        ) {
            delete_idx = Some(i);
        }
        layout = new_layout;
        layout.offset(0, 15);
    }
    if let Some(i) = delete_idx {
        app.attach_flag_state.single_flags.remove(i);
    }

    let style = Style::new(color_hex(0x909090), color_hex(0x202020))
        .bg_hover_color(color_hex(0x404040))
        .font_info(PLAY_BUTTON_FONT_INFO);

    let (mut button_layout, mut layout) = layout.split_hori(50, layout.height());
    let button_width = 124;
    button_layout.set_width(124);
    button_layout.set_x(layout.center().x() - button_width / 2);

    if draw_button(&mut app.context, "Add flag", style, button_layout) {
        let mut single_flag = SingleFlag::default();
        single_flag.switch.toggled = true;
        app.attach_flag_state.single_flags.push(single_flag);
    };

    // Draw flag pairing options
    if switch(
        &mut app.context,
        &mut app.attach_flag_state.bind_flags_switch,
        "Toggle Bind Flags",
        &mut layout,
    ) {
        let layout = rect!(layout.x(), layout.y() + 10, layout.width(), layout.height());
        let (_, mut layout) = layout.split_hori(10, layout.height());

        textbox(
            &mut app.context,
            &mut app.attach_flag_state.regex_textbox,
            Some("Video Regex:"),
            true,
            24,
            &mut layout,
        );

        let mut delete_idx = None;
        for (i, bind_flag) in app.attach_flag_state.bind_flags.iter_mut().enumerate() {
            let flag_toggle = switch(
                &mut app.context,
                &mut bind_flag.switch,
                "Toggle Flag",
                &mut layout,
            );

            layout = layout.pad_top(15);

            textbox(
                &mut app.context,
                &mut bind_flag.flag_textbox,
                Some("Flag:"),
                flag_toggle,
                45,
                &mut layout,
            );

            let deliminator_toggle = switch(
                &mut app.context,
                &mut bind_flag.deliminator_switch,
                "Use non-space deliminator",
                &mut layout,
            );

            textbox(
                &mut app.context,
                &mut bind_flag.deliminator_textbox,
                Some("Deliminator:"),
                deliminator_toggle && flag_toggle,
                45,
                &mut layout,
            );

            textbox(
                &mut app.context,
                &mut bind_flag.regex_textbox,
                Some("Flag Regex:"),
                flag_toggle,
                45,
                &mut layout,
            );

            textbox(
                &mut app.context,
                &mut bind_flag.search_path_textbox,
                Some("Search path:"),
                flag_toggle,
                45,
                &mut layout,
            );

            let (mut delete_button_layout, new_layout) = layout.split_hori(40, layout.height());
            let delete_center = delete_button_layout.center();
            delete_button_layout.set_width(100);
            delete_button_layout.center_on(delete_center);
            let delete_button_style = Style::new(color_hex(0xB0B0B0), color_hex(0x901010))
                .bg_hover_color(color_hex(0xAB2020))
                .font_info(PLAY_BUTTON_FONT_INFO);
            if draw_button(
                &mut app.context,
                "Delete",
                delete_button_style,
                delete_button_layout,
            ) {
                delete_idx = Some(i);
            }
            layout = new_layout;
            layout.offset(0, 15);
        }
        if let Some(i) = delete_idx {
            app.attach_flag_state.bind_flags.remove(i);
        }

        let style = Style::new(color_hex(0x909090), color_hex(0x202020))
            .bg_hover_color(color_hex(0x404040))
            .font_info(PLAY_BUTTON_FONT_INFO);

        let (mut button_layout, layout) = layout.split_hori(50, layout.height());
        let button_width = 124;
        button_layout.set_width(124);
        button_layout.set_x(layout.center().x() - button_width / 2);

        if draw_button(&mut app.context, "Add bind flag", style, button_layout) {
            let mut bind_flag = BindFlag::default();
            bind_flag.switch.toggled = true;
            app.attach_flag_state.bind_flags.push(bind_flag);
        };

        app.attach_flag_state.scroll.max_scroll =
            button_layout.bottom() - app.attach_flag_state.scroll.scroll;
    } else {
        app.attach_flag_state.scroll.max_scroll =
            layout.top() - app.attach_flag_state.scroll.scroll;
    }
}
