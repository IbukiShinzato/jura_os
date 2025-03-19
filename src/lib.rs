// lib.rsに移行することで他のクレートや結合テスト実行ファイルがインクルード可能になる

// 呼び出し規約が不安定 => #![feature(abi_x86_interrupt)]が必要
// abi(Application Binary Interface)
#![feature(abi_x86_interrupt)]
#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
// 関数はunstableよりfeatureを使って有効化
#![feature(alloc_error_handler)]

// 標準ライブラリの一部
extern crate alloc;

use core::panic::PanicInfo;

pub mod allocator;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;
pub mod task;
pub mod vga_buffer;

pub trait TestTable {
    fn run(&self) -> ();
}

impl<T> TestTable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

// Fn()トレイトのトレイトオブジェクト参照のスライス
pub fn test_runner(tests: &[&dyn TestTable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        // portの作成
        let mut port = Port::new(0xf4);
        // portに終了コードを書き込む
        port.write(exit_code as u32);
    }
}

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe {
        interrupts::PICS.lock().initialize();
    }
    // 割り込みコントローラからの信号を受け入れる
    // タイム割り込みのハンドラ未定義のためダブルフォルト発生
    x86_64::instructions::interrupts::enable();
}

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// `cargo test`のときのエントリポイント
#[cfg(test)]
// #[no_mangle]
// pub extern "C" fn _start() -> ! {
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    init();
    test_main();
    hlt_loop();
}

// 次の割り込みまで CPU を停止させる(hlt命令) => CPU使用時間を減らす
pub fn hlt_loop() -> ! {
    loop {
        // アセンブリ命令の薄いwrapper
        x86_64::instructions::hlt();
    }
}

// allocateのpanic時のハンドラ
#[alloc_error_handler]
fn alloc_error_hander(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout);
}
