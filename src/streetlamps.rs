const NUM_LAMPS: usize = 6;

use embassy_time::{Duration, Timer};
use rand::RngCore;

use crate::{pins::GpioPin, state::SharedStateMutex};

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy)]
pub enum StreetlampMode {
    Off,
    On,
    Flickering { chance: u32 },
}

pub struct StreetlampsRunner<T, R, const L: usize>
where
    T: GpioPin,
    R: RngCore,
{
    rng: R,
    shared_state: SharedStateMutex,
    lamp_pins: [T; L],
}

impl<T: GpioPin, R: RngCore, const L: usize> StreetlampsRunner<T, R, L> {
    pub fn new(lamp_pins: [T; L], rng: R, shared_state: SharedStateMutex) -> Self {
        Self {
            rng,
            shared_state,
            lamp_pins,
        }
    }

    pub async fn run(&mut self) -> ! {
        loop {
            Timer::after(Duration::from_millis(100)).await;

            {
                let SharedStateMutex(mutex) = self.shared_state;
                let state = mutex.lock().await;
                for i in 0..L {
                    let pin = &mut self.lamp_pins[i];
                    let mode = if state.streetlamps_enabled {
                        &state.streetlamps_modes[i]
                    } else {
                        &StreetlampMode::Off
                    };

                    match mode {
                        StreetlampMode::Off => pin.set_low(),
                        StreetlampMode::On => pin.set_high(),
                        StreetlampMode::Flickering { chance } => {
                            if self.rng.next_u32() % 100 < *chance {
                                pin.set_high();
                            } else {
                                pin.set_low();
                            }
                        }
                    }
                }
            }
        }
    }
}
