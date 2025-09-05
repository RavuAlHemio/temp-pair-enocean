#![no_main]
#![no_std]


mod blinky_led;
mod crc8;
mod enocean;
mod flash;
mod i2c;
mod hmi_display;
mod spi;
mod temp_display;
mod uart;


use core::panic::PanicInfo;

use cortex_m_rt::entry;
use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::spi1::cr1::BR;

use crate::blinky_led::{BlinkyLed, BlinkyLedA8};
use crate::i2c::{I2c, I2c2, I2cAddress};
use crate::spi::{Spi, Spi1, SpiMode};
use crate::temp_display::{Brightness, TempDisplayState};
use crate::uart::{Uart, Usart2, Usart3};


pub const CLOCK_SPEED_HZ: u32 = 25_000_000;

const ADDR_I2C_SPI: I2cAddress = I2cAddress::new(0b0101000).unwrap();
const ADDR_I2C_EXP: I2cAddress = I2cAddress::new(0b1110000).unwrap();


#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    let peripherals = unsafe { Peripherals::steal() };
    loop {
        BlinkyLedA8::turn_on(&peripherals);
        for _ in 0..1024*1024 {
            cortex_m::asm::nop();
        }
        BlinkyLedA8::turn_off(&peripherals);
        for _ in 0..1024*1024 {
            cortex_m::asm::nop();
        }
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
/// The 7-segment driver chip (TLC5947) is controlled via an I2C-to-SPI bridge, but just in case, it
/// allows up to 30 MHz (15 MHz in 50%-duty cascade operation). The flash chip (AT25FF321A)
/// absolutely bottoms out at 30 MHz for the slow-read category, so 25 MHz is fine.
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
        .ot10().open_drain()
        .ot11().open_drain()
    );
    peripherals.GPIOC.otyper().modify(|_, w| w
        .ot15().push_pull()
    );
    peripherals.GPIOD.otyper().modify(|_, w| w
        .ot8().push_pull()
        .ot9().push_pull()
        .ot11().push_pull()
        .ot12().push_pull()
    );
    peripherals.GPIOE.otyper().modify(|_, w| w
        .ot7().push_pull()
        .ot8().push_pull()
    );

    // set pulling on input ports and SPI SCK
    peripherals.GPIOA.pupdr().modify(|_, w| w
        .pupdr5().pull_down() // idle SPI1 SCK polarity: low
    );
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
        .moder10().alternate() // I2C2
        .moder11().alternate() // I2C2
        .moder14().input() // HMI button push interrupt
    );
    peripherals.GPIOC.moder().modify(|_, w| w
        .moder15().output() // reset EnOcean module
    );
    peripherals.GPIOD.moder().modify(|_, w| w
        .moder8().alternate() // USART3
        .moder9().alternate() // USART3
        .moder11().output() // I2C-SPI bridge reset
        .moder12().output() // flash write protection
    );
    peripherals.GPIOE.moder().modify(|_, w| w
        .moder7().output() // reset flash chip
        .moder8().output() // flash chip select for SPI1
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


#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum AppState {
    #[default] Idle,
    NewSetup(usize),
}
impl AppState {
    pub fn incremented(&self) -> Self {
        match self {
            Self::Idle => Self::NewSetup(1),
            Self::NewSetup(i) => if *i < 27 {
                Self::NewSetup(*i + 1)
            } else {
                Self::Idle
            },
        }
    }
}


#[entry]
fn main() -> ! {
    let mut peripherals = unsafe { Peripherals::steal() };

    setup_clocks(&mut peripherals);
    setup_pins(&mut peripherals);

    // set up peripherals:
    // * I2C2 (buttons & LEDs, light sensor, 7seg via I2C-SPI bridge)
    // * SPI1 (flash)
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

    // EnOcean speed is always 57_600 b/s
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
    BlinkyLedA8::set_up(&peripherals);
    BlinkyLedA8::turn_on(&peripherals);

    // reset the I2C-SPI bridge for the 7-seg displays
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr11().low()
    );
    for _ in 0..1024 {
        cortex_m::asm::nop();
    }
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr11().high()
    );
    for _ in 0..1024 {
        cortex_m::asm::nop();
    }

    // configure the I2C-SPI bridge
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_SPI,
        &[
            0xF0, // configure SPI interface
            (
                (0b00 << 6) // reserved bits
                | (0b0 << 5) // MSB first
                | (0b0 << 4) // reserved bit
                | (0b00 << 2) // SPI mode 0
                | (0b00 << 0) // 1875 kHz
            ),
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_SPI,
        &[
            0xF6, // GPIO enable
            (
                (0b00000 << 3) // reserved bits
                | (0b1 << 2) // CS2 is GPIO
                | (0b1 << 1) // CS1 is GPIO
                | (0b1 << 0) // CS0 is GPIO
            ),
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_SPI,
        &[
            0xF7, // GPIO mode
            (
                (0b00 << 6) // reserved bits
                | (0b00 << 4) // CS2 is a floating input
                | (0b00 << 2) // CS1 is a floating input
                | (0b01 << 0) // CS0 is a push-pull output ("latch" command to chip 1)
            ),
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_SPI,
        &[
            0xF4, // GPIO write
            (
                (0b00000 << 3) // reserved bits
                | (0b0 << 2) // CS2 is an input anyway
                | (0b0 << 1) // CS1 is an input anyway
                | (0b0 << 0) // set CS0 low (no latching on chip 1)
            ),
        ],
    );

    // configure the I2C port expander
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_EXP,
        &[
            0x02, // polarity inversion
            0b0000_0000, // invert polarity of no ports
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_EXP,
        &[
            0x03, // I/O direction
            (
                (0b1111 << 4) // IO4 through IO7 are unused, set them as inputs
                | (0b1 << 3) // IO3 is an input (~{INT}, unused)
                | (0b0 << 2) // IO2 is an output (PWM, "blank" to both chips)
                | (0b0 << 1) // IO1 is an output (AN, "latch" command to chip 2)
                | (0b0 << 0) // IO0 is an output (~{RST}, used for ClickID)
            ),
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_EXP,
        &[
            0x01, // GPIO output
            (
                (0b00000 << 3) // IO3 through IO7 are inputs
                | (0b0 << 2) // request that displays not be shut off
                | (0b0 << 1) // no latch to chip 2
                | (0b1 << 0) // ~{RST} up so that ClickID does not interfere
            ),
        ],
    );
    I2c2::write_data(
        &peripherals,
        ADDR_I2C_EXP,
        &[
            0x4F, // output behavior configuration
            0, // push-pull
        ],
    );

    // 0x00 is actually the broadcast address, but AMS was kinda stupid
    const ADDR_8800: I2cAddress = I2cAddress::new(0x00).unwrap();
    const REG_8800_SHUTDOWN: u8 = 0x0C;
    const VALUE_8800_SHUTDOWN_NOSHUT_DEFAULTS: u8 = 0x01;
    const REG_8800_SCANLIMIT: u8 = 0x0B;
    const VALUE_8800_SCANLIMIT_ALL_DIGITS: u8 = 0b111;
    const REG_8800_KEYA: u8 = 0x1C;
    const REG_8800_LED_ROW_0: u8 = 0x01;

    I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_SHUTDOWN, VALUE_8800_SHUTDOWN_NOSHUT_DEFAULTS]);
    I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_SCANLIMIT, VALUE_8800_SCANLIMIT_ALL_DIGITS]);
    I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_LED_ROW_0, 0, 0, 0, 0, 0, 0, 0, 0]);

    // do a JEDEC reset on flash
    crate::flash::jedec_reset(&peripherals);
    // wait a bit
    for _ in 0..10_000 {
        cortex_m::asm::nop();
    }

    // pull ~{HOLD}/~{RESET} high (because it's probably configured as HOLD)
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr7().high()
    );
    // sleep a bit to ensure flash chip gets the hint
    for _ in 0..1024 {
        cortex_m::asm::nop();
    }

    // pull ~{write-prot} high
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr12().high()
    );
    // sleep a bit to ensure flash chip gets the hint
    for _ in 0..1024 {
        cortex_m::asm::nop();
    }

    /*
    // nuke the flash chip
    do_with_flash_chip_selected(&peripherals, |p| {
        let mut asplode1 = [0x66];
        Spi1::communicate_bytes(p, &mut asplode1)
    });
    for _ in 0..4 {
        cortex_m::asm::nop();
    }
    do_with_flash_chip_selected(&peripherals, |p| {
        let mut asplode2 = [0x99];
        Spi1::communicate_bytes(p, &mut asplode2)
    });
    // t_SWRST = 200µs, 200µs * 25MHz = 5000
    for _ in 0..5000 {
        cortex_m::asm::nop();
    }
    */

    // enable writing
    do_with_flash_chip_selected(&peripherals, |p|
        crate::flash::enable_writing(p)
    );
    // sleep a bit to ensure flash chip gets the hint
    for _ in 0..1024 {
        cortex_m::asm::nop();
    }
    // read status registers
    let status_register = do_with_flash_chip_selected(&peripherals, |p|
        crate::flash::read_all_status_registers(p)
    );
    I2c2::write_data(
        &peripherals, ADDR_8800,
        &[
            REG_8800_LED_ROW_0,
            status_register[0], status_register[1],
            status_register[2], status_register[3],
            status_register[4], 0,
            0, 0,
        ],
    );
    // pull ~{write-prot} low
    peripherals.GPIOD.odr().modify(|_, w| w
        .odr12().low()
    );

    // read outside and inside address and packet format from flash
    let mut address_buffer = [
        0, 0, 0, 0, // outside address
        0, 0, 0, // outside packet format
        0, 0, 0, 0, // inside address
        0, 0, 0, // inside packet format
    ];
    do_with_flash_chip_selected(&peripherals, |p|
        crate::flash::read(p, crate::flash::Address::new(0).unwrap(), &mut address_buffer)
    );
    /*
    // visualize what is programmed into Flash
    I2c2::write_data(
        &peripherals, ADDR_8800,
        &[
            REG_8800_LED_ROW_0,
            address_buffer[0], address_buffer[1], address_buffer[2], address_buffer[3],
            address_buffer[4], address_buffer[5], address_buffer[6], address_buffer[7],
        ],
    );
    */

    let mut outside_address =
        u32::from(address_buffer[0]) << 24
        | u32::from(address_buffer[1]) << 16
        | u32::from(address_buffer[2]) <<  8
        | u32::from(address_buffer[3]) <<  0;
    let mut outside_format =
        u32::from(address_buffer[4]) << 16
        | u32::from(address_buffer[5]) <<  8
        | u32::from(address_buffer[6]) <<  0;
    let mut inside_address =
        u32::from(address_buffer[7]) << 24
        | u32::from(address_buffer[8]) << 16
        | u32::from(address_buffer[9]) <<  8
        | u32::from(address_buffer[10]) <<  0;
    let mut inside_format =
        u32::from(address_buffer[11]) << 16
        | u32::from(address_buffer[12]) <<  8
        | u32::from(address_buffer[13]) <<  0;

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

    // set the brightness to full by default
    let fullbright = Brightness::new(0x0FFF).unwrap();
    top_display.set_brightness(fullbright);
    bottom_display.set_brightness(fullbright);

    update_displays(&peripherals, &mut top_display, &mut bottom_display, true);

    BlinkyLedA8::turn_off(&peripherals);

    let mut app_state = AppState::Idle;
    let mut new_setup_nibbles: [u8; 28] = [0; 28];
    loop {
        // EnOcean logic
        let packet_result = crate::enocean::process_one_packet(&peripherals);
        act_upon_one_packet(
            packet_result,
            outside_address, outside_format,
            inside_address, inside_format,
            &mut top_display,
            &mut bottom_display,
        );

        // HMI logic
        if peripherals.GPIOB.idr().read().idr14().is_low() {
            // 8800 wants us to read the buttons
            I2c2::write_data(&peripherals, ADDR_8800, &[REG_8800_KEYA]);
            let mut key_values = [0u8; 2];
            I2c2::read_data(&peripherals, ADDR_8800, &mut key_values);

            // by default: 0 pressed, 1 not pressed
            let negated_all_key_values =
                u16::from(key_values[0]) << 8
                | u16::from(key_values[1]);
            let all_key_values = !negated_all_key_values;

            // popcount
            let pop_count = all_key_values.count_ones();
            if pop_count == 0 {
                // nothing pressed; do nothing
            } else if pop_count > 1 {
                // multiple buttons pressed; go back to idle
                app_state = AppState::Idle;
            } else {
                // a single button; now we have to make hard decisions
                let value: u8 = match all_key_values {
                    0x0001 => 0,
                    0x0002 => 1,
                    0x0004 => 2,
                    0x0008 => 3,
                    0x0010 => 4,
                    0x0020 => 5,
                    0x0040 => 6,
                    0x0080 => 7,
                    0x0100 => 8,
                    0x0200 => 9,
                    0x0400 => 10,
                    0x0800 => 11,
                    0x1000 => 12,
                    0x2000 => 13,
                    0x4000 => 14,
                    0x8000 => 15,
                    _ => unreachable!(), // popcount would not be 1
                };

                match app_state {
                    AppState::Idle => {
                        // the first byte of a new setup
                        new_setup_nibbles.fill(0);
                        new_setup_nibbles[0] = value;
                    },
                    AppState::NewSetup(i) => {
                        // a subsequent byte
                        new_setup_nibbles[i] = value;
                    },
                }

                app_state = app_state.incremented();

                match app_state {
                    AppState::Idle => {
                        // and now the magic happens

                        // move the nibbles into the correct variables
                        outside_address =
                            u32::from(new_setup_nibbles[ 0]) << 28
                            | u32::from(new_setup_nibbles[ 1]) << 24
                            | u32::from(new_setup_nibbles[ 2]) << 20
                            | u32::from(new_setup_nibbles[ 3]) << 16
                            | u32::from(new_setup_nibbles[ 4]) << 12
                            | u32::from(new_setup_nibbles[ 5]) <<  8
                            | u32::from(new_setup_nibbles[ 6]) <<  4
                            | u32::from(new_setup_nibbles[ 7]) <<  0;
                        outside_format =
                            u32::from(new_setup_nibbles[ 8]) << 20
                            | u32::from(new_setup_nibbles[ 9]) << 16
                            | u32::from(new_setup_nibbles[10]) << 12
                            | u32::from(new_setup_nibbles[11]) <<  8
                            | u32::from(new_setup_nibbles[12]) <<  4
                            | u32::from(new_setup_nibbles[13]) <<  0;
                        inside_address =
                            u32::from(new_setup_nibbles[14]) << 28
                            | u32::from(new_setup_nibbles[15]) << 24
                            | u32::from(new_setup_nibbles[16]) << 20
                            | u32::from(new_setup_nibbles[17]) << 16
                            | u32::from(new_setup_nibbles[18]) << 12
                            | u32::from(new_setup_nibbles[19]) <<  8
                            | u32::from(new_setup_nibbles[20]) <<  4
                            | u32::from(new_setup_nibbles[21]) <<  0;
                        inside_format =
                            u32::from(new_setup_nibbles[22]) << 20
                            | u32::from(new_setup_nibbles[23]) << 16
                            | u32::from(new_setup_nibbles[24]) << 12
                            | u32::from(new_setup_nibbles[25]) <<  8
                            | u32::from(new_setup_nibbles[26]) <<  4
                            | u32::from(new_setup_nibbles[27]) <<  0;

                        // erase the first block of flash
                        // pull ~{write-prot} high
                        peripherals.GPIOD.odr().modify(|_, w| w
                            .odr12().high()
                        );

                        // enable writing
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::enable_writing(p)
                        );
                        // start erasing first 4k
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::start_erase_4_kibibytes(p, crate::flash::Address::new(0).unwrap())
                        );
                        // wait until erasing is done
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::wait_while_busy(p)
                        );
                        // enable writing again
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::enable_writing(p)
                        );
                        // prepare writing buffer
                        let writing_buffer = [
                            ((outside_address >> 24) & 0xFF) as u8,
                            ((outside_address >> 16) & 0xFF) as u8,
                            ((outside_address >>  8) & 0xFF) as u8,
                            ((outside_address >>  0) & 0xFF) as u8,
                            ((outside_format >> 16) & 0xFF) as u8,
                            ((outside_format >>  8) & 0xFF) as u8,
                            ((outside_format >>  0) & 0xFF) as u8,
                            ((inside_address >> 24) & 0xFF) as u8,
                            ((inside_address >> 16) & 0xFF) as u8,
                            ((inside_address >>  8) & 0xFF) as u8,
                            ((inside_address >>  0) & 0xFF) as u8,
                            ((inside_format >> 16) & 0xFF) as u8,
                            ((inside_format >>  8) & 0xFF) as u8,
                            ((inside_format >>  0) & 0xFF) as u8,
                        ];
                        // write at location
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::write(p, crate::flash::Address::new(0).unwrap(), &writing_buffer)
                        );
                        // wait until writing is done
                        do_with_flash_chip_selected(&peripherals, |p|
                            crate::flash::wait_while_busy(p)
                        );

                        // pull ~{write-prot} low
                        peripherals.GPIOD.odr().modify(|_, w| w
                            .odr12().low()
                        );

                        // now the variables are updated and the state is persisted

                        // turn off the displays
                        top_display.set_digit(0, b' ', false);
                        top_display.set_digit(1, b' ', false);
                        top_display.set_digit(2, b' ', false);
                        bottom_display.set_digit(0, b' ', false);
                        bottom_display.set_digit(1, b' ', false);
                        bottom_display.set_digit(2, b' ', false);

                        // we can go back to regular temperature processing
                    },
                    AppState::NewSetup(next_nibble_index) => {
                        if next_nibble_index <= 8 {
                            // outside address
                            show_nibbles_starting_at(&new_setup_nibbles, 0, next_nibble_index, &mut top_display, &mut bottom_display);
                        } else if next_nibble_index <= 14 {
                            // outside format
                            show_nibbles_starting_at(&new_setup_nibbles, 8, next_nibble_index, &mut top_display, &mut bottom_display);
                        } else if next_nibble_index <= 22 {
                            // inside address
                            show_nibbles_starting_at(&new_setup_nibbles, 14, next_nibble_index, &mut top_display, &mut bottom_display);
                        } else {
                            // inside format
                            show_nibbles_starting_at(&new_setup_nibbles, 22, next_nibble_index, &mut top_display, &mut bottom_display);
                        }
                    },
                }
            }

            // finally, update the displays if something changed
            update_displays(
                &peripherals,
                &mut top_display,
                &mut bottom_display,
                false,
            );
        }
    }
}

fn do_with_flash_chip_selected<T, P: FnMut(&Peripherals) -> T>(
    peripherals: &Peripherals,
    mut procedure: P,
) -> T {
    // pull chip select low
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().low()
    );

    // run the procedure
    let ret = procedure(peripherals);

    // pull chip select high
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );

    cortex_m::asm::nop();

    ret
}

fn show_nibbles_starting_at(
    new_setup_nibbles: &[u8],
    start_nibble_index: usize,
    next_nibble_index: usize,
    top_display: &mut TempDisplayState,
    bottom_display: &mut TempDisplayState,
) {
    let nibble_slice = if next_nibble_index < start_nibble_index {
        // obviously invalid; use an empty slice
        &[]
    } else if next_nibble_index > start_nibble_index + 6 {
        // won't fit; trim from left
        &new_setup_nibbles[next_nibble_index-6..next_nibble_index]
    } else {
        // take as much as we can
        &new_setup_nibbles[start_nibble_index..next_nibble_index]
    };

    top_display.set_nibble_digit(0, if nibble_slice.len() > 0 { nibble_slice[0] } else { 0x10 }, false);
    top_display.set_nibble_digit(1, if nibble_slice.len() > 1 { nibble_slice[1] } else { 0x10 }, false);
    top_display.set_nibble_digit(2, if nibble_slice.len() > 2 { nibble_slice[2] } else { 0x10 }, false);
    bottom_display.set_nibble_digit(0, if nibble_slice.len() > 3 { nibble_slice[3] } else { 0x10 }, false);
    bottom_display.set_nibble_digit(1, if nibble_slice.len() > 4 { nibble_slice[4] } else { 0x10 }, false);
    bottom_display.set_nibble_digit(2, if nibble_slice.len() > 5 { nibble_slice[5] } else { 0x10 }, false);
}

fn act_upon_one_packet(
    packet_result: crate::enocean::PacketResult,
    outside_address: u32,
    outside_format: u32,
    inside_address: u32,
    inside_format: u32,
    top_display: &mut TempDisplayState,
    bottom_display: &mut TempDisplayState,
) {
    // needs to be an EnOcean packet
    let (packet_type, payload) = match packet_result {
        enocean::PacketResult::Packet { packet_type, payload }
            => (packet_type, payload),
        _ => return,
    };

    // needs to be an ERP1 packet
    if packet_type != crate::enocean::PacketType::RadioErp1 {
        return;
    }

    // must have at least 1 byte for packet type
    let payload_data = payload.data();
    if payload_data.len() < 1 {
        return;
    }

    let (data_slice, sender) = match payload_data[0] {
        0xF6|0xD5 => {
            // one type identifier, one byte of data, four of sender, one of status
            if payload_data.len() != 7 {
                return;
            }

            let sender = u32::from_be_bytes(payload_data[2..6].try_into().unwrap());
            (&payload_data[1..2], sender)
        },
        0xA5 => {
            // one type identifier, four bytes of data, four of sender, one of status
            if payload_data.len() != 10 {
                return;
            }

            let sender = u32::from_be_bytes(payload_data[5..9].try_into().unwrap());
            (&payload_data[1..5], sender)
        },
        0xD2 => {
            // one type identifier, variable number of bytes of data, four of sender, one of status
            // (the additional CRC only shows up in the radio protocol;
            // the serial protocol does its own CRCs)
            if payload_data.len() < 6 {
                return;
            }

            let data = &payload_data[1..payload_data.len()-5];
            let sender = u32::from_be_bytes(payload_data[payload_data.len()-5..payload_data.len()-1].try_into().unwrap());
            (data, sender)
        },
        _ => {
            // some other type of radio packet, we don't care
            return;
        },
    };

    if sender == outside_address {
        // is the packet in the correct format?
        // ff-xx-xx
        if !format_matches(outside_format, payload_data[0]) {
            // no, this packet is in a different format
            return;
        }

        // decode the temperature value
        decode_temperature(outside_format, data_slice, top_display);
    } else if sender == inside_address {
        if !format_matches(inside_format, payload_data[0]) {
            // no, this packet is in a different format
            return;
        }

        // decode the temperature value
        decode_temperature(inside_format, data_slice, bottom_display);
    }
}

fn format_matches(
    known_format: u32,
    packet_format: u8,
) -> bool {
    // known_format is ff-xx-xx
    let expected_format = ((known_format >> 16) & 0xFF) as u8;
    expected_format == packet_format
}

fn decode_temperature(
    format: u32,
    data_slice: &[u8],
    display: &mut TempDisplayState,
) {
    if format == 0xA5_09_04 {
        // HHHH_HHHH CCCC_CCCC TTTT_TTTT 0000_Lxx0
        let data = match data_slice.try_into() {
            Ok(ds) => u32::from_be_bytes(ds),
            Err(_) => {
                // wrong format
                return;
            },
        };

        if data & 0b1000 == 0 {
            // this is a teach-in packet, ignore it
            return;
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

        display.set_digit(0, temperature_digit_0, false);
        display.set_digit(1, temperature_digit_1, true);
        display.set_digit(2, temperature_digit_2, false);
    } else if format == 0xA5_04_03 {
        // HHHH_HHHH 0000_00TT TTTT_TTTT 0000_L00x
        let data = match data_slice.try_into() {
            Ok(ds) => u32::from_be_bytes(ds),
            Err(_) => {
                // wrong format
                return;
            },
        };

        if data & 0b1000 == 0 {
            // this is a teach-in packet, ignore it
            return;
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
            display.set_digit(0, temperature_digit_0, false);
            display.set_digit(1, temperature_digit_1, false);
            display.set_digit(2, temperature_digit_2, false);
        } else if temperature_tenth_celsius < 0 {
            // -T.T
            let abs_temp = -temperature_tenth_celsius;
            let temperature_digit_0 = b'-';
            let temperature_digit_1 = b'0' + u8::try_from(abs_temp / 10).unwrap();
            let temperature_digit_2 = b'0' + u8::try_from(abs_temp % 10).unwrap();
            display.set_digit(0, temperature_digit_0, false);
            display.set_digit(1, temperature_digit_1, true);
            display.set_digit(2, temperature_digit_2, false);
        } else if temperature_tenth_celsius < 100 {
            let temperature_digit_0 = b' ';
            let temperature_digit_1 = b'0' + u8::try_from(temperature_tenth_celsius / 10).unwrap();
            let temperature_digit_2 = b'0' + u8::try_from(temperature_tenth_celsius % 10).unwrap();
            display.set_digit(0, temperature_digit_0, false);
            display.set_digit(1, temperature_digit_1, true);
            display.set_digit(2, temperature_digit_2, false);
        } else {
            let temperature_digit_0 = b'0' + u8::try_from(temperature_tenth_celsius / 100).unwrap();
            let temperature_digit_1 = b'0' + u8::try_from((temperature_tenth_celsius / 10) % 10).unwrap();
            let temperature_digit_2 = b'0' + u8::try_from(temperature_tenth_celsius % 10).unwrap();
            display.set_digit(0, temperature_digit_0, false);
            display.set_digit(1, temperature_digit_1, true);
            display.set_digit(2, temperature_digit_2, false);
        }
    } else {
        // don't know how to decode this format
    }
}

fn update_displays(
    peripherals: &Peripherals,
    top_display: &mut TempDisplayState,
    bottom_display: &mut TempDisplayState,
    force: bool,
) {
    // all CS pins on the I2C-SPI bridge are set to GPIO
    // but according to the datasheet we can't pass 0, so pass 1
    const CHIP_SELECT_PATTERN: u8 = 0b001;

    if force || top_display.is_dirty() {
        // send top display data via I2C/SPI
        top_display.send_via_i2c_spi_bridge::<I2c2>(
            &peripherals,
            ADDR_I2C_SPI,
            CHIP_SELECT_PATTERN,
            true,
        );
        // pull the chip 1 XLAT pin up, wait a bit, then pull it down again
        I2c2::write_data(
            &peripherals,
            ADDR_I2C_SPI,
            &[
                0xF4, // GPIO output
                (
                    (0b00000 << 3) // reserved pins
                    | (0b0 << 2) // CS2 is an input anyway
                    | (0b0 << 1) // CS1 is an input anyway
                    | (0b1 << 0) // pull CS0 (chip 1 XLAT) up
                ),
            ],
        );
        for _ in 0..1024 {
            cortex_m::asm::nop();
        }
        I2c2::write_data(
            &peripherals,
            ADDR_I2C_SPI,
            &[
                0xF4,
                (
                    (0b00000 << 3)
                    | (0b0 << 2)
                    | (0b0 << 1)
                    | (0b0 << 0) // down this time
                ),
            ],
        );
    }

    if force || bottom_display.is_dirty() {
        // same for the bottom display
        bottom_display.send_via_i2c_spi_bridge::<I2c2>(
            &peripherals,
            ADDR_I2C_SPI,
            CHIP_SELECT_PATTERN,
            true,
        );
        I2c2::write_data(
            &peripherals,
            ADDR_I2C_EXP,
            &[
                0x01, // GPIO output
                (
                    (0b0000 << 4) // IO4-IO7 unused and configured as inputs
                    | (0b0 << 3) // IO3 is an input
                    | (0b0 << 2) // IO2 is "blank" and should be off
                    | (0b1 << 1) // IO1 is "latch" for chip 2, this is the important one
                    | (0b1 << 0) // IO0 is ~{ClickID} so keep it high
                ),
            ],
        );
        for _ in 0..1024 {
            cortex_m::asm::nop();
        }
        I2c2::write_data(
            &peripherals,
            ADDR_I2C_EXP,
            &[
                0x01,
                (
                    (0b0000 << 4)
                    | (0b0 << 3)
                    | (0b0 << 2)
                    | (0b0 << 1) // and down again
                    | (0b1 << 0)
                ),
            ],
        );
    }
}
