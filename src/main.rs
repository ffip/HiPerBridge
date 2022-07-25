#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use config::{load_config, save_config};
use druid::{commands::CLOSE_WINDOW, WidgetExt as _, *};
use hiper::run_hiper_in_thread;
use scl_gui_widgets::{widget_ext::WidgetExt as _, widgets::*};

mod app_state;
mod config;
mod hiper;
mod log_parser;
mod open_url;
mod ui;

pub type DynResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use app_state::AppState;
use ui::*;
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::{core::PCWSTR, w};

fn main() {
    // Check if is admin
    unsafe {
        if !IsUserAnAdmin().as_bool() {
            use std::os::windows::ffi::OsStrExt;
            let current_exe = std::env::current_exe().unwrap();
            let current_exe = current_exe.as_os_str();
            let current_exe = current_exe.encode_wide().chain(Some(0)).collect::<Vec<_>>();
            println!("Not in Admin! Restarting as admin!");
            ShellExecuteW(
                None,
                w!("runas"),
                PCWSTR::from_raw(current_exe.as_ptr()),
                w!(""),
                w!(""),
                1,
            );
            return;
        }
    }

    let mut state = AppState::default();

    load_config(&mut state);

    let size = (280., 232. + 32.);

    AppLauncher::with_window(
        WindowDesc::new(
            WindowWidget::new("HiPer Bridge", ui::ui_builder())
                .on_command(SET_DISABLED, |_, disabled, data| {
                    data.disabled = *disabled;
                })
                .on_command(SET_START_TEXT, |_, text, data| {
                    data.start_button = *text;
                })
                .on_command(SET_IP, |_, ip, data| {
                    data.ip = ip.to_owned();
                })
                .on_command(SET_WARNING, |_, warning, data| {
                    data.warning = warning.to_owned();
                })
                .on_command(REQUEST_RESTART, |ctx, _, data| {
                    if data.auto_restart && !data.ip.is_empty() {
                        let token = data.inner_token.to_owned();
                        let use_tun = data.use_tun;
                        let ctx = ctx.get_external_handle();
                        run_hiper_in_thread(ctx, token, use_tun);
                    }
                })
                .on_notify(BACK_PAGE_CLICKED, |ctx, _, _| {
                    ctx.submit_command(QUERY_POP_PAGE.with("main"));
                    ctx.submit_command(ENABLE_BACK_PAGE.with(false));
                })
                .on_notify(QUERY_CLOSE_WINDOW, |ctx, _, data| {
                    if !data.disabled {
                        data.disabled = true;
                        let wid = ctx.window_id();
                        let state = data.to_owned();
                        let ctx = ctx.get_external_handle();
                        std::thread::spawn(move || {
                            save_config(&state);
                            hiper::stop_hiper(ctx.to_owned());
                            let _ = ctx.submit_command(CLOSE_WINDOW, (), Target::Window(wid));
                        });
                    }
                })
                .disabled_if(|data, _| data.disabled),
        )
        .set_position({
            #[cfg(windows)]
            {
                let monitors = Screen::get_monitors();
                let screen = monitors.iter().find(|a| a.is_primary()).unwrap();
                let screen_rect = screen.virtual_work_rect();
                druid::Point::new(
                    (screen_rect.width() - size.0) / 2.,
                    (screen_rect.height() - size.1) / 2.,
                )
            }
            #[cfg(not(windows))]
            {
                druid::Point::new(
                    (screen_rect.width() - size.0) / 2.,
                    (screen_rect.height() - size.1) / 2.,
                )
            }
        })
        .resizable(false)
        .window_size(size)
        .window_size_policy(WindowSizePolicy::User)
        .title("HiPer Bridge")
        .show_titlebar(false),
    )
    .configure_env(|env, _| {
        scl_gui_widgets::theme::color::set_color_to_env(
            env,
            scl_gui_widgets::theme::color::Theme::Light,
        );

        env.set(
            scl_gui_widgets::theme::icons::SETTINGS.0,
            include_str!("../assets/setting-path.txt"),
        );
        env.set(
            scl_gui_widgets::theme::icons::SETTINGS.1,
            Color::Rgba32(0x212121FF),
        );
        env.set(
            scl_gui_widgets::theme::icons::SETTINGS.2,
            Color::Rgba32(0xFFFFFFFF),
        );

        env.set(
            crate::ui::CLIPBOARD_TEXT_PATH,
            include_str!("../assets/clipboard-text.txt"),
        );
        env.set(crate::ui::CLIPBOARD_TEXT_COLOR, Color::Rgba32(0x212121FF));

        // Theme
        env.set(druid::theme::SCROLLBAR_WIDTH, 2.);
        env.set(
            druid::theme::SCROLLBAR_BORDER_COLOR,
            Color::Rgba32(0x7A7A7AFF),
        );

        env.set(druid::theme::SCROLLBAR_COLOR, Color::Rgba32(0x7A7A7AFF));
        env.set(druid::theme::BUTTON_BORDER_RADIUS, 2.);
        env.set(druid::theme::BUTTON_BORDER_WIDTH, 0.);
        env.set(druid::theme::BUTTON_DARK, Color::Rgba32(0xC6C6C6FF));
        env.set(druid::theme::BUTTON_LIGHT, Color::Rgba32(0xE0E0E0FF));
        env.set(
            druid::theme::TEXT_COLOR,
            env.get(scl_gui_widgets::theme::color::base::HIGH),
        );
        env.set(
            druid::theme::TEXTBOX_INSETS,
            Insets::new(12.0, 6.0, 12.0, 6.0),
        );

        env.set(
            scl_gui_widgets::theme::color::main::PRIMARY,
            Color::Rgba32(0x0071DCFF),
        );
        env.set(
            scl_gui_widgets::theme::color::main::SECONDARY,
            Color::Rgba32(0x0057AAFF),
        );
        env.set(druid::theme::PRIMARY_LIGHT, Color::Rgba32(0x0071DCFF));
        env.set(druid::theme::PRIMARY_DARK, Color::Rgba32(0x75DEFFFF));
        env.set(druid::theme::FOREGROUND_LIGHT, Color::Rgba32(0x0071DCFF));
        env.set(druid::theme::FOREGROUND_DARK, Color::Rgba32(0x75DEFFFF));

        env.set(
            scl_gui_widgets::theme::color::accent::ACCENT,
            Color::Rgba32(0x0071DCFF),
        );
        env.set(
            scl_gui_widgets::theme::color::accent::ACCENT_1,
            Color::Rgba32(0x0057AAFF),
        );
        env.set(
            scl_gui_widgets::theme::color::accent::ACCENT_LIGHT_1,
            Color::Rgba32(0x339CFFFF),
        );
        env.set(
            scl_gui_widgets::theme::color::accent::ACCENT_DARK_1,
            Color::Rgba32(0x0057AAFF),
        );

        env.set(
            druid::theme::BACKGROUND_LIGHT,
            env.get(scl_gui_widgets::theme::color::alt::HIGH),
        );
        env.set(
            druid::theme::BACKGROUND_DARK,
            env.get(scl_gui_widgets::theme::color::alt::HIGH),
        );
        env.set(
            druid::theme::SELECTED_TEXT_BACKGROUND_COLOR,
            env.get(scl_gui_widgets::theme::color::accent::ACCENT),
        );
        env.set(
            druid::theme::CURSOR_COLOR,
            env.get(scl_gui_widgets::theme::color::base::HIGH),
        );
        env.set(druid::theme::TEXTBOX_BORDER_WIDTH, 1.);
        env.set(druid::theme::TEXTBOX_BORDER_RADIUS, 2.);
    })
    .launch(state)
    .unwrap();

    hiper::stop_hiper_directly();
}
