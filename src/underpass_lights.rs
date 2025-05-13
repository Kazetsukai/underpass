use core::cmp::max;

use defmt::{debug, Format};
use embassy_rp::dma::Channel;
use embassy_rp::gpio::Pin;
use embassy_rp::pio::PioPin;
use embassy_rp::Peripheral;
use embassy_time::{Duration, Ticker};
use rand::RngCore;

use embassy_rp::peripherals::{DMA_CH0, PIO0};
use embassy_rp::pio::Pio;
use embassy_rp::pio_programs::ws2812::{PioWs2812, PioWs2812Program};
use smart_leds::RGB8;

use crate::state::SharedStateMutex;

const NUM_LEDS_PER_LANE: usize = 8;
const NUM_LANES: usize = 2;
const NUM_LEDS: usize = NUM_LANES * NUM_LEDS_PER_LANE;
const LED_POSITIONS: [u16; NUM_LEDS_PER_LANE] = [
    // Left and right lanes are the same in reverse
    2000, 7500, 11500, 17000, 21300, 26800, 30800, 36300,
];

const MAX_CARS: usize = 10;
const MAX_CAR_DISTANCE: i32 = 30000;

// 1km/h is 0.27777m/s, or 277.77mm/s
// 1km/h real scale = 4.34mm/s at 1/64 scale

// Positions of LEDs: 20mm, 75mm, 115mm, 170mm, 213mm, 268mm, 308mm, 363mm

#[derive(serde::Deserialize, serde::Serialize, Format, Clone, Copy, PartialEq)]
pub enum LightingState {
    Off,
    SingleColour(RGB8),
    RainbowCycle,
    Cars { default_color: RGB8 },
}

pub struct UnderpassLightsRunner<R, T>
where
    R: RngCore,
    T: PioPin,
{
    pio: Pio<'static, PIO0>,
    data_pin: T,
    dma: DMA_CH0,
    rng: R,
    shared_state: SharedStateMutex,
}

#[derive(Clone, Copy, defmt::Format)]
struct CarState {
    position: i32,
    speed: i32,
    lane: u8,
}

impl<R: RngCore, T: PioPin> UnderpassLightsRunner<R, T> {
    pub fn new(
        pio: Pio<'static, PIO0>,
        data_pin: T,
        dma: DMA_CH0,
        rng: R,
        shared_state: SharedStateMutex,
    ) -> Self {
        Self {
            pio,
            data_pin,
            dma,
            rng,
            shared_state,
        }
    }

    pub async fn run(mut self) -> ! {
        let Pio {
            mut common, sm0, ..
        } = self.pio;

        let mut data = [RGB8::default(); NUM_LEDS];

        let mut cycle: u16 = 0;

        let mut cars: [Option<CarState>; MAX_CARS] = [None; MAX_CARS];
        let mut car_light: [RGB8; NUM_LEDS];

        let program = PioWs2812Program::new(&mut common);
        let mut ws2812 = PioWs2812::new(&mut common, sm0, self.dma, self.data_pin, &program);
        let mut ticker = Ticker::every(Duration::from_millis(10));
        let mut dirty = true;
        let mut last_state = LightingState::Off;
        loop {
            let SharedStateMutex(mutex) = self.shared_state;
            {
                let state = mutex.lock().await;

                match state.underpass_lights_state {
                    LightingState::Off => {
                        if last_state != state.underpass_lights_state {
                            for i in 0..NUM_LEDS {
                                data[i] = RGB8::default();
                            }
                            dirty = true;
                        }
                    }
                    LightingState::SingleColour(colour) => {
                        if last_state != state.underpass_lights_state {
                            for i in 0..NUM_LEDS {
                                data[i] = colour;
                            }
                            dirty = true;
                        }
                    }
                    LightingState::RainbowCycle => {
                        for i in 0..NUM_LEDS {
                            data[i] = wheel(
                                ((((i * 256) as u16 / NUM_LEDS as u16).wrapping_add(cycle)) & 255)
                                    as u8,
                            );
                        }
                        dirty = true;
                    }
                    LightingState::Cars { default_color } => {
                        car_light = [RGB8::default(); NUM_LEDS];

                        for car_state in cars.iter_mut() {
                            if let Some(car) = car_state {
                                car.position += car.speed as i32;

                                for i in 0..NUM_LEDS_PER_LANE {
                                    let led_pos = LED_POSITIONS[i] as i32;
                                    if car.position > led_pos - MAX_CAR_DISTANCE
                                        && car.position < led_pos + MAX_CAR_DISTANCE
                                    {
                                        let dist = (car.position - led_pos).abs();
                                        let div: u8 = if car.position < led_pos { 1 } else { 1 };

                                        let power: u8 = max(
                                            80 * (MAX_CAR_DISTANCE - dist) / (MAX_CAR_DISTANCE),
                                            0,
                                        )
                                            as u8;
                                        let falloff_power: u8 = max(
                                            80 * (max(MAX_CAR_DISTANCE - dist * 4, 0))
                                                / (MAX_CAR_DISTANCE),
                                            0,
                                        )
                                            as u8;
                                        if car.position > led_pos {
                                            car_light[i as usize] += RGB8::new(
                                                falloff_power,
                                                falloff_power,
                                                falloff_power / 3,
                                            ) / div;
                                            car_light[i as usize] += RGB8::new(power, 0, 0);
                                        } else {
                                            car_light[i as usize] +=
                                                RGB8::new(power, power, power / 3) / div;
                                            car_light[i as usize] += RGB8::new(falloff_power, 0, 0);
                                        }
                                    }
                                }

                                if car.position
                                    > LED_POSITIONS[NUM_LEDS_PER_LANE - 1] as i32 + MAX_CAR_DISTANCE
                                {
                                    *car_state = None;
                                }
                            }
                        }

                        for i in 0..NUM_LEDS {
                            data[i] = default_color + car_light[i];
                        }
                        dirty = true;

                        if cycle % 500 == 10 {
                            debug!("Car: {}", cars[0]);
                            for i in 0..MAX_CARS {
                                if cars[i].is_none() {
                                    cars[i] = Some(CarState {
                                        position: -MAX_CAR_DISTANCE,
                                        speed: 434 as i32,
                                        lane: (self.rng.next_u32() % NUM_LANES as u32) as u8,
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }

                last_state = state.underpass_lights_state;
            }

            cycle = cycle.wrapping_add(1);
            if dirty {
                ws2812.write(&data).await;
            }

            ticker.next().await;
        }
    }
}

/// Input a value 0 to 255 to get a color value
/// The colours are a transition r - g - b - back to r.
fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}
