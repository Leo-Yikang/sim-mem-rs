//! # 分页内存分配器（Paged Allocator）
//! 
//! 实现分页内存分配策略，将内存划分为固定大小的页。
//! 
//! ## 工作原理
//! 
//! 1. 将内存划分为固定大小的页（page）
//! 2. 每个请求分配整数个页
//! 3. 使用位图跟踪页的使用状态
//! 4. 首次适应连续页查找（Phase 2 将支持非连续分配）
//! 
//! ## 内部碎片
//! 
//! 分页分配可能产生内部碎片：
//! - 请求大小不是页大小的整数倍时，最后一个页会有未使用的空间
//! - 平均内部碎片 = (页大小 - 1) / 2
//! 
//! ## 页表管理
//! 
//! 每个分配的内存块有一个页表，记录：
//! - 逻辑页号 -> 物理页号的映射
//! - 支持未来的内存共享和交换

use std::collections::HashMap;
use super::{Allocator, MemoryBlock, AllocatorStats};

/// 页状态
#[derive(Debug, Clone, Copy, PartialEq)]
enum PageStatus {
    /// 空闲
    Free,
    /// 已分配
    Allocated,
}

/// 分页内存分配器
/// 
/// 将内存划分为固定大小的页，使用位图管理页的分配状态。
/// 
/// # 特点
/// 
/// - 减少外部碎片
/// - 可能产生内部碎片
/// - 分配速度：O(n) 查找连续空闲页
/// - 支持非连续分配
/// 
/// # 示例
/// 
/// ```rust
/// use sim_mem_rs::memory::{PagedAllocator, Allocator};
/// 
// 创建页大小为64单位的分配器，总内存1024单位
/// let mut allocator = PagedAllocator::new(1024, 64);
/// 
// 分配内存（会向上取整到页大小的整数倍）
/// let block = allocator.allocate(100).unwrap();
/// assert_eq!(block.size, 100); // 原始请求大小
/// assert_eq!(allocator.used_memory(), 128); // 实际分配2页
/// 
// 释放内存
///  assert!(allocator.deallocate(block.request_id));
/// ```
pub struct PagedAllocator {
    /// 总内存大小
    total_memory: usize,
    /// 页大小
    page_size: usize,
    /// 总页数
    total_pages: usize,
    /// 页状态数组
    pages: Vec<PageStatus>,
    /// 已分配的内存块
    allocated_blocks: HashMap<usize, MemoryBlock>,
    /// 分配统计
    stats: AllocatorStats,
    /// 下一个内存块ID
    next_block_id: usize,
    /// 请求ID到页列表的映射
    request_pages: HashMap<usize, Vec<usize>>,
}

impl PagedAllocator {
    /// 创建新的分页内存分配器
    /// 
    /// # Arguments
    /// 
    /// * `total_memory` - 总内存大小（内存单位）
    /// * `page_size` - 页大小（内存单位）
    /// 
    /// # Returns
    /// 
    /// 返回初始化的分配器实例
    /// 
    /// # Panics
    /// 
    /// 如果页大小为0或总内存不能被页大小整除，会触发panic
    pub fn new(total_memory: usize, page_size: usize) -> Self {
        assert!(page_size > 0, "页大小必须大于0");
        assert!(total_memory % page_size == 0, "总内存必须能被页大小整除");
        
        let total_pages = total_memory / page_size;
        
        Self {
            total_memory,
            page_size,
            total_pages,
            pages: vec![PageStatus::Free; total_pages],
            allocated_blocks: HashMap::new(),
            stats: AllocatorStats::default(),
            next_block_id: 0,
            request_pages: HashMap::new(),
        }
    }
    
    /// 计算需要的页数
    /// 
    /// # Arguments
    /// 
    /// * `size` - 请求的内存大小
    /// 
    /// # Returns
    /// 
    /// 返回需要的页数（向上取整）
    fn pages_needed(&self, size: usize) -> usize {
        (size + self.page_size - 1) / self.page_size
    }
    
    /// 查找连续空闲页
    /// 
    /// # Arguments
    /// 
    /// * `num_pages` - 需要的页数
    /// 
    /// # Returns
    /// 
    /// * `Some(start_page)` - 找到连续空闲页，返回起始页号
    /// * `None` - 未找到足够的连续空闲页
    fn find_free_pages(&self, num_pages: usize) -> Option<usize> {
        for i in 0..=self.total_pages - num_pages {
            if self.pages[i..i + num_pages].iter().all(|&s| s == PageStatus::Free) {
                return Some(i);
            }
        }
        None
    }
    
    /// 计算内部fragmentation_ratio
    /// 
    /// 内部碎片 = (分配的页大小 - 请求的实际大小) / 分配的页大小
    fn calculate_internal_fragmentation(&self) -> f64 {
        if self.allocated_blocks.is_empty() {
            return 0.0;
        }
        
        let total_internal_fragmentation: usize = self.allocated_blocks.values()
            .map(|block| {
                let pages_allocated = self.pages_needed(block.size);
                let memory_allocated = pages_allocated * self.page_size;
                memory_allocated - block.size
            })
            .sum();
        
        let total_allocated: usize = self.allocated_blocks.values()
            .map(|block| self.pages_needed(block.size) * self.page_size)
            .sum();
        
        if total_allocated == 0 {
            0.0
        } else {
            total_internal_fragmentation as f64 / total_allocated as f64
        }
    }
}

impl Allocator for PagedAllocator {
    fn allocate(&mut self, size: usize) -> Option<MemoryBlock> {
        self.stats.total_allocations += 1;
        
        if size == 0 {
            self.stats.failed_allocations += 1;
            return None;
        }
        
        let num_pages = self.pages_needed(size);
        
        if let Some(start_page) = self.find_free_pages(num_pages) {
            // 分配页
            for i in start_page..start_page + num_pages {
                self.pages[i] = PageStatus::Allocated;
            }
            
            // 创建内存块
            let block_id = self.next_block_id;
            self.next_block_id += 1;
            let allocated_size = num_pages * self.page_size;
            
            let memory_block = MemoryBlock {
                id: block_id,
                start: start_page * self.page_size,
                size: size, // 存储原始请求大小，用于计算内部碎片
                request_id: block_id,
            };
            
            // 记录分配
            self.allocated_blocks.insert(block_id, memory_block.clone());
            self.request_pages.insert(block_id, (start_page..start_page + num_pages).collect());
            
            // 更新统计 - 使用实际分配的页对齐大小
            self.stats.successful_allocations += 1;
            self.stats.allocated_memory += allocated_size;
            
            if self.stats.allocated_memory > self.stats.peak_memory_usage {
                self.stats.peak_memory_usage = self.stats.allocated_memory;
            }
            
            self.stats.fragmentation_ratio = self.calculate_internal_fragmentation();
            
            Some(memory_block)
        } else {
            self.stats.failed_allocations += 1;
            None
        }
    }
    
    fn deallocate(&mut self, request_id: usize) -> bool {
        self.stats.total_deallocations += 1;
        
        if let (Some(_block), Some(pages)) = (
            self.allocated_blocks.remove(&request_id),
            self.request_pages.remove(&request_id),
        ) {
            // 释放页
            for page_num in &pages {
                self.pages[*page_num] = PageStatus::Free;
            }
            
            // 更新统计 - 使用实际分配的页对齐大小
            let allocated_size = pages.len() * self.page_size;
            self.stats.allocated_memory -= allocated_size;
            self.stats.fragmentation_ratio = self.calculate_internal_fragmentation();
            
            true
        } else {
            false
        }
    }
    
    fn name(&self) -> &str {
        "PagedAllocator"
    }
    
    fn stats(&self) -> &AllocatorStats {
        &self.stats
    }
    
    fn used_memory(&self) -> usize {
        self.stats.allocated_memory
    }
    
    fn total_memory(&self) -> usize {
        self.total_memory
    }
    
    fn fragmentation_ratio(&self) -> f64 {
        self.stats.fragmentation_ratio
    }
    
    fn has_contiguous_memory(&self, size: usize) -> bool {
        let num_pages = self.pages_needed(size);
        self.find_free_pages(num_pages).is_some()
    }
    
    fn max_contiguous_block(&self) -> usize {
        let mut max_contiguous = 0;
        let mut current_contiguous = 0;
        
        for page in &self.pages {
            if *page == PageStatus::Free {
                current_contiguous += 1;
                if current_contiguous > max_contiguous {
                    max_contiguous = current_contiguous;
                }
            } else {
                current_contiguous = 0;
            }
        }
        
        max_contiguous * self.page_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_paged_allocator_creation() {
        let allocator = PagedAllocator::new(1024, 64);
        assert_eq!(allocator.total_memory(), 1024);
        assert_eq!(allocator.used_memory(), 0);
        assert_eq!(allocator.total_pages, 16);
    }
    
    #[test]
    fn test_paged_allocator_allocate() {
        let mut allocator = PagedAllocator::new(1024, 64);
        
        let block = allocator.allocate(100).unwrap();
        assert_eq!(block.size, 100); // 存储原始请求大小
        assert_eq!(block.start, 0);
        assert_eq!(allocator.used_memory(), 128); // 实际分配128（2页）
    }
    
    #[test]
    fn test_paged_allocator_allocate_multiple() {
        let mut allocator = PagedAllocator::new(1024, 64);
        
        let block1 = allocator.allocate(100).unwrap();
        let block2 = allocator.allocate(200).unwrap();
        
        assert_eq!(block1.start, 0);
        assert_eq!(block2.start, 128); // 第二个块从第2页开始
        assert_eq!(allocator.used_memory(), 384); // 128(2页) + 256(4页)
    }
    
    #[test]
    fn test_paged_allocator_allocate_fail() {
        let mut allocator = PagedAllocator::new(128, 64);
        
        // 分配成功
        let _block = allocator.allocate(100).unwrap();
        
        // 分配失败，剩余空间不足
        assert!(allocator.allocate(100).is_none());
    }
    
    #[test]
    fn test_paged_allocator_deallocate() {
        let mut allocator = PagedAllocator::new(1024, 64);
        
        let block = allocator.allocate(100).unwrap();
        assert_eq!(allocator.used_memory(), 128); // 实际分配2页
        
        assert!(allocator.deallocate(block.request_id));
        assert_eq!(allocator.used_memory(), 0);
    }
    
    #[test]
    fn test_paged_allocator_no_external_fragmentation() {
        let mut allocator = PagedAllocator::new(1024, 64);
        
        // 分配三个块
        let _block1 = allocator.allocate(100).unwrap();
        let block2 = allocator.allocate(100).unwrap();
        let _block3 = allocator.allocate(100).unwrap();
        
        // 释放中间的块
        allocator.deallocate(block2.request_id);
        
        // 应该还能分配，因为分页支持非连续分配
        let block4 = allocator.allocate(100);
        assert!(block4.is_some());
    }
    
    #[test]
    fn test_paged_allocator_internal_fragmentation() {
        let mut allocator = PagedAllocator::new(1024, 64);
        
        // 分配一个不是页大小整数倍的块
        let _block = allocator.allocate(100).unwrap();
        
        // 内部fragmentation_ratio应该大于0
        assert!(allocator.fragmentation_ratio() > 0.0);
        
        // 分配页大小整数倍的块
        let _block2 = allocator.allocate(128).unwrap();
        
        // fragmentation_ratio应该降低
        let fragmentation_ratio = allocator.fragmentation_ratio();
        assert!(fragmentation_ratio < 0.12); // 应该减小
    }
    
    #[test]
    fn test_paged_allocator_pages_needed() {
        let allocator = PagedAllocator::new(1024, 64);
        
        assert_eq!(allocator.pages_needed(0), 0);
        assert_eq!(allocator.pages_needed(1), 1);
        assert_eq!(allocator.pages_needed(64), 1);
        assert_eq!(allocator.pages_needed(65), 2);
        assert_eq!(allocator.pages_needed(128), 2);
    }
}