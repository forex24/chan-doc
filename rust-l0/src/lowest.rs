//! 将新增的已确认线段投影为 L0，并只向完成账本追加。
//!
//! L0 类型本身表示最低级已完成线段，不携带高层走势的层级标签或子材料。
//! 线段/L0 失败时回退这两层；已经接收该 Bar 的形态层保留推进结果。

use crate::f1::feature::{PriceInterval, SegmentDirection};
use crate::f1::morphology::{
    MorphologyDelta, MorphologyState, advance_morphology, new_morphology_state,
};
use crate::f1::segment::{
    ConfirmedSegment, SegmentError, SegmentState, advance_segments, new_segment_state,
};
use crate::f1::stroke::{StrokeState, center_position};
use crate::f1::{F1InputError, F1State, FractalKind};
use crate::input::{BarStreamIdentity, MarketDirection, QualityBar};
use chrono::{DateTime, Utc};

/// L0 端点；结构位置和实际极值时间是两种不同的坐标。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarketEndpoint {
    /// 分型中心在去包含 K 线数组中的下标，不是原始 Bar 序号。
    pub market_order: usize,
    /// 该端点极值实际来源 Bar 的开始时间。
    pub open_time: DateTime<Utc>,
    /// 端点价格。
    pub price: f64,
}

/// 一条由已确认线段投影得到的 L0；更高层走势需定义自己的类型。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LowestMovement {
    /// 起点；应与上一条 L0 的终点完全相同。
    pub start_endpoint: MarketEndpoint,
    /// 终点；结构位置和时间均晚于起点。
    pub end_endpoint: MarketEndpoint,
    /// 线段方向；L0 不存在零位移方向。
    pub direction: SegmentDirection,
    /// 整条线段的价格范围，包含内部极值，不只取两端点。
    pub full_range: PriceInterval,
    /// 线段确认后可发布该 L0 的时间。
    pub known_at: DateTime<Utc>,
}

impl LowestMovement {
    /// 检查端点顺序、确认时间、方向和完整价格范围是否自洽。
    ///
    /// 不检查相邻 L0 的衔接；该条件由管线追加时验证。
    pub fn is_valid(&self) -> bool {
        [
            self.start_endpoint.price,
            self.end_endpoint.price,
            self.full_range.low,
            self.full_range.high,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.start_endpoint.market_order < self.end_endpoint.market_order
            && self.start_endpoint.open_time < self.end_endpoint.open_time
            && self.end_endpoint.open_time <= self.known_at
            && self.full_range.low <= self.full_range.high
            && self.full_range.low <= self.start_endpoint.price
            && self.start_endpoint.price <= self.full_range.high
            && self.full_range.low <= self.end_endpoint.price
            && self.end_endpoint.price <= self.full_range.high
            && match self.direction {
                SegmentDirection::Up => self.start_endpoint.price < self.end_endpoint.price,
                SegmentDirection::Down => self.end_endpoint.price < self.start_endpoint.price,
            }
    }
}

/// 沿用 F1 的完整左边界前提；该声明不授予 F2 整体完成或升层资格。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompleteBoundary {
    /// 首条 L0 起点在去包含 K 线数组中的下标。
    pub root_point: usize,
    /// 第一根合并 K 线使用的包含方向；必须明确为向上或向下。
    pub initial_inclusion_direction: MarketDirection,
    /// 边界证据获知时间，须落在首条 L0 起点时间与确认时间之间。
    pub known_at: DateTime<Utc>,
}

/// Bar 到 L0 的组合状态；各层已确认前缀只能追加。
#[derive(Clone, Debug, PartialEq)]
pub struct F1PipelineState {
    /// 去包含、分型和笔的状态。
    pub morphology: MorphologyState,
    /// 线段识别状态与已确认线段账本。
    pub segment: SegmentState,
    /// 已完成 L0 账本，与已确认线段逐条对应。
    pub lowest_movements: Vec<LowestMovement>,
    /// 调用方提供的左边界前提。
    pub left_boundary: CompleteBoundary,
}

/// 单次闭合 Bar 推进新产生的结果。
#[derive(Clone, Debug, PartialEq)]
pub struct F1PipelineDelta {
    /// 去包含、分型、笔的增量。
    pub morphology: MorphologyDelta,
    /// 本次新确认的线段，按发布顺序排列。
    pub segments: Vec<ConfirmedSegment>,
    /// 本次新完成的 L0，与 segments 逐条对应。
    pub lowest_movements: Vec<LowestMovement>,
}

/// 完整管线失败的层次。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineError {
    /// 形态层拒绝输入，整条管线保持原状态。
    F1(F1InputError),
    /// 线段源或状态不合法；保留推进后的形态层。
    Segment(SegmentError),
    /// L0 几何、相邻衔接或左边界校验失败；保留推进后的形态层。
    LowestMovementInvalid,
}

/// 从分型中心找回极值来源 Bar 的时间，而非使用合并 K 线的起始时间。
fn point_endpoint(f1: &F1State, point_index: usize, price: &f64) -> Option<MarketEndpoint> {
    let point = &f1.confirmed.get(point_index)?.point;
    let center = center_position(&f1.combined, point)?;
    let open_time = match point.kind {
        FractalKind::Top => f1.combined[center].high_slot,
        FractalKind::Bottom => f1.combined[center].low_slot,
    };
    Some(MarketEndpoint {
        market_order: point.center_index,
        open_time,
        price: *price,
    })
}

/// 将一条已确认线段投影为 L0；来源下标或几何不合法时返回 None。
pub fn movement_of_segment(
    f1: &F1State,
    strokes: &StrokeState,
    segment: &ConfirmedSegment,
) -> Option<LowestMovement> {
    let start_point = strokes
        .confirmed
        .get(segment.begin_stroke_index)?
        .geometry
        .begin_index;
    // end_stroke_index 是下一段首笔，取它的起点作为当前段终点。
    let end_point = strokes
        .confirmed
        .get(segment.end_stroke_index)?
        .geometry
        .begin_index;
    let movement = LowestMovement {
        start_endpoint: point_endpoint(f1, start_point, &segment.start_price)?,
        end_endpoint: point_endpoint(f1, end_point, &segment.end_price)?,
        direction: segment.direction,
        full_range: segment.price_interval,
        known_at: segment.times.confirmed_at,
    };
    movement.is_valid().then_some(movement)
}

/// 建立空管线；首次推进拒绝未知包含方向，前三笔冻结时校验起点重叠，
/// 首次发布时再核验 root_point 和边界获知时间。
pub fn new_pipeline_state(
    stream: BarStreamIdentity,
    boundary: CompleteBoundary,
) -> F1PipelineState {
    F1PipelineState {
        morphology: new_morphology_state(stream, boundary.initial_inclusion_direction),
        segment: new_segment_state(),
        lowest_movements: Vec::new(),
        left_boundary: boundary,
    }
}

/// 后续段共享端点、方向交替且获知时间不回退；首段校验显式左边界。
fn can_append_lowest(
    previous: Option<&LowestMovement>,
    next: &LowestMovement,
    boundary: &CompleteBoundary,
) -> bool {
    if let Some(last) = previous {
        last.end_endpoint == next.start_endpoint
            && last.direction != next.direction
            && last.known_at <= next.known_at
    } else {
        boundary.root_point == next.start_endpoint.market_order
            && next.start_endpoint.open_time <= boundary.known_at
            && boundary.known_at <= next.known_at
    }
}

/// 消费一根新获准的闭合 Bar，仅投影新增线段，不重建旧 L0。
///
/// 形态层拒绝时返回原状态；线段或 L0 失败时返回已推进的形态层、
/// 原线段状态和原 L0 账本。后一种错误不能再次提交同一根 Bar；
/// 调用方必须先处理错误原因，再继续推进。
#[expect(
    clippy::result_large_err,
    reason = "成功分支已经携带更大的状态与增量；错误按值归还状态，避免额外分配"
)]
pub fn advance_pipeline(
    state: F1PipelineState,
    bar: &QualityBar,
) -> Result<(F1PipelineState, F1PipelineDelta), (F1PipelineState, PipelineError)> {
    let F1PipelineState {
        morphology,
        segment,
        lowest_movements,
        left_boundary,
    } = state;
    let (morphology, morphology_delta) = match advance_morphology(morphology, bar) {
        Ok(value) => value,
        Err((preserved, reason)) => {
            return Err((
                F1PipelineState {
                    morphology: preserved,
                    segment,
                    lowest_movements,
                    left_boundary,
                },
                PipelineError::F1(reason),
            ));
        }
    };

    // 线段可能回放先前的笔；保留本轮推进前快照以支持失败时的层级回退。
    let preserved_segment = segment.clone();
    let (segment, segments) = match advance_segments(segment, &morphology.f1, &morphology.stroke) {
        Ok(value) => value,
        Err((_, reason)) => {
            return Err((
                F1PipelineState {
                    morphology,
                    segment: preserved_segment,
                    lowest_movements,
                    left_boundary,
                },
                PipelineError::Segment(reason),
            ));
        }
    };
    let mut movement_delta = Vec::with_capacity(segments.len());
    for completed in &segments {
        let next = movement_of_segment(&morphology.f1, &morphology.stroke, completed);
        let preceding = movement_delta.last().or_else(|| lowest_movements.last());
        let Some(next) = next.filter(|next| can_append_lowest(preceding, next, &left_boundary))
        else {
            return Err((
                F1PipelineState {
                    morphology,
                    segment: preserved_segment,
                    lowest_movements,
                    left_boundary,
                },
                PipelineError::LowestMovementInvalid,
            ));
        };
        movement_delta.push(next);
    }

    // 整个新增批次通过后才写入账本，避免前几段成功、后续失败时留下半批结果。
    let mut lowest_movements = lowest_movements;
    lowest_movements.extend(movement_delta.iter().copied());
    Ok((
        F1PipelineState {
            morphology,
            segment,
            lowest_movements,
            left_boundary,
        },
        F1PipelineDelta {
            morphology: morphology_delta,
            segments,
            lowest_movements: movement_delta,
        },
    ))
}
