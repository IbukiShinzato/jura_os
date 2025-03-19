#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(jura_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

// allocクレートを有効化
extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;

entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    use jura_os::allocator;
    use jura_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::VirtAddr;

    jura_os::init();
    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap intialization failed");

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    jura_os::test_panic_handler(&info);
}

#[test_case]
fn simple_allocation() {
    use alloc::boxed::Box;
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(43);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 43);
}

#[test_case]
fn large_vec() {
    use alloc::vec::Vec;

    let mut vec = Vec::new();
    let n = 1000;
    for i in 0..=n {
        vec.push(i)
    }
    assert_eq!(vec.iter().sum::<u64>(), (n * (n + 1)) / 2);
}

#[test_case]
fn many_boxes() {
    use alloc::boxed::Box;
    use jura_os::allocator::HEAP_SIZE;

    for i in 0..HEAP_SIZE {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
}

#[test_case]
fn many_boxes_long_lived() {
    use alloc::boxed::Box;
    let long_lived = Box::new(1);
    for i in 0..jura_os::allocator::HEAP_SIZE {
        // self.allocationsが1増えて1減るが、実際には使用できるメモリが左にずれていきいずれ使えなくなる
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1);
}
