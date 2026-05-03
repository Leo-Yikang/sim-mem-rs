
## Phase 0: 核心框架与基础基准测试 (MVP Release - v0.2.0)
目标： 完成离散事件仿真（DES）的基础骨架，建立连续内存分配（Naive）与分页内存分配（Paged）的性能对比基准。

## Phase 1: 引入调度器机制 (v0.3 - 动态批处理时代)
当前你的引擎可能是“请求来了就有资源就跑”，这一阶段需要引入真正的 LLM 调度理念，模拟 Orca 和 vLLM 的核心调度逻辑。

### v0.3.1: 拆分调度器 (Scheduler) 模块

功能: 在 Engine 和 Allocator 之间增加一层 Scheduler。引擎负责时间推进，调度器负责决定哪些 Request 可以运行。

解释: 真实系统中，分配内存和“决定谁来跑”是分开的。调度器需要维护三个队列：Waiting (等待执行)、Running (正在执行)、Swapped/Preempted (被抢占)。

### v0.3.2: 连续批处理 (Continuous Batching / Iteration-level scheduling)

功能: 实现 token 级别的调度。每个时钟周期（Tick），调度器检查当前 Running 队列，如果有人生成结束（EOS），立刻将其踢出并从 Waiting 队列补入新的请求。

解释: 这是目前所有高效 LLM 引擎的标配，打破了静态 Batching 的短板，极大提高利用率。

### v0.3.3: 细化吞吐与延迟指标 (Metrics 2.0)

功能: 不再只看利用率和碎片率，加入 LLM 核心业务指标：

TTFT (Time To First Token): 首 token 延迟（模拟 Prefill 阶段消耗）。

TPOT (Time Per Output Token): 每次解码的延迟（模拟 Decode 阶段消耗）。

JCT (Job Completion Time): 端到端完成时间。

解释: 科研论文中评估分配器好坏，最终都要落脚到这些用户体验指标上。

## Phase 2: 高级内存管理机制 (v0.4 - 逼近 vLLM 完全体)
这是最硬核的部分，实现这部分后，你的模拟器就可以用来做主流学术论文的 Baseline 了。

### v0.4.1: 抢占与重计算/交换 (Preemption & Swapping)

功能: 当 Running 队列中的请求继续生成 token，导致显存 OOM 时，调度器需要选择一个受害者（通常是最后进入的请求），将其踢出。

实现:

Recomputation: 丢弃其所有 KV Cache Block，下次轮到它时从头开始 Prefill。

Swapping: 将 Block 移动到 CPU 内存池（引入一个慢速的 CpuAllocator），下次执行时再 Swap in。

### v0.4.2: 物理块与逻辑块解耦 (Logical to Physical Mapping)

功能: 完善 PagedAllocator，实现类似操作系统的页表（Page Table）。每个 Request 拥有一张逻辑页表，映射到物理 Block 数组。

解释: vLLM 的 PagedAttention 核心就是这个。这为你后续实现复杂的共享机制打下基础。

### v0.4.3: 前缀缓存共享 (Prefix Caching / Radix Attention)

功能: 模拟 SGLang 或 vLLM 的 Chunked Prefill & Prefix Cache。如果两个 Request 有相同的前缀（比如同一个 System Prompt），它们的物理 Block 应该是共享的（引入引用计数 Reference Count）。

实现: 可以在 allocator 目录下新增 RadixAllocator。

## Phase 3: 真实负载与多维度开销建模 (v0.5 - 提升仿真保真度)
只有用真实数据集跑出的数据，才能说服审稿人。

### v0.5.1: 真实负载追踪回放 (Trace Replay)

功能: 改进 workload_gen.py，支持直接读取开源的真实对话数据集（如 ShareGPT, Alpaca, LMSYS-Chat-1M），提取其 prompt/completion 长度分布并生成 workload。

功能: 引入到达率（Arrival Rate）的泊松过程模拟，测试不同 QPS (Queries Per Second) 压力下系统的崩溃点。

### v0.5.2: 引入模型权重与静态开销估算

功能: 显存不能 100% 用于 KV Cache。增加启动参数 -m <model_size> (如 LLaMA3-8B)，在初始化时自动扣除模型权重（Weights）和临时激活（Activation）占用的显存，只把剩余的留给 Allocator。

### v0.5.3: 变长耗时机制 (Variable Tick Duration)

功能: 目前的 Time 可能是 1个单位 = 1个 token。真实情况下，Prefill 阶段计算 1000 个 token 和 Decode 阶段生成 1 个 token 的耗时是不同的。需要根据 Concurrency 和计算量动态决定每一步的时间跨度。

## Phase 4: 前沿技术架构扩展 (v0.6 - 冲击顶会水平)
当你的模拟器能够模拟多卡和新型注意力机制时，它就成了一个强大的科研探索工具。

### v0.6.1: 异构与多层级内存模拟 (Multi-tier Memory)

功能: 不仅仅区分 GPU 和 CPU 内存，加入带宽（Bandwidth）概念。Swap in/out 时计算数据传输带来的时间延迟（Tick 惩罚）。

### v0.6.2: GQA/MQA (分组查询注意力) 内存换算

功能: Request 中需要加入模型参数的感知。不同模型（如 MHA vs GQA）一个 token 占据的 KV Cache 大小完全不同。实现基于 (层数 * 隐层维度 / head数 * block_size) 的真实 Byte 级别内存模拟，而不仅仅是 "1 Token = 1 内存单位"。

### v0.6.3: 多卡张量并行 / 流水线并行模拟 (分布式扩展)

功能: 模拟多个 Worker（多张卡），每个卡有自己的 Allocator，但在张量并行下它们的状态必须同步；在流水线并行下，内存分配呈现明显的波峰波谷错位。

## Phase 5: 生态打磨与科研可视化 (v1.0 - 开源发布级)
使你的项目变得“好用”、“好看”、“好扩展”。

### v1.0.1: 基于配置文件的全自动化实验跑批

功能: 编写类似 experiments.yaml，一键运行 Naive vs Paged vs Radix 在不同 QPS、不同模型大小下的十组对比实验，并自动生成报告。

### v1.0.2: 高级科研级可视化

功能: 改进 Python 绘图脚本。生成科研论文常用的 CDF (累积分布函数) 图（用于展示 TTFT/TPOT P99 尾部延迟），以及堆叠柱状图（展示显存到底用在了 Weights, KV Cache, Fragmentation 还是 Reserved 上）。

### v1.0.3: 提供 Python Bindings (可选)

功能: 使用 pybind11 将 C++ 引擎封装成 Python 库，这样其他研究人员可以用 Python 编写调度策略，底层由你的 C++ 高速执行。