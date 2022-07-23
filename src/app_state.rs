use druid::{Data, Lens};

#[derive(Debug, Clone, Data, Lens)]
pub struct AppState {
    pub is_in_admin: bool,
    pub disabled: bool,
    pub token: String,
    pub start_button: &'static str,
    pub ip: String,
    pub warning: String,
    pub use_tun: bool,
}
