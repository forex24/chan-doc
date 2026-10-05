//! Bar 质量门、去包含、分型、笔、线段和 L0 投影。
//!
//! 价格使用有限 f64，绝对时间使用 `DateTime<Utc>`，周期使用 `TimeDelta`。
//! 数量和事件序号使用 u64，集合下标使用 usize；不依赖 F2 或证明运行时。
#![deny(missing_docs)]

pub mod f1;
pub mod input;
pub mod lowest;
pub mod quality;
