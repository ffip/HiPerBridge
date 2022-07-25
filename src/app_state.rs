use druid::{Data, Lens};

#[derive(Debug, Clone, Data, Lens)]
pub struct AppState {
    pub disabled: bool,
    pub token: String,
    pub inner_token: String,
    pub token_modified: bool,
    pub start_button: &'static str,
    pub ip: String,
    pub warning: String,
    pub use_tun: bool,
    pub auto_restart: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            disabled: false,
            token: "".into(),
            inner_token: "".into(),
            token_modified: false,
            start_button: "启动",
            ip: "".into(),
            warning: "".into(),
            use_tun: true,
            auto_restart: true,
        }
    }
}
