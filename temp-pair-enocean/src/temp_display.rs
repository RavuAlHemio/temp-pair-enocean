/// Temperature display logic.
use bitflags::bitflags;
use stm32f7::stm32f745::Peripherals;

use crate::spi::{Spi, Spi1};


/// A 12-bit brightness value.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Brightness {
    inner: u16,
}
impl Brightness {
    pub const fn new(inner: u16) -> Option<Self> {
        if inner <= 0x0FFF {
            Some(Self { inner })
        } else {
            None
        }
    }

    pub const fn as_u16(&self) -> u16 { self.inner }
}
impl TryFrom<u16> for Brightness {
    type Error = ();
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Self::new(value).ok_or(())
    }
}
impl From<Brightness> for u16 {
    fn from(value: Brightness) -> Self { value.as_u16() }
}


bitflags! {
    #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct SegmentCombo : u8 {
        const DECIMAL_POINT = 0b0000_0001;
        const MIDDLE = 0b0000_0010;
        const TOP_LEFT = 0b0000_0100;
        const BOTTOM_LEFT = 0b0000_1000;
        const BOTTOM = 0b0001_0000;
        const BOTTOM_RIGHT = 0b0010_0000;
        const TOP_RIGHT = 0b0100_0000;
        const TOP = 0b1000_0000;
    }
}


const SUPPORTED_CHARACTERS_SORTED: [u8; 24] = [
    b' ', b'-', b'0', b'1',
    b'2', b'3', b'4', b'5',
    b'6', b'7', b'8', b'9',
    b'A', b'B', b'C', b'D',
    b'E', b'F', b'a', b'b',
    b'c', b'd', b'e', b'f',
];
// same order as SUPPORTED_CHARACTERS_SORTED
const CHARACTER_SEGMENTS: [SegmentCombo; 24] = {
    const M: u8 = SegmentCombo::MIDDLE.bits();
    const T: u8 = SegmentCombo::TOP.bits();
    const TL: u8 = SegmentCombo::TOP_LEFT.bits();
    const BL: u8 = SegmentCombo::BOTTOM_LEFT.bits();
    const B: u8 = SegmentCombo::BOTTOM.bits();
    const BR: u8 = SegmentCombo::BOTTOM_RIGHT.bits();
    const TR: u8 = SegmentCombo::TOP_RIGHT.bits();

    [
        SegmentCombo::from_bits_retain(0), // space
        SegmentCombo::from_bits_retain(M), // -
        SegmentCombo::from_bits_retain(T | TL | BL | B | BR | TR), // 0
        SegmentCombo::from_bits_retain(TR | BR), // 1
        SegmentCombo::from_bits_retain(T | TR | M | BL | B), // 2
        SegmentCombo::from_bits_retain(T | TR | M | BR | B), // 3
        SegmentCombo::from_bits_retain(TL | M | TR | BR), // 4
        SegmentCombo::from_bits_retain(T | TL | M | BR | B), // 5
        SegmentCombo::from_bits_retain(T | TL | BL | B | BR | M), // 6
        SegmentCombo::from_bits_retain(TL | T | TR | BR), // 7
        SegmentCombo::from_bits_retain(T | TL | TR | M | BL | BR | B), // 8
        SegmentCombo::from_bits_retain(T | TL | TR | M | BR | B), // 9
        SegmentCombo::from_bits_retain(BL | TL | T | TR | BR | M), // A
        SegmentCombo::from_bits_retain(TL | BL | B | BR | M), // b
        SegmentCombo::from_bits_retain(T | TL | BL | B), // C
        SegmentCombo::from_bits_retain(TR | BL | B | BR | M), // d
        SegmentCombo::from_bits_retain(T | TL | M | BL | B), // E
        SegmentCombo::from_bits_retain(T | TL | M | BL), // F
        SegmentCombo::from_bits_retain(T | TR | M | BL | BR | B), // a
        SegmentCombo::from_bits_retain(TL | BL | B | BR | M), // b
        SegmentCombo::from_bits_retain(M | BL | B), // c
        SegmentCombo::from_bits_retain(TR | BL | B | BR | M), // d
        SegmentCombo::from_bits_retain(T | TL | TR | M | BL | B), // e
        SegmentCombo::from_bits_retain(T | TL | M | BL), // F
    ]
};


pub struct TempDisplayState {
    lit_segments: [SegmentCombo; 3],
    brightness: Brightness,
    reversed_order: bool,
}
impl TempDisplayState {
    pub fn new(reversed_order: bool) -> Self {
        Self {
            lit_segments: [SegmentCombo::empty(); 3],
            brightness: Brightness::new(1).unwrap(),
            reversed_order,
        }
    }

    fn to_spi_bytes(&self) -> [u8; 36] {
        const ELEMENTS: [SegmentCombo; 8] = [
            SegmentCombo::DECIMAL_POINT,
            SegmentCombo::MIDDLE,
            SegmentCombo::TOP_LEFT,
            SegmentCombo::BOTTOM_LEFT,
            SegmentCombo::BOTTOM,
            SegmentCombo::BOTTOM_RIGHT,
            SegmentCombo::TOP_RIGHT,
            SegmentCombo::TOP,
        ];

        let mut ret = [0u8; 36];
        for (i, lit_segment) in self.lit_segments.iter().copied().enumerate() {
            let digit_ret_offset = 12 * i;

            // technically u24
            // from:
            // 0000 0000 aaaa aaaa aaaa bbbb bbbb bbbb
            // 0000 0000 cccc cccc cccc dddd dddd dddd
            // 0000 0000 eeee eeee eeee ffff ffff ffff
            // 0000 0000 gggg gggg gggg hhhh hhhh hhhh
            // to:
            // aaaa aaaa | aaaa bbbb | bbbb bbbb
            // cccc cccc | cccc dddd | dddd dddd
            // eeee eeee | eeee ffff | ffff ffff
            // gggg gggg | gggg hhhh | hhhh hhhh
            // (12 bytes per 7-seg display => 36 per controller)
            let mut brightness_pairs = [0u32; 4];
            for (element_pair, brightness_pair) in ELEMENTS.chunks(2).zip(brightness_pairs.iter_mut()) {
                let first_element = element_pair[0];
                let second_element = element_pair[1];

                if lit_segment.contains(first_element) {
                    *brightness_pair |= u32::from(self.brightness.as_u16()) << 12;
                }
                if lit_segment.contains(second_element) {
                    *brightness_pair |= u32::from(self.brightness.as_u16()) << 0;
                }
            }

            for (j, brightness_pair) in brightness_pairs.iter().copied().enumerate() {
                let segment_ret_offset = 3 * j;
                ret[digit_ret_offset + segment_ret_offset + 0] =
                    ((brightness_pair >> 16) & 0xFF) as u8;
                ret[digit_ret_offset + segment_ret_offset + 1] =
                    ((brightness_pair >> 8) & 0xFF) as u8;
                ret[digit_ret_offset + segment_ret_offset + 2] =
                    ((brightness_pair >> 0) & 0xFF) as u8;
            }
        }

        ret
    }

    pub fn set_brightness(&mut self, brightness: Brightness) {
        self.brightness = brightness;
    }

    pub fn set_segments(&mut self, position: usize, segments: SegmentCombo) {
        assert!(position < 3);
        let real_position = if self.reversed_order { 2 - position } else { position };
        self.lit_segments[real_position] = segments;
    }

    pub fn set_digit(&mut self, position: usize, ascii_digit: u8, decimal_point: bool) {
        assert!(position < 3);
        let digit_pos = match SUPPORTED_CHARACTERS_SORTED.binary_search(&ascii_digit) {
            Ok(dp) => dp,
            Err(_) => 0,
        };
        let decimal_point_segment = if decimal_point {
            SegmentCombo::DECIMAL_POINT
        } else {
            SegmentCombo::empty()
        };
        self.set_segments(position, CHARACTER_SEGMENTS[digit_pos] | decimal_point_segment);
    }

    pub fn set_nibble_digit(&mut self, position: usize, nibble: u8, decimal_point: bool) {
        assert!(position < 3);
        let ascii_digit = if nibble >= 0x10 {
            b' '
        } else {
            [
                b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7',
                b'8', b'9', b'A', b'b', b'C', b'd', b'E', b'F',
            ][usize::from(nibble)]
        };
        self.set_digit(position, ascii_digit, decimal_point);
    }

    pub fn send_via_spi(&self, peripherals: &Peripherals) {
        let mut spi_bytes = self.to_spi_bytes();
        Spi1::communicate_bytes(&peripherals, &mut spi_bytes);
    }
}
