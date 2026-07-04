#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Ticker, Timer};
use esp_hal::Blocking;
use esp_hal::analog::adc::*;
use esp_hal::clock::CpuClock;
use esp_hal::interrupt::software::SoftwareInterrupt;
use esp_hal::peripherals::{ADC1, GPIO0, GPIO20};
use esp_hal::timer::timg::TimerGroup;
use esp_hal::uart::*;
use esp_println::logger::init_logger;
use esp_rtos::embassy::InterruptExecutor;
use static_cell::StaticCell;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

static ENTROPY_QUEUE: Channel<CriticalSectionRawMutex, u8, 4> = Channel::new();

static EXECUTOR_CELL: StaticCell<InterruptExecutor<1>> = StaticCell::new();
static SW_INT_CELL: StaticCell<SoftwareInterrupt<1>> = StaticCell::new();

#[embassy_executor::task]
async fn hard_realtime_adc_read(
    mut adc1: Adc<'static, ADC1<'static>, Blocking>,
    mut pin: AdcPin<GPIO0<'static>, ADC1<'static>>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(1));
    let mut random_byte: u8 = 0;
    let mut bit_count: u8 = 0;

    loop {
        ticker.next().await;

        if let Ok(value) = adc1.read_oneshot(&mut pin) {
            let chaotic_bit = (value & 0b1) as u8;
            random_byte = (random_byte << 1) | chaotic_bit;
            bit_count += 1;
            if bit_count == 8 {
                let _ = ENTROPY_QUEUE.try_send(random_byte);

                bit_count = 0;
                random_byte = 0;
            }
        }
    }
}

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // generator version: 1.1.0

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    let analog_pin = peripherals.GPIO0;
    let mut adc1_config = AdcConfig::new();
    let pin = adc1_config.enable_pin(analog_pin, Attenuation::_0dB);
    let adc1 = Adc::new(peripherals.ADC1, adc1_config);
    init_logger(log::LevelFilter::Info);

    let static_sw_int_1 = SW_INT_CELL.init(sw_interrupt.software_interrupt1);

    let mut uart_send = UartTx::new(peripherals.UART0, Config::default()).unwrap();

    let high_prio_exec = EXECUTOR_CELL.init(InterruptExecutor::new(static_sw_int_1.reborrow()));
    let spawner_high_prio = high_prio_exec.start(esp_hal::interrupt::Priority::Priority2);
    spawner_high_prio
        .spawn(hard_realtime_adc_read(adc1, pin))
        .unwrap();

    loop {
        let new_byte = ENTROPY_QUEUE.receive().await;
        match uart_send.write(&[new_byte]) {
            Ok(x) => (), // log::info!("Sent {x} bytes via uart"),
            Err(_) => log::warn!("Failed to send bytes via uart"),
        }
    }
}
