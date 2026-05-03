//! # 性能指标收集模块
//! 
//! 本模块实现了仿真过程中的性能指标收集和分析，包括：
//! - 内存使用指标
//! - 分配成功率
//! - 碎片率
//! - 请求完成统计
//! 
//! ## 指标分类
//! 
//! ### 内存指标
//! - 已分配内存大小
//! - 峰值内存使用量
//! - 内存碎片率
//! 
//! ### 分配指标
//! - 总分配次数
//! - 成功分配次数
//! - 失败分配次数
//! - 分配成功率
//! 
//! ### 请求指标
//! - 已完成请求数量
//! - 平均请求完成时间
//! 
//! ## 使用方式
//! 
//! ```rust
/// use sim_mem_rs::metrics::SimulationMetrics;
/// 
/// let mut metrics = SimulationMetrics::new();
/// metrics.record_allocation(true, 100);
/// metrics.record_request_completion(50);
/// 
/// let report = metrics.finalize();
/// println!("分配成功率: {:.2}%", report.success_rate * 100.0);
/// ```

use serde::Serialize;
use crate::memory::AllocatorStats;
use crate::memory::Allocator;

/// 仿真指标收集器
/// 
/// 收集和分析仿真过程中的各种性能指标
#[derive(Debug, Clone)]
pub struct SimulationMetrics {
    /// 分配器名称
    pub allocator_name: String,
    /// 分配统计
    pub allocator_stats: AllocatorStats,
    /// 时间序列数据
    time_series: Vec<TimeSeriesPoint>,
    /// 总分配次数
    total_allocations: u64,
    /// 成功分配次数
    successful_allocations: u64,
    /// 失败分配次数
    failed_allocations: u64,
    /// 总释放次数
    total_deallocations: u64,
    /// 已完成请求数量
    completed_requests: u64,
    /// 请求完成时间总和
    total_completion_time: u64,
    /// 错误信息
    errors: Vec<String>,
}

/// 时间序列数据点
#[derive(Debug, Clone, Serialize)]
pub struct TimeSeriesPoint {
    /// 时间戳
    pub time: u64,
    /// 已分配内存
    pub allocated_memory: usize,
    /// 碎片率
    pub fragmentation: f64,
    /// 活跃请求数
    pub active_requests: u64,
}

/// 性能报告
#[derive(Debug, Clone, Serialize)]
pub struct PerformanceReport {
    /// 分配器名称
    pub allocator_name: String,
    /// 仿真时长
    pub simulation_duration: u64,
    /// 总分配次数
    pub total_allocations: u64,
    /// 成功分配次数
    pub successful_allocations: u64,
    /// 失败分配次数
    pub failed_allocations: u64,
    /// 分配成功率
    pub success_rate: f64,
    /// 峰值内存使用量
    pub peak_memory_usage: usize,
    /// 平均内存使用量
    pub avg_memory_usage: f64,
    /// 最终碎片率
    pub final_fragmentation: f64,
    /// 平均碎片率
    pub avg_fragmentation: f64,
    /// 已完成请求数量
    pub completed_requests: u64,
    /// 平均请求完成时间
    pub avg_completion_time: f64,
    /// 时间序列数据
    pub time_series: Vec<TimeSeriesPoint>,
}

impl SimulationMetrics {
    /// 创建新的指标收集器
    pub fn new() -> Self {
        Self {
            allocator_name: String::new(),
            allocator_stats: AllocatorStats::default(),
            time_series: Vec::new(),
            total_allocations: 0,
            successful_allocations: 0,
            failed_allocations: 0,
            total_deallocations: 0,
            completed_requests: 0,
            total_completion_time: 0,
            errors: Vec::new(),
        }
    }
    
    /// 记录分配操作
    /// 
    /// # Arguments
    /// 
    /// * `success` - 是否分配成功
    /// * `size` - 分配的内存大小
    pub fn record_allocation(&mut self, success: bool, _size: usize) {
        self.total_allocations += 1;
        if success {
            self.successful_allocations += 1;
        } else {
            self.failed_allocations += 1;
        }
    }
    
    /// 记录释放操作
    /// 
    /// # Arguments
    /// 
    /// * `size` - 释放的内存大小
    pub fn record_deallocation(&mut self, _size: usize) {
        self.total_deallocations += 1;
    }
    
    /// 记录碎片率
    /// 
    /// # Arguments
    /// 
    /// * `fragmentation` - 当前碎片率
    pub fn record_fragmentation(&mut self, _fragmentation: f64) {
        // 碎片率记录在时间序列中
    }
    
    /// 记录请求完成
    /// 
    /// # Arguments
    /// 
    /// * `completion_time` - 请求完成时间
    pub fn record_request_completion(&mut self, completion_time: u64) {
        self.completed_requests += 1;
        self.total_completion_time += completion_time;
    }
    
    /// 记录错误
    /// 
    /// # Arguments
    /// 
    /// * `error` - 错误信息
    pub fn record_error(&mut self, error: String) {
        self.errors.push(error);
    }
    
    /// 记录时间步
    /// 
    /// # Arguments
    /// 
    /// * `time` - 当前时间
    /// * `allocator` - 内存分配器
    pub fn record_time_step(&mut self, time: u64, allocator: &dyn Allocator) {
        let point = TimeSeriesPoint {
            time,
            allocated_memory: allocator.used_memory(),
            fragmentation: allocator.fragmentation_ratio(),
            active_requests: self.total_allocations - self.total_deallocations,
        };
        self.time_series.push(point);
    }
    
    /// 完成指标收集，生成报告
    /// 
    /// # Returns
    /// 
    /// 返回性能报告
    pub fn finalize(&mut self) -> PerformanceReport {
        // 计算统计信息
        let success_rate = if self.total_allocations > 0 {
            self.successful_allocations as f64 / self.total_allocations as f64
        } else {
            0.0
        };
        
        let peak_memory = self.time_series.iter()
            .map(|p| p.allocated_memory)
            .max()
            .unwrap_or(0);
        
        let avg_memory = if !self.time_series.is_empty() {
            self.time_series.iter()
                .map(|p| p.allocated_memory)
                .sum::<usize>() as f64 / self.time_series.len() as f64
        } else {
            0.0
        };
        
        let final_fragmentation = self.time_series.last()
            .map(|p| p.fragmentation)
            .unwrap_or(0.0);
        
        let avg_fragmentation = if !self.time_series.is_empty() {
            self.time_series.iter()
                .map(|p| p.fragmentation)
                .sum::<f64>() / self.time_series.len() as f64
        } else {
            0.0
        };
        
        let avg_completion_time = if self.completed_requests > 0 {
            self.total_completion_time as f64 / self.completed_requests as f64
        } else {
            0.0
        };
        
        let simulation_duration = self.time_series.last()
            .map(|p| p.time)
            .unwrap_or(0);
        
        PerformanceReport {
            allocator_name: self.allocator_name.clone(),
            simulation_duration,
            total_allocations: self.total_allocations,
            successful_allocations: self.successful_allocations,
            failed_allocations: self.failed_allocations,
            success_rate,
            peak_memory_usage: peak_memory,
            avg_memory_usage: avg_memory,
            final_fragmentation,
            avg_fragmentation,
            completed_requests: self.completed_requests,
            avg_completion_time,
            time_series: self.time_series.clone(),
        }
    }
    
    /// 获取时间序列数据
    pub fn time_series(&self) -> &[TimeSeriesPoint] {
        &self.time_series
    }
    
    /// 获取错误信息
    pub fn errors(&self) -> &[String] {
        &self.errors
    }
}

/// 比较多个性能报告
/// 
/// # Arguments
/// 
/// * `reports` - 性能报告列表
/// 
/// # Returns
/// 
/// 返回格式化的比较结果
pub fn compare_reports(reports: &[PerformanceReport]) -> String {
    let mut result = String::new();
    
    result.push_str("性能对比报告\n");
    result.push_str("============\n\n");
    
    for report in reports {
        result.push_str(&format!("分配器: {}\n", report.allocator_name));
        result.push_str(&format!("  仿真时长: {}\n", report.simulation_duration));
        result.push_str(&format!("  总分配次数: {}\n", report.total_allocations));
        result.push_str(&format!("  分配成功率: {:.2}%\n", report.success_rate * 100.0));
        result.push_str(&format!("  峰值内存使用: {}\n", report.peak_memory_usage));
        result.push_str(&format!("  平均内存使用: {:.2}\n", report.avg_memory_usage));
        result.push_str(&format!("  最终碎片率: {:.4}\n", report.final_fragmentation));
        result.push_str(&format!("  平均碎片率: {:.4}\n", report.avg_fragmentation));
        result.push_str(&format!("  完成请求数: {}\n", report.completed_requests));
        result.push_str(&format!("  平均完成时间: {:.2}\n\n", report.avg_completion_time));
    }
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simulation_metrics_creation() {
        let metrics = SimulationMetrics::new();
        assert_eq!(metrics.total_allocations, 0);
        assert_eq!(metrics.successful_allocations, 0);
    }
    
    #[test]
    fn test_record_allocation() {
        let mut metrics = SimulationMetrics::new();
        
        metrics.record_allocation(true, 100);
        metrics.record_allocation(true, 200);
        metrics.record_allocation(false, 300);
        
        assert_eq!(metrics.total_allocations, 3);
        assert_eq!(metrics.successful_allocations, 2);
        assert_eq!(metrics.failed_allocations, 1);
    }
    
    #[test]
    fn test_finalize() {
        let mut metrics = SimulationMetrics::new();
        metrics.allocator_name = "TestAllocator".to_string();
        
        metrics.record_allocation(true, 100);
        metrics.record_request_completion(50);
        
        let report = metrics.finalize();
        assert_eq!(report.allocator_name, "TestAllocator");
        assert_eq!(report.success_rate, 1.0);
    }
}