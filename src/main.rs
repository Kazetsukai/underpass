#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

mod network;
mod pins;
mod state;
mod streetlamps;
mod underpass_lights;
mod usb_device;
mod usb_ethernet;
mod web;

mod rp;

static DEVICE_NAME: &str = "Underpass Diorama";
static DEVICE_HOST: &str = "road";

static OUR_IP: Ipv4Addr = Ipv4Addr::new(10, 42, 0, 1);
static DNS_SERVERS: [Ipv4Addr; 1] = [OUR_IP];

const MTU: usize = 1514;

use {
    core::net::Ipv4Addr,
    defmt::info,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::{
        adc, bind_interrupts,
        clocks::RoscRng,
        gpio::{AnyPin, Level, Output},
        i2c::InterruptHandler,
        peripherals::{I2C1, PIN_8, PIO0, USB},
        pio::Pio,
        usb::{self, Driver},
    },
    embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex},
    embassy_time::{Duration, Timer},
    embassy_usb::{class::cdc_ncm::embassy_net::Device, UsbDevice},
    panic_probe as _,
    picoserve::make_static,
    rand::RngCore,
    smart_leds::RGB8,
    state::{AppState, SharedState, SharedStateMutex},
    streetlamps::StreetlampsRunner,
};

bind_interrupts!(struct Irqs {
    I2C1_IRQ => InterruptHandler<I2C1>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    ADC_IRQ_FIFO => adc::InterruptHandler;
    PIO0_IRQ_0 => embassy_rp::pio::InterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(AnyPin::from(p.PIN_22), Level::Low);

    let shared_state = SharedStateMutex(
        make_static!(Mutex<CriticalSectionRawMutex, SharedState>, Mutex::new(SharedState {
            streetlamps_enabled: true,
            streetlamps_brightness: 255,
            streetlamps_modes: [
                streetlamps::StreetlampMode::On,
                streetlamps::StreetlampMode::On,
                streetlamps::StreetlampMode::On,
                streetlamps::StreetlampMode::On,
                streetlamps::StreetlampMode::On,
                streetlamps::StreetlampMode::On,
            ],
            underpass_lights_state: underpass_lights::LightingState::Cars {
                default_color: RGB8::new(40, 20, 2),
            }
        })),
    );

    Timer::after_millis(100).await;

    // Generate random seed
    let mut rng = RoscRng;
    let seed = rng.next_u64();

    let usb_driver = Driver::new(p.USB, Irqs);

    let mut builder = usb_device::get_usb_builder(usb_driver);
    let (ncm_runner, device) = usb_ethernet::make_usb_ethernet_device(&mut builder);
    let (net_runner, stack) = network::make_network_stack(device, seed);
    let streetlamps_runner = streetlamps::StreetlampsRunner::new(
        [
            Output::new(p.PIN_2, Level::Low),
            Output::new(p.PIN_3, Level::Low),
            Output::new(p.PIN_4, Level::Low),
            Output::new(p.PIN_5, Level::Low),
            Output::new(p.PIN_6, Level::Low),
            Output::new(p.PIN_7, Level::Low),
        ],
        RoscRng,
        shared_state,
    );
    let usb = builder.build();
    let (app, config) = web::make_web_app();

    spawner.must_spawn(blinker(led, Duration::from_millis(500)));

    spawner.must_spawn(usb_task(usb));
    info!("USB task started");

    spawner.must_spawn(usb_ncm_task(ncm_runner));
    info!("USB NCM task started");

    spawner.must_spawn(network::net_task(net_runner));
    info!("Network task started");

    // Spawn network service tasks
    spawner.must_spawn(network::dhcp_task(stack));
    info!("DHCP server task started");

    spawner.must_spawn(network::mdns_task(stack));
    info!("mDNS server task started");

    for id in 0..web::WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web::web_task(
            id,
            stack,
            AppState {
                shared: shared_state,
            },
            app,
            config,
        ));
    }
    info!("Web task started");

    spawner.must_spawn(streetlamp_task(streetlamps_runner));
    info!("Streetlamp task started");

    spawner.must_spawn(underpass_lights_task(
        underpass_lights::UnderpassLightsRunner::new(
            Pio::new(p.PIO0, Irqs),
            p.PIN_8,
            p.DMA_CH0,
            RoscRng,
            shared_state,
        ),
    ));

    loop {
        Timer::after(Duration::from_secs(3)).await;
    }
}

#[embassy_executor::task]
async fn blinker(mut led: Output<'static>, interval: Duration) {
    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

#[embassy_executor::task]
pub(crate) async fn usb_task(mut usb: UsbDevice<'static, Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
pub(crate) async fn usb_ncm_task(
    class: embassy_usb::class::cdc_ncm::embassy_net::Runner<'static, Driver<'static, USB>, MTU>,
) -> ! {
    class.run().await
}

#[embassy_executor::task]
pub(crate) async fn net_task(mut runner: embassy_net::Runner<'static, Device<'static, MTU>>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn streetlamp_task(mut runner: StreetlampsRunner<Output<'static>, RoscRng, 6>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn underpass_lights_task(
    runner: underpass_lights::UnderpassLightsRunner<RoscRng, PIN_8>,
) -> ! {
    runner.run().await
}
