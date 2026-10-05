//! 市场流身份与 Bar 事件。价格使用有限 f64，绝对时间使用 UTC DateTime。
//!
//! `slot` 和 `known_at` 为绝对时间，`timeframe` 为持续时长；
//! 活动 Bar 只能更新尾部，已闭合 Bar 经质量门后才能进入结构管线。

use chrono::{DateTime, TimeDelta, Utc};

/// 去包含时使用的市场方向；保留原实现的初始方向口径。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketDirection {
    /// 高低点向上移动。
    Upward,
    /// 高低点向下移动。
    Downward,
    /// 无净位移；旧包含算法在吸收时按向下分支处理，不代表方向已获证。
    NoNetDisplacement,
}

/// 一个市场、标的和周期共同确定唯一 Bar 流。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BarStreamIdentity {
    /// 市场标识；不能为空。
    pub market: String,
    /// 标的标识；不能为空。
    pub instrument: String,
    /// 单根 Bar 的时间跨度；必须大于零。
    pub timeframe: TimeDelta,
}

/// 质量门的完整输入载荷；公开构造不等于已经通过质量检查。
#[derive(Clone, Debug, PartialEq)]
pub struct QualityBar {
    /// 所属市场流。
    pub stream: BarStreamIdentity,
    /// Bar 开始时间；同一流内用它标识一根 Bar。
    pub slot: DateTime<Utc>,
    /// 开盘价，必须为有限数值。
    pub open: f64,
    /// 最高价，必须为有限数值；不得低于开盘价或收盘价。
    pub high: f64,
    /// 最低价，必须为有限数值；不得高于开盘价或收盘价。
    pub low: f64,
    /// 收盘价或活动 Bar 的最新价，必须为有限数值。
    pub close: f64,
    /// 成交量，使用 u64，单位由调用方固定。
    pub volume: u64,
    /// 可选成交额；缺失与零值不同。
    pub turnover: Option<u64>,
    /// 该版本数据的获知时间；闭合时不得早于 slot + timeframe。
    pub known_at: DateTime<Utc>,
    /// 是否已经闭合；闭合历史不得被普通更新改写。
    pub closed: bool,
}

/// 按严格连续的事件序号驱动质量门；首个序号为 1。
#[derive(Clone, Debug, PartialEq)]
pub enum BarStreamEvent {
    /// 追加新 Bar，或幂等重送当前闭合尾部。
    Append {
        /// 本次事件序号，应等于上次成功接收的序号加一。
        event_sequence: u64,
        /// 要追加的完整 Bar。
        bar: QualityBar,
    },
    /// 替换同一身份的活动尾；可以更新行情，也可以将尾部闭合。
    UpdateActiveTail {
        /// 本次事件序号；被拒绝的事件不消耗序号。
        event_sequence: u64,
        /// 更新后的完整 Bar，不是局部字段补丁。
        bar: QualityBar,
    },
}
