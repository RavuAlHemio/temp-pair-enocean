#![no_main]
#![no_std]


mod crc8;
mod enocean;
mod i2c;
mod spi;
mod uart;


use core::panic::PanicInfo;

use cortex_m_rt::entry;
use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::spi1::cr1::BR;

use crate::i2c::{I2c, I2c2, I2cAddress};
use crate::spi::{Spi, Spi1, SpiMode};
use crate::uart::{Uart, Usart2, Usart3};


pub const CLOCK_SPEED_HZ: u32 = 16_000_000;


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
///                     └───────┘ ┌┴───────┐
///                               │ SPI1   │
///                               │ 16 MHz │
///                               └────────┘
/// ```
///
/// The EnOncean module (on USART2) requires 57600 b/s, so we must solve:
///
/// 57 600 b/s = 16 000 000 b Hz / USARTDIV
///
/// which gives 277.7...; we can fit 278 (0x116) in a 16-bit register.
///
/// The emergency USART we can configure for any speed; even a USARTDIV of 1666 (0x682) for the
/// venerable 9600 b/s fits.
///
/// For I2C, honestly, just steal Table 187 from the reference manual:
///
/// PRESC = 0x3, SCLL = 0xC7, SCLH = 0xC3, SDADEL = 0x2, SCLDEL = 0x4
///
/// For SPI1, we get power-of-two prescalers from /2 to /256. The 7-segment driver chip (TLC5947)
/// allows up to 30 MHz (15 MHz in 50%-duty cascade operation) and the flash chip (AT25FF321A)
/// absolutely bottoms out at 30 MHz for the slow-read category, so 16 MHz is totally fine.
///
/// The board has an external oscillator, though, so let's use that:
///
/// ```plain
/// ╭────────╮ ╒══════╕
/// │ HSE    ├─┤ HPRE ├───┬─────────┬─────────╴╴╴──┐ AHB (max. 216 MHz)
/// │ 16 MHz │ │   /1 ├┐  │         │              │
/// ╰────────╯ └──────┘│ ┌┴───────┐┌┴───────┐     ┌┴───────┐
///                    │ │ SYSCLK ││ GPIOA  │ ... │ GPIOE  │
///                    │ │ 16 MHz ││ 16 MHz │     │ 16 MHz │
///                    │ └────────┘└────────┘     └────────┘
///                    │╒═══════╕ 
///                    ├┤ PPRE1 ├──┬─────────┬─────────┐ APB1 (max. 54 MHz)
///                    ││    /1 │  │         │         │
///                    │└───────┘ ┌┴───────┐┌┴───────┐┌┴───────┐
///                    │          │ USART2 ││ USART3 ││ I2C2   │
///                    │          │ 16 MHz ││ 16 MHz ││ 16 MHz │
///                    │          └────────┘└────────┘└────────┘
///                    │╒═══════╕
///                    └┤ PPRE2 ├──┐ APB2 (max. 108 MHz)
///                     │    /1 │  │
///                     └───────┘ ┌┴───────┐
///                               │ SPI1   │
///                               │ 16 MHz │
///                               └────────┘
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
    // 0 MHz < 16 MHz < 30 MHz => 0 wait states
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

    // set UART2 and I2C ports to fast
    peripherals.GPIOA.ospeedr().modify(|_, w| w
        .ospeedr2().high_speed()
        .ospeedr3().high_speed()
    );
    peripherals.GPIOB.ospeedr().modify(|_, w| w
        .ospeedr10().high_speed()
        .ospeedr11().high_speed()
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
    // * speed divisor must be at least 2 => we go from 16 MHz to 8 MHz
    // * 7seg: max 30 MHz with standalone operation, max 15 MHz in cascade => OK
    // * flash: slowest command is 0x03 at 40 MHz => OK
    //
    // notes on bit order:
    // * 7seg: bits get shifted in from LSB side, fall out of MSB side => MSB first
    // * flash: MSB first
    Spi1::set_up_as_controller(
        &peripherals,
        BR::Div2,
        SpiMode::WriteFallingOrCsReadRising,
        false,
    );

    // speed is always 57_600 b/s
    Usart2::set_up(
        &peripherals,
        divide_u32_to_u16_round(16_000_000, 57_600),
    );

    // use the venerable 9600 b/s
    Usart3::set_up(
        &peripherals,
        divide_u32_to_u16_round(16_000_000, 9_600),
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

    // 0x00 is actually the broadcast address, but AMS was kinda stupid
    const ADDR_8800: I2cAddress = I2cAddress::new(0x00).unwrap();
    const REG_8800_DIGIT0: u8 = 0x01;
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

    loop {
        crate::enocean::process_one_packet(&peripherals);
    }
}
