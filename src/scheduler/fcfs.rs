//! # FCFS 调度器（First-Come, First-Served）
//!
//! 经典的 *静态批处理* 基线：严格按到达顺序处理请求，且 **同一时刻只允许
//! 一个请求驻留在 Running 队列**。等当前请求 decode 完毕、释放显存后，
//! 才会从 Waiting 队列取出下一个。
//!
//! ## 行为特征
//!
//! - 队头阻塞（HOL blocking）显著：长请求会延后整队后续请求的 TTFT。
//! - 显存利用率低：单请求驻留无法充分利用 GPU。
//! - 优点：实现简单，作为 Continuous Batching 的对照组。
//!
//! ## 复杂度
//!
//! - `schedule()` 单次决策：O(running.len())，由于 running 至多 1 个，近似 O(1)。

use super::{RequestPhase, ScheduleDecision, Scheduler, SchedulerContext};
use crate::memory::Allocator;

/// FCFS 调度器（一次只跑一个请求）。
pub struct FcfsScheduler;

impl FcfsScheduler {
    /// 创建一个新的 FCFS 调度器实例。
    pub fn new() -> Self {
        Self
    }
}

impl Default for FcfsScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler for FcfsScheduler {
    fn name(&self) -> &str {
        "FcfsScheduler"
    }

    /// 调度逻辑（每个 tick 调用一次）：
    ///
    /// 1. 若 Running 中存在请求：
    ///    - 已完成（tokens_generated >= output_tokens）→ `to_finish`，
    ///      之后槽位空出可继续 admit；
    ///    - 否则 → `to_decode`（每 tick 1 token）。
    /// 2. 若槽位空闲，从 Waiting 队首取 1 个尝试 admit：
    ///    - allocator.allocate(prompt + output) 成功 → 加入 `to_prefill`，
    ///      同 tick 完成 prefill，下一 tick 起开始 decode。
    fn schedule(
        &mut self,
        ctx: &mut SchedulerContext,
        allocator: &mut dyn Allocator,
        current_time: u64,
    ) -> ScheduleDecision {
        let mut decision = ScheduleDecision::default();

        // ---- 1. 处理 Running 队列中的当前请求 ----
        let mut slot_free = ctx.running.is_empty();
        if let Some(&id) = ctx.running.first() {
            let req = &mut ctx.requests[id];
            if req.is_finished() {
                req.phase = RequestPhase::Finished;
                req.finished_at = Some(current_time);
                decision.to_finish.push(id);
                slot_free = true;
            } else if req.phase == RequestPhase::Decoding {
                decision.to_decode.push(id);
            }
            // 若 phase 仍是 Prefilling，说明上一 tick 刚 admit；本 tick 不再
            // 重复触发 prefill（已在 admit 当 tick 计入 to_prefill）。
            // 简化模型下 Prefilling 阶段持续 0 tick，立即转 Decoding。
        }

        // 若本 tick 标记完成的请求即是 running[0]，移出之以释放槽位
        for id in &decision.to_finish {
            ctx.remove_from_running(*id);
        }

        // ---- 2. 槽位空闲时 admit 下一个 ----
        if slot_free {
            if let Some(&next_id) = ctx.waiting.front() {
                let need = ctx.requests[next_id].request.prompt_tokens
                    + ctx.requests[next_id].request.output_tokens;
                if let Some(block) = allocator.allocate(need) {
                    let id = ctx.waiting.pop_front().unwrap();
                    let req = &mut ctx.requests[id];
                    req.block_id = Some(block.id);
                    req.phase = RequestPhase::Decoding; // 0-tick prefill
                    req.admitted_at = Some(current_time);
                    decision.to_prefill.push(id);
                    ctx.running.push(id);
                }
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
    fn test_fcfs_admits_one_at_a_time() {
        let mut sched = FcfsScheduler::new();
        let mut ctx = SchedulerContext::new();
        let mut alloc = NaiveAllocator::new(1024);

        ctx.admit_arrival(make_request(0, 10, 3));
        ctx.admit_arrival(make_request(1, 10, 3));

        // tick 0：admit r0，进入 prefilling
        let d = sched.schedule(&mut ctx, &mut alloc, 0);
        assert_eq!(ctx.running, vec![0]);
        assert_eq!(d.to_prefill, vec![0]);
        // r1 仍在 waiting
        assert_eq!(ctx.waiting.front(), Some(&1));
    }

    #[test]
    fn test_fcfs_decode_until_finish_then_swap() {
        let mut sched = FcfsScheduler::new();
        let mut ctx = SchedulerContext::new();
        let mut alloc = NaiveAllocator::new(1024);
        ctx.admit_arrival(make_request(0, 5, 2));
        ctx.admit_arrival(make_request(1, 5, 2));

        // tick 0：admit + prefill r0（同 tick 完成 prefill）
        let d = sched.schedule(&mut ctx, &mut alloc, 0);
        assert_eq!(d.to_prefill, vec![0]);

        // tick 1：decode r0 第 1 个 token（手动模拟 engine 累加）
        let d = sched.schedule(&mut ctx, &mut alloc, 1);
        assert_eq!(d.to_decode, vec![0]);
        ctx.requests[0].tokens_generated += 1;

        // tick 2：decode r0 第 2 个 token
        let d = sched.schedule(&mut ctx, &mut alloc, 2);
        assert_eq!(d.to_decode, vec![0]);
        ctx.requests[0].tokens_generated += 1;

        // tick 3：r0 已生成 2 个 token，finished；同 tick admit r1
        let d = sched.schedule(&mut ctx, &mut alloc, 3);
        assert_eq!(d.to_finish, vec![0]);
        assert_eq!(ctx.running, vec![1]);
    }
}
