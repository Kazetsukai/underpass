use bincode::serde::{decode_from_slice, encode_into_slice};
use defmt::Format;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use sequential_storage::map::Value;

use crate::streetlamps::StreetlampMode;
use crate::underpass_lights::LightingState;

#[derive(serde::Deserialize, serde::Serialize, Clone, Format, PartialEq)]
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

impl<'a> Value<'a> for SharedState {
    fn serialize_into(
        &self,
        buffer: &mut [u8],
    ) -> Result<usize, sequential_storage::map::SerializationError> {
        // Serialise with Serde
        encode_into_slice(self, buffer, bincode::config::standard())
            .map_err(|_| sequential_storage::map::SerializationError::BufferTooSmall)
    }

    fn deserialize_from(
        buffer: &'a [u8],
    ) -> Result<Self, sequential_storage::map::SerializationError>
    where
        Self: Sized,
    {
        decode_from_slice::<Self, _>(buffer, bincode::config::standard())
            .map_err(|_| sequential_storage::map::SerializationError::InvalidData)
            .map(|(state, _)| state)
    }
}

const _: () = {
    // This check should be a bit smaller than the buffer allocated
    // as bincode may use more space than the Rust representation
    check_size::<SharedState, 100>();
};

const fn check_size<T, const N: usize>() {
    if core::mem::size_of::<T>() > N {
        panic!("the size of type shouldn't be so big")
    }
}
