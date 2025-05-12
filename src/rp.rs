// RP chip implementations
use crate::pins::GpioPin;
use embassy_rp::gpio::Output;

impl GpioPin for Output<'_> {
    fn set_high(&mut self) {
        self.set_high();
    }

    fn set_low(&mut self) {
        self.set_low();
    }
}
