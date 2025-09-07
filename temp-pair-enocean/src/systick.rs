use cortex_m::Peripherals;
use cortex_m_rt::exception;
use critical_section::Mutex;
use vcell::VolatileCell;


// ST RM0385 § 5.2 says: "The RCC feeds the external clock of the Cortex System Timer (SysTick) with
// the AHB clock (HCLK) divided by 8."
const FIXED_PRESCALER: u32 = 8;


static COUNTER: Mutex<VolatileCell<u32>> = Mutex::new(VolatileCell::new(0));


pub fn set_up(core_peripherals: &Peripherals) {
    // trigger every millisecond (1/1000 s)
    let sys_tick_period = (crate::CLOCK_SPEED_HZ / FIXED_PRESCALER) / 1000;
    assert!(sys_tick_period > 1);
    let reload_value = sys_tick_period - 1;
    assert!(reload_value <= 0x00FF_FFFF);

    // set it up
    unsafe {
        // clear counter to zero
        core_peripherals.SYST.cvr.write(0);

        // set the reload value
        core_peripherals.SYST.rvr.write(reload_value);

        // turn on interrupt and enable the counter
        const ENABLE: u32 = 0b1;
        const TICKINT: u32 = 0b10;
        core_peripherals.SYST.csr.modify(|val| val | ENABLE | TICKINT);
    }
}

pub fn get_counter() -> u32 {
    critical_section::with(|cs| {
        COUNTER.borrow(cs)
            .get()
    })
}

#[exception]
fn SysTick() {
    critical_section::with(|cs| {
        let guard = COUNTER.borrow(cs);
        guard.set(guard.get().wrapping_add(1));
    });
}
