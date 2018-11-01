#![no_std]
#![no_main]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate panic_halt;

use cortex_m_rt::entry;

#[macro_use(interrupt)]
extern crate stm32f4;
use stm32f4::stm32f405;

#[entry]
fn start() -> ! {
    // Acquire the device peripherals. They can only be taken once ever.
    let device_peripherals = stm32f405::Peripherals::take().unwrap();
    let mut core_peripherals = stm32f405::CorePeripherals::take().unwrap();

    // Get a reference to GPIOA and RCC to save typing.
    let gpioa = &device_peripherals.GPIOA;
    let rcc = &device_peripherals.RCC;
    let tim2 = &device_peripherals.TIM2;
    let nvic = &mut core_peripherals.NVIC;

    // Enable the GPIOA clock and set PA8 to be an output
    rcc.ahb1enr.modify(|_, w| w.gpioaen().enabled());
    gpioa.moder.modify(|_, w| w.moder8().output());

    // Set up the timer for slow interrupt generation
    // NOTE(unsafe): The psc field has not been sufficiently documented
    // to allow safe writing of arbitrary integer values, so we have to
    // use unsafe here. This could be fixed by improving the SVD file.
    rcc.apb1enr.modify(|_, w| w.tim2en().enabled());
    tim2.dier.write(|w| w.uie().enabled());
    tim2.psc.write(|w| unsafe { w.psc().bits(1000) });
    tim2.arr.write(|w| w.arr().bits(2000));
    tim2.cr1.write(|w| w.cen().enabled());

    // Enable the timer interrupt in the NVIC.
    nvic.enable(stm32f405::Interrupt::TIM2);

    // The main thread can now go to sleep.
    // WFI (wait for interrupt) puts the core in sleep until an interrupt occurs.
    loop {
        cortex_m::asm::wfi();
    }
}

// Set `tim2` to be the interrupt handler for TIM2.
interrupt!(TIM2, tim2);

/// Interrupt handler for TIM2
fn tim2() {
    // NOTE(unsafe): We have to use unsafe to access the peripheral
    // registers in this interrupt handler because we already used `take()`
    // in the main code. In this case all our uses are safe, not least because
    // the main thread only calls `wfi()` after enabling the interrupt, so
    // no race conditions or other unsafe behaviour is possible.
    // For ways to avoid using unsafe here, consult the Concurrency chapter:
    // https://rust-embedded.github.io/book/concurrency/concurrency.html

    // Clear the UIF bit to indicate the interrupt has been serviced
    unsafe { (*stm32f405::TIM2::ptr()).sr.modify(|_, w| w.uif().clear_bit()) };

    // Read ODR8 to see if the pin is set, and if so, clear it,
    // otherwise, set it. We use the atomic BSRR register to
    // set/reset it without needing to read-modify-write ODR.
    let ptr = stm32f405::GPIOA::ptr();
    match unsafe { (*ptr).odr.read().odr8() } {
        stm32f405::gpioa::odr::ODR15R::HIGH => {
            unsafe { (*ptr).bsrr.write(|w| w.br8().set_bit()) };
        },
        stm32f405::gpioa::odr::ODR15R::LOW => {
            unsafe { (*ptr).bsrr.write(|w| w.bs8().set_bit()) };
        },
    }
}
