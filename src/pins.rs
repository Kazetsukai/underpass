pub trait GpioPin {
    fn set_high(&mut self);
    fn set_low(&mut self);
}

pub trait PwmPin {
    fn set_duty(&mut self, duty: u16);
    fn set_enabled(&mut self, enabled: bool);
}
