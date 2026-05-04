//! # 调度器模块（Scheduler）
//!
//! 本模块在 `Engine` 与 `Allocator` 之间引入一层 **调度器**，
//! 用于决定每个 tick 哪些请求可以执行（Prefill / Decode），从而模拟
//! 真实 LLM 推理引擎（Orca、vLLM 等）的核心调度逻辑。
//!
//! ## 三大请求队列
//!
//! 调度器维护三个队列：
//! - **Waiting**：刚到达、尚未分配显存的请求；等待被 *admit*。
//! - **Running**：已分配显存、正在执行 Prefill/Decode 的请求。
//! - **Preempted / Swapped**：曾被踢出的请求（Phase 1 暂未启用，预留接口）。
//!
//! ## 请求生命周期
//!
//! ```text
//!  Arrival ──► [Waiting] ──admit──► [Running] ──Prefill──► Decode ──► Finished
//!                                       │  ▲
//!                                       └──┘ 每 tick 生成 1 个 token
//!                                       │  preempt
//!                                       ▼
//!                                  [Preempted]   (Phase 2)
//! ```
//!
//! ## 调度策略对比
//!
//! | 策略 | 入队粒度 | 适用场景 | 优点 | 缺点 |
//! |------|---------|---------|------|------|
//! | FCFS | 请求级别 | 静态批处理基线 | 简单、公平 | 队头阻塞、利用率低 |
//! | Continuous Batching | token / iteration 级 | 现代 LLM 引擎 | 吞吐高、TTFT 低 | 调度复杂 |
//!
//! ## 设计要点
//!
//! - `Scheduler` trait 是 *无状态决策函数*：输入当前队列与分配器视图，
//!   输出本 tick 应当执行的请求列表与新增/释放的内存操作。
//! - 真实状态（队列内容、tokens_generated 计数等）由 `SchedulerContext`
//!   持有，便于 Engine 集中管理。
//! - 内存操作通过 `Allocator` trait 完成，Scheduler 不直接持有内存。

pub mod continuous_batching;
pub mod fcfs;

pub use continuous_batching::ContinuousBatchingScheduler;
pub use fcfs::FcfsScheduler;

use crate::memory::Allocator;
use crate::workload::Request;

/// 单个请求在调度器中的状态。
///
/// 字段中包含本次仿真所需的所有动态信息：进度计数、分配到的内存块 id、
/// 时间戳（用于计算 TTFT / TPOT / JCT）。
#[derive(Debug, Clone)]
pub struct ScheduledRequest {
    /// 原始请求
    pub request: Request,
    /// 已分配的内存块 id（来自 Allocator），未分配时为 `None`
    pub block_id: Option<usize>,
    /// 当前阶段
    pub phase: RequestPhase,
    /// 已生成的 decode token 数
    pub tokens_generated: usize,
    /// 进入 Running 队列的时间（即 prefill 完成时间）
    pub admitted_at: Option<u64>,
    /// 第一个 token 产出时间，用于计算 TTFT
    pub first_token_at: Option<u64>,
    /// 上一次产出 token 的时间，用于计算 TPOT 增量
    pub last_token_at: Option<u64>,
    /// 完成时间（最后一个 token 产出时间），用于计算 JCT
    pub finished_at: Option<u64>,
    /// 累计 TPOT（每 token 间隔之和），用于求平均
    pub tpot_accum: u64,
    /// 累计 TPOT 计数（产生过几次 inter-token 间隔）
    pub tpot_count: u64,
}

impl ScheduledRequest {
    /// 包装一个新到达的请求，初始处于 `Waiting` 阶段。
    pub fn new(request: Request) -> Self {
        Self {
            request,
            block_id: None,
            phase: RequestPhase::Waiting,
            tokens_generated: 0,
            admitted_at: None,
            first_token_at: None,
            last_token_at: None,
            finished_at: None,
            tpot_accum: 0,
            tpot_count: 0,
        }
    }

    /// 该请求已生成的 token 是否达到目标 `output_tokens`。
    pub fn is_finished(&self) -> bool {
        self.tokens_generated >= self.request.output_tokens
    }
}

/// 请求所处阶段。
///
/// Phase 1 仅启用 `Waiting / Prefilling / Decoding / Finished`。
/// `Preempted` 留作 Phase 2 抢占机制的占位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestPhase {
    /// 等待被 admit（尚未分配显存）
    Waiting,
    /// 当前 tick 正在执行 prefill（一次性分配 prompt_tokens 单位显存）
    Prefilling,
    /// 已完成 prefill，每个 tick 产出一个 token
    Decoding,
    /// 已完成所有 output_tokens 的生成，可释放内存
    Finished,
    /// 已被抢占（Phase 2 预留）
    Preempted,
}

/// 一次调度决策的产物。
///
/// `Engine` 依据返回值更新事件队列、释放/分配内存。
#[derive(Debug, Default)]
pub struct ScheduleDecision {
    /// 本 tick 选中要执行 prefill 的请求 id 列表
    pub to_prefill: Vec<usize>,
    /// 本 tick 选中要执行 decode 的请求 id 列表
    pub to_decode: Vec<usize>,
    /// 本 tick 已完成、应该释放内存的请求 id 列表
    pub to_finish: Vec<usize>,
}

/// 调度器抽象。
///
/// 实现者负责依据当前上下文产出 [`ScheduleDecision`]。
/// 默认实现是 *纯函数式* 的：所有可变状态通过 `&mut SchedulerContext`
/// 传入，便于切换策略并独立测试。
pub trait Scheduler {
    /// 调度器名称（用于日志、报告）。
    fn name(&self) -> &str;

    /// 在 `current_time` 这一 tick 上做一次调度决策。
    ///
    /// # Arguments
    ///
    /// * `ctx` - 调度上下文（队列 + 全部请求状态），可被修改。
    /// * `allocator` - 当前内存分配器视图，可被修改（执行 allocate/deallocate）。
    /// * `current_time` - 当前仿真时间。
    ///
    /// # Returns
    ///
    /// 返回本 tick 的调度结果，由 Engine 据此推进时间和指标。
    fn schedule(
        &mut self,
        ctx: &mut SchedulerContext,
        allocator: &mut dyn Allocator,
        current_time: u64,
    ) -> ScheduleDecision;
}

/// 调度器共享上下文。
///
/// 集中保存所有请求状态（按 id 索引）以及三个队列的索引。
/// 设计为 `pub` 以便不同调度器实现复用同一份数据结构。
#[derive(Debug, Default)]
pub struct SchedulerContext {
    /// 全部请求按 id 索引（id 与下标一一对应）
    pub requests: Vec<ScheduledRequest>,
    /// 等待队列：尚未 admit 的请求 id（FIFO）
    pub waiting: std::collections::VecDeque<usize>,
    /// 运行队列：已 admit 的请求 id
    pub running: Vec<usize>,
    /// 抢占队列：被踢出但保留状态的请求 id（Phase 2 启用）
    pub preempted: Vec<usize>,
}

impl SchedulerContext {
    /// 创建空上下文。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个新到达的请求。
    ///
    /// 会按 `request.id` 写入 `requests` 数组（自动扩容填充占位），
    /// 并将其追加到 `waiting` 队尾。
    pub fn admit_arrival(&mut self, request: Request) {
        let id = request.id;
        // 扩容到能容纳 id 的尺寸（用占位项填充）
        while self.requests.len() <= id {
            // 用一个空 Request 占位；实际访问时只会读取已写入的索引
            self.requests.push(ScheduledRequest::new(Request {
                id: self.requests.len(),
                arrival_time: 0,
                lifetime: 0,
                memory_size: 0,
                prompt_tokens: 0,
                output_tokens: 0,
            }));
        }
        self.requests[id] = ScheduledRequest::new(request);
        self.waiting.push_back(id);
    }

    /// 从 `running` 队列中移除指定 id（保持顺序）。
    pub fn remove_from_running(&mut self, id: usize) {
        if let Some(pos) = self.running.iter().position(|&x| x == id) {
            self.running.remove(pos);
        }
    }
}
