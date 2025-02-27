#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod vga_buffer;

// エントリポイント
#[no_mangle]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}

// panic時に呼び出される
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
