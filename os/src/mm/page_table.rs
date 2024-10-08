//! Implementation of [`PageTableEntry`] and [`PageTable`].
//! 这里用于定义页表项pte内部的实现
//! 并实现整个页表page table的访问
//! 一个page_table是一个按照va高位的vpn为键值构建的，
//! 以下一级pte为中间点权值
//! 以pfn为叶节点权值的字典树
//!
//! 一个pte指向了一个携带了标志位的物理页帧
//! pte的高44位是物理页帧的高位，低10位是各类标志位

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

bitflags! {
    /// page table entry flags
    /// 这些是pte低10位的标志位
    pub struct PTEFlags: u8 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// page table entry structure
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// page table structure
/// page table里保存了所有frame tracker的信息
/// 并给定了进入mmu查找时的根节点ppn为root_ppn
/// 这里并没有直接使用frames，是基于RAII的思想，
/// 将frametracker的生命周期通过这个vec绑定在结构体上
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    /// 与下面的基本相同
    /// 唯一不同的是如果发现!valid
    /// 则会aclloc一个frame并加入到当前结构体中
    /// 这样就可以保证 需要访问到的路径上的节点一定是valid
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            // 注意这里，i=2就是叶节点的情况，但是这里我们通过break跳过了!valid时的flag=V创建过程
            // 这样达成的效果是，到目标节点路径上的所有点都被创建为valid
            // 然后目标节点，如果没有被alloc过，那就为!valid
            // 可以看后面的map中的assert，就可以理解了
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 通过vpn找到指向pa的pte
    /// vpn 3*9 bit index 这三个index就是页表索引
    /// 首先通过保存的根ppn，转换为对应的pa
    /// 然后通过index加上偏移量，找到对应的下一级pte
    /// 如果找到最后一级，则返回，否则递归地使用下一级pte.ppn检索
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 进行一次va -> pa的映射过程
    /// 由于find_create过程保证了路径上的valid + 目标节点的!valid所以可以assert
    /// 这种处理方式，比起直接插入，多的这个assert可以保证不重复
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        // valid判定：如果已经分配过则panic
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        // 置为valid
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    /// 解除一个vpn的映射
    /// 基本与上面的map过程相同，找到目标节点并判断是否可以释放即可
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    /// vpn -> pte
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    /// 生成一个可以用于设置csr.satp的数据
    /// 保存的是标志sv39的flag + 根物理页帧入口地址
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// translate a pointer to a mutable u8 Vec through page table
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}
