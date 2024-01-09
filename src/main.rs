#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{ sync::{ Arc, Mutex }, time::Instant };

use config::{ load_config, save_config };
use druid::{ commands::{ CLOSE_ALL_WINDOWS, CONFIGURE_WINDOW, QUIT_APP }, WidgetExt as _, * };
use hiper::run_hiper_in_thread;
use scl_gui_widgets::{ widgets::*, WidgetExt as _ };

mod app_state;
mod config;
mod hiper;
mod icons;
mod log_parser;
mod open_url;
mod plugin;
mod ui;
mod utils;
#[cfg(target_os = "macos")]
mod mac;

pub type DynResult<T = ()> = std::result::Result<T, anyhow::Error>;

use app_state::AppState;
use ui::*;

fn main() {
    #[cfg(target_os = "linux")]
    {
        if !nix::unistd::getuid().is_root() {
            println!("HiPer Bridge requires root user to run!");
            println!("Use sudo/su to rerun to start as a root user!");
            return;
        }
    }

    let mut state = AppState::default();

    load_config(&mut state);

    let size = (295.0, 232.0 + 32.0);

    plugin::dispatch_event_and_wait("hb-launch");

    let saved_app_state = Arc::new(Mutex::new(state));
    loop {
        let saved_app_state_c = saved_app_state.clone();
        let cloned_app_state = { saved_app_state.lock().unwrap().to_owned() };

        let app = AppLauncher::with_window({
            let desc = WindowDesc::new(
                WindowWidget::new("HiPer Bridge", ui::ui_builder())
                    .on_command(SHOW_HIPER_WINDOW, |ctx, _, _data| {
                        ctx.submit_command(
                            CONFIGURE_WINDOW.with(
                                WindowConfig::default().set_window_state(WindowState::Restored)
                            )
                        );
                    })
                    .on_command(SET_DISABLED, |_, disabled, data| {
                        data.disabled = *disabled;
                    })
                    .on_command(SET_START_TEXT, |_, text, data| {
                        data.start_button = *text;
                    })
                    .on_command(SET_IP, |_ctx, ip, data| {
                        data.ip = ip.to_owned();
                    })
                    .on_command(SET_WARNING, |_, warning, data| {
                        data.warning = warning.to_owned();
                    })
                    .on_command(REQUEST_RESTART, |ctx, _, data| {
                        if !data.auto_restart | data.disabled | data.ip.is_empty() {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        if !data.disabled {
                            let token = data.token.to_owned();
                            let ctx = ctx.get_external_handle();
                            run_hiper_in_thread(
                                ctx,
                                token,
                                data.use_tun,
                                data.use_tcp,
                                data.use_igmp,
                                data.fast_mode,
                                data.debug_mode,
                                data.kill_hiper_when_start
                            );
                        }
                    })
                    .on_notify(BACK_PAGE_CLICKED, |ctx, _, _| {
                        ctx.submit_command(QUERY_POP_PAGE.with("main"));
                        ctx.submit_command(ENABLE_BACK_PAGE.with(false));
                    })
                    .on_notify(QUERY_CLOSE_WINDOW, move |ctx, _, data| {
                        if !data.disabled {
                            println!("Saving State");
                            let state = data.to_owned();
                            let mut saved_app_state = saved_app_state_c.lock().unwrap();
                            *saved_app_state = state;
                            let states = saved_app_state.to_owned();
                            save_config(&states);
                            ctx.submit_command(CLOSE_ALL_WINDOWS);
                            #[cfg(target_os = "macos")]
                            println!("Closing Window");
                            ctx.submit_command(QUIT_APP);
                        }
                    })
                    .disabled_if(|data, _| data.disabled)
            )
                .set_position({
                    let monitors = Screen::get_monitors();
                    let screen = monitors
                        .iter()
                        .find(|a| a.is_primary())
                        .unwrap_or_else(|| monitors.first().unwrap());
                    let screen_rect = screen.virtual_work_rect();
                    druid::Point::new(
                        (screen_rect.width() - size.0) / 2.0,
                        (screen_rect.height() - size.1) / 2.0
                    )
                })
                .resizable(false)
                .window_size(size)
                .window_size_policy(WindowSizePolicy::User)
                .title("HiPer Bridge");
            #[cfg(target_os = "macos")]
            {
                desc
            }
            #[cfg(not(target_os = "macos"))]
            {
                desc.show_titlebar(false)
            }
        }).configure_env(|env, _| {
            scl_gui_widgets::theme::color::set_color_to_env(
                env,
                scl_gui_widgets::theme::color::Theme::Light
            );

            env.set(icons::SETTINGS.0, include_str!("../assets/setting-path.txt"));
            env.set(icons::SETTINGS.1, Color::Rgba32(0x212121ff));
            env.set(icons::SETTINGS.2, Color::Rgba32(0xffffffff));

            env.set(crate::ui::CLIPBOARD_TEXT_PATH, include_str!("../assets/clipboard-text.txt"));
            env.set(crate::ui::CLIPBOARD_TEXT_COLOR, Color::Rgba32(0x212121ff));

            // Theme
            env.set(druid::theme::SCROLLBAR_WIDTH, 2.0);
            env.set(druid::theme::SCROLLBAR_BORDER_COLOR, Color::Rgba32(0x7a7a7aff));

            env.set(druid::theme::SCROLLBAR_COLOR, Color::Rgba32(0x7a7a7aff));
            env.set(druid::theme::BUTTON_BORDER_RADIUS, 2.0);
            env.set(druid::theme::BUTTON_BORDER_WIDTH, 0.0);
            env.set(druid::theme::BUTTON_DARK, Color::Rgba32(0xc6c6c6ff));
            env.set(druid::theme::BUTTON_LIGHT, Color::Rgba32(0xe0e0e0ff));
            env.set(druid::theme::TEXT_COLOR, env.get(scl_gui_widgets::theme::color::base::HIGH));
            env.set(druid::theme::TEXTBOX_INSETS, Insets::new(12.0, 6.0, 12.0, 6.0));

            env.set(scl_gui_widgets::theme::color::main::PRIMARY, Color::Rgba32(0x0071dcff));
            env.set(scl_gui_widgets::theme::color::main::SECONDARY, Color::Rgba32(0x0057aaff));
            env.set(druid::theme::PRIMARY_LIGHT, Color::Rgba32(0x0071dcff));
            env.set(druid::theme::PRIMARY_DARK, Color::Rgba32(0x75deffff));
            env.set(druid::theme::FOREGROUND_LIGHT, Color::Rgba32(0x0071dcff));
            env.set(druid::theme::FOREGROUND_DARK, Color::Rgba32(0x75deffff));

            env.set(scl_gui_widgets::theme::color::accent::ACCENT, Color::Rgba32(0x0071dcff));
            env.set(scl_gui_widgets::theme::color::accent::ACCENT_1, Color::Rgba32(0x0057aaff));
            env.set(
                scl_gui_widgets::theme::color::accent::ACCENT_LIGHT_1,
                Color::Rgba32(0x339cffff)
            );
            env.set(
                scl_gui_widgets::theme::color::accent::ACCENT_DARK_1,
                Color::Rgba32(0x0057aaff)
            );

            env.set(
                druid::theme::BACKGROUND_LIGHT,
                env.get(scl_gui_widgets::theme::color::alt::HIGH)
            );
            env.set(
                druid::theme::BACKGROUND_DARK,
                env.get(scl_gui_widgets::theme::color::alt::HIGH)
            );
            env.set(
                druid::theme::SELECTED_TEXT_BACKGROUND_COLOR,
                env.get(scl_gui_widgets::theme::color::accent::ACCENT)
            );
            env.set(druid::theme::CURSOR_COLOR, env.get(scl_gui_widgets::theme::color::base::HIGH));
            env.set(druid::theme::TEXTBOX_BORDER_WIDTH, 1.0);
            env.set(druid::theme::TEXTBOX_BORDER_RADIUS, 2.0);
        });

        app.launch(cloned_app_state).unwrap();

        if !hiper::is_running() {
            break;
        }

        let t = Instant::now();

        // 恢复窗口关闭期间的运行时间
        saved_app_state.lock().unwrap().run_time += t.elapsed().as_secs() as usize;
    }
    hiper::stop_hiper_directly();

    plugin::dispatch_event_and_wait("hb-exit");
}
