use stm32f7::stm32f745::Peripherals;


macro_rules! make_gpio_output {
    (
        $name:ident,
        $pin_bank:ident,
        $pin:expr $(,)?
    ) => {
        pub struct $name;
        impl GpioOutput for $name {
            fn set_up(peripherals: &Peripherals) {
                // clock to GPIO peripheral
                peripherals.RCC.ahb1enr().modify(|_, w|
                    make_gpio_output!(@clock_field, $pin_bank, w).enabled()
                );

                // pin to output
                make_gpio_output!(@gpio_peripheral, $pin_bank, peripherals).moder().modify(|_, w| w
                    .moder($pin).output()
                );

                // output to push-pull
                make_gpio_output!(@gpio_peripheral, $pin_bank, peripherals).otyper().modify(|_, w| w
                    .ot($pin).push_pull()
                );
            }

            fn turn_on(peripherals: &Peripherals) {
                make_gpio_output!(@gpio_peripheral, $pin_bank, peripherals).odr().modify(|_, w| w
                    .odr($pin).high()
                );
            }

            fn turn_off(peripherals: &Peripherals) {
                make_gpio_output!(@gpio_peripheral, $pin_bank, peripherals).odr().modify(|_, w| w
                    .odr($pin).low()
                );
            }
        }
    };
    (@clock_field, A, $register:expr) => {$register.gpioaen()};
    (@clock_field, B, $register:expr) => {$register.gpioben()};
    (@clock_field, C, $register:expr) => {$register.gpiocen()};
    (@clock_field, D, $register:expr) => {$register.gpioden()};
    (@clock_field, E, $register:expr) => {$register.gpioeen()};
    (@clock_field, F, $register:expr) => {$register.gpiofen()};
    (@clock_field, G, $register:expr) => {$register.gpiogen()};
    (@clock_field, H, $register:expr) => {$register.gpiohen()};
    (@clock_field, I, $register:expr) => {$register.gpioien()};
    (@clock_field, J, $register:expr) => {$register.gpiojen()};
    (@clock_field, K, $register:expr) => {$register.gpioken()};
    (@gpio_peripheral, A, $peripherals:expr) => {$peripherals.GPIOA};
    (@gpio_peripheral, B, $peripherals:expr) => {$peripherals.GPIOB};
    (@gpio_peripheral, C, $peripherals:expr) => {$peripherals.GPIOC};
    (@gpio_peripheral, D, $peripherals:expr) => {$peripherals.GPIOD};
    (@gpio_peripheral, E, $peripherals:expr) => {$peripherals.GPIOE};
    (@gpio_peripheral, F, $peripherals:expr) => {$peripherals.GPIOF};
    (@gpio_peripheral, G, $peripherals:expr) => {$peripherals.GPIOG};
    (@gpio_peripheral, H, $peripherals:expr) => {$peripherals.GPIOH};
    (@gpio_peripheral, I, $peripherals:expr) => {$peripherals.GPIOI};
    (@gpio_peripheral, J, $peripherals:expr) => {$peripherals.GPIOJ};
    (@gpio_peripheral, K, $peripherals:expr) => {$peripherals.GPIOK};
}


pub trait GpioOutput {
    fn set_up(peripherals: &Peripherals);
    fn turn_on(peripherals: &Peripherals);
    fn turn_off(peripherals: &Peripherals);

    fn set_high(peripherals: &Peripherals) { Self::turn_on(peripherals) }
    fn set_low(peripherals: &Peripherals) { Self::turn_off(peripherals) }
}


make_gpio_output!(BlinkyLedA8, A, 8);
make_gpio_output!(BlinkyLedC8, C, 8);
make_gpio_output!(TempDisplayBridgeNotReset, D, 11);
make_gpio_output!(FlashNotChipSelect, E, 8);
make_gpio_output!(FlashNotHoldOrNotReset, E, 7);
make_gpio_output!(FlashWriteProtect, D, 12);
make_gpio_output!(EnOceanNotReset, C, 15);
