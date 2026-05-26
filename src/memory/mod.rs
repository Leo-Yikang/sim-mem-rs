//! # 内存分配器模块
//! 
//! 本模块定义了内存分配器的接口和实现，包括：
//! - `Allocator` trait：内存分配器的统一接口
//! - `NaiveAllocator`：连续内存分配器
//! - `PagedAllocator`：分页内存分配器
//! 
//! ## 内存分配策略
//! 
//! ### 连续分配（Naive）
//! 类似传统的内存分配方式，需要连续的内存空间。
//! 优点：实现简单，分配速度快
//! 缺点：容易产生外部碎片
//! 
//! ### 分页分配（Paged）
//! 将内存划分为固定大小的页，支持非连续分配。
//! 优点：减少外部碎片，支持内存共享
//! 缺点：可能产生内部碎片，需要页表管理

/// 内存块
/// 
/// 表示一块已分配的内存区域
#[derive(Debug, Clone)]
pub struct MemoryBlock {
    /// 内存块ID
    pub id: usize,
    /// 起始地址
    pub start: usize,
    /// 大小（内存单位）
    pub size: usize,
    /// 关联的请求ID
    pub request_id: usize,
}

/// 内存分配统计信息
#[derive(Debug, Clone, Default)]
pub struct AllocatorStats {
    /// 总分配次数
    pub total_allocations: u64,
    /// 成功分配次数
    pub successful_allocations: u64,
    /// 失败分配次数
    pub failed_allocations: u64,
    /// 总释放次数
    pub total_deallocations: u64,
    /// 当前已分配内存大小
    pub allocated_memory: usize,
    /// 峰值内存使用量
    pub peak_memory_usage: usize,
    /// 内存碎片率（0.0 - 1.0）
    pub fragmentation_ratio: f64,
}

/// 内存分配器特征（trait）
/// 
/// 定义了内存分配器的统一接口，所有分配器实现都需要实现此特征。
/// 
/// # 设计原则
/// 
/// 1. **统一接口**：所有分配器提供相同的操作接口
/// 2. **可扩展性**：可以轻松添加新的分配策略
/// 3. **性能监控**：内置统计信息收集
/// 
/// # 实现示例
/// 
/// ```rust,ignore
/// use sim_mem_rs::memory::{Allocator, MemoryBlock, AllocatorStats};
// see NaiveAllocator or PagedAllocator for full implementations
/// ```
pub trait Allocator {
    /// 分配内存
    /// 
    /// # Arguments
    /// 
    /// * `size` - 请求的内存大小（内存单位）
    /// 
    /// # Returns
    /// 
    /// * `Some(MemoryBlock)` - 分配成功，返回内存块信息
    /// * `None` - 分配失败，内存不足或无法找到合适的内存块
    fn allocate(&mut self, size: usize) -> Option<MemoryBlock>;
    
    /// 释放内存
    /// 
    /// # Arguments
    /// 
    /// * `request_id` - 要释放的请求ID
    /// 
    /// # Returns
    /// 
    /// * `true` - 释放成功
    /// * `false` - 释放失败（未找到对应的内存块）
    fn deallocate(&mut self, request_id: usize) -> bool;
    
    /// 获取分配器名称
    fn name(&self) -> &str;
    
    /// 获取分配统计信息
    fn stats(&self) -> &AllocatorStats;
    
    /// 获取当前内存使用量
    fn used_memory(&self) -> usize;
    
    /// 获取总内存大小
    fn total_memory(&self) -> usize;
    
    /// 获取内存碎片率
    fn fragmentation_ratio(&self) -> f64;
    
    /// 检查是否有足够的连续内存
    fn has_contiguous_memory(&self, size: usize) -> bool;
    
    /// 获取最大连续内存块大小
    fn max_contiguous_block(&self) -> usize;
}

mod naive;
mod paged;

pub use naive::NaiveAllocator;
pub use paged::PagedAllocator;