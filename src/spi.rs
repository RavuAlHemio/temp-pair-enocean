use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::spi1;
use stm32f7::stm32f745::spi1::cr1::BR;


#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SpiMode {
    /// Mode 0: write SCLK↘ or CS↘, read SCLK↗
    WriteFallingOrCsReadRising = 0,

    /// Mode 1: write SCLK↗, read SCLK↘
    WriteRisingReadFalling = 1,

    /// Mode 2: write SCLK↗ or CS↘, read SCLK↘
    WriteRisingOrCsReadFalling = 2,

    /// Mode 3: write SCLK↘, read SCLK↗
    WriteFallingReadRising = 3,
}
impl SpiMode {
    /// Common SPI clock polarity value for this mode.
    pub fn cpol(&self) -> bool {
        match self {
            Self::WriteFallingOrCsReadRising => false,
            Self::WriteRisingReadFalling => false,
            Self::WriteRisingOrCsReadFalling => true,
            Self::WriteFallingReadRising => true,
        }
    }

    /// Common SPI clock phase value for this mode.
    pub fn cpha(&self) -> bool {
        match self {
            Self::WriteFallingOrCsReadRising => false,
            Self::WriteRisingReadFalling => true,
            Self::WriteRisingOrCsReadFalling => false,
            Self::WriteFallingReadRising => true,
        }
    }
}


pub trait Spi {
    fn get_peripheral(peripherals: &Peripherals) -> &spi1::RegisterBlock;
    fn enable_peripheral_clock(peripherals: &Peripherals);


    // notes on polarity:
    // 7seg: shift in on rising edge, shift out on falling edge (SPI mode 0)
    // flash: SPI mode 0 or 3 (sampled when chip select is pulled low)
    fn set_up_as_controller(peripherals: &Peripherals, speed_divisor: BR, mode: SpiMode, lsb_first: bool) {
        let spi = Self::get_peripheral(peripherals);

        // assumes pins are already set up

        // gimme clock
        Self::enable_peripheral_clock(peripherals);

        // set up
        spi.cr1().modify(|_, w| w
            .br().variant(speed_divisor)
            .cpol().bit(mode.cpol())
            .cpha().bit(mode.cpha())
            .rxonly().clear_bit() // bidirectional communication
            .bidimode().unidirectional() // one line per direction
            .lsbfirst().bit(lsb_first)
            .crcen().disabled() // no CRC
            .ssm().enabled() // chip select pin is controlled by software (and we will control it manually)
            .mstr().master() // controller role
        );
        spi.cr2().modify(|_, w| w
            .ds().eight_bit() // eight bits per transfer
            .ssoe().disabled() // don't enable chip select (we manage it independently)
            .nssp().no_pulse() // no chip select pulsing after each byte (we managed it independently anyway)
            .frxth().quarter() // trigger RXNE event if queue is 1/4 full (8 bits)
            .txdmaen().clear_bit() // no DMA when transmitting
            .rxdmaen().clear_bit() // no DMA when receiving
        );

        // turn on
        spi.cr1().modify(|_, w| w
            .spe().set_bit()
        );
    }

    /// Reads and writes via SPI.
    ///
    /// Outgoing data is taken from `data` and replaced with incoming data.
    fn communicate_bytes(peripherals: &Peripherals, data: &mut [u8]) {
        let spi = Self::get_peripheral(peripherals);

        // wait until previous transfer is complete
        while spi.sr().read().bsy().bit_is_set() {
        }

        for b in data {
            // write a byte
            spi.dr().modify(|_, w| w
                .dr().set(*b as u16)
            );

            // wait for the transfer to complete
            while spi.sr().read().bsy().bit_is_set() {
            }

            // read a byte
            *b = (spi.dr().read().dr().bits() & 0xFF) as u8;
        }
    }
}


pub struct Spi1;
impl Spi for Spi1 {
    fn get_peripheral(peripherals: &Peripherals) -> &spi1::RegisterBlock {
        &*peripherals.SPI1
    }

    fn enable_peripheral_clock(peripherals: &Peripherals) {
        peripherals.RCC.apb2enr().modify(|_, w| w
            .spi1en().set_bit()
        );
    }
}
