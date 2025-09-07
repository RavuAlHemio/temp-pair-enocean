//! Code for interfacing with the Renesas AT25FF321A flash chip via SPI.


use stm32f7::stm32f745::Peripherals;

use crate::spi::{Spi, Spi1};


type FlashSpi = Spi1;


const CMD_WRITE_ENABLE: u8 = 0x06;
const CMD_READ_STATUS_REGISTER_1: u8 = 0x05;
const CMD_READ_STATUS_REGISTER_2: u8 = 0x35;
const CMD_READ_STATUS_REGISTER_3: u8 = 0x15;
const CMD_READ_STATUS_REGISTERS: u8 = 0x65;
const CMD_ERASE_4K: u8 = 0x20;
const CMD_ERASE_32K: u8 = 0x52;
const CMD_ERASE_64K: u8 = 0xD8;
const CMD_PROGRAM: u8 = 0x02;
const CMD_READ_PIPELINED: u8 = 0x0B;


/// A 24-bit flash memory address.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Address {
    inner: u32,
}
impl Address {
    pub const fn new(value: u32) -> Option<Self> {
        if value <= 0x00FF_FFFF {
            Some(Self { inner: value })
        } else {
            None
        }
    }

    pub const fn as_u32(&self) -> u32 { self.inner }
}
impl TryFrom<u32> for Address {
    type Error = ();
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(())
    }
}
impl From<Address> for u32 {
    fn from(value: Address) -> Self { value.as_u32() }
}


macro_rules! impl_block_erase {
    ($name:ident, $opcode:expr) => {
        /// Erases the given block of bytes.
        ///
        /// The correct order of operations is:
        ///
        /// 1. Pull ~{WP} high.
        /// 2. Pull ~{CS} low, call [`enable_writing`], pull ~{CS} high.
        /// 3. Pull ~{CS} low, call one of the `start_erase_*` functions, pull ~{CS} high.
        /// 4. Pull ~{CS} low, call [`wait_while_busy`], pull ~{CS} high.
        /// 5. Pull ~{WP} low.
        pub fn $name(peripherals: &Peripherals, block_addr: Address) {
            let mut command_list: [u8; 4] = [
                $opcode,
                ((block_addr.as_u32() >> 16) & 0xFF) as u8,
                ((block_addr.as_u32() >>  8) & 0xFF) as u8,
                ((block_addr.as_u32() >>  0) & 0xFF) as u8,
            ];
            FlashSpi::communicate_bytes(peripherals, &mut command_list);
        }
    }
}
impl_block_erase!(start_erase_4_kibibytes, CMD_ERASE_4K);
impl_block_erase!(start_erase_32_kibibytes, CMD_ERASE_32K);
impl_block_erase!(start_erase_64_kibibytes, CMD_ERASE_64K);


pub fn enable_writing(peripherals: &Peripherals) {
    let mut buf = [CMD_WRITE_ENABLE];
    FlashSpi::communicate_bytes(peripherals, &mut buf);
}


pub fn wait_while_busy(peripherals: &Peripherals) {
    // send "read status register 1" to the flash chip
    let mut buf = [CMD_READ_STATUS_REGISTER_1];
    FlashSpi::communicate_bytes(peripherals, &mut buf);

    loop {
        // as long as we keep ~{CS} asserted, the flash chip will keep sending us
        // the (current) value of SR1

        buf[0] = 0x00;
        FlashSpi::communicate_bytes(peripherals, &mut buf);

        // read ~{RDY}/BSY bit
        if buf[0] & (1 << 0) == 0 {
            // device is ready
            break;
        }
    }
}

pub fn is_busy(peripherals: &Peripherals) -> bool {
    // send "read status register 1" to the flash chip
    let mut buf = [CMD_READ_STATUS_REGISTER_1, 0x00];
    FlashSpi::communicate_bytes(peripherals, &mut buf);
    (buf[1] & (1 << 0)) != 0
}

/// Writes the given block of bytes. The bytes must have previously been erased.
///
/// The correct order of operations is:
///
/// 1. Pull ~{WP} high.
/// 2. Pull ~{CS} low, call [`enable_writing`], pull ~{CS} high.
/// 3. Pull ~{CS} low, call [`write`], pull ~{CS} high.
/// 4. Pull ~{CS} low, call [`wait_while_busy`], pull ~{CS} high.
/// 5. Pull ~{WP} low.
pub fn write(peripherals: &Peripherals, addr: Address, values: &[u8]) {
    let mut command_start: [u8; 4] = [
        CMD_PROGRAM,
        ((addr.as_u32() >> 16) & 0xFF) as u8,
        ((addr.as_u32() >>  8) & 0xFF) as u8,
        ((addr.as_u32() >>  0) & 0xFF) as u8,
    ];
    FlashSpi::communicate_bytes(peripherals, &mut command_start);
    for b in values {
        let mut buf = [*b];
        FlashSpi::communicate_bytes(peripherals, &mut buf);
    }
}

/// Reads the given block of bytes. The bytes must have previously been erased.
///
/// You must pull ~{CS} low before calling `read`, then pull it high again when `read` completes.
pub fn read(peripherals: &Peripherals, addr: Address, values: &mut [u8]) {
    let mut command_start: [u8; 5] = [
        CMD_READ_PIPELINED,
        ((addr.as_u32() >> 16) & 0xFF) as u8,
        ((addr.as_u32() >>  8) & 0xFF) as u8,
        ((addr.as_u32() >>  0) & 0xFF) as u8,
        0x00, // dummy byte due to pipelining delay
    ];
    FlashSpi::communicate_bytes(peripherals, &mut command_start);
    FlashSpi::communicate_bytes(peripherals, values);
}

macro_rules! impl_read_status_register {
    ($name:ident, $opcode:expr) => {
        /// Reads the given status register.
        ///
        /// You must pull ~{CS} low before calling this function, then pull it high again when it
        /// completes.
        pub fn $name(peripherals: &Peripherals) -> u8 {
            let mut buffer: [u8; 2] = [$opcode, 0x00];
            FlashSpi::communicate_bytes(peripherals, &mut buffer);
            buffer[1]
        }
    }
}

impl_read_status_register!(read_status_register_1, CMD_READ_STATUS_REGISTER_1);
impl_read_status_register!(read_status_register_2, CMD_READ_STATUS_REGISTER_2);
impl_read_status_register!(read_status_register_3, CMD_READ_STATUS_REGISTER_3);

pub fn read_all_status_registers(peripherals: &Peripherals) -> [u8; 5] {
    let mut buffer: [u8; 8] = [
        CMD_READ_STATUS_REGISTERS,
        0x01, // start with the first status register
        0x00, // dummy byte,
        0, 0, 0, 0, 0, // bytes for the status register values
    ];
    FlashSpi::communicate_bytes(peripherals, &mut buffer);
    buffer[3..8].try_into().unwrap()
}

pub fn jedec_reset(peripherals: &Peripherals) {
    // grab the SCK and COPI pins for a quick second
    peripherals.GPIOA.moder().modify(|_, w| w
        .moder5().output() // SPI1 SCK
        .moder7().output() // SPI1 COPI
    );

    // pull and keep SCK low
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr5().low()
    );

    // pull ~{CS} down
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().low()
    );

    // COPI down
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr7().low()
    );

    // ~{CS} up
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );

    // COPI up
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr7().high()
    );

    // bounce ~{CS}
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().low()
    );
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );

    // COPI down
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr7().low()
    );

    // bounce ~{CS}
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().low()
    );
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );

    // COPI up
    peripherals.GPIOA.odr().modify(|_, w| w
        .odr7().high()
    );

    // bounce ~{CS}
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().low()
    );
    peripherals.GPIOE.odr().modify(|_, w| w
        .odr8().high()
    );

    // give the pins back to SPI
    peripherals.GPIOA.moder().modify(|_, w| w
        .moder5().alternate() // SPI1 SCK
        .moder7().alternate() // SPI1 COPI
    );
}
