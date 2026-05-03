//!  # 工作负载生成器模块
//!  
//!  本模块实现了大模型训练场景的工作负载生成，包括：
//!  - 请求（Request）定义
//!  - 工作负载生成器（WorkloadGenerator）
//!  - 各种分布模型的支持
//!  
//!  ## 请求模型
//!  
//!  每个请求代表一个大模型训练任务，包含：
//!  - 到达时间：请求到达系统的时间
//!  - 生命周期：请求需要占用内存的时间
//!  - 内存大小：请求需要的内存大小
//!  
//!  ## 分布模型
//!  
//!  使用统计分布模拟真实场景：
//!  - 到达时间：指数分布（模拟泊松过程）
//!  - 生命周期：正态分布
//!  - 内存大小：正态分布
//!  
//!  ## 使用场景
//!  
//!  1. **基准测试**：生成固定模式的请求，比较不同分配器性能
//!  2. **压力测试**：生成大量请求，测试系统极限
//!  3. **真实模拟**：使用真实数据分布，评估实际性能

use rand;
use rand_distr::{Distribution, Exp, Normal};

/// 训练请求
/// 
/// 表示一个大模型训练任务
#[derive(Debug, Clone)]
pub struct Request {
    /// 请求ID
    pub id: usize,
    /// 到达时间
    pub arrival_time: u64,
    /// 生命周期（占用内存的时间）
    pub lifetime: u64,
    /// 需要的内存大小（内存单位）
    pub memory_size: usize,
}

/// 工作负载生成器
/// 
/// 生成模拟大模型训练场景的请求序列。
/// 
/// # 特点
/// 
/// - 支持多种统计分布
/// - 可配置的请求参数
/// - 生成确定性或随机序列
/// 
/// # 示例
/// 
/// ```rust
/// use sim_mem_rs::workload::WorkloadGenerator;
/// 
// 创建生成器：100个请求，平均生命周期50，平均内存大小10
/// let mut generator = WorkloadGenerator::new(100, 50, 10);
/// 
// 生成请求（传入足够大的时间以触发请求生成）
/// let request = generator.next_request(1000).unwrap();
/// println!("请求 {} 到达时间 {} 需要 {} 内存", 
///          request.id, request.arrival_time, request.memory_size);
/// ```
pub struct WorkloadGenerator {
    /// 总请求数量
    total_requests: usize,
    /// 已生成的请求数量
    generated: usize,
    /// 平均生命周期
    avg_lifetime: u64,
    /// 平均内存大小
    avg_memory_size: usize,
    /// 下一个请求ID
    next_id: usize,
    /// 随机数生成器
    rng: rand::rngs::ThreadRng,
    /// 到达时间分布（指数分布）
    arrival_distribution: Exp<f64>,
    /// 生命周期分布（正态分布）
    lifetime_distribution: Normal<f64>,
    /// 内存大小分布（正态分布）
    memory_distribution: Normal<f64>,
    /// 下一个到达时间
    next_arrival_time: u64,
}

impl WorkloadGenerator {
    /// 创建新的工作负载生成器
    /// 
    /// # Arguments
    /// 
    /// * `total_requests` - 总请求数量
    /// * `avg_lifetime` - 平均生命周期（时间单位）
    /// * `avg_memory_size` - 平均内存大小（内存单位）
    /// 
    /// # Returns
    /// 
    /// 返回初始化的工作负载生成器
    pub fn new(total_requests: usize, avg_lifetime: u64, avg_memory_size: usize) -> Self {
        let mut rng = rand::thread_rng();
        
        // 创建指数分布，lambda = 1/mean
        // 到达率 = 1请求/时间单位
        let arrival_distribution = Exp::new(1.0).unwrap();
        
        // 创建正态分布，标准差为均值的20%
        let lifetime_stddev = avg_lifetime as f64 * 0.2;
        let lifetime_distribution = Normal::new(avg_lifetime as f64, lifetime_stddev).unwrap();
        
        let memory_stddev = avg_memory_size as f64 * 0.3;
        let memory_distribution = Normal::new(avg_memory_size as f64, memory_stddev).unwrap();
        
        // 生成第一个到达时间
        let first_arrival: f64 = arrival_distribution.sample(&mut rng);
        
        Self {
            total_requests,
            generated: 0,
            avg_lifetime,
            avg_memory_size,
            next_id: 0,
            rng,
            arrival_distribution,
            lifetime_distribution,
            memory_distribution,
            next_arrival_time: first_arrival as u64,
        }
    }
    
    /// 生成下一个请求
    /// 
    /// # Arguments
    /// 
    /// * `current_time` - 当前仿真时间
    /// 
    /// # Returns
    /// 
    /// * `Some(Request)` - 生成的请求
    /// * `None` - 已达到总请求数量
    pub fn next_request(&mut self, current_time: u64) -> Option<Request> {
        if self.generated >= self.total_requests {
            return None;
        }
        
        // 如果下一个请求的到达时间还没到，返回None
        if self.next_arrival_time > current_time {
            return None;
        }
        
        // 生成请求参数
        let lifetime = self.generate_lifetime();
        let memory_size = self.generate_memory_size();
        
        let request = Request {
            id: self.next_id,
            arrival_time: self.next_arrival_time,
            lifetime,
            memory_size,
        };
        
        // 更新状态
        self.next_id += 1;
        self.generated += 1;
        
        // 生成下一个到达时间
        let inter_arrival: f64 = self.arrival_distribution.sample(&mut self.rng);
        self.next_arrival_time += inter_arrival as u64;
        
        Some(request)
    }
    
    /// 生成生命周期
    /// 
    /// 使用正态分布生成，确保不小于1
    pub fn generate_lifetime(&mut self) -> u64 {
        let lifetime: f64 = self.lifetime_distribution.sample(&mut self.rng);
        (lifetime.max(1.0)) as u64
    }
    
    /// 生成内存大小
    /// 
    /// 使用正态分布生成，确保不小于1
    fn generate_memory_size(&mut self) -> usize {
        let size: f64 = self.memory_distribution.sample(&mut self.rng);
        (size.max(1.0)) as usize
    }
    
    /// 获取平均生命周期
    pub fn avg_lifetime(&self) -> u64 {
        self.avg_lifetime
    }
    
    /// 获取平均内存大小
    pub fn avg_memory_size(&self) -> usize {
        self.avg_memory_size
    }
    
    /// 获取总请求数量
    pub fn total_requests(&self) -> usize {
        self.total_requests
    }
    
    /// 获取已生成的请求数量
    pub fn generated(&self) -> usize {
        self.generated
    }
}

/// 确定性工作负载生成器
/// 
/// 生成固定模式的请求序列，用于可重复的基准测试
pub struct DeterministicWorkloadGenerator {
    /// 请求列表
    requests: Vec<Request>,
    /// 当前索引
    current_index: usize,
}

impl DeterministicWorkloadGenerator {
    /// 创建确定性工作负载生成器
    /// 
    /// # Arguments
    /// 
    /// * `num_requests` - 请求数量
    /// * `interval` - 请求间隔（时间单位）
    /// * `lifetime` - 固定生命周期
    /// * `memory_size` - 固定内存大小
    pub fn new(num_requests: usize, interval: u64, lifetime: u64, memory_size: usize) -> Self {
        let requests = (0..num_requests)
            .map(|i| Request {
                id: i,
                arrival_time: i as u64 * interval,
                lifetime,
                memory_size,
            })
            .collect();
        
        Self {
            requests,
            current_index: 0,
        }
    }
    
    /// 获取下一个请求
    pub fn next_request(&mut self, current_time: u64) -> Option<Request> {
        if self.current_index >= self.requests.len() {
            return None;
        }
        
        let request = &self.requests[self.current_index];
        if request.arrival_time <= current_time {
            self.current_index += 1;
            Some(request.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_workload_generator_creation() {
        let generator = WorkloadGenerator::new(100, 50, 10);
        assert_eq!(generator.total_requests(), 100);
        assert_eq!(generator.avg_lifetime(), 50);
        assert_eq!(generator.avg_memory_size(), 10);
    }
    
    #[test]
    fn test_workload_generator_generates_requests() {
        let mut generator = WorkloadGenerator::new(10, 50, 10);
        
        let mut requests = Vec::new();
        while let Some(request) = generator.next_request(1000) {
            requests.push(request);
        }
        
        assert_eq!(requests.len(), 10);
        assert_eq!(requests[0].id, 0);
    }
    
    #[test]
    fn test_deterministic_workload_generator() {
        let mut generator = DeterministicWorkloadGenerator::new(5, 10, 20, 100);
        
        let request1 = generator.next_request(0).unwrap();
        assert_eq!(request1.arrival_time, 0);
        assert_eq!(request1.lifetime, 20);
        
        let request2 = generator.next_request(10).unwrap();
        assert_eq!(request2.arrival_time, 10);
    }
}