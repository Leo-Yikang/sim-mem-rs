//! # 连续批处理调度器（Continuous Batching / Iteration-level Scheduling）
//!
//! 现代 LLM 推理引擎（vLLM、Orca、TensorRT-LLM）采用的核心调度策略：
//! **每个 tick（iteration）都重新决策**，而非等整个 batch 完成后再切换。
//!
//! ## 核心思想
//!
//! 1. 每个 tick，对 Running 队列中已完成的请求 **立即释放显存** 并把它们移出。
//! 2. 在剩余显存预算允许的前提下，**尽量多地** 从 Waiting 队列 admit 新请求。
//! 3. 对 Running 中所有未完成请求 **并行** 推进 1 个 token（decode）或完成 prefill。
//!
//! 与 FCFS 的关键差异：
//!
//! | 维度 | FCFS | Continuous Batching |
//! |------|------|---------------------|
//! | 同时活跃请求数 | 1 | 受显存限制（理论上无上限） |
//! | 队头阻塞 | 严重 | 几乎无 |
//! | 平均 TTFT | 高（需排队等到上一个完整结束） | 低（一旦有显存立即上车） |
//! | 实现复杂度 | 极简 | 中等 |
//!
//! ## 显存预算
//!
//! 调度器通过尝试调用 `Allocator::allocate(prompt_tokens + output_tokens)` 来判断
//! 是否能 admit；若失败则保持该请求在 Waiting 中等待下一个 tick 重试。
//!
//! Phase 1 中显存一次性按 *最大占用量*（prompt + output）预留，避免 Phase 2 的
//! 抢占机制干扰指标对比。Phase 2 将引入按 token 增量分配 + 抢占。
//!
//! ## 可选参数
//!
//! - `max_batch_size`：限制 Running 队列大小（None 表示无上限），用于模拟硬件
//!   并行度上限或对比实验。

use super::{RequestPhase, ScheduleDecision, Scheduler, SchedulerContext};
use crate::memory::Allocator;

/// 连续批处理调度器。
pub struct ContinuousBatchingScheduler {
    /// 单 tick 最多并行的请求数；`None` 表示不限制。
    max_batch_size: Option<usize>,
}

impl ContinuousBatchingScheduler {
    /// 创建一个不限制 batch 大小的调度器。
    pub fn new() -> Self {
        Self {
            max_batch_size: None,
        }
    }

    /// 创建一个限制 batch 大小的调度器。
    ///
    /// # Arguments
    ///
    /// * `max_batch_size` - 同时驻留 Running 队列的请求数上限。
    pub fn with_max_batch_size(max_batch_size: usize) -> Self {
        Self {
            max_batch_size: Some(max_batch_size),
        }
    }
}

impl Default for ContinuousBatchingScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for ContinuousBatchingScheduler {
    fn name(&self) -> &str {
        "ContinuousBatchingScheduler"
    }

    /// 调度逻辑（每个 tick 调用一次）：
    ///
    /// **阶段 1：清理已完成请求**
    /// 遍历 `running`，把 `is_finished()` 的请求转入 `to_finish`，
    /// Engine 据此释放对应内存块。
    ///
    /// **阶段 2：admit 新请求**
    /// 在不超过 `max_batch_size` 的前提下，尽可能从 `waiting` 头部
    /// 取请求并尝试分配显存；分配失败则停止本 tick 的 admission（避免
    /// 跳过队头造成饥饿）。
    ///
    /// **阶段 3：推进 prefill / decode**
    /// 对 Running 中每个请求执行一次：刚 admit 的视为本 tick 完成 prefill
    /// 并立即转入 Decoding；其余跑一次 decode。
    fn schedule(
        &mut self,
        ctx: &mut SchedulerContext,
        allocator: &mut dyn Allocator,
        current_time: u64,
    ) -> ScheduleDecision {
        let mut decision = ScheduleDecision::default();

        // ---- 阶段 1：清理已完成请求 ----
        // 收集需要释放的 id（不可在迭代过程中修改 running）
        let finished_ids: Vec<usize> = ctx
            .running
            .iter()
            .copied()
            .filter(|&id| ctx.requests[id].is_finished())
            .collect();
        for id in &finished_ids {
            let req = &mut ctx.requests[*id];
            req.phase = RequestPhase::Finished;
            req.finished_at = Some(current_time);
            decision.to_finish.push(*id);
        }
        // 从 running 中移除（保持原顺序）
        ctx.running.retain(|id| !finished_ids.contains(id));

        // ---- 阶段 2：尽可能 admit 新请求 ----
        loop {
            // batch 容量已满则停止
            if let Some(cap) = self.max_batch_size {
                if ctx.running.len() >= cap {
                    break;
                }
            }
            let Some(&next_id) = ctx.waiting.front() else {
                break;
            };
            let need = ctx.requests[next_id].request.prompt_tokens
                + ctx.requests[next_id].request.output_tokens;
            match allocator.allocate(need) {
                Some(block) => {
                    let id = ctx.waiting.pop_front().unwrap();
                    let req = &mut ctx.requests[id];
                    req.block_id = Some(block.id);
                    req.phase = RequestPhase::Prefilling;
                    req.admitted_at = Some(current_time);
                    ctx.running.push(id);
                }
                None => {
                    // 显存不足，本 tick 不再尝试，等待下个 tick
                    break;
                }
            }
        }

        // ---- 阶段 3：推进 prefill / decode ----
        for &id in &ctx.running {
            let req = &mut ctx.requests[id];
            match req.phase {
                RequestPhase::Prefilling => {
                    decision.to_prefill.push(id);
                    req.phase = RequestPhase::Decoding;
                }
                RequestPhase::Decoding if !req.is_finished() => {
                    decision.to_decode.push(id);
                }
                _ => {}
            }
        }

        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::NaiveAllocator;
    use crate::workload::Request;

    fn make_request(id: usize, prompt: usize, output: usize) -> Request {
        Request {
            id,
            arrival_time: 0,
            lifetime: 1,
            memory_size: prompt,
            prompt_tokens: prompt,
            output_tokens: output,
        }
    }

    #[test]
    fn test_cb_admits_multiple_in_one_tick() {
        let mut sched = ContinuousBatchingScheduler::new();
        let mut ctx = SchedulerContext::new();
        let mut alloc = NaiveAllocator::new(1024);
        for i in 0..3 {
            ctx.admit_arrival(make_request(i, 10, 5));
        }
        // tick 0：理论上可同时 admit 三个（显存足够）
        sched.schedule(&mut ctx, &mut alloc, 0);
        assert_eq!(ctx.running.len(), 3);
        assert!(ctx.waiting.is_empty());
    }

    #[test]
    fn test_cb_respects_max_batch_size() {
        let mut sched = ContinuousBatchingScheduler::with_max_batch_size(2);
        let mut ctx = SchedulerContext::new();
        let mut alloc = NaiveAllocator::new(1024);
        for i in 0..5 {
            ctx.admit_arrival(make_request(i, 10, 5));
        }
        sched.schedule(&mut ctx, &mut alloc, 0);
        assert_eq!(ctx.running.len(), 2);
    }

    #[test]
    fn test_cb_finish_releases_slot() {
        let mut sched = ContinuousBatchingScheduler::new();
        let mut ctx = SchedulerContext::new();
        let mut alloc = NaiveAllocator::new(1024);
        ctx.admit_arrival(make_request(0, 5, 1));
        // tick 0: prefill
        sched.schedule(&mut ctx, &mut alloc, 0);
        // 模拟 decode 1 次后触发完成
        ctx.requests[0].tokens_generated = 1;
        // tick 1: 检测 finished
        let d = sched.schedule(&mut ctx, &mut alloc, 1);
        assert_eq!(d.to_finish, vec![0]);
        assert!(ctx.running.is_empty());
    }
}
