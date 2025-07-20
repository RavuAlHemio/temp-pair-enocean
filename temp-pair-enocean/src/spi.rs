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

    fn set_up_as_controller(peripherals: &Peripherals, speed_divisor: BR, mode: SpiMode, lsb_first: bool) {
        let spi = Self::get_peripheral(peripherals);

        // assumes pins are already set up

        // gimme clock
        Self::enable_peripheral_clock(peripherals);

        // turn off to perform configuration
        spi.cr1().modify(|_, w| w
            .spe().disabled()
        );

        // set up
        spi.cr1().modify(|_, w| w
            .br().variant(speed_divisor)
            .cpol().bit(mode.cpol())
            .cpha().bit(mode.cpha())
            .rxonly().full_duplex() // bidirectional communication
            .bidimode().unidirectional() // one line per direction
            .bidioe().output_disabled() // irrelevant for unidirectional mode
            .lsbfirst().bit(lsb_first)
            .crcen().disabled() // no CRC
            .crcl().eight_bit() // 8-bit CRC (but actually no CRC)
            .crcnext().tx_buffer() // next CRC is from Tx buffer (but actually no CRC)
            .ssm().enabled() // we control the chip select pin
            .ssi().slave_not_selected() // and currently no chip is selected
            .mstr().master() // controller role
        );
        spi.cr2().modify(|_, w| w
            .ds().eight_bit() // eight bits per transfer
            .ssoe().enabled() // enable chip select (although we manage it independently)
            .nssp().no_pulse() // no chip select pulsing after each byte (we manage it independently anyway)
            .frxth().quarter() // trigger RXNE event if queue is 1/4 full (8 bits)
            .txdmaen().clear_bit() // no DMA when transmitting
            .rxdmaen().clear_bit() // no DMA when receiving
        );

        // turn on
        spi.cr1().modify(|_, w| w
            .spe().enabled()
        );
    }

    /// Reads and writes via SPI.
    ///
    /// Outgoing data is taken from `data` and replaced with incoming data.
    fn communicate_bytes(peripherals: &Peripherals, data: &mut [u8]) {
        let spi = Self::get_peripheral(peripherals);

        // pretend that chip select is low
        spi.cr1().modify(|_, w| w
            .ssi().slave_selected()
        );

        for b in data {
            // wait until previous transfer is complete
            while spi.sr().read().txe().is_not_empty() {
            }

            // write a byte
            // (we must use dr8() here, otherwise SPI will try to send the 16 bits as 2 bytes)
            spi.dr8().modify(|_, w| w
                .dr().set(*b)
            );

            // wait until we have something to read
            while spi.sr().read().rxne().is_empty() {
            }

            // read a byte
            // (we must use dr8() here, otherwise SPI will try to read the 16 bits as 2 bytes)
            *b = spi.dr8().read().dr().bits();
        }

        // pretend that chip select is high
        spi.cr1().modify(|_, w| w
            .ssi().slave_not_selected()
        );
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
