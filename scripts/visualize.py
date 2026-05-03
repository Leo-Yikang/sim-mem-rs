#!/usr/bin/env python3
"""
sim-mem-rs 可视化脚本

读取 benchmark_results.json 数据，生成性能对比图表：
1. 内存使用时间序列对比图
2. 碎片率变化对比图
3. 分配成功率对比图

使用方法：
    python3 visualize.py <results.json> <output_dir>
"""

import json
import sys
import os
from pathlib import Path

try:
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    import numpy as np
except ImportError:
    print("错误: 需要安装 matplotlib")
    print("运行: pip install matplotlib numpy")
    sys.exit(1)


def load_data(json_path: str) -> list:
    """加载JSON结果数据"""
    with open(json_path, 'r') as f:
        return json.load(f)


def plot_memory_usage(reports: list, output_dir: str):
    """生成内存使用时间序列对比图"""
    fig, ax = plt.subplots(figsize=(12, 6))
    
    colors = ['#2196F3', '#F44336', '#4CAF50', '#9C27B0', '#FF9800']
    
    for i, report in enumerate(reports):
        time_series = report.get('time_series', [])
        if not time_series:
            continue
        
        times = [p['time'] for p in time_series]
        memory = [p['allocated_memory'] for p in time_series]
        
        ax.plot(times, memory, color=colors[i % len(colors)], 
                linewidth=2, label=report['allocator_name'], alpha=0.8)
    
    ax.set_xlabel('Time', fontsize=12)
    ax.set_ylabel('Allocated Memory', fontsize=12)
    ax.set_title('Memory Usage Over Time', fontsize=14)
    ax.legend(loc='upper left', fontsize=10)
    ax.grid(True, alpha=0.3)
    
    plt.tight_layout()
    output_path = os.path.join(output_dir, 'memory_usage.png')
    fig.savefig(output_path, dpi=150)
    plt.close(fig)
    print(f"  -> {output_path}")


def plot_fragmentation(reports: list, output_dir: str):
    """生成碎片率变化对比图"""
    fig, ax = plt.subplots(figsize=(12, 6))
    
    colors = ['#2196F3', '#F44336', '#4CAF50', '#9C27B0', '#FF9800']
    
    for i, report in enumerate(reports):
        time_series = report.get('time_series', [])
        if not time_series:
            continue
        
        times = [p['time'] for p in time_series]
        fragmentation = [p['fragmentation'] for p in time_series]
        
        ax.plot(times, fragmentation, color=colors[i % len(colors)], 
                linewidth=2, label=report['allocator_name'], alpha=0.8)
    
    ax.set_xlabel('Time', fontsize=12)
    ax.set_ylabel('Fragmentation Ratio', fontsize=12)
    ax.set_title('Fragmentation Over Time', fontsize=14)
    ax.legend(loc='upper left', fontsize=10)
    ax.grid(True, alpha=0.3)
    ax.set_ylim(0, 1)
    
    plt.tight_layout()
    output_path = os.path.join(output_dir, 'fragmentation.png')
    fig.savefig(output_path, dpi=150)
    plt.close(fig)
    print(f"  -> {output_path}")


def plot_success_rate(reports: list, output_dir: str):
    """生成分配成功率对比图"""
    fig, ax = plt.subplots(figsize=(10, 6))
    
    names = [r['allocator_name'] for r in reports]
    rates = [r['success_rate'] * 100 for r in reports]
    
    colors = ['#2196F3', '#F44336', '#4CAF50', '#9C27B0', '#FF9800']
    bars = ax.bar(names, rates, color=colors[:len(names)], width=0.5, alpha=0.8)
    
    # 在柱状图上显示数值
    for bar, rate in zip(bars, rates):
        ax.text(bar.get_x() + bar.get_width()/2, bar.get_height() + 0.5,
                f'{rate:.2f}%', ha='center', va='bottom', fontsize=11, fontweight='bold')
    
    ax.set_ylabel('Success Rate (%)', fontsize=12)
    ax.set_title('Allocation Success Rate Comparison', fontsize=14)
    ax.set_ylim(0, 105)
    ax.grid(True, alpha=0.3, axis='y')
    
    plt.tight_layout()
    output_path = os.path.join(output_dir, 'allocation_success_rate.png')
    fig.savefig(output_path, dpi=150)
    plt.close(fig)
    print(f"  -> {output_path}")


def print_summary(reports: list):
    """打印性能摘要"""
    print("\n" + "=" * 60)
    print("性能对比摘要")
    print("=" * 60)
    
    for report in reports:
        print(f"\n分配器: {report['allocator_name']}")
        print(f"  仿真时长: {report['simulation_duration']}")
        print(f"  总分配次数: {report['total_allocations']}")
        print(f"  成功分配: {report['successful_allocations']}")
        print(f"  失败分配: {report['failed_allocations']}")
        print(f"  成功率: {report['success_rate'] * 100:.2f}%")
        print(f"  峰值内存: {report['peak_memory_usage']}")
        print(f"  平均内存: {report['avg_memory_usage']:.2f}")
        print(f"  最终碎片率: {report['final_fragmentation']:.4f}")
        print(f"  平均碎片率: {report['avg_fragmentation']:.4f}")
        print(f"  完成请求: {report['completed_requests']}")
        print(f"  平均完成时间: {report['avg_completion_time']:.2f}")


def main():
    if len(sys.argv) < 3:
        print("用法: python3 visualize.py <results.json> <output_dir>")
        sys.exit(1)
    
    json_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    if not os.path.exists(json_path):
        print(f"错误: 文件不存在 {json_path}")
        sys.exit(1)
    
    os.makedirs(output_dir, exist_ok=True)
    
    print(f"加载数据: {json_path}")
    reports = load_data(json_path)
    
    print(f"找到 {len(reports)} 个报告")
    print_summary(reports)
    
    print("\n生成图表...")
    plot_memory_usage(reports, output_dir)
    plot_fragmentation(reports, output_dir)
    plot_success_rate(reports, output_dir)
    
    print(f"\n所有图表已生成到 {output_dir}/")


if __name__ == '__main__':
    main()
