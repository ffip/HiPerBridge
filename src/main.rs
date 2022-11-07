#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use config::{load_config, save_config};
use druid::{
    commands::{CLOSE_ALL_WINDOWS, CONFIGURE_WINDOW, QUIT_APP},
    WidgetExt as _, *,
};
use hiper::run_hiper_in_thread;
use scl_gui_widgets::{widgets::*, WidgetExt as _};

mod app_state;
mod config;
mod hiper;
mod icons;
mod log_parser;
mod open_url;
mod plugin;
mod tray;
mod ui;
mod utils;
#[cfg(target_os = "macos")]
mod mac;

pub type DynResult<T = ()> = std::result::Result<T, anyhow::Error>;

use app_state::AppState;
use ui::*;

#[cfg(windows)]
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
#[cfg(windows)]
use windows::{core::PCWSTR, w};

fn main() {
    // Check if is admin
    #[cfg(windows)]
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
    #[cfg(target_os = "linux")]
    {
        if !nix::unistd::getuid().is_root() {
            println!("HiPer Bridge requires root user to run!");
            println!("Use sudo/su to rerun to start as a root user!");
            return;
        }
    }

    let mut state = AppState::default();
    tray::init_tray();
    tray::set_tooltip("HiPer Bridge");

    load_config(&mut state);

    if state.kill_hiper_when_start {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let _ = std::process::Command::new("taskkill.exe")
                .arg("/F")
                .arg("/IM")
                .arg("hiper.exe")
                .creation_flags(0x08000000)
                .status();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("kill")
                .arg("-9")
                .arg("hiper")
                .status();
        }
    }

    let size = (295., 232. + 32.);

    plugin::dispatch_event_and_wait("hb-launch");

    let saved_app_state = Arc::new(Mutex::new(state));
    loop {
        let saved_app_state_c = saved_app_state.clone();
        let cloned_app_state = { saved_app_state.lock().unwrap().to_owned() };

        let app = AppLauncher::with_window({
            let desc =
                WindowDesc::new(
                    WindowWidget::new("HiPer Bridge", ui::ui_builder())
                        .on_command(SHOW_HIPER_WINDOW, |ctx, _, _data| {
                            ctx.submit_command(CONFIGURE_WINDOW.with(
                                WindowConfig::default().set_window_state(WindowState::Restored),
                            ));
                        })
                        .on_command(SET_DISABLED, |_, disabled, data| {
                            data.disabled = *disabled;
                        })
                        .on_command(SET_START_TEXT, |_, text, data| {
                            data.start_button = *text;
                        })
                        .on_command(SET_IP, |_ctx, ip, data| {
                            data.ip = ip.to_owned();
                            tray::set_icon(!data.ip.is_empty());
                        })
                        .on_command(SET_VALID, |_, valid_at, data| {
                            data.valid_at = valid_at.to_owned();
                        })
                        .on_command(SET_WARNING, |_, warning, data| {
                            data.warning = warning.to_owned();
                        })
                        .on_command(REQUEST_RESTART, |ctx, _, data| {
                            if data.auto_restart && !data.ip.is_empty() {
                                let token = data.token.to_owned();
                                let use_tun = data.use_tun;
                                let ctx = ctx.get_external_handle();
                                run_hiper_in_thread(ctx, token, use_tun, data.debug_mode);
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
                                ctx.submit_command(CLOSE_ALL_WINDOWS);
                                #[cfg(target_os = "macos")]
                                println!("Closing Window");
                                ctx.submit_command(QUIT_APP);
                            }
                        })
                        .disabled_if(|data, _| data.disabled),
                )
                .set_position({
                    let monitors = Screen::get_monitors();
                    let screen = monitors
                        .iter()
                        .find(|a| a.is_primary())
                        .unwrap_or_else(|| monitors.first().unwrap());
                    let screen_rect = screen.virtual_work_rect();
                    druid::Point::new(
                        (screen_rect.width() - size.0) / 2.,
                        (screen_rect.height() - size.1) / 2.,
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
        })
        .configure_env(|env, _| {
            scl_gui_widgets::theme::color::set_color_to_env(
                env,
                scl_gui_widgets::theme::color::Theme::Light,
            );

            env.set(
                icons::SETTINGS.0,
                include_str!("../assets/setting-path.txt"),
            );
            env.set(icons::SETTINGS.1, Color::Rgba32(0x212121FF));
            env.set(icons::SETTINGS.2, Color::Rgba32(0xFFFFFFFF));

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
        });

        tray::set_ctx(app.get_external_handle());

        app.launch(cloned_app_state).unwrap();

        if !hiper::is_running() {
            break;
        }

        let t = Instant::now();

        tray::notify("HiPer Bridge 正在后台运行", "右键托盘图标重新打开主窗口");

        if let tray::TrayMessage::Exit = tray::take_command() {
            let state = saved_app_state.lock().unwrap().to_owned();
            save_config(&state);
            break;
        }

        // 恢复窗口关闭期间的运行时间
        saved_app_state.lock().unwrap().run_time += t.elapsed().as_secs() as usize;
    }
    hiper::stop_hiper_directly();

    tray::uninit_tray();

    plugin::dispatch_event_and_wait("hb-exit");
}
