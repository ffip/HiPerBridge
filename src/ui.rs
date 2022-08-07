use druid::{
    widget::{Flex, TextBox},
    *,
};

use scl_gui_widgets::{
    theme::icons::{IconColorKey, IconKeyPair, IconPathKey},
    widget_ext::WidgetExt as _,
    widgets::*,
};

use std::{fmt::Write, time::Duration};

use crate::{
    app_state::{AppState, TimerTokenData},
    hiper::{run_hiper_in_thread, stop_hiper},
    open_url::open_url,
};

pub const CLIPBOARD_TEXT_ICON: IconKeyPair = (
    CLIPBOARD_TEXT_PATH,
    CLIPBOARD_TEXT_COLOR,
    CLIPBOARD_TEXT_COLOR,
);
pub const CLIPBOARD_TEXT_COLOR: IconColorKey = IconColorKey::new("clipboard-text-color");
pub const CLIPBOARD_TEXT_PATH: IconPathKey = IconPathKey::new("clipboard-text-path");

pub const SET_START_TEXT: Selector<&str> = Selector::new("set-start-text");
pub const SET_IP: Selector<String> = Selector::new("set-ip");
pub const SET_WARNING: Selector<String> = Selector::new("set-warning");
pub const SET_DISABLED: Selector<bool> = Selector::new("set-disabled");
pub const REQUEST_RESTART: Selector = Selector::new("request-restart");

fn main_page() -> Box<dyn Widget<AppState>> {
    Flex::column()
        // .with_child(label::new("HiPer Bridge").with_font(typography::SUBHEADER))
        .with_child(label::new("轻快若风 x 安如磐石 - 最佳跨区域组网方案"))
        .with_spacer(10.)
        .with_flex_child(
            label::dynamic(|data: &AppState, _| data.warning.to_owned())
                .with_text_color(Color::Rgba32(0x9D5D00FF))
                .scroll()
                .vertical()
                .expand(),
            1.,
        )
        .with_child(
            Flex::row()
                .with_flex_child(
                    label::dynamic(|data: &AppState, _| {
                        if data.ip.is_empty() {
                            "".into()
                        } else {
                            let sec = data.run_time % 60;
                            let min = data.run_time / 60;
                            let hour = min / 60;
                            let day = hour / 24;
                            let min = min % 60;
                            let hour = hour % 60;

                            let mut run_time_formated = String::with_capacity(16);

                            if day > 0 {
                                let _ = write!(run_time_formated, "{}:", day);
                            }

                            if day > 0 || hour > 0 {
                                let _ = write!(run_time_formated, "{:02}:", hour);
                            }

                            let _ = write!(run_time_formated, "{:02}:{:02}", min, sec);

                            format!(
                                "Hiper 正在运行！\n网络地址：{}\n运行时间：{}",
                                data.ip, run_time_formated
                            )
                        }
                    })
                    .with_text_color(Color::Rgba32(0x0F7B0FFF))
                    .expand_width(),
                    1.,
                )
                .with_child(
                    IconButton::new(CLIPBOARD_TEXT_ICON)
                        .with_flat(true)
                        .on_click(|_, data: &mut AppState, _| {
                            use clipboard::ClipboardProvider;
                            #[cfg(windows)]
                            {
                                if let Ok(mut cb) =
                                    clipboard::windows_clipboard::WindowsClipboardContext::new()
                                {
                                    let _ = cb.set_contents(data.ip.to_owned());
                                }
                            }
                        }),
                )
                .show_if(|data: &AppState, _| !data.ip.is_empty()),
        )
        .with_spacer(5.)
        .with_child(
            TextBox::new()
                .with_placeholder("凭证密钥")
                .lens(AppState::token)
                .on_change(|_, old_data, data, _| {
                    data.token_modified |= old_data.token != data.token;
                })
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
                            if data.token_modified {
                                data.inner_token = data.token.to_owned();
                                if !data.inner_token.is_empty() {
                                    data.token = "••••••••".into();
                                }
                                data.token_modified = false;
                            }
                            let token = data.inner_token.to_owned();
                            let use_tun = data.use_tun;
                            match data.start_button {
                                "启动" => {
                                    run_hiper_in_thread(ctx, token, use_tun);
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
                        .expand_width()
                        .disabled_if(|data: &AppState, _| data.token.trim().is_empty()),
                    1.,
                )
                .with_spacer(10.)
                .with_child(
                    IconButton::new(scl_gui_widgets::theme::icons::SETTINGS).on_click(
                        |ctx, _, _| {
                            ctx.submit_command(ENABLE_BACK_PAGE.with(true));
                            ctx.submit_command(PUSH_PAGE.with("setting"));
                        },
                    ),
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
        .with_child(label::new("选项"))
        .with_spacer(10.)
        .with_child(label::new("使用 WinTUN 而非 WinTAP"))
        .with_spacer(5.)
        .with_child(
            ToggleSwitch::new()
                .lens(AppState::use_tun)
                .disabled_if(|data: &AppState, _| !data.ip.is_empty()),
        )
        .with_spacer(10.)
        .with_child(label::new("发生错误时自动重启"))
        .with_spacer(5.)
        .with_child(ToggleSwitch::new().lens(AppState::auto_restart))
        .with_spacer(10.)
        .with_child(label::new("关于"))
        .with_spacer(10.)
        .with_child(label::new("HiPer Bridge v0.0.5"))
        .with_child(label::new("轻量级 HiPer Plus 启动器"))
        .with_child(label::new("By SteveXMH"))
        .with_spacer(10.)
        .with_child(label::new("HiPer / Matrix"))
        .with_child(label::new("一款轻量、敏捷、去中心化的跨区域组网系统"))
        .with_spacer(10.)
        .with_child(Button::new("用户服务协议").on_click(|_, _, _| {
            open_url("https://mcer.cn/agreement");
        }))
        .with_spacer(5.)
        .with_child(Button::new("隐私条款").on_click(|_, _, _| {
            open_url("https://mcer.cn/privacy");
        }))
        .cross_axis_alignment(widget::CrossAxisAlignment::Fill)
        .padding((10., 10.))
        .scroll()
        .vertical()
        .expand()
        .boxed()
}

pub struct AppWrapper {
    inner: WidgetPod<AppState, Box<dyn Widget<AppState>>>,
}

impl AppWrapper {
    pub fn new(inner: impl Widget<AppState> + 'static) -> Self {
        Self {
            inner: WidgetPod::new(inner).boxed(),
        }
    }
}

impl Widget<AppState> for AppWrapper {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppState, env: &Env) {
        if let Event::Timer(tt) = event {
            if &data.run_timer.0 == tt {
                data.run_time += 1;
                data.run_timer = TimerTokenData(ctx.request_timer(Duration::from_secs(1)));
                ctx.request_update();
            }
        } else if let Event::Command(cmd) = event {
            if let Some(ip) = cmd.get(SET_IP) {
                if ip.is_empty() {
                    data.run_timer = TimerTokenData::default();
                } else {
                    data.run_time = 0;
                    data.run_timer = TimerTokenData(ctx.request_timer(Duration::from_secs(1)));
                }
            }
        }
        self.inner.event(ctx, event, data, env)
    }

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &AppState, env: &Env) {
        self.inner.lifecycle(ctx, event, data, env)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, _old_data: &AppState, data: &AppState, env: &Env) {
        self.inner.update(ctx, data, env)
    }

    fn layout(
        &mut self,
        ctx: &mut LayoutCtx,
        bc: &BoxConstraints,
        data: &AppState,
        env: &Env,
    ) -> Size {
        self.inner.layout(ctx, bc, data, env)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) {
        self.inner.paint(ctx, data, env)
    }
}

pub fn ui_builder() -> impl Widget<AppState> {
    AppWrapper::new(
        PageSwitcher::new()
            .with_page("main", Box::new(main_page))
            .with_page("setting", Box::new(setting_page)),
    )
}
