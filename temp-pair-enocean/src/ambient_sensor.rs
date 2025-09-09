//! Ambient light sensor based on the MikroElektronika Ambient 24 Click board.
//!
//! The central component on this board is a Vishay VEML4031X00, which communicates via I2C.


use stm32f7::stm32f745::Peripherals;

use crate::i2c::{I2c, I2cAddress};


#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AmbientLightSensor {
    pub i2c_address: I2cAddress,
}
impl AmbientLightSensor {
    pub fn set_up<I: I2c>(&self, peripherals: &Peripherals) {
        I::write_data(
            peripherals,
            self.i2c_address,
            &[
                0x00, // ALS_CONF_0 (followed by ALS_CONF_1, increments automatically)
                (
                    (0b0 << 7) // reserved
                    | (0b111 << 4) // 400ms integration time
                    | (0b0 << 3) // continuous measurement
                    | (0b0 << 2) // no measurement trigger
                    | (0b0 << 1) // no interrupt
                    | (0b1 << 0) // turn on the sensor (1/2)
                ),
                (
                    (0b1 << 7) // turn on the sensor (2/2)
                    | (0b0 << 6) // use whole photodiode
                    | (0b0 << 5) // reserved
                    | (0b00 << 3) // x1 gain
                    | (0b00 << 1) // no interrupt hysteresis (we're not using the interrupt anyway)
                    | (0b1 << 0) // perform internal calibration
                ),
            ],
        );
    }

    pub fn read_ambient_light<I: I2c>(&self, peripherals: &Peripherals) -> u16 {
        let write_buf = [
            0x10, // ALS_DATA_L (followed by ALS_DATA_H, increments automatically)
        ];
        let mut read_buf = [0u8; 2];
        I::write_data(
            peripherals,
            self.i2c_address,
            &write_buf,
        );
        I::read_data(
            peripherals,
            self.i2c_address,
            &mut read_buf,
        );
        u16::from_le_bytes(read_buf)
    }
}
