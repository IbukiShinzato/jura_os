use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::structures::paging::PhysFrame;
use x86_64::PhysAddr;
use x86_64::{
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, Size4KiB},
    VirtAddr,
};

// 常にNoneを返すFrameAllocator
pub struct EmptyFrameAllocator;
unsafe impl FrameAllocator<Size4KiB> for EmptyFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        None
    }
}

// ブートローダのメモリマップから、使用可能なフレームを返すBootInfoFrameAllocator
#[allow(dead_code)]
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// 渡されたメモリマップからFrameAllocatorを作る。
    ///
    /// この関数はunsafeである：呼び出し元は渡された
    /// メモリマップが有効であることを保証しなければ
    /// ならない。特に、`USABLE`なフレームは実際に
    /// 未使用でなくてはならない。
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        Self {
            memory_map,
            next: 0,
        }
    }

    #[allow(dead_code)]
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // メモリマップからusableな領域を得る
        let regions = self.memory_map.iter();
        let usable_regions = regions.filter(|r| r.region_type == MemoryRegionType::Usable);
        // それぞれの領域をアドレス範囲にmapで変換する
        let addr_ranges = usable_regions.map(|r| r.range.start_addr()..r.range.end_addr());
        // フレームの開始アドレスのイテレータへと変換する
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // 開始アドレスから`PhysFrame`型を作る
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

// Offsetを使用して全ての仮想アドレスをマッピングしている
// <'static> よりカーネルが実行している間ずっと有効である
// ここでoffsetを使用したpegetableを生成することによってoffsetを気にしないでメモリ操作が可能になる
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table_frame = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table_frame, physical_memory_offset)
}

/// 有効なレベル4テーブルへの可変参照を返す。
///
/// この関数はunsafeである：全物理メモリが、渡された
/// `physical_memory_offset`（だけずらしたうえ）で
/// 仮想メモリへとマップされていることを呼び出し元が
/// 保証しなければならない。また、`&mut`参照が複数の
/// 名称を持つこと (mutable aliasingといい、動作が未定義)
/// につながるため、この関数は一度しか呼び出してはならない。
/// &mut PageTableなのはページテーブルの中身を書き換える必要があるため
// unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    // 使用可能なページテーブルの物理メモリ上のアドレスが格納されている。
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();

    // virtual_address = physical_address + offset
    let virt = physical_memory_offset + phys.as_u64();

    // 生ポインタ　変換　 => unsafeでの処理が必要
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

/// 与えられた仮想アドレスを対応する物理アドレスに変換し、
/// そのアドレスがマップされていないなら`None`を返す。
///
/// この関数はunsafeである。なぜなら、呼び出し元は全物理メモリが与えられた
/// `physical_memory_offset`（だけずらした上）でマップされていることを
/// 保証しなくてはならないからである。
pub unsafe fn translate_addr(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    translate_addr_inner(addr, physical_memory_offset)
}

/// `translate_addr`により呼び出される非公開関数。
///
/// Rustはunsafeな関数の全体をunsafeブロックとして扱ってしまうので、
/// unsafeの範囲を絞るためにこの関数はunsafeにしていない。
/// この関数をモジュール外から呼び出すときは、
/// unsafeな関数`translate_addr`を使って呼び出すこと。
fn translate_addr_inner(addr: VirtAddr, physical_memory_offset: VirtAddr) -> Option<PhysAddr> {
    use x86_64::registers::control::Cr3;
    use x86_64::structures::paging::page_table::FrameError;

    // 有効なページテーブルの物理アドレスが格納されているレジスタ
    let (level_4_table_frame, _) = Cr3::read();

    // アドレスから取得されるそれぞれの層のindex
    let table_indexes = [
        addr.p4_index(),
        addr.p3_index(),
        addr.p2_index(),
        addr.p1_index(),
    ];
    let mut frame = level_4_table_frame;

    // L4 -> L3 -> L2 -> L1 -> Physcal Address
    // 複数層のページテールを辿る
    for &index in &table_indexes {
        // フレームをページテーブルの参照に変換する
        // 全ての階層でoffsetは必要
        let virt = physical_memory_offset + frame.start_address().as_u64();
        let table_ptr: *const PageTable = virt.as_ptr();
        let table = unsafe { &*table_ptr };

        // ページテーブルエントリを読んで、`frame`を更新する
        let entry = &table[index];
        frame = match entry.frame() {
            Ok(frame) => frame,
            Err(FrameError::FrameNotPresent) => return None,
            // Huge Pageにマップされている場合
            Err(FrameError::HugeFrame) => panic!("huge pages not supported"),
        };
    }

    Some(frame.start_address() + u64::from(addr.page_offset()))
}

// mapperを使用したmapping
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    // ジェネリクストレイトでありサイズは4KiB
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    // FIXME: unsafeであり、テストのためにのみ行う
    // VGAのアドレスはすでにマップ済みなので２回マップをすると未定義動作を起こす可能性あり
    let map_to_result = unsafe { mapper.map_to(page, frame, flags, frame_allocator) };

    // TLBは半透過的なのでマッピングをする度にflushが必要
    map_to_result.expect("map_to failed").flush();
}
