# AGENTS.md - sim-mem-rs 项目知识库

> 面向后续 AI agent 的快速上手指南。阅读本文档即可理解项目全貌，无需重新通读源码。

---

## 项目身份

| 属性 | 值 |
|------|-----|
| **名称** | sim-mem-rs (v0.2.0) |
| **语言** | Rust (edition 2021, nightly toolchain) |
| **目标** | 面向大模型训练的内存模拟器，使用 DES 评估不同分配策略 |
| **当前阶段** | **Phase 0 已完成** — 核心框架 + Naive/Paged 基准对比 |
| **代码位置** | `/home/lykspeaking/git/mine/sim-mem-rs` |
| **依赖** | rand, rand_distr, serde, serde_json, clap |
| **可视化** | Python 脚本 (`scripts/visualize.py`, 需要 matplotlib) |

---

## 核心架构

```
用户请求 ──► WorkloadGenerator ──► Event Queue (BinaryHeap) ──► Engine ──► Allocator ──► Metrics
                                      ▲                              │
                                      └── 事件可产生新事件 ←──────────┘
```

### 数据流
1. `WorkloadGenerator` 按统计分布生成 `Request`（到达时间、生命周期、内存大小）
2. Engine 将所有 Request 转为 `Event` 放入最小堆事件队列
3. 主循环：pop 事件 → 推进时间 → 调用 Allocator → 记录 Metrics → 可能 push 新事件
4. 仿真结束后 `Metrics::finalize()` 产出 `PerformanceReport`（含时间序列）
5. JSON 序列化后由 Python 脚本生成 3 张 PNG 图表

### 模块地图

| 文件 | 职责 | 关键类型 |
|------|------|---------|
| `src/lib.rs` | 公共 API，`run_simulation()`, `run_benchmark()` | `SimulationConfig`, `SimulationResult` |
| `src/engine/mod.rs` | DES 引擎，事件队列，时间推进 | `SimulationEngine`, `Event`, `EventType` |
| `src/memory/mod.rs` | Allocator trait 定义 | `Allocator`, `MemoryBlock`, `AllocatorStats` |
| `src/memory/naive.rs` | 首次适应连续分配器 | `NaiveAllocator` |
| `src/memory/paged.rs` | 固定页大小分配器 | `PagedAllocator` |
| `src/workload/mod.rs` | 请求生成（指数+正态分布） | `WorkloadGenerator`, `DeterministicWorkloadGenerator` |
| `src/metrics/mod.rs` | 指标采集 + 性能报告 | `SimulationMetrics`, `PerformanceReport` |
| `src/main.rs` | CLI（clap derive） | `benchmark`, `simulate` 子命令 |
| `benches/allocator_benchmarks.rs` | Criterion 微基准 | — |
| `scripts/visualize.py` | matplotlib 图表生成 | — |

---

## 设计决策与模式

### 1. Allocator trait（可扩展性核心）
所有分配器实现 `Allocator` trait，共 9 个方法：
```rust
fn allocate(&mut self, size: usize) -> Option<MemoryBlock>
fn deallocate(&mut self, request_id: usize) -> bool
fn name(&self) -> &str
fn stats(&self) -> &AllocatorStats
fn used_memory(&self) -> usize
fn total_memory(&self) -> usize
fn fragmentation_ratio(&self) -> f64
fn has_contiguous_memory(&self, size: usize) -> bool
fn max_contiguous_block(&self) -> usize
```
添加新分配器只需实现此 trait，在 `memory/mod.rs` 中声明模块，在 `lib.rs` 的 `run_benchmark` 中加入即可。

### 2. MemoryBlock.size 语义（重要！）
- **NaiveAllocator**: `block.size` = 实际分配大小（= 请求大小，因为没有内部碎片）
- **PagedAllocator**: `block.size` = **原始请求大小**（非页对齐后的大小），实际物理占用 = `pages_needed(size) * page_size`
- 这个差异是为了正确计算内部碎片：`total_internal_fragmentation = Σ(物理分配 - block.size)`
- 统计中的 `allocated_memory` 使用页对齐值，而非 `block.size`

### 3. 事件队列
- 使用 `BinaryHeap<Event>`（最大堆 → 反转比较实现最小堆）
- `Event` 按 `time` 升序，同时间按 `EventType` 排序
- `EventType` 必须实现 `Ord`（derive 即可）

### 4. 工作负载生成
- 到达间隔：指数分布 `Exp(λ=1)`，模拟泊松过程
- 生命周期：正态分布，标准差 = 均值 × 0.2，最小值 1
- 内存大小：正态分布，标准差 = 均值 × 0.3，最小值 1
- 确定性版本 `DeterministicWorkloadGenerator` 用于可重复测试

### 5. PagedAllocator 约束
- `total_memory % page_size == 0`（创建时 assert）
- `page_size > 0`
- 分页使用首次适应连续页查找（非 buddy system）
- 内部碎片率 = `Σ(页对齐分配 - 原始请求) / Σ(页对齐分配)`

### 6. 可视化方案
- Rust 端输出 JSON → `scripts/visualize.py` → matplotlib PNG
- 之前尝试过 plotters（Rust），但需要 `fontconfig` 系统库，在无 sudo 环境下不可用
- Python 方案无系统依赖（只需 `pip install matplotlib numpy`）

---

## 运行命令

```bash
# 测试
cargo test                          # 全部 (25 unit + 7 doctest)
cargo test --lib                    # 仅库测试
cargo test -- --nocapture           # 显示输出

# 运行
cargo run --release -- benchmark -d 1000 -m 1024 -r 100 -o output
cargo run --release -- simulate -a naive -m 1024 -r 50

# 基准测试
cargo bench                         # Criterion 微基准 → target/criterion/report/index.html

# 仅检查编译
cargo check
```

---

## 测试数据逻辑

| 测试类别 | 数量 | 说明 |
|---------|------|------|
| 单元测试 | 25 | 每模块独立测试，覆盖创建/分配/释放/合并/碎片计算 |
| 文档测试 | 7 | lib.rs + 各模块 docstring 中的示例代码 |
| 基准测试 | 4 | Criterion 微基准（allocate/deallocate） |
| 集成测试 | 0 | 待添加（`tests/` 目录预留） |

测试数据的统计分布逻辑见上方 "工作负载生成"。

---

## 已知问题与注意事项

1. **PagedAllocator 在默认参数下表现差**：页大小 64 > 平均请求 10，导致 85%+ 内部碎片。这不是 bug，是需要后续 Phase 引入动态页大小或调度器来解决的设计权衡。
2. **`next_request_id` 字段未使用**：`SimulationEngine` 中有此字段但从未读取。为 Phase 1 预留（调度器需要）。
3. **`allocated_memory` 语义在 Naive/Paged 中不同**：Naive 用 `block.size`，Paged 用 `pages.len() * page_size`。如需统一，考虑在 `MemoryBlock` 中增加 `allocated_size` 字段。
4. **日志系统未启用**：Cargo.toml 中移除 `env_logger` 和 `log` 依赖。Phase 1 可以加回来。

---

## Phase 0 → Phase 1 扩展指南

Phase 1 目标：引入调度器机制（v0.3 — 动态批处理时代）

需要的改动：
1. **新增 `src/scheduler/mod.rs`**：`Scheduler` trait + 实现（`FcfsScheduler`, `ContinuousBatchingScheduler`）
2. **修改 `engine/mod.rs`**：在 Engine 和 Allocator 之间插入 Scheduler 层，维护 Waiting/Running/Preempted 三个队列
3. **新增指标**：`TTFT`（首 token 延迟）、`TPOT`（每 token 延迟）、`JCT`（作业完成时间）
4. **扩展 `MemoryBlock`**：增加 `logical_pages` 和 `physical_pages` 字段为此预留

---

## 首次使用检查清单

- [ ] `cargo check` 通过
- [ ] `cargo test` 全部通过 (25/25)
- [ ] `cargo run --release -- benchmark` 产生 `output/*.png`
- [ ] Python: `pip install matplotlib numpy` 后 `python3 scripts/visualize.py output/benchmark_results.json output/` 正常
