# Rust L0

本目录实现 **Bar → 质量门 → 去包含 → 分型 → 笔 → 线段 → L0**。只包含最低层结构，不包含 F2 的中枢、递归升层、同级分解、背驰或策略。

## 数据类型

- 价格：`f64`。质量门拒绝 `NaN` 和正负无穷；比较仍使用规则要求的严格大小或相等关系，不加入模糊容差。
- 绝对时间：`chrono::DateTime<Utc>`。包括 Bar 开始时间、极值来源时间、形成时间和确认时间。时间不在内部转换为 Unix 秒。
- 周期：`chrono::TimeDelta`，必须为正时长。闭合时间使用 `checked_add_signed`，越界则拒绝。
- 数量、成交额和事件序号：`u64`。数量单位由调用方固定；可选成交额用 `Option<u64>`。事件序号耗尽时拒绝后续事件，不回绕。
- 集合下标：`usize`。分型中心直接访问去包含 K 线数组，不再构造大整数并线性查找。
- 小型值对象使用 `Copy`；包含字符串或集合的状态使用 `Clone`；只读序列接口接收切片。

外部数据在输入边界解析成 UTC 时间。日志和导出示例使用 `to_rfc3339()`，保留时区及亚秒精度。二进制浮点价格不再承诺任意精度整数的精确性；原来依赖超大整数的输入不属于当前合同。

## 正式入口

1. 用 `quality::empty_bar_quality_state(stream)` 建立质量账本，通过 `apply_quality_event` 输入事件。
2. 用 `lowest::new_pipeline_state(stream, boundary)` 建立结构管线。
3. 仅将 `GateAccepted.structure_material` 中新产生的闭合 Bar 交给 `lowest::advance_pipeline`。活动尾更新和幂等重送不一定产生材料。
4. 消费 `F1PipelineDelta.lowest_movements`，或读取 `F1PipelineState.lowest_movements` 的完整账本。

`LowestMovement` 直接包含起点、终点、方向、全价格范围和确认时间。`market_order` 表示分型中心在去包含 K 线数组中的下标，`open_time` 表示真实极值来源 Bar 的开始时间，两者不能混用。

`full_range` 来自起终端点实际时间之间的原始 Bar 闭区间。`F1State.raw` 只保留计算所需的时间、高低价和获知时间。这样可以保留未达到成笔条件的短波动，以及被包含合并去掉的原始极值；终点之后才吸收到同一合并 K 的 Bar 不计入前段。

`CompleteBoundary` 仍是原 F1 实现的调用前提。首条输入不是天然完整市场起点；其 `root_point` 必须与首条 L0 起点的合并 K 线下标一致。它不授予 F2 整体完成或升层资格。R4 的已批准业务边界见 [规则总表 §2.5](../缠论业务规则总表.md#25-r4未知左边界下的局部能力已批准)，本库尚未实现 F2/R4。

初始包含方向必须明确为 `Upward` 或 `Downward`；`NoNetDisplacement` 在首次推进时返回 `F1(InvalidInitialDirection)`。第三笔冻结时，管线检查声明起点的前三笔公共重叠；不成立则返回 `Segment(InvalidInitialOverlap)`，不发布 L0，也不静默跳过前面的笔。该起点不能靠追加行情修复，调用方须提供合法左边界及其相应输入重新回放。

候选按原始笔顺序处理。较早候选已形成第一特征分型，仍等待真实笔破坏或第二序列时，后面的内部候选不能抢先发布。原文67/71/78课确定确认、等待和失效条件；多候选的具体调度采用参考项目 `chan-core-2026-final@758953c` 的 `ChanSegmentCalculator::cal_seg_sure/treat_fx_eigen` 工作方式。参考项目的第二序列递归与排除包含不在此引入范围，仍执行77/78课的普通包含、严格分型和不递归规定。

价格区间按闭区间处理，单价相触归入无缺口；真实笔破坏仍使用严格突破。两者含义不同，此处与参考项目 `EigenFx::update_fx/actual_break` 的比较边界一致。第二序列右元素在同一根笔上越过旧极值且已形成严格分型时，先按第二序列确认；只有尚未形成第二分型就创新高/新低，才取消旧候选。这两个时序均有完整回放。

错误返回的状态边界：

- 质量门拒绝：账本和事件序号不变。
- 形态层拒绝：整条结构管线不变。
- 线段或 L0 失败：保留已经推进的形态层，恢复本轮推进前的线段状态和 L0 账本。不能再次提交同一根 Bar；先处理错误原因，再继续推进。

字段、函数和关键算法的中文说明见各模块源码及 `cargo doc` 生成的文档。完整组合用法见 [组合 E2E](tests/pipeline_e2e.rs)。

## 一字板包含规则

在通常的向上 `max/max`、向下 `min/min` 合并之外，吸收参考项目 `chan-core-2026-final` 的一字板特例：

- 向上合并时，若新 Bar 的 `high == low == 当前合并 K 线 high`，保留原高低价。
- 向下合并时，若新 Bar 的 `high == low == 当前合并 K 线 low`，保留原高低价。
- 两种情况都继续记录新 Bar 的开始时间和最大获知时间；形成时间及极值来源保持原值。连续一字板重复应用同一规则。
- 内部一字板、反向边缘一字板及普通 Bar 仍按通常包含规则处理；非包含的一字板建立新合并 K 线。正式管线拒绝尚未确定的初始方向。

这样可避免顺向边缘一字板把原价格区间压成一点。只引入这个区间特例，同价极值仍保留本库规定的最早来源。参考定位：`chan-core-2026-final@758953c` 的 `crates/chan-engine/src/kline.rs::merge_klu_into_klc`，以及 `pychan/Combiner/KLine_Combiner.py::try_add`。

## 旧接口迁移

这是一次公开数据类型调整，调用方需要同步迁移。

| 旧接口 | 当前接口 |
| --- | --- |
| `Int`、`Nat` 与 `number` 模块 | 原生 `f64`、`u64`、`usize` 和 chrono 日期/时长 |
| `KnownCompleteMovement.key` / `MovementKey` | 扁平的 `LowestMovement` |
| `level_ordinal`、`composition`、`direct_materials` | 删除；L0 类型已确定层级及来源种类 |
| `state.lowest.completed` | `state.lowest_movements` |
| `movement_key_is_valid` | `LowestMovement::is_valid` |
| 手写 `copy_*` 函数 | `Clone::clone` 或直接复制 `Copy` 值 |
| 笔的两个同步分型计数 | `processed_point_count` |
| 笔的重复 `begin_point_index` | `geometry.begin_index` |
| 跨调用的 `PendingReverse` | 反向笔成立时在同一调用内确认前笔 |
| `SegmentTimes.occurred_at` | 删除重复时间，使用 `candidate_known_at` |
| 未使用的 `input::CompleteBoundary` / `LeftMarketBoundary` | 只保留管线实际使用的 `lowest::CompleteBoundary` |

## 构建和验证

需要支持 Rust 2024 edition 的工具链。直接依赖只有 chrono，关闭时钟和本地时区功能，只启用 `std`。

```sh
cargo fmt --manifest-path rust-l0/Cargo.toml --check
cargo clippy --offline --release --manifest-path rust-l0/Cargo.toml --all-targets -- -D warnings
cargo test --offline --release --manifest-path rust-l0/Cargo.toml
cargo build --offline --release --manifest-path rust-l0/Cargo.toml
cargo doc --offline --no-deps --manifest-path rust-l0/Cargo.toml
```

首次获取依赖时可去掉 `--offline`。测试不需要 Verus、Dafny、Python 或原工作树。

## 验证来源及边界

- 保留既有质量、包含/分型、笔、线段测试场景。原始 Dafny 执行观测在 [fixtures](tests/fixtures) 和 [历史抽取证据](evidence/extraction-receipt.json) 中保持原样。
- 原 249 条冻结观测中，q20 使用超出当前数值/日期合同的任意精度数据，明确排除；u05 的4条未知方向默认向下观测由明确拒绝的新合同替代；其余244条继续比较。历史夹具保持原样。`tests/common` 只在期望侧将历史场景的整数时间按 Unix 秒转换为 RFC 3339。
- 新增质量门到 L0 的组合回放基线，录自重构前 `54c2ded`。上下镜像各 128 根 Bar，共 14 条 L0；另一组使用小数价格、1970 年以前的 UTC 日期和纳秒时间，按相同已冻结期望进行变换对照。输出位于 `target/e2e/`。
- 类型边界 E2E 检查有限极值价格、每个 OHLC 字段的非有限值拒绝、日期加法越界、非正周期及事件序号耗尽。
- [一字板 E2E](tests/limit_board_e2e.rs) 在实现前复现压扁区间问题。12 组边界场景覆盖上下方向及普通包含；另在上下镜像各 128 根基础 Bar 中插入 256 根连续一字板，共处理 768 根含一字板 Bar，逐步对照未插入序列的 L0 发布结果，共完成 14 条 L0。检查冻结前缀、极值来源及时间推进，生成 `target/e2e/limit-board-*.txt`。
- [启动边界 E2E](tests/f1_start_e2e.rs) 先复现后修复：上下镜像各122根 Bar，在第三笔冻结后拒绝无公共重叠的首段；继续输入仍不发布伪 L0，不移动声明起点。另一场景验证未知初始方向在修改状态前被拒绝。修复前反例及源文件摘要见 [收据](evidence/f1-initial-overlap-before.json)。
- [原文规则 E2E](tests/f1_rules_e2e.rs) 用15组数值结构及上下镜像，从2,968根 Bar 检查无缺口、跨分界包含、第一序列同价、有缺口待确认、第二序列不递归、第二序列普通包含和严格分型、首笔真实破坏、候选失效、内部极值、单价重叠及两类等待候选的发布顺序，并检查缺口确认后的连续三段衔接。期望端点、候选时间、发布时间和确认分支事先明确；这些数值结构不是对示意图像素的行情还原。抢先发布的修复前反例见 [收据](evidence/f1-candidate-order-before.json)。
- [原始全区间 E2E](tests/segment_range_e2e.rs) 覆盖不足成笔的反向波动、包含折叠掉的原始极值及终点后的来源排除，上下镜像各两种情形。修复前4例均漏报内部极值，见 [收据](evidence/f1-full-range-before.json)。
- [真实 OHLC 回放](tests/market_replay_e2e.rs) 使用4,320根连续1分钟 Bar及其镜像，共发布50条 L0，检查历史冻结、镜像和实际原始 Bar 价格范围。输入片段已保存在本库，来源、SHA-256和UTC时间解释见 [输入收据](evidence/market-fixture.json)。这是有限片段的正式路径检查，不是独立业务 oracle 或全年回放。
- 删除任意精度数值模块及其专属测试。旧抽取日志只描述历史版本，不能当作本次验证结果。
- 未重新执行形式化证明、BTC/EUR 全年回放或性能基准；本次结果不证明全部缠论规则，也不证明 F2。

## 历史来源

代码最初从 `/Users/gxj/.codex/worktrees/f2-combination-closure/dafny/f2-authority/verus-core` 抽取，使用 Verus 官方宏的 `EraseAll` 转换剥离证明；逐文件来源见 [抽取收据](evidence/extraction-receipt.json)。

当前版本已在此独立库中简化数据类型和状态表示，不再是未经修改的执行层副本。参考 `chan-core-2026-final` 的原生类型、值枚举、模型注释方式及上述一字板特例；其余算法业务口径以本库冻结行为和 [规则总表](../缠论业务规则总表.md) 为准。
