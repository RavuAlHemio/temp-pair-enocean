#![no_main]
#![no_std]


mod crc8;
mod enocean;
mod i2c;
mod hmi_display;
mod spi;
mod temp_display;
mod uart;


use core::panic::PanicInfo;

use cortex_m_rt::entry;
use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::spi1::cr1::BR;

use crate::i2c::{I2c, I2c2, I2cAddress};
use crate::spi::{Spi, Spi1, SpiMode};
use crate::temp_display::TempDisplayState;
use crate::uart::{Uart, Usart2, Usart3};


pub const CLOCK_SPEED_HZ: u32 = 25_000_000;


#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    loop {
    }
}


/// Reconfigures the clocks of the microcontroller.
///
/// By default, the clocks are set up as follows:
/// ```plain
/// ╭────────╮ ╒══════╕
/// │ HSI    ├─┤ HPRE ├───┬─────────┬─────────╴╴╴──┐ AHB (max. 216 MHz)
/// │ 16 MHz │ │   /1 ├┐  │         ╳              ╳
/// ╰────────╯ └──────┘│ ┌┴───────┐┌┴───────┐     ┌┴───────┐
///                    │ │ SYSCLK ││ GPIOA  │ ... │ GPIOE  │
///                    │ │ 16 MHz ││ 16 MHz │     │ 16 MHz │
///                    │ └────────┘└────────┘     └────────┘
///                    │╒═══════╕
///                    ├┤ PPRE1 ├──┬─────────┬─────────┐ APB1 (max. 54 MHz)
///                    ││    /1 │  ╳         ╳         ╳
///                    │└───────┘ ┌┴───────┐┌┴───────┐┌┴───────┐
///                    │          │ USART2 ││ USART3 ││ I2C2   │
///                    │          │ 16 MHz ││ 16 MHz ││ 16 MHz │
///                    │          └────────┘└────────┘└────────┘
///                    │╒═══════╕
///                    └┤ PPRE2 ├──┐ APB2 (max. 108 MHz)
///                     │    /1 │  ╳
///                     └───────┘ ┌┴─────────┐
///                               │ SPI1     │
///                               │ 16 MHz   │
///                               ╞═  ═  ═  ═╡
///                               │ PRESC /2 │
///                               │ 8 MHz    │
///                               └──────────┘
/// ```
///
/// The board has an external oscillator Y1 with 25 MHz.
///
/// The EnOncean module (on USART2) requires 57600 b/s, so we must solve:
///
/// 57 600 b/s = 25 000 000 b Hz / USARTDIV
///
/// which gives 434.027...; we can fit 434 (0x1B2) in a 16-bit register.
///
/// The emergency USART we can configure for any speed; even a USARTDIV of 2604 (0xA2C) for the
/// venerable 9600 b/s fits.
///
/// For I2C, I use the I2C timing calculator at https://ondrahosek.com/stm32-i2c-timing-calc/ with:
///
/// * target I2C bus frequency = 100 kHz
/// * I2C peripheral clock frequency = 25000 kHz
/// * I2C mode = standard
/// * analog filter = yes
/// * digital filter count = 15
/// * rise time = 1000 ns
/// * fall time = 300 ns
///
/// which gives:
///
/// PRESC = 1, SDADEL = 0, SCLDEL = 15, SCLL = 49, SCLH = 40
///
/// For SPI1, we get power-of-two prescalers from /2 to /256, which means we top out at 12.5 MHz.
/// The 7-segment driver chip (TLC5947) allows up to 30 MHz (15 MHz in 50%-duty cascade operation)
/// and the flash chip (AT25FF321A) absolutely bottoms out at 30 MHz for the slow-read category, so
/// 25 MHz is fine.
///
/// Set it all up:
///
/// ```plain
/// ╭────────╮ ╒══════╕
/// │ HSE    ├─┤ HPRE ├───┬─────────┬─────────╴╴╴──┐ AHB (max. 216 MHz)
/// │ 25 MHz │ │   /1 ├┐  │         │              │
/// ╰────────╯ └──────┘│ ┌┴───────┐┌┴───────┐     ┌┴───────┐
///                    │ │ SYSCLK ││ GPIOA  │ ... │ GPIOE  │
///                    │ │ 25 MHz ││ 25 MHz │     │ 25 MHz │
///                    │ └────────┘└────────┘     └────────┘
///                    │╒═══════╕ 
///                    ├┤ PPRE1 ├──┬─────────┬─────────┐ APB1 (max. 54 MHz)
///                    ││    /1 │  │         │         │
///                    │└───────┘ ┌┴───────┐┌┴───────┐┌┴───────┐
///                    │          │ USART2 ││ USART3 ││ I2C2   │
///                    │          │ 25 MHz ││ 25 MHz ││ 25 MHz │
///                    │          └────────┘└────────┘└────────┘
///                    │╒═══════╕
///                    └┤ PPRE2 ├──┐ APB2 (max. 108 MHz)
///                     │    /1 │  │
///                     └───────┘ ┌┴─────────┐
///                               │ SPI1     │
///                               │ 25 MHz   │
///                               ╞═  ═  ═  ═╡
///                               │ PRESC /2 │
///                               │ 12.5 MHz │
///                               └──────────┘
/// ```
fn setup_clocks(peripherals: &mut Peripherals) {
    // start up the external high-speed oscillator (HSE)

    // HSEBYP=0: crystal between OSCIN and OSCOUT
    // HSEBYP=1: clock on OSCIN while OSCOUT is floating
    // we have a crystal, not a clock
    peripherals.RCC.cr().modify(|_, w| w
        .hsebyp().clear_bit()
    );

    // turn on HSE
    peripherals.RCC.cr().modify(|_, w| w
        .hseon().set_bit()
    );

    // wait for HSE to become ready
    while peripherals.RCC.cr().read().hserdy().is_not_ready() {
    }

    // set flash wait states
    // we run on 3.3V, which means steps of 30 MHz
    // 0 MHz < 25 MHz < 30 MHz => 0 wait states
    peripherals.FLASH.acr().modify(|_, w| w
        .latency().ws0()
    );

    // set prescalers to /1
    peripherals.RCC.cfgr().modify(|_, w| w
        .hpre().div1() // warning: max. 216 MHz
        .ppre2().div1() // warning: max. 108 MHz
        .ppre1().div1() // warning: max. 54 MHz
    );

    // switch clock input over to HSE
    peripherals.RCC.cfgr().modify(|_, w| w
        .sw().hse()
    );

    // wait until clock input switches over
    while !peripherals.RCC.cfgr().read().sws().is_hse() {
    }

    // feed the clock to the peripherals we want
    peripherals.RCC.ahb1enr().modify(|_, w| w
        .gpioaen().enabled()
        .gpioben().enabled()
        .gpiocen().enabled()
        .gpioden().enabled()
        .gpioeen().enabled()
        .gpiogen().enabled()
    );
    peripherals.RCC.apb1enr().modify(|_, w| w
        .usart2en().enabled()
        .usart3en().enabled()
        .i2c2en().enabled()
    );
    peripherals.RCC.apb2enr().modify(|_, w| w
        .spi1en().enabled()
    );
}

fn setup_pins(peripherals: &mut Peripherals) {
    // choose alternate functions
    peripherals.GPIOA.afrl().modify(|_, w| w
        .afrl2().af7() // PA2 to USART2 Tx
        .afrl3().af7() // PA3 to USART2 Rx
        .afrl5().af5() // PA5 to SPI1 SCK
        .afrl6().af5() // PA6 to SPI1 CIPO
        .afrl7().af5() // PA7 to SPI1 COPI
    );
    peripherals.GPIOB.afrh().modify(|_, w| w
        .afrh10().af4() // PB10 to I2C2 SCL
        .afrh11().af4() // PB11 to I2C2 SDA
    );
    peripherals.GPIOD.afrh().modify(|_, w| w
        .afrh8().af7() // PD8 to USART3 Tx
        .afrh9().af7() // PD9 to USART3 Rx
    );

    // set push-pull on output ports except I2C
    peripherals.GPIOA.otyper().modify(|_, w| w
        .ot2().push_pull()
        .ot3().push_pull()
        .ot5().push_pull()
        .ot6().push_pull()
        .ot7().push_pull()
    );
    peripherals.GPIOB.otyper().modify(|_, w| w
        .ot0().push_pull()
        .ot10().open_drain()
        .ot11().open_drain()
    );
    peripherals.GPIOC.otyper().modify(|_, w| w
        .ot6().push_pull()
        .ot15().push_pull()
    );
    peripherals.GPIOD.otyper().modify(|_, w| w
        .ot8().push_pull()
        .ot9().push_pull()
        .ot12().push_pull()
        .ot13().push_pull()
    );
    peripherals.GPIOE.otyper().modify(|_, w| w
        .ot7().push_pull()
        .ot8().push_pull()
    );
    peripherals.GPIOG.otyper().modify(|_, w| w
        .ot11().push_pull()
    );

    // set pulling on input ports
    peripherals.GPIOB.pupdr().modify(|_, w| w
        .pupdr14().pull_up() // AS1115 datasheet says: either floating or GND
    );
    peripherals.GPIOD.pupdr().modify(|_, w| w
        .pupdr15().floating() // not used
    );

    // set port modes (input/output/analog/alternate)
    peripherals.GPIOA.moder().modify(|_, w| w
        .moder2().alternate() // USART2
        .moder3().alternate() // USART2
        .moder5().alternate() // SPI1
        .moder6().alternate() // SPI1
        .moder7().alternate() // SPI1
    );
    peripherals.GPIOB.moder().modify(|_, w| w
        .moder0().output() // 7seg chip 2 select
        .moder10().alternate() // I2C2
        .moder11().alternate() // I2C2
        .moder14().input() // HMI button push interrupt
    );
    peripherals.GPIOC.moder().modify(|_, w| w
        .moder6().output() // blank 7seg displays
        .moder15().output() // reset EnOcean module
    );
    peripherals.GPIOD.moder().modify(|_, w| w
        .moder8().alternate() // USART3
        .moder9().alternate() // USART3
        .moder12().output() // flash write protection
        .moder13().output() // 7seg chip 1 select
    );
    peripherals.GPIOE.moder().modify(|_, w| w
        .moder7().output() // reset flash chip
        .moder8().output() // flash chip select for SPI1
    );
    peripherals.GPIOG.moder().modify(|_, w| w
        .moder11().output() // reset 7seg (but actually only ClickID)
    );

    // set UART2, I2C and SPI ports to fast
    peripherals.GPIOA.ospeedr().modify(|_, w| w
        .ospeedr2().high_speed()
        .ospeedr3().high_speed()
        .ospeedr5().high_speed()
        .ospeedr6().high_speed()
        .ospeedr7().high_speed()
    );
    peripherals.GPIOB.ospeedr().modify(|_, w| w
        .ospeedr10().high_speed()
        .ospeedr11().high_speed()
    );

    // set SPI chip-selects all high
    peripherals.GPIOB.odr().modify(|_, w| w
        .odr0().high()
    );
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr13().high()
    );
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );
}


const fn divide_u32_to_u16_round(dividend: u32, divisor: u32) -> u16 {
    let quotient = (dividend + (divisor / 2)) / divisor;
    assert!(quotient <= (u16::MAX as u32));
    quotient as u16
}


#[entry]
fn main() -> ! {
    let mut peripherals = unsafe { Peripherals::steal() };

    setup_clocks(&mut peripherals);
    setup_pins(&mut peripherals);

    // set up peripherals:
    // * I2C2 (buttons & LEDs, light sensor)
    // * SPI1 (flash, 7seg)
    // * USART2 (EnOcean)
    // * USART3 (debugging)

    // not much to set here, hehe
    I2c2::set_up_as_controller(&peripherals);

    // notes on polarity:
    // * 7seg: shift in on rising edge, shift out on falling edge (SPI mode 0)
    // * flash: SPI mode 0 or 3 (sampled when chip select is pulled low)
    //
    // notes on speed:
    // * speed divisor must be at least 2 => we go from 25 MHz to 12.5 MHz
    // * 7seg: max 30 MHz with standalone operation, max 15 MHz in cascade => OK
    // * flash: slowest command is 0x03 at 40 MHz => OK
    //
    // notes on bit order:
    // * 7seg: bits get shifted in from LSB side, fall out of MSB side => MSB first
    // * flash: MSB first
    Spi1::set_up_as_controller(
        &peripherals,
        BR::Div256,
        SpiMode::WriteFallingOrCsReadRising,
        false,
    );

    // speed is always 57_600 b/s
    Usart2::set_up(
        &peripherals,
        divide_u32_to_u16_round(CLOCK_SPEED_HZ, 57_600),
    );

    // use the venerable 9600 b/s
    Usart3::set_up(
        &peripherals,
        divide_u32_to_u16_round(CLOCK_SPEED_HZ, 9_600),
    );

    // LED blinky
    peripherals.RCC.ahb1enr().modify(|_, w| w
        .gpioaen().enabled() // clock to GPIOA
    );
    peripherals.GPIOA.moder().modify(|_, w| w
        .moder8().output()
    );
    peripherals.GPIOA.otyper().modify(|_, w| w
        .ot8().push_pull()
    );
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr8().high()
    );

    // turn on 7-seg displays
    peripherals.GPIOC.odr().modify(|_, w| w
        .odr6().low()
    );

    // 0x00 is actually the broadcast address, but AMS was kinda stupid
    const ADDR_8800: I2cAddress = I2cAddress::new(0x00).unwrap();
    const REG_8800_SHUTDOWN: u8 = 0x0C;
    const VALUE_8800_SHUTDOWN_NOSHUT_DEFAULTS: u8 = 0x01;
    const REG_8800_SCANLIMIT: u8 = 0x0B;
    const VALUE_8800_SCANLIMIT_ALL_DIGITS: u8 = 0b111;

    I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_SHUTDOWN, VALUE_8800_SHUTDOWN_NOSHUT_DEFAULTS]);
    I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_SCANLIMIT, VALUE_8800_SCANLIMIT_ALL_DIGITS]);

    peripherals.GPIOA.odr().modify(|_, w| w
        .odr8().low()
    );

    // pull PC15 low to reset EnOcean module
    peripherals.GPIOC.odr().modify(|_, w| w
        .odr15().low()
    );

    // wait a bit
    for _ in 0..4*1024*1024 {
        cortex_m::asm::nop();
    }

    // pull PC15 high to unreset EnOcean module
    peripherals.GPIOC.odr().modify(|_, w| w
        .odr15().high()
    );

    let mut top_display = TempDisplayState::new(true);
    let mut bottom_display = TempDisplayState::new(false);

    update_displays(&peripherals, &top_display, &bottom_display);

    loop {
        let packet_result = crate::enocean::process_one_packet(&peripherals);
        let display_updated = act_upon_one_packet(
            packet_result,
            &mut top_display,
            &mut bottom_display,
        );
        if display_updated {
            update_displays(&peripherals, &top_display, &bottom_display);
        }
    }
}

fn act_upon_one_packet(
    packet_result: crate::enocean::PacketResult,
    top_display: &mut TempDisplayState,
    bottom_display: &mut TempDisplayState,
) -> bool {
    let mut display_updated = false;

    // needs to be an EnOcean packet
    let (packet_type, payload) = match packet_result {
        enocean::PacketResult::Packet { packet_type, payload }
            => (packet_type, payload),
        _ => return display_updated,
    };

    // needs to be an ERP1 packet
    if packet_type != crate::enocean::PacketType::RadioErp1 {
        return display_updated;
    }

    // must have at least 1 byte for packet type
    let payload_data = payload.data();
    if payload_data.len() < 1 {
        return display_updated;
    }

    match payload_data[0] {
        0xF6|0xD5 => {
            // one type identifier, one byte of data, four of sender, one of status
            if payload_data.len() != 7 {
                return display_updated;
            }

            // not really interesting to us
            return display_updated;
        },
        0xA5 => {
            // one type identifier, four bytes of data, four of sender, one of status
            if payload_data.len() != 10 {
                return display_updated;
            }

            let data = u32::from_be_bytes(payload_data[1..5].try_into().unwrap());
            let sender = u32::from_be_bytes(payload_data[5..9].try_into().unwrap());

            if sender == 0x00_00_00_01 {
                // inside temperature according to A5-09-04
                // HHHH_HHHH CCCC_CCCC TTTT_TTTT 0000_Lxx0

                if data & 0b1000 == 0 {
                    // this is a teach-in packet, ignore it
                    return display_updated;
                }

                // 8 bits of temperature in units of 0.2 °C
                let temperature_bits = ((data >> 8) & 0xFF) as u16;
                let temperature_tenth_celsius = temperature_bits * 2;

                let temperature_digit_0 = if temperature_tenth_celsius >= 100 {
                    b'0' + u8::try_from(temperature_tenth_celsius / 100).unwrap()
                } else {
                    b' '
                };
                // digit 1 is before the decimal point so always there even if it's zero
                let temperature_digit_1 = b'0' + u8::try_from((temperature_tenth_celsius / 10) % 10).unwrap();
                let temperature_digit_2 = b'0' + u8::try_from(temperature_tenth_celsius % 10).unwrap();

                bottom_display.set_digit(0, temperature_digit_0, false);
                bottom_display.set_digit(1, temperature_digit_1, true);
                bottom_display.set_digit(2, temperature_digit_2, false);
                display_updated = true;
            } else if sender == 0x00_00_00_02 {
                // outside temperature according to A5-04-03
                // HHHH_HHHH 0000_00TT TTTT_TTTT 0000_L00x

                if data & 0b1000 == 0 {
                    // this is a teach-in packet, ignore it
                    return display_updated;
                }

                // 10 bits of temperature from -20 to +60 °C
                // let's aim for a single decimal digit
                let temperature_bits = (data >> 8) & 0x3FF;
                let temperature_tenth_celsius = ((temperature_bits * 800) / 1024) as i32 - 200;

                if temperature_tenth_celsius <= -10 {
                    // -TT
                    let abs_temp = (-temperature_tenth_celsius) / 10;
                    let temperature_digit_0 = b'-';
                    let temperature_digit_1 = b'0' + u8::try_from(abs_temp / 10).unwrap();
                    let temperature_digit_2 = b'0' + u8::try_from(abs_temp % 10).unwrap();
                    top_display.set_digit(0, temperature_digit_0, false);
                    top_display.set_digit(1, temperature_digit_1, false);
                    top_display.set_digit(2, temperature_digit_2, false);
                } else if temperature_tenth_celsius < 0 {
                    // -T.T
                    let abs_temp = -temperature_tenth_celsius;
                    let temperature_digit_0 = b'-';
                    let temperature_digit_1 = b'0' + u8::try_from(abs_temp / 10).unwrap();
                    let temperature_digit_2 = b'0' + u8::try_from(abs_temp % 10).unwrap();
                    top_display.set_digit(0, temperature_digit_0, false);
                    top_display.set_digit(1, temperature_digit_1, true);
                    top_display.set_digit(2, temperature_digit_2, false);
                } else if temperature_tenth_celsius < 100 {
                    let temperature_digit_0 = b' ';
                    let temperature_digit_1 = b'0' + u8::try_from(temperature_tenth_celsius / 10).unwrap();
                    let temperature_digit_2 = b'0' + u8::try_from(temperature_tenth_celsius % 10).unwrap();
                    top_display.set_digit(0, temperature_digit_0, false);
                    top_display.set_digit(1, temperature_digit_1, true);
                    top_display.set_digit(2, temperature_digit_2, false);
                } else {
                    let temperature_digit_0 = b'0' + u8::try_from(temperature_tenth_celsius / 100).unwrap();
                    let temperature_digit_1 = b'0' + u8::try_from((temperature_tenth_celsius / 10) % 10).unwrap();
                    let temperature_digit_2 = b'0' + u8::try_from(temperature_tenth_celsius % 10).unwrap();
                    top_display.set_digit(0, temperature_digit_0, false);
                    top_display.set_digit(1, temperature_digit_1, true);
                    top_display.set_digit(2, temperature_digit_2, false);
                }
                display_updated = true;
            }
        },
        _ => {
            // some other type of radio packet, we don't care
        },
    }

    display_updated
}

fn update_displays(
    peripherals: &Peripherals,
    top_display: &TempDisplayState,
    bottom_display: &TempDisplayState,
) {
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr13().low() // CS1 low
    );
    top_display.send_via_spi(&peripherals);
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr13().high() // CS1 high
    );

    peripherals.GPIOB.odr().modify(|_, w| w
        .odr0().low() // CS2 low
    );
    bottom_display.send_via_spi(&peripherals);
    peripherals.GPIOB.odr().modify(|_, w| w
        .odr0().high() // CS2 high
    );
}
