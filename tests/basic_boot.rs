// 結合テストの実行可能ファイルは、テストモードでないときはビルドされない
// serial_printlnやexit_qemuにはアクセス不可

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(jura_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use jura_os::println;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

#[allow(dead_code)]
fn test_runner(_tests: &[&dyn Fn()]) {
    // 未実装のpanic
    unimplemented!();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[test_case]
fn test_println() {
    println!("test_println output");
}
