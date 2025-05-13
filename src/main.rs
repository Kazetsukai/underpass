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

const FLASH_SIZE: usize = 2 * 1024 * 1024; // 2MB
const FLASH_SIZE_U32: u32 = FLASH_SIZE as u32;
const FLASH_STORE_LOCATION: Range<u32> = (FLASH_SIZE_U32 - 128 * 1024)..FLASH_SIZE_U32; // 128KB

use {
    core::{net::Ipv4Addr, ops::Range},
    defmt::info,
    defmt_rtt as _,
    embassy_executor::Spawner,
    embassy_rp::{
        adc, bind_interrupts,
        clocks::RoscRng,
        flash::Flash,
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
    sequential_storage::{
        cache::NoCache,
        map::{fetch_item, store_item},
    },
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

    let mut diag_lights = [
        Output::new(p.PIN_16, Level::Low),
        Output::new(p.PIN_17, Level::Low),
        Output::new(p.PIN_18, Level::Low),
        Output::new(p.PIN_19, Level::Low),
    ];

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

    let mut flash: Flash<'_, _, _, FLASH_SIZE> = Flash::new(p.FLASH, p.DMA_CH1);
    let mut data_buffer = [0; 128];

    let val = fetch_item::<u8, SharedState, _>(
        &mut flash,
        FLASH_STORE_LOCATION.clone(),
        &mut NoCache::new(),
        &mut data_buffer,
        &1,
    )
    .await;
    match val {
        Ok(Some(val)) => {
            info!("Fetched value: {:?}", val);
            let SharedStateMutex(mutex) = shared_state;
            let mut state = mutex.lock().await;
            *state = val;
        }
        Err(err) => info!("Failed to fetch value: {:?}", err),
        _ => info!("Failed to fetch value"),
    }

    spawner.must_spawn(blinker(led, Duration::from_millis(500)));

    spawner.must_spawn(usb_task(usb));
    info!("USB task started");

    spawner.must_spawn(usb_ncm_task(ncm_runner));
    info!("USB NCM task started");
    diag_lights[0].set_high();

    spawner.must_spawn(network::net_task(net_runner));
    info!("Network task started");
    diag_lights[1].set_high();

    // Spawn network service tasks
    spawner.must_spawn(network::dhcp_task(stack));
    info!("DHCP server task started");

    spawner.must_spawn(network::mdns_task(stack));
    info!("mDNS server task started");
    diag_lights[2].set_high();

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
    info!("Underpass lights task started");
    diag_lights[3].set_high();

    let mut old_state = {
        let SharedStateMutex(mutex) = shared_state;
        let state = mutex.lock().await;
        state.clone()
    };

    loop {
        Timer::after(Duration::from_secs(3)).await;
        // Check if state has changed
        let SharedStateMutex(mutex) = shared_state;
        let state = { (*mutex.lock().await).clone() };
        if state != old_state {
            info!("State changed: {:?}", state);
            // Update the flash memory with the new state
            let result = store_item::<u8, SharedState, _>(
                &mut flash,
                FLASH_STORE_LOCATION.clone(),
                &mut NoCache::new(),
                &mut data_buffer,
                &1,
                &state,
            )
            .await;
            match result {
                Ok(_) => info!("Stored state"),
                Err(_) => info!("Failed to store state"),
            }

            old_state = state;
        }
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
