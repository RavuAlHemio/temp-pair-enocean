use stm32f7::stm32f745::i2c1;
use stm32f7::stm32f745::Peripherals;


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
        // calculated speed parameters from values:
        // target I2C bus frequency: 100 kHz
        // I2C peripheral clock frequency: 16_000 kHz
        // I2C mode: standard
        // analog filter active: yes
        // digital noise filter count: 15
        // rise time (ns): 1000
        // fall time (ns): 300
        i2c.timingr().modify(|_, w| w
            .presc().set(1)
            .sdadel().set(0)
            .scldel().set(9)
            .scll().set(28)
            .sclh().set(23)
        );

        // turn on
        i2c.cr1().modify(|_, w| w
            .pe().enabled()
        );
    }
}
