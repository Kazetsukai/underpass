use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use picoserve::response::IntoResponse;

use crate::streetlamps::StreetlampMode;
use crate::underpass_lights::LightingState;

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct SharedState {
    pub streetlamps_enabled: bool,
    pub streetlamps_brightness: u8,
    pub streetlamps_modes: [StreetlampMode; 6],
    pub underpass_lights_state: LightingState,
}

#[derive(Clone, Copy)]
pub struct SharedStateMutex(pub &'static Mutex<CriticalSectionRawMutex, SharedState>);

pub struct AppState {
    pub shared: SharedStateMutex,
}
impl picoserve::extract::FromRef<AppState> for SharedStateMutex {
    fn from_ref(state: &AppState) -> Self {
        state.shared
    }
}
