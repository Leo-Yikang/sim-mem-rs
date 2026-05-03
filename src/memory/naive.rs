//! # 连续内存分配器（Naive Allocator）
//! 
//! 实现简单的连续内存分配策略，类似传统的内存分配方式。
//! 
//! ## 工作原理
//! 
//! 1. 维护一个空闲内存块列表
//! 2. 分配时找到第一个足够大的连续空闲块（首次适应算法）
//! 3. 释放时将内存块标记为空闲，并合并相邻的空闲块
//! 
//! ## 碎片问题
//! 
//! 连续分配容易产生外部碎片：
//! - 当分配和释放频繁交替时，内存中会出现许多小的空闲块
//! - 这些空闲块总和可能足够，但单个块不满足请求大小
//! - 导致分配失败，降低内存利用率

use std::collections::HashMap;
use super::{Allocator, MemoryBlock, AllocatorStats};

/// 空闲内存块
#[derive(Debug, Clone)]
struct FreeBlock {
    /// 起始地址
    start: usize,
    /// 大小
    size: usize,
}

/// 连续内存分配器
/// 
/// 使用首次适应算法进行连续内存分配。
/// 
/// # 特点
/// 
/// - 分配速度快：O(n) 时间复杂度
/// - 容易产生外部碎片
/// - 实现简单，适合作为基准比较
/// 
/// # 示例
/// 
/// ```rust
/// use sim_mem_rs::memory::{NaiveAllocator, Allocator};
/// 
/// let mut allocator = NaiveAllocator::new(1024);
/// 
/// // 分配内存
/// let block = allocator.allocate(100).unwrap();
/// assert_eq!(block.size, 100);
/// 
/// // 释放内存
/// assert!(allocator.deallocate(block.request_id));
/// ```
pub struct NaiveAllocator {
    /// 总内存大小
    total_memory: usize,
    /// 空闲内存块列表
    free_blocks: Vec<FreeBlock>,
    /// 已分配的内存块
    allocated_blocks: HashMap<usize, MemoryBlock>,
    /// 分配统计
    stats: AllocatorStats,
    /// 下一个内存块ID
    next_block_id: usize,
}

impl NaiveAllocator {
    /// 创建新的连续内存分配器
    /// 
    /// # Arguments
    /// 
    /// * `total_memory` - 总内存大小（内存单位）
    /// 
    /// # Returns
    /// 
    /// 返回初始化的分配器实例，所有内存都是空闲的
    pub fn new(total_memory: usize) -> Self {
        Self {
            total_memory,
            free_blocks: vec![FreeBlock {
                start: 0,
                size: total_memory,
            }],
            allocated_blocks: HashMap::new(),
            stats: AllocatorStats::default(),
            next_block_id: 0,
        }
    }
    
    /// 合并相邻的空闲内存块
    /// 
    /// 遍历空闲块列表，合并相邻的块以减少碎片。
    /// 时间复杂度：O(n log n) 由于排序
    fn merge_free_blocks(&mut self) {
        if self.free_blocks.is_empty() {
            return;
        }
        
        // 按起始地址排序
        self.free_blocks.sort_by_key(|block| block.start);
        
        let mut merged = Vec::new();
        let mut current = self.free_blocks[0].clone();
        
        for i in 1..self.free_blocks.len() {
            let next = &self.free_blocks[i];
            
            // 检查是否相邻
            if current.start + current.size == next.start {
                // 合并
                current.size += next.size;
            } else {
                // 不相邻，保存当前块，开始新块
                merged.push(current);
                current = next.clone();
            }
        }
        
        merged.push(current);
        self.free_blocks = merged;
    }
    
    /// 计算外部碎片率
    /// 
    /// 碎片率 = 1 - (最大空闲块 / 总空闲内存)
    /// 
    /// 当碎片率接近1时，表示内存碎片严重
    fn calculate_fragmentation(&self) -> f64 {
        let total_free: usize = self.free_blocks.iter().map(|b| b.size).sum();
        
        if total_free == 0 {
            return 0.0;
        }
        
        let max_free = self.free_blocks.iter().map(|b| b.size).max().unwrap_or(0);
        
        1.0 - (max_free as f64 / total_free as f64)
    }
}

impl Allocator for NaiveAllocator {
    fn allocate(&mut self, size: usize) -> Option<MemoryBlock> {
        self.stats.total_allocations += 1;
        
        // 首次适应算法：找到第一个足够大的空闲块
        let block_index = self.free_blocks.iter().position(|block| block.size >= size);
        
        if let Some(index) = block_index {
            let free_block = &self.free_blocks[index];
            let start = free_block.start;
            let remaining = free_block.size - size;
            
            // 创建分配的内存块
            let block_id = self.next_block_id;
            self.next_block_id += 1;
            
            let memory_block = MemoryBlock {
                id: block_id,
                start,
                size,
                request_id: block_id, // 简化：使用block_id作为request_id
            };
            
            // 更新空闲块列表
            if remaining > 0 {
                // 分割空闲块
                self.free_blocks[index] = FreeBlock {
                    start: start + size,
                    size: remaining,
                };
            } else {
                // 完全使用，移除空闲块
                self.free_blocks.remove(index);
            }
            
            // 记录分配
            self.allocated_blocks.insert(block_id, memory_block.clone());
            self.stats.successful_allocations += 1;
            self.stats.allocated_memory += size;
            
            // 更新峰值内存使用量
            if self.stats.allocated_memory > self.stats.peak_memory_usage {
                self.stats.peak_memory_usage = self.stats.allocated_memory;
            }
            
            // 更新碎片率
            self.stats.fragmentation_ratio = self.calculate_fragmentation();
            
            Some(memory_block)
        } else {
            // 分配失败
            self.stats.failed_allocations += 1;
            None
        }
    }
    
    fn deallocate(&mut self, request_id: usize) -> bool {
        self.stats.total_deallocations += 1;
        
        if let Some(block) = self.allocated_blocks.remove(&request_id) {
            // 添加到空闲块列表
            self.free_blocks.push(FreeBlock {
                start: block.start,
                size: block.size,
            });
            
            // 更新统计
            self.stats.allocated_memory -= block.size;
            
            // 合并相邻的空闲块
            self.merge_free_blocks();
            
            // 更新碎片率
            self.stats.fragmentation_ratio = self.calculate_fragmentation();
            
            true
        } else {
            false
        }
    }
    
    fn name(&self) -> &str {
        "NaiveAllocator"
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
        self.free_blocks.iter().any(|block| block.size >= size)
    }
    
    fn max_contiguous_block(&self) -> usize {
        self.free_blocks.iter().map(|block| block.size).max().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_naive_allocator_creation() {
        let allocator = NaiveAllocator::new(1024);
        assert_eq!(allocator.total_memory(), 1024);
        assert_eq!(allocator.used_memory(), 0);
        assert_eq!(allocator.max_contiguous_block(), 1024);
    }
    
    #[test]
    fn test_naive_allocator_allocate() {
        let mut allocator = NaiveAllocator::new(1024);
        
        let block = allocator.allocate(100).unwrap();
        assert_eq!(block.size, 100);
        assert_eq!(block.start, 0);
        assert_eq!(allocator.used_memory(), 100);
        assert_eq!(allocator.max_contiguous_block(), 924);
    }
    
    #[test]
    fn test_naive_allocator_allocate_multiple() {
        let mut allocator = NaiveAllocator::new(1024);
        
        let block1 = allocator.allocate(100).unwrap();
        let block2 = allocator.allocate(200).unwrap();
        
        assert_eq!(block1.start, 0);
        assert_eq!(block2.start, 100);
        assert_eq!(allocator.used_memory(), 300);
    }
    
    #[test]
    fn test_naive_allocator_allocate_fail() {
        let mut allocator = NaiveAllocator::new(100);
        
        // 分配成功
        let _block = allocator.allocate(80).unwrap();
        
        // 分配失败，剩余空间不足
        assert!(allocator.allocate(30).is_none());
    }
    
    #[test]
    fn test_naive_allocator_deallocate() {
        let mut allocator = NaiveAllocator::new(1024);
        
        let block = allocator.allocate(100).unwrap();
        assert_eq!(allocator.used_memory(), 100);
        
        assert!(allocator.deallocate(block.request_id));
        assert_eq!(allocator.used_memory(), 0);
    }
    
    #[test]
    fn test_naive_allocator_merge_free_blocks() {
        let mut allocator = NaiveAllocator::new(1000);
        
        // 分配三个块
        let block1 = allocator.allocate(100).unwrap();
        let block2 = allocator.allocate(100).unwrap();
        let block3 = allocator.allocate(100).unwrap();
        
        // 释放中间的块 - 此时空闲区域有2个: [100-200]和[300-1000]
        allocator.deallocate(block2.request_id);
        // 最大连续空闲块应该是 [300-1000] = 700
        assert_eq!(allocator.max_contiguous_block(), 700);
        
        // 释放两边的块
        allocator.deallocate(block1.request_id);
        allocator.deallocate(block3.request_id);
        
        // 所有块应该合并成一个大的空闲块
        assert_eq!(allocator.max_contiguous_block(), 1000);
        assert_eq!(allocator.used_memory(), 0);
    }
    
    #[test]
    fn test_naive_allocator_fragmentation() {
        let mut allocator = NaiveAllocator::new(1000);
        
        // 初始没有碎片
        assert_eq!(allocator.fragmentation_ratio(), 0.0);
        
        // 分配一些块
        let block1 = allocator.allocate(100).unwrap();
        let block2 = allocator.allocate(100).unwrap();
        let block3 = allocator.allocate(100).unwrap();
        
        // 释放中间的块，产生碎片
        allocator.deallocate(block2.request_id);
        
        // 碎片率应该大于0
        assert!(allocator.fragmentation_ratio() > 0.0);
    }
}