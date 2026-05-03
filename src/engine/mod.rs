//! # 离散事件仿真引擎
//! 
//! 本模块实现了离散事件仿真（DES）的核心引擎，负责：
//! - 事件队列管理
//! - 时间推进
//! - 仿真状态维护
//! 
//! ## 核心概念
//! 
//! ### 事件（Event）
//! 仿真中的基本单元，包含：
//! - 触发时间
//! - 事件类型
//! - 相关数据
//! 
//! ### 事件队列
//! 使用优先队列（最小堆）管理事件，确保按时间顺序处理。
//! 
//! ### 仿真循环
//! 1. 从事件队列取出最早事件
//! 2. 更新仿真时间
//! 3. 处理事件，可能生成新事件
//! 4. 重复直到仿真结束

use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::fmt;

use crate::memory::{Allocator, AllocatorStats};
use crate::workload::{WorkloadGenerator, Request};
use crate::metrics::SimulationMetrics;

/// 事件类型
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    /// 请求到达事件
    RequestArrival,
    /// 内存分配事件
    MemoryAllocation,
    /// 内存释放事件
    MemoryDeallocation,
    /// 请求完成事件
    RequestCompletion,
}

/// 仿真事件
#[derive(Debug, Clone)]
pub struct Event {
    /// 事件触发时间
    pub time: u64,
    /// 事件类型
    pub event_type: EventType,
    /// 关联的请求ID（如果有）
    pub request_id: Option<usize>,
    /// 事件附带数据
    pub data: Option<EventData>,
}

/// 事件附带数据
#[derive(Debug, Clone)]
pub enum EventData {
    /// 内存请求大小
    MemorySize(usize),
    /// 请求生命周期
    Lifetime(u64),
    /// 错误信息
    Error(String),
}

/// 事件排序实现（按时间升序）
impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // 注意：BinaryHeap是最大堆，所以我们反转比较顺序
        other.time.cmp(&self.time)
            .then_with(|| other.event_type.cmp(&self.event_type))
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.event_type == other.event_type
    }
}

impl Eq for Event {}

/// 仿真引擎状态
#[derive(Debug, Clone, PartialEq)]
pub enum SimulationState {
    /// 未开始
    NotStarted,
    /// 运行中
    Running,
    /// 已暂停
    Paused,
    /// 已完成
    Completed,
    /// 出错
    Error(String),
}

impl fmt::Display for SimulationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimulationState::NotStarted => write!(f, "未开始"),
            SimulationState::Running => write!(f, "运行中"),
            SimulationState::Paused => write!(f, "已暂停"),
            SimulationState::Completed => write!(f, "已完成"),
            SimulationState::Error(msg) => write!(f, "出错: {}", msg),
        }
    }
}

/// 离散事件仿真引擎
/// 
/// 负责管理事件队列、推进仿真时间、处理事件。
/// 
/// # 示例
/// 
/// ```rust
/// use sim_mem_rs::engine::SimulationEngine;
/// use sim_mem_rs::memory::NaiveAllocator;
/// use sim_mem_rs::workload::WorkloadGenerator;
/// 
/// let mut engine = SimulationEngine::new(1000);
/// let allocator = Box::new(NaiveAllocator::new(1024));
/// let workload = WorkloadGenerator::new(100, 10, 50);
/// 
/// engine.run(allocator, workload);
/// ```
pub struct SimulationEngine {
    /// 当前仿真时间
    current_time: u64,
    /// 仿真总时长
    duration: u64,
    /// 事件队列（最小堆）
    event_queue: BinaryHeap<Event>,
    /// 仿真状态
    state: SimulationState,
    /// 性能指标收集器
    metrics: SimulationMetrics,
    /// 当前活跃的请求数量
    active_requests: usize,
    /// 已完成的请求数量
    completed_requests: usize,
    /// 请求ID计数器
    next_request_id: usize,
}

impl SimulationEngine {
    /// 创建新的仿真引擎
    /// 
    /// # Arguments
    /// 
    /// * `duration` - 仿真总时长（时间单位）
    /// 
    /// # Returns
    /// 
    /// 返回初始化的仿真引擎实例
    pub fn new(duration: u64) -> Self {
        Self {
            current_time: 0,
            duration,
            event_queue: BinaryHeap::new(),
            state: SimulationState::NotStarted,
            metrics: SimulationMetrics::new(),
            active_requests: 0,
            completed_requests: 0,
            next_request_id: 0,
        }
    }
    
    /// 添加事件到队列
    /// 
    /// # Arguments
    /// 
    /// * `event` - 要添加的事件
    pub fn add_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }
    
    /// 获取当前仿真时间
    pub fn current_time(&self) -> u64 {
        self.current_time
    }
    
    /// 获取仿真状态
    pub fn state(&self) -> &SimulationState {
        &self.state
    }
    
    /// 获取性能指标
    pub fn metrics(&self) -> &SimulationMetrics {
        &self.metrics
    }
    
    /// 获取分配器统计信息
    pub fn allocator_stats(&self) -> &AllocatorStats {
        // 这里需要从分配器获取，暂时返回空统计
        // 实际实现会在run方法中更新
        &self.metrics.allocator_stats
    }
    
    /// 获取分配器名称
    pub fn allocator_name(&self) -> &str {
        &self.metrics.allocator_name
    }
    
    /// 运行仿真
    /// 
    /// # Arguments
    /// 
    /// * `allocator` - 内存分配器实现
    /// * `workload` - 工作负载生成器
    /// 
    /// # Panics
    /// 
    /// 如果仿真已经在运行，会触发panic
    pub fn run(&mut self, mut allocator: Box<dyn Allocator>, mut workload: WorkloadGenerator) {
        if self.state == SimulationState::Running {
            panic!("仿真已经在运行中");
        }
        
        self.state = SimulationState::Running;
        self.metrics.allocator_name = allocator.name().to_string();
        
        // 生成所有在仿真时长内的请求事件
        while let Some(request) = workload.next_request(self.duration) {
            let event = Event {
                time: request.arrival_time,
                event_type: EventType::RequestArrival,
                request_id: Some(request.id),
                data: Some(EventData::MemorySize(request.memory_size)),
            };
            self.add_event(event);
        }
        
        // 主仿真循环
        while self.current_time < self.duration {
            if let Some(event) = self.event_queue.pop() {
                // 更新仿真时间
                self.current_time = event.time;
                
                // 处理事件
                self.process_event(event, &mut allocator, &mut workload);
                
                // 更新指标
                self.metrics.record_time_step(self.current_time, &*allocator);
            } else {
                // 没有更多事件，仿真结束
                break;
            }
        }
        
        self.state = SimulationState::Completed;
        self.metrics.finalize();
    }
    
    /// 处理单个事件
    /// 
    /// # Arguments
    /// 
    /// * `event` - 要处理的事件
    /// * `allocator` - 内存分配器
    /// * `workload` - 工作负载生成器
    fn process_event(
        &mut self,
        event: Event,
        allocator: &mut Box<dyn Allocator>,
        workload: &mut WorkloadGenerator,
    ) {
        match event.event_type {
            EventType::RequestArrival => {
                self.handle_request_arrival(event, allocator, workload);
            }
            EventType::MemoryAllocation => {
                self.handle_memory_allocation(event, allocator);
            }
            EventType::MemoryDeallocation => {
                self.handle_memory_deallocation(event, allocator);
            }
            EventType::RequestCompletion => {
                self.handle_request_completion(event, allocator);
            }
        }
    }
    
    /// 处理请求到达事件
    fn handle_request_arrival(
        &mut self,
        event: Event,
        allocator: &mut Box<dyn Allocator>,
        workload: &mut WorkloadGenerator,
    ) {
        let request_id = event.request_id.unwrap();
        let memory_size = match event.data {
            Some(EventData::MemorySize(size)) => size,
            _ => panic!("请求到达事件缺少内存大小信息"),
        };
        
        // 尝试分配内存
        if let Some(block) = allocator.allocate(memory_size) {
            // 分配成功
            self.active_requests += 1;
            self.metrics.record_allocation(true, memory_size);
            
            // 计算请求生命周期
            let lifetime = workload.generate_lifetime();
            
            // 添加内存释放事件 — 使用 allocator 返回的 block.id 作为释放 key，
            // 而非 workload 的 request_id。因为当部分分配失败时，二者不再一致。
            let deallocation_event = Event {
                time: self.current_time + lifetime,
                event_type: EventType::MemoryDeallocation,
                request_id: Some(block.id),
                data: Some(EventData::MemorySize(memory_size)),
            };
            self.add_event(deallocation_event);
            
            // 添加请求完成事件
            let completion_event = Event {
                time: self.current_time + lifetime,
                event_type: EventType::RequestCompletion,
                request_id: Some(request_id),
                data: None,
            };
            self.add_event(completion_event);
        } else {
            // 分配失败，内存不足
            self.metrics.record_allocation(false, memory_size);
            self.metrics.record_fragmentation(allocator.fragmentation_ratio());
        }
    }
    
    /// 处理内存分配事件
    fn handle_memory_allocation(&mut self, event: Event, _allocator: &mut Box<dyn Allocator>) {
        // 在Phase 0中，内存分配在请求到达时处理
        // 这个事件类型为未来扩展预留
        let _ = event;
    }
    
    /// 处理内存释放事件
    fn handle_memory_deallocation(&mut self, event: Event, allocator: &mut Box<dyn Allocator>) {
        let request_id = event.request_id.unwrap();
        let memory_size = match event.data {
            Some(EventData::MemorySize(size)) => size,
            _ => panic!("内存释放事件缺少内存大小信息"),
        };
        
        // 释放内存
        if allocator.deallocate(request_id) {
            self.metrics.record_deallocation(memory_size);
        } else {
            self.metrics.record_error("内存释放失败".to_string());
        }
    }
    
    /// 处理请求完成事件
    fn handle_request_completion(&mut self, _event: Event, _allocator: &mut Box<dyn Allocator>) {
        self.active_requests -= 1;
        self.completed_requests += 1;
        self.metrics.record_request_completion(self.current_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::NaiveAllocator;
    use crate::workload::WorkloadGenerator;
    
    #[test]
    fn test_simulation_engine_creation() {
        let engine = SimulationEngine::new(1000);
        assert_eq!(engine.current_time(), 0);
        assert_eq!(*engine.state(), SimulationState::NotStarted);
    }
    
    #[test]
    fn test_event_ordering() {
        let event1 = Event {
            time: 10,
            event_type: EventType::RequestArrival,
            request_id: None,
            data: None,
        };
        
        let event2 = Event {
            time: 5,
            event_type: EventType::RequestArrival,
            request_id: None,
            data: None,
        };
        
        // 时间早的事件应该排在前面（BinaryHeap是最大堆，所以比较反转）
        assert!(event1 < event2);
    }
    
    #[test]
    fn test_simulation_runs() {
        let mut engine = SimulationEngine::new(100);
        let allocator = Box::new(NaiveAllocator::new(100));
        let workload = WorkloadGenerator::new(10, 5, 10);
        
        engine.run(allocator, workload);
        
        assert_eq!(*engine.state(), SimulationState::Completed);
        assert!(engine.current_time() <= 100);
    }
}