//! HMI display and buttons based on the MikroElektronika 8800 Retro Click.
//!
//! The central component on this board is an AMS AS1115, an I2C-enabled LED driver and button
//! scanner.


use stm32f7::stm32f745::Peripherals;

use crate::i2c::{I2c, I2cAddress};


// 3x5 hex font
// leave 1 column of pixels between chars
//
// a character
//
// abc
// def
// ghi
// jkl
// mno
//
// is encoded as a 16-bit value as
//
// 0abc_defg_hijk_lmno
#[allow(unused)]
const FONT: [u16; 16] = [
    0b010_101_101_101_010, // 0
    0b001_011_001_001_001, // 1
    0b110_001_010_100_111, // 2
    0b110_001_010_001_110, // 3
    0b101_101_111_001_001, // 4
    0b111_100_111_001_111, // 5
    0b011_100_111_101_111, // 6
    0b111_001_001_001_001, // 7
    0b111_101_111_101_111, // 8
    0b111_101_111_001_111, // 9
    0b111_101_111_101_101, // A
    0b110_101_110_101_110, // B
    0b011_100_100_100_011, // C
    0b110_101_101_101_110, // D
    0b111_100_110_100_111, // E
    0b111_100_110_100_100, // F
];


#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HmiDisplay {
    pub i2c_address: I2cAddress,
}
impl HmiDisplay {
    pub fn set_up<I: I2c>(&self, peripherals: &Peripherals) {
        // configure the I2C-SPI bridge
        I::write_data(
            &peripherals,
            self.i2c_address,
            &[
                0x0C, // shutdown
                (
                    (0b0 << 7) // do not shut down
                    | (0b000000 << 1) // don't-care bits
                    | (0b1 << 0) // reset feature register
                ),
            ],
        );
        I::write_data(
            &peripherals,
            self.i2c_address,
            &[
                0x0B, // scan-limit register
                (
                    (0b00000 << 3) // don't-care bits
                    | (0b111 << 0) // show all digits
                ),
            ],
        );
        I::write_data(
            &peripherals,
            self.i2c_address,
            &[
                0x01, // first LED row
                0, 0, 0, 0, 0, 0, 0, 0, // clear all eight LED rows
            ],
        );
    }

    pub fn write_to_display<I: I2c>(&self, peripherals: &Peripherals, data: &[u8]) {
        assert!(data.len() <= 8);
        let mut final_data = [0u8; 9];
        final_data[0] = 0x01; // register for first row (automatically increments after each byte)
        final_data[1..1+data.len()].copy_from_slice(data);
        I::write_data(
            &peripherals,
            self.i2c_address,
            &final_data[..1+data.len()],
        );
    }

    pub fn read_buttons<I: I2c>(&self, peripherals: &Peripherals) -> [u8; 2] {
        let mut ret = [0u8; 2];
        I::write_data(
            &peripherals,
            self.i2c_address,
            &[
                0x1C, // KEYA (first button state register, automatically increments after each byte)
            ],
        );
        I::read_data(
            &peripherals,
            self.i2c_address,
            &mut ret,
        );
        ret
    }
}
