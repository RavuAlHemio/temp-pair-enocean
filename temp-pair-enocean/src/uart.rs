use core::cell::RefCell;

use critical_section::Mutex;
use stm32f7::stm32f745::Peripherals;
use stm32f7::stm32f745::{interrupt, usart1};
use tpe_ring_buffer::RingBuffer;


pub trait Uart {
    fn get_peripheral(peripherals: &Peripherals) -> &usart1::RegisterBlock;
    fn enable_peripheral_clock(peripherals: &Peripherals);
    fn take_byte() -> Option<u8>;
    fn copy_buffer(buffer: &mut [u8]) -> usize;

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

            .rxneie().enabled()
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
        uart.cr3().modify(|_, w| w
            .ovrdis().disabled() // disable overrun because we don't know what to do anyway
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


macro_rules! implement_uart {
    (
        $struct_name:ident,
        $peripheral_name:ident,
        $rcc_enable_register:ident,
        $rcc_field:ident,
        $buffer_name:ident,
        $buffer_size:expr,
        $interrupt_name:ident $(,)?
    ) => {
        static $buffer_name: Mutex<RefCell<RingBuffer<u8, $buffer_size>>> = Mutex::new(RefCell::new(RingBuffer::new()));

        pub struct $struct_name;
        impl Uart for $struct_name {
            fn get_peripheral(peripherals: &Peripherals) -> &usart1::RegisterBlock {
                &*peripherals.$peripheral_name
            }

            fn enable_peripheral_clock(peripherals: &Peripherals) {
                peripherals.RCC.$rcc_enable_register().modify(|_, w| w
                    .$rcc_field().set_bit()
                );
            }

            fn take_byte() -> Option<u8> {
                critical_section::with(|cs| {
                    $buffer_name.borrow_ref_mut(cs)
                        .read()
                })
            }

            fn copy_buffer(buffer: &mut [u8]) -> usize {
                let mut byte_count = 0;
                critical_section::with(|cs| {
                    for byte in buffer {
                        match $buffer_name.borrow_ref_mut(cs).read() {
                            Some(b) => {
                                *byte = b;
                                byte_count += 1;
                            },
                            None => break,
                        }
                    }
                });
                byte_count
            }
        }

        #[interrupt]
        fn $interrupt_name() {
            let peripherals = unsafe { Peripherals::steal() };
            let uart = &peripherals.$peripheral_name;
            while uart.isr().read().rxne().is_data_ready() {
                let read_full_byte = uart.rdr().read().rdr().bits();
                let read_byte = (read_full_byte & 0xFF) as u8;
                critical_section::with(|cs| {
                    $buffer_name.borrow_ref_mut(cs)
                        .write(read_byte);
                });
            }
        }
    };
}


//implement_uart!(Usart1, USART2, apb2enr, usart1en, USART1_BUFFER, 32, USART1);
implement_uart!(Usart2, USART2, apb1enr, usart2en, USART2_BUFFER, 32, USART2);
implement_uart!(Usart3, USART3, apb1enr, usart3en, USART3_BUFFER, 32, USART3);
//implement_uart!(Uart4, UART4, apb1enr, uart4en, UART4_BUFFER, 32, UART4);
//implement_uart!(Uart5, UART5, apb1enr, uart5en, UART5_BUFFER, 32, UART5);
//implement_uart!(Usart6, USART6, apb2enr, usart6en, USART6_BUFFER, 32, USART6);
//implement_uart!(Uart7, UART7, apb1enr, uart7en, UART7_BUFFER, 32, UART7);
//implement_uart!(Uart8, UART8, apb1enr, uart8en, UART8_BUFFER, 32, UART8);
