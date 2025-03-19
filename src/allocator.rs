#[allow(unused_imports)]
use alloc::alloc::{GlobalAlloc, Layout};
#[allow(unused_imports)]
use core::ptr::null_mut;
use fixed_size_block::FixedSizeBlockAllocator;
// use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

pub mod bump;
pub mod fixed_size_block;
pub mod linked_list;

pub struct Dummy;

// Heap領域の先頭アドレス
pub const HEAP_START: usize = 0x_4444_4444_0000;

// 100 * 1024byte = 100 * 1KiB = 100KiB
pub const HEAP_SIZE: usize = 100 * 1024;

#[global_allocator]
// グローバルアロケーターとして登録
// static ALLOCATOR: Dummy = Dummy;
// static ALLOCATOR: LockedHeap = LockedHeap::empty();
// static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());
// static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());
static ALLOCATOR: Locked<FixedSizeBlockAllocator> = Locked::new(FixedSizeBlockAllocator::new());

// // unsafe traitを実装する際にはimplブロック全体をunsafeにする(実装自体がunsafe)
// // alloc_zeroedとreallocはデフォルトで実装済み
// // Layout構造体 {size, align(メモリの配置がどの境界(バイト数)に揃えられるか)}
// unsafe impl GlobalAlloc for Dummy {
//     // 常にnullポインタを返す
//     unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
//         // crate::println!("Dummy allocator used!");

//         // 割り当て失敗
//         null_mut()
//     }

//     unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
//         panic!("dealloc should be never called");
//     }
// }

// 4KiBのみの制限
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        // 終端は含まないので -1
        let heap_end = heap_start + HEAP_SIZE - 1u64;

        // アドレスを使用してPage型に変換
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        // 読み書き可能なフラグ
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        // flushでTLBを更新
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

// ジェネリクスで他の型でも対応可
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

// addrをalignに丸め込む
// addrがalign上にある(addr % align == 0)ならそのまま返すが、そうでなければ右に余分を持ってalign上に持ってくる
fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}
