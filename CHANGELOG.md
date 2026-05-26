# Changelog

All notable changes to sim-mem-rs will be documented in this file.

## [Unreleased]
### v0.3.0 (Phase 1 Complete)
- Added scheduler module with FCFS and Continuous Batching strategies
- Added LLM metrics: TTFT, TPOT, JCT (avg/p99)
- Extended Request model with prompt_tokens / output_tokens fields
- Added `run_scheduled()` engine path (tick-based)
- CLI: added `schedule-benchmark` subcommand and `--scheduler` flag