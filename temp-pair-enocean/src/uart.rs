use core::cell::RefCell;

use critical_section::Mutex;
use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::{interrupt, usart1};
use tpe_ring_buffer::RingBuffer;


pub trait Uart {
    fn get_peripheral(peripherals: &Peripherals) -> &usart1::RegisterBlock;
    fn enable_peripheral_clock(peripherals: &Peripherals);
    fn take_byte() -> Option<u8>;

    fn set_up(peripherals: &Peripherals, speed_divisor: u16) {
        let uart = Self::get_peripheral(peripherals);

        // assumes pins are already set up

        // gimme clock
        Self::enable_peripheral_clock(peripherals);

        // set up
        uart.cr1().modify(|_, w| w
            .m0().bit8() // 8 bits per byte
            .m1().m0() // yes, 8 bits per byte
            .over8().oversampling16() // sample 16 bits, not 8
            .pce().disabled() // no hardware parity calculation
        );
        uart.brr().modify(|_, w| w
            .brr().set(speed_divisor)
        );
        uart.cr2().modify(|_, w| w
            .stop().stop1() // 1 stop bit
            .txinv().standard() // transmission pin not inverted
            .rxinv().standard() // reception pin not inverted
            .datainv().positive() // data polarity not inverted
            .msbfirst().clear_bit() // RS232 says least significant byte first
        );

        uart.cr1().modify(|_, w| w
            .ue().enabled() // turn on UART
        );

        uart.cr1().modify(|_, w| w
            .re().enabled() // turn on reception
            .te().enabled() // turn on transmission
        );
    }

    /// Writes via UART.
    fn write(peripherals: &Peripherals, data: &[u8]) {
        let uart = Self::get_peripheral(peripherals);

        for b in data {
            // wait until transmit buffer is empty
            while uart.isr().read().txe().is_full() {
            }

            // write the byte
            uart.tdr().modify(|_, w| w
                .tdr().set(*b as u16)
            );
        }

        // wait until transmit buffer is empty one last time
        while uart.isr().read().txe().is_full() {
        }
    }
}

static USART1_BUFFER: Mutex<RefCell<RingBuffer<u8, 32>>> = Mutex::new(RefCell::new(RingBuffer::new()));

pub struct Usart1;
impl Uart for Usart1 {
    fn get_peripheral(peripherals: &Peripherals) -> &usart1::RegisterBlock {
        &*peripherals.USART1
    }

    fn enable_peripheral_clock(peripherals: &Peripherals) {
        peripherals.RCC.apb2enr().modify(|_, w| w
            .usart1en().set_bit()
        );
    }

    fn take_byte() -> Option<u8> {
        critical_section::with(|cs| {
            USART1_BUFFER.borrow_ref_mut(cs)
                .read()
        })
    }
}

#[interrupt]
fn USART1() {
    let peripherals = unsafe { Peripherals::steal() };
    let interrupt_state = peripherals.USART1.isr().read();
    if interrupt_state.rxne().is_data_ready() {
        let read_full_byte = peripherals.USART1.rdr().read().rdr().bits();
        let read_byte = (read_full_byte & 0xFF) as u8;
        critical_section::with(|cs| {
            USART1_BUFFER.borrow_ref_mut(cs)
                .write(read_byte);
        });
    }
}
