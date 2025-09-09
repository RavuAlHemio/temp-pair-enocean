use stm32f7::stm32f745::i2c1;
use stm32f7::stm32f745::Peripherals;


#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct I2cAddress(u8);
impl I2cAddress {
    pub const fn new(address: u8) -> Option<Self> {
        if address & 0b1000_0000 != 0 {
            None
        } else {
            Some(Self(address))
        }
    }

    pub const fn as_u8(&self) -> u8 {
        self.0
    }
}


pub trait I2c {
    fn get_peripheral(peripherals: &Peripherals) -> &i2c1::RegisterBlock;
    fn enable_peripheral_clock(peripherals: &Peripherals);

    fn set_up_as_controller(peripherals: &Peripherals) {
        let i2c = Self::get_peripheral(peripherals);

        // assumes pins are already set up

        // gimme clock
        Self::enable_peripheral_clock(peripherals);

        // set up noise filter
        i2c.cr1().modify(|_, w| w
            .anfoff().enabled() // analog filter enabled
            .dnf().filter15() // 15-period digital filter
            .txdmaen().disabled() // no DMA for transmission
            .rxdmaen().disabled() // no DMA for reception
            .sbc().disabled() // this option may only be enabled if we're the peripheral
            .nostretch().disabled() // this option may only be enabled if we're the peripheral
            .smbhen().disabled() // ignore the SMBus host address
            .smbden().disabled() // ignore the SMBus default address
            .alerten().disabled() // no SMBus alerts
            .pecen().disabled() // no packet error checking
        );
        i2c.cr2().modify(|_, w| w
            .add10().bit7() // 7-bit addresses
        );
        // see comment of crate::setup_clocks for origin of values
        i2c.timingr().modify(|_, w| w
            .presc().set(1)
            .sdadel().set(0)
            .scldel().set(15)
            .scll().set(49)
            .sclh().set(40)
        );

        // turn on
        i2c.cr1().modify(|_, w| w
            .pe().enabled()
        );
    }

    fn write_data(peripherals: &Peripherals, address: I2cAddress, data: &[u8]) {
        let i2c = Self::get_peripheral(peripherals);

        assert!(data.len() <= 0xFF);

        // set address and write bit
        i2c.cr2().modify(|_, w| w
            .sadd().set((address.as_u8() << 1) as u16) // 7-bit addresses are shifted one left
            .rd_wrn().write() // we are writing
            .nbytes().set(data.len() as u8)
            .reload().clear_bit() // no reloading after 255 bytes
            .autoend().clear_bit() // we will issue the STOP condition ourselves
        );

        // wait until bus is idle
        while i2c.isr().read().busy().is_busy() {
        }

        // go go go!
        i2c.cr2().modify(|_, w| w
            .start().set_bit()
        );

        for &byte in data {
            // wait until the write register is empty
            while i2c.isr().read().txe().is_not_empty() {
            }

            // write
            i2c.txdr().modify(|_, w| w
                .txdata().set(byte)
            );
        }

        // wait until the transfer is complete
        while i2c.isr().read().tc().is_not_complete() {
        }

        // we are done
        i2c.cr2().modify(|_, w| w
            .stop().set_bit()
        );
    }

    fn read_data(peripherals: &Peripherals, address: I2cAddress, data: &mut [u8]) {
        let i2c = Self::get_peripheral(peripherals);

        assert!(data.len() <= 0xFF);

        // set address and write bit
        i2c.cr2().modify(|_, w| w
            .sadd().set((address.as_u8() << 1) as u16) // 7-bit addresses are shifted one left
            .rd_wrn().read() // we are reading
            .nbytes().set(data.len() as u8)
            .reload().clear_bit() // no reloading after 255 bytes
            .autoend().clear_bit() // we will issue the STOP condition ourselves
        );

        // wait until bus is idle
        while i2c.isr().read().busy().is_busy() {
        }

        // go go go!
        i2c.cr2().modify(|_, w| w
            .start().set_bit()
        );

        for byte in data {
            // wait until the read register is full
            while i2c.isr().read().rxne().is_empty() {
            }
            *byte = i2c.rxdr().read().rxdata().bits();
        }

        // wait until transfer is complete
        while i2c.isr().read().tc().is_not_complete() {
        }

        // we are done
        i2c.cr2().modify(|_, w| w
            .stop().set_bit()
        );
    }

    /// Writes then reads data via the I2C bus without relinquishing it.
    ///
    /// A sequence of [`write_data`](I2c::write_data) followed by [`read_data`](I2c::read_data)
    /// performs a Start condition, the write, a Stop condition, a Start condition, the read, and a
    /// Stop condition.
    ///
    /// This function instead performs a Start condition, the write, a repeated Start condition, the
    /// read, and a Stop condition. This may be required by some hardware; otherwise, the written
    /// register number may be forgotten before the read.
    fn write_then_read_data(peripherals: &Peripherals, address: I2cAddress, write_data: &[u8], read_data: &mut [u8]) {
        let i2c = Self::get_peripheral(peripherals);

        assert!(write_data.len() <= 0xFF);
        assert!(read_data.len() <= 0xFF);

        // set address and write bit
        i2c.cr2().modify(|_, w| w
            .sadd().set((address.as_u8() << 1) as u16) // 7-bit addresses are shifted one left
            .rd_wrn().write() // we are writing
            .nbytes().set(write_data.len() as u8)
            .reload().clear_bit() // no reloading after 255 bytes
            .autoend().clear_bit() // we will issue the STOP condition ourselves
        );

        // wait until bus is idle
        while i2c.isr().read().busy().is_busy() {
        }

        // go go go!
        i2c.cr2().modify(|_, w| w
            .start().set_bit()
        );

        for &byte in write_data {
            // wait until the write register is empty
            while i2c.isr().read().txe().is_not_empty() {
            }

            // write
            i2c.txdr().modify(|_, w| w
                .txdata().set(byte)
            );
        }

        // wait until the transfer is complete
        while i2c.isr().read().tc().is_not_complete() {
        }

        // issue a repeated START, now with reading
        i2c.cr2().modify(|_, w| w
            .sadd().set((address.as_u8() << 1) as u16) // 7-bit addresses are shifted one left
            .rd_wrn().read() // we are reading
            .nbytes().set(read_data.len() as u8)
            .reload().clear_bit() // no reloading after 255 bytes
            .autoend().clear_bit() // we will issue the STOP condition ourselves
            .start().set_bit() // (repeated) start
        );

        for byte in read_data {
            // wait until the read register is full
            while i2c.isr().read().rxne().is_empty() {
            }
            *byte = i2c.rxdr().read().rxdata().bits();
        }

        // wait until transfer is complete
        while i2c.isr().read().tc().is_not_complete() {
        }

        // we are done
        i2c.cr2().modify(|_, w| w
            .stop().set_bit()
        );
    }
}

macro_rules! implement_i2c {
    (
        $struct_name:ident,
        $peripheral_name:ident,
        $rcc_enable_register:ident,
        $rcc_field:ident $(,)?
    ) => {
        pub struct $struct_name;
        impl I2c for $struct_name {
            fn get_peripheral(peripherals: &Peripherals) -> &i2c1::RegisterBlock {
                &*peripherals.$peripheral_name
            }

            fn enable_peripheral_clock(peripherals: &Peripherals) {
                peripherals.RCC.$rcc_enable_register().modify(|_, w| w
                    .$rcc_field().set_bit()
                );
            }
        }
    };
}

implement_i2c!(I2c2, I2C2, apb1enr, i2c2en);
