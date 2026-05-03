# sim-mem-rs - 大模型训练内存模拟器

面向大模型训练的内存模拟器，使用离散事件仿真（DES）技术，评估不同内存分配策略在大模型训练场景下的性能表现。

## 项目结构

```
sim-mem-rs/
├── Cargo.toml              # 项目配置与依赖
├── README.md               # 项目文档
├── src/
│   ├── main.rs             # 命令行入口
│   ├── lib.rs              # 库入口，核心接口
│   ├── engine/
│   │   └── mod.rs          # 离散事件仿真引擎 (DES)
│   ├── memory/
│   │   ├── mod.rs          # 内存分配器接口 (Allocator trait)
│   │   ├── naive.rs        # 连续内存分配器 (首次适应算法)
│   │   └── paged.rs        # 分页内存分配器 (页表管理)
│   ├── workload/
│   │   └── mod.rs          # 工作负载生成器
│   ├── metrics/
│   │   └── mod.rs          # 性能指标收集与分析
├── benches/
│   └── allocator_benchmarks.rs  # Criterion 基准测试
├── scripts/
│   └── visualize.py         # Python 可视化脚本
├── output/                  # 图表输出目录
└── tests/                   # 集成测试
```

## 各模块职责

### engine - 仿真引擎
离散事件仿真核心，管理事件队列和时间推进。使用最小堆实现事件优先级队列。

**核心流程**：
1. 从事件队列取出最早事件
2. 更新仿真时间为事件时间
3. 处理事件（请求到达、内存分配/释放、请求完成）
4. 可能生成新事件加入队列

### memory - 内存分配器
定义 `Allocator` trait 及其实现：
- **NaiveAllocator**: 首次适应连续分配，简单但易产生外部碎片
- **PagedAllocator**: 固定大小页分配，减少外部碎片但可能产生内部碎片

### workload - 工作负载
生成模拟大模型训练请求，支持：
- 指数分布（请求到达间隔，模拟泊松过程）
- 正态分布（请求生命周期、内存大小）
- 确定性模式（可重复测试）

### metrics - 性能指标
收集和分析仿真过程中的关键指标：
- 内存使用率、峰值内存
- 分配成功率
- 碎片率（外部/内部）
- 请求完成时间

### visualization - 可视化
使用 plotters 生成 PNG 图表：
- 内存使用时间序列图
- 碎片率变化图
- 分配成功率对比图

## 快速开始

### 安装依赖

```bash
# Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python 可视化（可选，用于生成图表）
pip install matplotlib numpy
```

### 运行测试

```bash
# 运行所有单元测试
cargo test

# 运行测试并显示输出
cargo test -- --nocapture

# 运行特定模块测试
cargo test --lib memory
```

### 运行基准测试

```bash
# 运行基准测试比较 Naive vs Paged 分配器
cargo run --release -- benchmark

# 自定义参数
cargo run --release -- benchmark -d 2000 -m 2048 -r 500 -o output
```

### 运行单个仿真

```bash
# 使用连续分配器
cargo run --release -- simulate -a naive -m 1024 -r 100

# 使用分页分配器
cargo run --release -- simulate -a paged -m 1024 -r 100
```

### 运行 Criterion 基准测试

```bash
cargo bench
# 结果在 target/criterion/report/index.html
```

## 测试数据说明

测试数据按照以下逻辑生成：

### 请求到达
使用**指数分布**（λ=1）模拟泊松到达过程，请求间隔独立同分布。

### 请求生命周期
使用**正态分布**，均值由参数指定，标准差为均值的20%，最小值为1。

### 内存请求大小
使用**正态分布**，均值由参数指定，标准差为均值的30%，最小值为1。

### 测试覆盖
- **单元测试**: 每个模块独立测试（25个测试用例）
- **Doctest**: 文档示例代码测试（8个测试用例）
- **基准测试**: Criterion 微基准测试

## 输出文件

运行 benchmark 命令后，在 `output/` 目录生成：

- `memory_usage.png` - 内存使用时间序列对比图
- `fragmentation.png` - 碎片率变化对比图
- `allocation_success_rate.png` - 分配成功率对比图

## 扩展指南

### 添加新的分配器

1. 在 `src/memory/` 下创建新文件
2. 实现 `Allocator` trait
3. 在 `src/memory/mod.rs` 中导出

```rust
pub struct MyAllocator { ... }

impl Allocator for MyAllocator {
    fn allocate(&mut self, size: usize) -> Option<MemoryBlock> { ... }
    fn deallocate(&mut self, request_id: usize) -> bool { ... }
    fn name(&self) -> &str { "MyAllocator" }
    fn stats(&self) -> &AllocatorStats { ... }
    fn used_memory(&self) -> usize { ... }
    fn total_memory(&self) -> usize { ... }
    fn fragmentation_ratio(&self) -> f64 { ... }
    fn has_contiguous_memory(&self, size: usize) -> bool { ... }
    fn max_contiguous_block(&self) -> usize { ... }
}
```

## 许可

MIT OR Apache-2.0