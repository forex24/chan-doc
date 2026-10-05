//! F1 的去包含、分型与增量状态；只消费质量门放行的闭合 Bar。

use chrono::{DateTime, Utc};
pub mod feature;
pub mod fractal;
pub mod inclusion;
pub mod morphology;
pub mod segment;
pub mod stroke;

use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};

/// 闭合 Bar 的结构计算投影，只保留算法需要的价格与时间。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawBar {
    /// 原始 Bar 开始时间。
    pub slot: DateTime<Utc>,
    /// 原始最低价。
    pub low: f64,
    /// 原始最高价。
    pub high: f64,
    /// 该闭合 Bar 的获知时间。
    pub known_at: DateTime<Utc>,
}
/// 按包含方向折叠后的 K 线；除最后一根外均已冻结。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombinedBar {
    /// 首个来源 Bar 的开始时间。
    pub first_slot: DateTime<Utc>,
    /// 最后一个来源 Bar 的开始时间。
    pub last_slot: DateTime<Utc>,
    /// 按方向合并后的低价。
    pub low: f64,
    /// 按方向合并后的高价。
    pub high: f64,
    /// 下一次包含合并使用的方向。
    pub direction: MarketDirection,
    /// 该合并 K 线首次形成的获知时间。
    pub formed_known_at: DateTime<Utc>,
    /// 全部已吸收来源的最大获知时间。
    pub known_at: DateTime<Utc>,
    /// 低价极值来源 Bar 的开始时间；同价保留最早来源。
    pub low_slot: DateTime<Utc>,
    /// 高价极值来源 Bar 的开始时间；同价保留最早来源。
    pub high_slot: DateTime<Utc>,
}
/// 分型类型；普通分型要求高低价都形成严格极值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FractalKind {
    /// 顶分型。
    Top,
    /// 底分型。
    Bottom,
}
/// 已检测到形状的分型；右邻冻结之前仍可变化。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FractalPoint {
    /// 顶或底。
    pub kind: FractalKind,
    /// 中心 K 线在 F1State.combined 中的下标。
    pub center_index: usize,
    /// 检测到当前形状时右邻 K 线的获知时间。
    pub detected_known_at: DateTime<Utc>,
}
/// 右邻 K 线已冻结的分型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfirmedFractal {
    /// 冻结的分型形状和中心位置。
    pub point: FractalPoint,
    /// 使右邻冻结的新合并 K 线的首次获知时间。
    pub confirmed_known_at: DateTime<Utc>,
}
/// 去包含与分型的增量状态。
#[derive(Clone, Debug, PartialEq)]
pub struct F1State {
    /// 唯一市场流身份。
    pub stream: BarStreamIdentity,
    /// 首次去包含使用的方向前提。
    pub initial_direction: MarketDirection,
    /// 去包含 K 线账本；最后一根为可吸收新输入的活动尾。
    pub combined: Vec<CombinedBar>,
    /// 按中心位置递增的已确认分型账本。
    pub confirmed: Vec<ConfirmedFractal>,
    /// 右邻仍为活动尾的候选分型。
    pub forming: Option<FractalPoint>,
}
/// 单根闭合 Bar 产生的去包含和分型变化。
#[derive(Clone, Debug, PartialEq)]
pub struct F1Delta {
    /// 被新的非包含 K 线推出活动尾的旧合并 K 线。
    pub finalized_combined: Option<CombinedBar>,
    /// 本次右邻冻结后新确认的分型。
    pub confirmed_fractal: Option<ConfirmedFractal>,
    /// 推进后仍待右邻冻结的候选分型。
    pub forming: Option<FractalPoint>,
    /// 是否发生包含吸收；首次建立尾部时为 false。
    pub tail_updated: bool,
}
/// 闭合 Bar 进入形态层前的拒绝原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum F1InputError {
    /// 输入尚未闭合。
    ActiveBar,
    /// OHLC、时间或市场流字段不合法。
    InvalidBar,
    /// 输入与状态的市场流不同。
    StreamMismatch,
    /// 输入开始时间不晚于已消费的最后一根 Bar。
    OutOfOrder,
}
/// 建立空的去包含和分型状态。
pub fn new_f1_state(stream: BarStreamIdentity, initial_direction: MarketDirection) -> F1State {
    F1State {
        stream,
        initial_direction,
        combined: Vec::new(),
        confirmed: Vec::new(),
        forming: None,
    }
}
/// 提取结构计算字段；调用方应先确认 Bar 已闭合并通过质量门。
pub fn raw_from_closed(bar: &QualityBar) -> RawBar {
    RawBar {
        slot: bar.slot,
        low: bar.low,
        high: bar.high,
        known_at: bar.known_at,
    }
}
