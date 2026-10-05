# Rust L0 独立副本

本目录只包含已有 Rust 实现中的 **Bar → 去包含 → 分型 → 笔 → 线段 → L0**，以及质量门和精确整数依赖。没有复制 F2 的中枢、解释集合、递归升层、同级分解、背驰、策略或来源调度引擎。

## 构建和验证

```sh
cargo test --manifest-path rust-l0/Cargo.toml
cargo build --release --manifest-path rust-l0/Cargo.toml
```

从本目录运行时省略 `--manifest-path`。需要支持 Rust 2024 edition 的工具链；本次使用 rustc 1.99.0。依赖只有 `num-bigint`、`num-traits` 及其传递依赖。**运行、构建和测试均不需要 Verus、Dafny、Python 或原工作树。** 首次构建需要下载 Cargo.lock 中的 crates，已有缓存时可加 `--offline`。

## 入口与模块

| 模块 | 用途 |
| --- | --- |
| `input` | Bar、市场流身份、追加与活动尾更新事件 |
| `quality` | 序号、流身份、时间、OHLC、活动尾及已闭合历史保护 |
| `f1::inclusion` / `fractal` | 去包含、分型检测与确认 |
| `f1::stroke` / `morphology` | 增量笔与合并推进 |
| `f1::feature` / `segment` | 特征序列、缺口/无缺口识别、线段推进 |
| `lowest` | 完成线段到 L0 的投影、追加账本、完整管线 |
| `number` | 任意精度整数、自然数和精确运算 |

正式组合入口：

1. `quality::empty_bar_quality_state` 建立质量状态；`apply_quality_event` 接收 `BarStreamEvent`。
2. 仅将 `GateAccepted.structure_material` 中的已闭合 Bar 交给 `lowest::advance_pipeline`。活动 Bar 更新可能没有结构材料。
3. 使用 `lowest::new_pipeline_state(stream, boundary)` 建立管线。`advance_pipeline` 返回新状态与增量；出错返回保留状态和错误。
4. 消费 `F1PipelineDelta.lowest_movements`。这些才是此次新增的完成 L0；不要将形成中的分型、笔或线段当成已完成 L0。

完整用法和逐 Bar 状态检查见 [线段 E2E](tests/f1_segment.rs) 的 `fixed_history_reaches_segment_owner`；质量输入用法见 [质量门测试](tests/quality.rs)。

`CompleteBoundary` 是沿用原实现的调用前提。**首条输入不是天然完整市场起点**，本次抽取没有新增未知左边界恢复算法，也不把该参数升级为 F2 的完成授权。`slot`、`timeframe`、`known_at` 的单位和市场流必须由调用方一致提供；价格缩放由调用方固定，内部不使用浮点替代。

原 L0 输出沿用通用 `MovementKey` 形状。因此类型中仍有 `level_ordinal` 和 `ConsolidationMovement` / `TrendMovement` 枚举标签，`input` 中仍有边界描述类型。它们仅是原有公共数据定义；本管线只产生 `level_ordinal = 0`、`FrozenLowestSegment`，不存在 L1+ 执行器。

## Verus 剥离方式

来源：`/Users/gxj/.codex/worktrees/f2-combination-closure/dafny/f2-authority/verus-core`。按文件记录的 SHA256 见 [抽取收据](evidence/extraction-receipt.json)。

采用 Verus 官方 `verus_builtin_macros 0.0.0-2026-09-20-0158` 的 `EraseAll` 语法转换，导出编译器执行层 token，再用 rustfmt 格式化。没有用正则猜测并删除嵌套的证明块。随后移除 `vstd` 导入、`Int` / `Nat` 的空 `View` 证明接口、`ExBigInt` 证明包装和两个只供证明的算术演示函数。原市场执行函数体保留；宏展开会移除内部注释，并可能留下无操作空块。

剥离后的代码不携带形式化证明保证。这里交付的是可独立编译的既有执行实现，不是重新设计的算法，也没有借本次复制修复或优化原实现。

## 本次验证及边界

- 迁移原有 14 项行为测试，覆盖质量拒绝时状态保持、包含处理、分型、笔及分块一致性、镜像行情、线段两类识别、实际破坏条件、完整管线和错误边界回滚。
- 对照素材来自本次执行原项目的 4 个既有 Dafny oracle（`run --no-verify`），共 249 条观测：质量 27、包含/分型 26、笔 62、线段 134。不是用抽取后的代码自生成期望值。
- 测试保留原场景和断言，只把外部 oracle 调用与 Python 比较器替换为冻结文本和逐值断言。对照文件在 [tests/fixtures](tests/fixtures)，原始输出与命令收据在 [evidence](evidence)。
- 未重做形式化证明，未执行 BTC/EUR 全年回放，未验证规则穷尽性。原实现的边界前提、效率和业务限制仍然存在。

完整业务讨论和未决问题见上级 [缠论业务规则总表](../缠论业务规则总表.md)。其中 F2 规则用于后续独立开发，不表示本目录已经实现 F2。
