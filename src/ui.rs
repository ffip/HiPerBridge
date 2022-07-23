use druid::{
    widget::{Flex, TextBox},
    *,
};

use scl_gui_widgets::widgets::*;

use crate::{
    app_state::AppState,
    hiper::{run_hiper, stop_hiper},
};

pub const SET_START_TEXT: Selector<&str> = Selector::new("set-start-text");
pub const SET_IP: Selector<String> = Selector::new("set-ip");
pub const SET_WARNING: Selector<String> = Selector::new("set-warning");
pub const SET_DISABLED: Selector<bool> = Selector::new("set-disabled");

fn main_page() -> Box<dyn Widget<AppState>> {
    Flex::column()
        // .with_child(label::new("HiPer Bridge").with_font(typography::SUBHEADER))
        .with_child(label::new("非官方轻量级 HiPer 启动器\nBy SteveXMH"))
        .with_flex_spacer(1.)
        .with_child(
            label::dynamic(|data: &AppState, _| data.warning.to_owned())
                .with_text_color(Color::Rgba32(0x9D5D00FF)),
        )
        .with_flex_spacer(1.)
        .with_child(label::dynamic(|data: &AppState, _| {
            if data.ip.is_empty() {
                "".into()
            } else {
                format!("Hiper 正在运行！\n你的 IP 联机地址：{}", data.ip)
            }
        }))
        .with_spacer(5.)
        .with_child(
            TextBox::new()
                .with_placeholder("凭证密钥（可选）")
                .lens(AppState::token)
                .disabled_if(|data, _| !data.ip.is_empty()),
        )
        .with_spacer(10.)
        .with_child(
            Flex::row()
                .with_flex_child(
                    Button::dynamic(|data: &AppState, _| data.start_button.to_owned())
                        .with_accent(true)
                        .on_click(|ctx, data, _| {
                            let ctx = ctx.get_external_handle();
                            let token = data.token.to_owned();
                            let use_tun = data.use_tun;
                            match data.start_button {
                                "启动" => {
                                    std::thread::spawn(move || {
                                        let _ =
                                            ctx.submit_command(SET_DISABLED, true, Target::Auto);
                                        run_hiper(ctx.to_owned(), token, use_tun);
                                        let _ =
                                            ctx.submit_command(SET_DISABLED, false, Target::Auto);
                                    });
                                }
                                "关闭" => {
                                    std::thread::spawn(move || {
                                        let _ =
                                            ctx.submit_command(SET_DISABLED, true, Target::Auto);
                                        stop_hiper(ctx.to_owned());
                                        let _ =
                                            ctx.submit_command(SET_DISABLED, false, Target::Auto);
                                    });
                                }
                                _ => {
                                    println!(
                                        "Warning: Unknown start button text {}",
                                        data.start_button
                                    );
                                }
                            }
                        })
                        .expand_width(),
                    1.,
                )
                .with_spacer(10.)
                .with_child(
                    IconButton::new(scl_gui_widgets::theme::icons::SETTINGS)
                        .on_click(|ctx, _, _| {
                            ctx.submit_command(ENABLE_BACK_PAGE.with(true));
                            ctx.submit_command(PUSH_PAGE.with("setting"));
                        })
                        .disabled_if(|data: &AppState, _| !data.ip.is_empty()),
                )
                .must_fill_main_axis(true),
        )
        // .must_fill_main_axis(true)
        .cross_axis_alignment(widget::CrossAxisAlignment::Fill)
        .padding((10., 10.))
        .boxed()
}

fn setting_page() -> Box<dyn Widget<AppState>> {
    Flex::column()
        .with_child(label::new("设置"))
        .with_spacer(10.)
        .with_child(label::new("使用 WinTUN 而非 TAP"))
        .with_spacer(5.)
        .with_child(ToggleSwitch::new().lens(AppState::use_tun))
        .cross_axis_alignment(widget::CrossAxisAlignment::Fill)
        .padding((10., 10.))
        .boxed()
}

pub fn ui_builder() -> impl Widget<AppState> {
    PageSwitcher::new()
        .with_page("main", Box::new(main_page))
        .with_page("setting", Box::new(setting_page))
}
