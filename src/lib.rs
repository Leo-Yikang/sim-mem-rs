//! # 大模型训练内存模拟器 (sim-mem-rs)
//! 
//! 本项目是一个面向大模型训练的内存模拟器，使用离散事件仿真（DES）技术，
//! 用于评估不同内存分配策略在大模型训练场景下的性能表现。
//! 
//! ## 项目结构
//! 
//! - `engine`: 离散事件仿真引擎，负责时间推进和事件调度
//! - `memory`: 内存分配器实现，包括连续分配（Naive）和分页分配（Paged）
//! - `workload`: 工作负载生成器，模拟大模型训练请求
//! - `metrics`: 性能指标收集和分析
//! - `visualization`: 可视化工具，生成性能对比图表
//! 
//! ## 核心概念
//! 
//! ### 离散事件仿真（DES）
//! 系统状态仅在事件发生时改变，通过事件队列管理时间推进。
//! 
//! ### 内存分配策略
//! 1. **连续分配（Naive）**: 类似传统内存分配，需要连续内存空间
//! 2. **分页分配（Paged）**: 将内存划分为固定大小的页，支持非连续分配
//! 
//! ## 使用示例
//! 
//! ```rust
//! use sim_mem_rs::engine::SimulationEngine;
//! use sim_mem_rs::memory::{NaiveAllocator, PagedAllocator};
//! use sim_mem_rs::workload::WorkloadGenerator;
//! 
//! // 创建仿真引擎
//! let mut engine = SimulationEngine::new(1000); // 1000个时间单位
//! 
//! // 创建内存分配器
//! let allocator = Box::new(NaiveAllocator::new(1024)); // 1024单位内存
//! 
//! // 生成工作负载
//! let workload = WorkloadGenerator::new(100, 10, 50);
//! 
//! // 运行仿真
//! engine.run(allocator, workload);
//! ```

pub mod engine;
pub mod memory;
pub mod workload;
pub mod metrics;

// 重新导出主要类型
pub use engine::SimulationEngine;
pub use memory::{Allocator, MemoryBlock, AllocatorStats};
pub use workload::{WorkloadGenerator, Request};
pub use metrics::{SimulationMetrics, PerformanceReport};

/// 仿真配置
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// 仿真总时长（时间单位）
    pub duration: u64,
    /// 内存总大小（内存单位）
    pub memory_size: usize,
    /// 请求数量
    pub num_requests: usize,
    /// 请求平均生命周期
    pub avg_lifetime: u64,
    /// 内存请求平均大小
    pub avg_memory_size: usize,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            duration: 1000,
            memory_size: 1024,
            num_requests: 100,
            avg_lifetime: 50,
            avg_memory_size: 10,
        }
    }
}

/// 仿真结果
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// 使用的分配器名称
    pub allocator_name: String,
    /// 性能指标
    pub metrics: SimulationMetrics,
    /// 内存分配统计
    pub allocator_stats: AllocatorStats,
}

/// 运行单个仿真实验
pub fn run_simulation(config: SimulationConfig, allocator: Box<dyn Allocator>) -> SimulationResult {
    let mut engine = SimulationEngine::new(config.duration);
    let workload = WorkloadGenerator::new(
        config.num_requests,
        config.avg_lifetime,
        config.avg_memory_size,
    );
    
    engine.run(allocator, workload);
    
    SimulationResult {
        allocator_name: engine.allocator_name().to_string(),
        metrics: engine.metrics().clone(),
        allocator_stats: engine.allocator_stats().clone(),
    }
}

/// 运行基准测试，比较不同分配器性能
pub fn run_benchmark(config: SimulationConfig) -> Vec<SimulationResult> {
    let allocators: Vec<Box<dyn Allocator>> = vec![
        Box::new(memory::NaiveAllocator::new(config.memory_size)),
        Box::new(memory::PagedAllocator::new(config.memory_size, 64)), // 64单位页大小
    ];
    
    allocators
        .into_iter()
        .map(|allocator| run_simulation(config.clone(), allocator))
        .collect()
}