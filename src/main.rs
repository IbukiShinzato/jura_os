#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(jura_os::test_runner)]
// testフレームワークのエントリ関数の設定
#![reexport_test_harness_main = "test_main"]

use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use jura_os::task::{executor::Executor, keyboard, Task};
use x86_64::{PhysAddr, VirtAddr};

// 型チェックをする
// _startエントリポイントを定義してくれる => #[no_mangle]も必要なくなる
entry_point!(kernel_main);

mod serial;
mod vga_buffer;

extern crate alloc;

// エントリポイント
// #[no_mangle]
// BootInfoはカーネルに渡される全ての情報を格納する
// Rustの設計上boot_infoは&'staticである
// pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use jura_os::allocator;
    use jura_os::memory::{self, BootInfoFrameAllocator};

    println!("Hello World{}", "!");

    // GDT, IDTなどの初期化
    jura_os::init();

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(physical_memory_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap intialization failed");

    #[cfg(test)]
    test_main();

    // execution
    example_mapping(boot_info);

    let mut executor = Executor::new();
    executor.spawn(Task::new(keyboard::print_keypress()));
    executor.run();
}

// この関数はpanic時に呼ばれる
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    jura_os::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    jura_os::test_panic_handler(info)
}

#[allow(dead_code)]
// page faultを起こしてみる
fn page_fault() {
    // なぜu8? -> ポインタとは指定したアドレスに含まれているデータの型であり、ポインタの型とアドレスの型は依存している。今回は8bit = 1byteの操作を行うのでu8
    // let ptr = 0xdeadbeef as *mut u8;
    let ptr = 0x206acf as *mut u8;

    // 読み込みは可能
    unsafe {
        #[allow(unused_variables)]
        let x = *ptr;
    };
    println!("read worked");

    // 書き込みは不可能、例外発生
    unsafe { *ptr = 21 };
    println!("write worked");
}

#[allow(dead_code)]
// level4 page_tableのアドレスを出力
fn look_l4_page_table_address() -> PhysAddr {
    // CR3は現在、有効なL4ページテーブルの先頭アドレスを格納している
    use x86_64::registers::control::Cr3;

    let (l4_page_table_address, _) = Cr3::read();
    println!(
        "Level 4 page table at: {:?}",
        l4_page_table_address.start_address()
    );

    l4_page_table_address.start_address()
}

#[allow(dead_code)]
fn output_l4_page_table(boot_info: &'static BootInfo) {
    use jura_os::memory::active_level_4_table;
    use x86_64::VirtAddr;

    // boot_infoからoffsetを取得
    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };

    for (i, entry) in l4_table.iter().enumerate() {
        // 空でないエントリのみを出力
        if !entry.is_unused() {
            println!(
                "L4 Entry {}: {:?}",
                i,
                entry.frame().unwrap().start_address()
            );
        }
    }
}

// OffsetPageTableを使用 => 仮想アドレスからアクセスする際に、offsetを考慮せずにアクセス可能である
#[allow(dead_code)]
fn output_l4_offset_page_table(boot_info: &'static BootInfo) {
    use jura_os::memory::init;
    use x86_64::VirtAddr;

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut l4_table = unsafe { init(physical_memory_offset) };

    for (i, entry) in l4_table.level_4_table().iter().enumerate() {
        if !entry.is_unused() {
            println!(
                "L4 Entry {}: {:?}",
                i,
                entry.frame().unwrap().start_address()
            );
        }
    }
}

#[allow(dead_code)]
fn address_translation(boot_info: &'static BootInfo) {
    #[allow(unused_imports)]
    use jura_os::memory::{init, translate_addr};
    use x86_64::structures::paging::Translate;

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);

    // ページに関して操作を行うobject
    let mapper = unsafe { init(physical_memory_offset) };

    let addresses = [
        0xb8000,
        0x201008,
        0x0100_0020_1a10,
        boot_info.physical_memory_offset,
    ];

    for &address in &addresses {
        let virt = VirtAddr::new(address);

        // // 通常のページテーブルでのアドレス変換
        // let phys = unsafe { translate_addr(virt, physical_memory_offset) };

        // OffsetPageTableでのアドレス変換
        // translate_addr()が実装済み
        let phys = mapper.translate_addr(virt);
        println!(
            "{:?} -> {:?}",
            virt,
            phys.expect("Failed to get physical_address")
        );
    }
}

#[allow(dead_code)]
fn example_mapping(boot_info: &'static BootInfo) {
    use jura_os::memory::{self, init};
    use x86_64::structures::paging::Page;

    let physical_memory_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { init(physical_memory_offset) };
    let mut frame_allocator = memory::EmptyFrameAllocator;

    // 未使用のページをマップする
    // containing_addressは指定したアドレスが含まれているページを取得
    // 0x0が成功するのはレベル1テーブルがすでに存在していたからである
    let page = Page::containing_address(VirtAddr::new(0x0));
    memory::create_example_mapping(page, &mut mapper, &mut frame_allocator);

    let page_ptr: *mut u64 = page.start_address().as_mut_ptr();

    // // 0x_f021_f077_f065_f04e = 白背景の“New!“
    // unsafe { page_ptr.offset(400).write_volatile(0x_e021_e077_e065_e04e) };

    // "Operating System" = 0x4f 0x70 0x65 0x72 0x61 0x74 0x69 0x6e 0x67 0x20 0x53 0x79 0x73 0x74 0x65 0x6d
    let (oper, atin, g_sy, stem) = (
        0x_e072_e065_e070_e04f,
        0x_e06e_e069_e074_e061,
        0x_e079_e053_e020_e067,
        0x_e06d_e065_e074_e073,
    );

    unsafe {
        page_ptr.offset(400).write_volatile(oper);
        page_ptr.offset(401).write_volatile(atin);
        page_ptr.offset(402).write_volatile(g_sy);
        page_ptr.offset(403).write_volatile(stem);
    }
}

#[allow(dead_code)]
fn checking_heap_allocation() {
    let heap_value = Box::new(21);
    println!("heap_value is {:p}", heap_value);

    let mut vec = Vec::new();
    for i in 0..500 {
        vec.push(i);
    }
    println!("vec value is {:p}", vec.as_slice());

    let reference_counted = Rc::new(vec![1, 2, 3]);
    let cloned_reference = Rc::clone(&reference_counted);
    println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );
    // reference_countdeをdrop
    core::mem::drop(reference_counted);
    println!(
        "reference count is {} now",
        Rc::strong_count(&cloned_reference)
    );
}

#[allow(dead_code)]
async fn async_number() -> u32 {
    42
}

#[allow(dead_code)]
async fn example_task() {
    let number = async_number().await;
    println!("async number: {}\n", number);
}
