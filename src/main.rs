//! # 大模型训练内存模拟器 - 命令行界面

use clap::{Parser, Subcommand};
use sim_mem_rs::{
    SimulationConfig, run_simulation, run_benchmark,
    memory::{NaiveAllocator, PagedAllocator},
};
use std::fs;
use std::path::Path;

/// 大模型训练内存模拟器
#[derive(Parser)]
#[command(name = "sim-mem")]
#[command(about = "大模型训练内存模拟器")]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 运行基准测试，比较不同分配器性能，生成JSON数据和可视化图表
    Benchmark {
        #[arg(short, long, default_value = "1000")]
        duration: u64,
        #[arg(short, long, default_value = "1024")]
        memory: usize,
        #[arg(short, long, default_value = "100")]
        requests: usize,
        #[arg(short, long, default_value = "output")]
        output: String,
    },
    /// 运行单个仿真实验
    Simulate {
        #[arg(short, long, value_enum, default_value = "naive")]
        allocator: AllocatorType,
        #[arg(short, long, default_value = "1000")]
        duration: u64,
        #[arg(short, long, default_value = "1024")]
        memory: usize,
        #[arg(short, long, default_value = "100")]
        requests: usize,
        #[arg(short, long, default_value = "50")]
        lifetime: u64,
        #[arg(long, default_value = "10")]
        memory_size: usize,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum AllocatorType {
    Naive,
    Paged,
}

fn main() {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Benchmark { duration, memory, requests, output } => {
            run_benchmark_command(duration, memory, requests, &output);
        }
        Commands::Simulate { allocator, duration, memory, requests, lifetime, memory_size } => {
            run_simulate_command(allocator, duration, memory, requests, lifetime, memory_size);
        }
    }
}

fn run_benchmark_command(duration: u64, memory: usize, requests: usize, output: &str) {
    println!("运行基准测试...");
    println!("仿真时长: {} 时间单位", duration);
    println!("内存大小: {} 内存单位", memory);
    println!("请求数量: {}", requests);
    
    let config = SimulationConfig {
        duration,
        memory_size: memory,
        num_requests: requests,
        avg_lifetime: 50,
        avg_memory_size: 10,
    };
    
    let results = run_benchmark(config);
    
    let reports: Vec<_> = results.iter()
        .map(|r| r.metrics.clone().finalize())
        .collect();
    
    println!("\n基准测试结果:");
    for report in &reports {
        println!("\n分配器: {}", report.allocator_name);
        println!("  分配成功率: {:.2}%", report.success_rate * 100.0);
        println!("  峰值内存使用: {}", report.peak_memory_usage);
        println!("  平均碎片率: {:.4}", report.avg_fragmentation);
        println!("  完成请求数: {}", report.completed_requests);
    }
    
    // 输出JSON数据
    fs::create_dir_all(output).expect("无法创建输出目录");
    let json_path = Path::new(output).join("benchmark_results.json");
    let json_data = serde_json::to_string_pretty(&reports).expect("序列化失败");
    fs::write(&json_path, &json_data).expect("写入JSON失败");
    println!("\nJSON数据已保存到 {:?}", json_path);
    
    // 调用Python可视化脚本
    let script_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let python_script = script_dir.join("scripts").join("visualize.py");
    
    if python_script.exists() {
        println!("正在生成可视化图表...");
        let status = std::process::Command::new("python3")
            .arg(&python_script)
            .arg(&json_path)
            .arg(output)
            .status();
        
        match status {
            Ok(s) if s.success() => println!("可视化图表已生成到 {} 目录", output),
            Ok(s) => eprintln!("Python脚本返回错误码: {}", s),
            Err(e) => eprintln!("无法运行Python可视化脚本: {}\n提示: 请安装Python3和matplotlib: pip install matplotlib", e),
        }
    } else {
        println!("提示: 创建 scripts/visualize.py 脚本以生成图表");
        println!("或者手动安装matplotlib后运行: python3 scripts/visualize.py {:?} {}", json_path, output);
    }
}

fn run_simulate_command(
    allocator_type: AllocatorType,
    duration: u64,
    memory: usize,
    requests: usize,
    lifetime: u64,
    memory_size: usize,
) {
    println!("运行仿真实验...");
    println!("分配器类型: {:?}", allocator_type);
    println!("仿真时长: {} 时间单位", duration);
    println!("内存大小: {} 内存单位", memory);
    println!("请求数量: {}", requests);
    
    let config = SimulationConfig {
        duration,
        memory_size: memory,
        num_requests: requests,
        avg_lifetime: lifetime,
        avg_memory_size: memory_size,
    };
    
    let allocator: Box<dyn sim_mem_rs::memory::Allocator> = match allocator_type {
        AllocatorType::Naive => Box::new(NaiveAllocator::new(memory)),
        AllocatorType::Paged => Box::new(PagedAllocator::new(memory, 64)),
    };
    
    let result = run_simulation(config, allocator);
    let report = result.metrics.clone().finalize();
    
    println!("\n仿真结果:");
    println!("分配器: {}", report.allocator_name);
    println!("分配成功率: {:.2}%", report.success_rate * 100.0);
    println!("峰值内存使用: {}", report.peak_memory_usage);
    println!("平均碎片率: {:.4}", report.avg_fragmentation);
}