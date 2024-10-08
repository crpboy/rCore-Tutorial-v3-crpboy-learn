//! The global allocator
//! 用于分配堆空间
//! 其中HEAP_ALLOCATOR用于进行实际的空间分配
//! 而它使用的是HEAP_SPACE占据的一块内存空间
//! HEAP_SPACE是一段连续固定的内存，专门用于保存堆中数据
//! 大小为0x30_0000，由config定义

use crate::config::KERNEL_HEAP_SIZE;
use buddy_system_allocator::LockedHeap;

/// 通过这个global_allocator来进行堆对象的识别
/// 这样编译器就会使用HEAP_ALLOCATOR来维护堆上数据了
#[global_allocator]
/// heap allocator instance
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::new();

#[alloc_error_handler]
/// panic when heap allocation error occurs
pub fn heap_alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

/// heap space ([u8; KERNEL_HEAP_SIZE])
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// initiate heap allocator
pub fn init_heap() {
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
        // lock: 创建互斥锁，防止被其他线程修改
        // init: 初始化 其中 start = HEAP_SPACE size = KERNEL_HEAP_SIZE
    }
}

#[allow(unused)]
pub fn heap_test() {
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let bss_range = sbss as usize..ebss as usize;
    let a = Box::new(5);
    assert_eq!(*a, 5);
    assert!(bss_range.contains(&(a.as_ref() as *const _ as usize)));
    drop(a);
    let mut v: Vec<usize> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    for (i, val) in v.iter().take(500).enumerate() {
        assert_eq!(*val, i);
    }
    assert!(bss_range.contains(&(v.as_ptr() as usize)));
    drop(v);
    println!("heap_test passed!");
}
