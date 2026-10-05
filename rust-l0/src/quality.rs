//! 唯一 Bar 账本和质量门；保留既有拒绝分支优先级，只向结构层发送闭合材料。

use crate::input::{BarStreamEvent, BarStreamIdentity, QualityBar};
use chrono::TimeDelta;

/// 质量门已接收的市场流状态。
#[derive(Clone, Debug, PartialEq)]
pub struct BarQualityState {
    /// 唯一市场流身份。
    pub stream: BarStreamIdentity,
    /// 已接收的 Bar；至多最后一根仍活动。
    pub bars: Vec<QualityBar>,
    /// 最后一次成功接收的序号；拒绝不消耗序号。
    pub last_event_sequence: u64,
}
/// 质量门的具体分类，用于区分拒绝原因。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityClassification {
    /// 无质量异常。
    NoQualityAnomaly,
    /// 追加时间未严格前进且不是尾部幂等重送。
    OutOfOrderAppend,
    /// 同一闭合尾部身份携带不同载荷。
    DuplicateIdentityDifferentPayload,
    /// 没有可更新的同身份活动尾，或尾部已经闭合。
    ActiveTailAlreadyPublished,
    /// 市场流身份不一致。
    StreamIdentityMismatch,
    /// 价格非有限、OHLC 不合法、获知时间不合法或时间加法溢出。
    InvalidBarPayload,
    /// 已有活动尾，必须先更新或闭合它。
    ActiveTailRequiresUpdate,
    /// 事件序号不连续，或上一个序号已达 u64 上限。
    EventSequenceConflict,
}
/// 质量结论的业务性质，与具体原因分开报告。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityFormalResult {
    /// 正常接收。
    NormalAcceptance,
    /// 当前操作不被允许。
    ExplicitRejection,
    /// 事件与输入事实或账本顺序冲突。
    FactConflict,
}
/// 成功接收事件后的具体动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityAcceptance {
    /// 追加一根新 Bar。
    AppendedBar,
    /// 幂等重送，没有新增结构材料。
    IdempotentDuplicate,
    /// 更新活动尾，尚未闭合。
    UpdatedActiveTail,
    /// 活动尾闭合，首次向结构层发布。
    ClosedActiveTail,
}
/// 质量门决策；接收返回新状态，拒绝返回原状态。
#[derive(Clone, Debug, PartialEq)]
pub enum QualityGateDecision {
    /// 输入被接收。
    GateAccepted {
        /// 业务性质。
        result: QualityFormalResult,
        /// 具体质量分类。
        classification: QualityClassification,
        /// 成功动作。
        acceptance: QualityAcceptance,
        /// 应用事件后的账本。
        next: BarQualityState,
        /// 唯一获准进入结构管线的新闭合 Bar；其他情况为 None。
        structure_material: Option<QualityBar>,
    },
    /// 输入被拒绝，账本不变。
    GateBlocked {
        /// 业务性质。
        result: QualityFormalResult,
        /// 具体质量分类。
        classification: QualityClassification,
        /// 未被改写的原账本。
        preserved: BarQualityState,
    },
}
/// 检查市场和标的非空、周期为正时长。
pub fn is_valid_stream_identity(stream: &BarStreamIdentity) -> bool {
    !stream.market.is_empty()
        && !stream.instrument.is_empty()
        && stream.timeframe > TimeDelta::zero()
}
/// 检查有限价格、OHLC 与时间顺序；闭合时间使用受检加法，越界时拒绝。
pub fn is_valid_quality_bar(bar: &QualityBar) -> bool {
    is_valid_stream_identity(&bar.stream)
        && [bar.open, bar.high, bar.low, bar.close]
            .into_iter()
            .all(f64::is_finite)
        && bar.low <= bar.open
        && bar.open <= bar.high
        && bar.low <= bar.close
        && bar.close <= bar.high
        && bar.slot <= bar.known_at
        && (!bar.closed
            || bar
                .slot
                .checked_add_signed(bar.stream.timeframe)
                .is_some_and(|close_time| close_time <= bar.known_at))
}
/// 建立空质量账本，首个成功事件序号必须为 1；流身份非法时原样返回。
pub fn empty_bar_quality_state(
    stream: BarStreamIdentity,
) -> Result<BarQualityState, BarStreamIdentity> {
    if !is_valid_stream_identity(&stream) {
        return Err(stream);
    }
    Ok(BarQualityState {
        stream,
        bars: Vec::new(),
        last_event_sequence: 0,
    })
}
/// 同一市场流和开始时间确定同一根 Bar。
fn same_bar_identity(first: &QualityBar, second: &QualityBar) -> bool {
    if first.stream != second.stream {
        return false;
    }
    first.slot == second.slot
}
/// 比较行情与获知元数据；身份由调用方另行比较。
fn same_bar_payload(first: &QualityBar, second: &QualityBar) -> bool {
    first.open == second.open
        && first.high == second.high
        && first.low == second.low
        && first.close == second.close
        && first.volume == second.volume
        && first.turnover == second.turnover
        && first.known_at == second.known_at
        && first.closed == second.closed
}
/// 构造拒绝结果并交还未修改的状态。
fn blocked(
    state: BarQualityState,
    result: QualityFormalResult,
    classification: QualityClassification,
) -> QualityGateDecision {
    QualityGateDecision::GateBlocked {
        result,
        classification,
        preserved: state,
    }
}
/// 构造成功结果，按需携带新闭合结构材料。
fn accepted(
    state: BarQualityState,
    acceptance: QualityAcceptance,
    structure_material: Option<QualityBar>,
) -> QualityGateDecision {
    QualityGateDecision::GateAccepted {
        result: QualityFormalResult::NormalAcceptance,
        classification: QualityClassification::NoQualityAnomaly,
        acceptance,
        next: state,
        structure_material,
    }
}
/// 依次检查序号、流身份、载荷、活动尾和时间顺序；只有完全相同的闭合尾允许幂等重送。
fn classify_append(
    mut state: BarQualityState,
    event_sequence: u64,
    bar: QualityBar,
) -> QualityGateDecision {
    if state.last_event_sequence.checked_add(1) != Some(event_sequence) {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::EventSequenceConflict,
        );
    }
    if bar.stream != state.stream {
        return blocked(
            state,
            QualityFormalResult::ExplicitRejection,
            QualityClassification::StreamIdentityMismatch,
        );
    }
    if !is_valid_quality_bar(&bar) {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::InvalidBarPayload,
        );
    }
    if let Some(last) = state.bars.last() {
        if !last.closed {
            return blocked(
                state,
                QualityFormalResult::ExplicitRejection,
                QualityClassification::ActiveTailRequiresUpdate,
            );
        }
        if bar.slot <= last.slot {
            if same_bar_identity(last, &bar) && same_bar_payload(last, &bar) {
                state.last_event_sequence = event_sequence;

                return accepted(state, QualityAcceptance::IdempotentDuplicate, None);
            }
            if same_bar_identity(last, &bar) {
                return blocked(
                    state,
                    QualityFormalResult::FactConflict,
                    QualityClassification::DuplicateIdentityDifferentPayload,
                );
            }
            return blocked(
                state,
                QualityFormalResult::FactConflict,
                QualityClassification::OutOfOrderAppend,
            );
        }
    }
    let material = if bar.closed { Some(bar.clone()) } else { None };
    state.bars.push(bar);
    state.last_event_sequence = event_sequence;

    accepted(state, QualityAcceptance::AppendedBar, material)
}
/// 仅替换同一身份的活动尾，获知时间不得回退；闭合时首次产生结构材料。
fn classify_update_active_tail(
    mut state: BarQualityState,
    event_sequence: u64,
    bar: QualityBar,
) -> QualityGateDecision {
    if state.last_event_sequence.checked_add(1) != Some(event_sequence) {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::EventSequenceConflict,
        );
    }
    if bar.stream != state.stream {
        return blocked(
            state,
            QualityFormalResult::ExplicitRejection,
            QualityClassification::StreamIdentityMismatch,
        );
    }
    if !is_valid_quality_bar(&bar) {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::InvalidBarPayload,
        );
    }
    let Some(last) = state.bars.last() else {
        return blocked(
            state,
            QualityFormalResult::ExplicitRejection,
            QualityClassification::ActiveTailAlreadyPublished,
        );
    };
    if !same_bar_identity(last, &bar) || last.closed {
        return blocked(
            state,
            QualityFormalResult::ExplicitRejection,
            QualityClassification::ActiveTailAlreadyPublished,
        );
    }
    if bar.known_at < last.known_at {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::InvalidBarPayload,
        );
    }
    let closing = bar.closed;
    let material = if closing { Some(bar.clone()) } else { None };
    state.bars.pop();
    state.bars.push(bar);
    state.last_event_sequence = event_sequence;

    accepted(
        state,
        if closing {
            QualityAcceptance::ClosedActiveTail
        } else {
            QualityAcceptance::UpdatedActiveTail
        },
        material,
    )
}
/// 应用追加或活动尾更新事件；拒绝时不修改账本和事件序号。
pub fn apply_quality_event(state: BarQualityState, event: BarStreamEvent) -> QualityGateDecision {
    match event {
        BarStreamEvent::Append {
            event_sequence,
            bar,
        } => classify_append(state, event_sequence, bar),
        BarStreamEvent::UpdateActiveTail {
            event_sequence,
            bar,
        } => classify_update_active_tail(state, event_sequence, bar),
    }
}
