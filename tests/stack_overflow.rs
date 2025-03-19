// stack_overflowにより発生するダブルフォルトハンドラの呼び出し確認
#![no_std]
#![no_main]
// 結合テストは完全に分けられた実行ファイルなので再記述
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;
use jura_os::{exit_qemu, serial_println, QemuExitCode};
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    // idtの用意
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                // double fault hadlerの追加
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(jura_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_println!("stack_overflow::stack_overflow...\t");

    jura_os::gdt::init();
    init_test_idt();

    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // lib.rsで実装済み
    jura_os::test_panic_handler(info)
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    // 再起の度にリターンアドレスがプッシュされる
    stack_overflow();

    // 末尾最適化を防ぐ
    volatile::Volatile::new(0).read();
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
