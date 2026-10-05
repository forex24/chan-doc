//! One authoritative Bar ledger and Dafny F2BarQualityGate branch order.

use crate::input::{BarStreamEvent, BarStreamIdentity, QualityBar};
use crate::number::Nat;
use std::cmp::Ordering;

#[derive(Debug, PartialEq, Eq)]
pub struct BarQualityState {
    pub stream: BarStreamIdentity,
    pub bars: Vec<QualityBar>,
    pub last_event_sequence: Nat,
}
#[derive(Debug, PartialEq, Eq)]
pub enum QualityClassification {
    NoQualityAnomaly,
    OutOfOrderAppend,
    DuplicateIdentityDifferentPayload,
    ActiveTailAlreadyPublished,
    StreamIdentityMismatch,
    InvalidBarPayload,
    ActiveTailRequiresUpdate,
    EventSequenceConflict,
}
#[derive(Debug, PartialEq, Eq)]
pub enum QualityFormalResult {
    NormalAcceptance,
    ExplicitRejection,
    FactConflict,
}
#[derive(Debug, PartialEq, Eq)]
pub enum QualityAcceptance {
    AppendedBar,
    IdempotentDuplicate,
    UpdatedActiveTail,
    ClosedActiveTail,
}
#[derive(Debug, PartialEq, Eq)]
pub enum QualityGateDecision {
    GateAccepted {
        result: QualityFormalResult,
        classification: QualityClassification,
        acceptance: QualityAcceptance,
        next: BarQualityState,
        structure_material: Option<QualityBar>,
    },
    GateBlocked {
        result: QualityFormalResult,
        classification: QualityClassification,
        preserved: BarQualityState,
    },
}
pub fn is_valid_stream_identity(stream: &BarStreamIdentity) -> bool {
    !stream.market.is_empty()
        && !stream.instrument.is_empty()
        && stream.timeframe.cmp_exact(&Nat::zero()) == Ordering::Greater
}
pub fn is_valid_quality_bar(bar: &QualityBar) -> bool {
    is_valid_stream_identity(&bar.stream)
        && bar.low.cmp_exact(&bar.open) != Ordering::Greater
        && bar.open.cmp_exact(&bar.high) != Ordering::Greater
        && bar.low.cmp_exact(&bar.close) != Ordering::Greater
        && bar.close.cmp_exact(&bar.high) != Ordering::Greater
        && bar.slot.cmp_exact(&bar.known_at) != Ordering::Greater
        && (!bar.closed
            || bar.slot.add(&bar.stream.timeframe).cmp_exact(&bar.known_at) != Ordering::Greater)
}
pub fn empty_bar_quality_state(
    stream: BarStreamIdentity,
) -> Result<BarQualityState, BarStreamIdentity> {
    if !is_valid_stream_identity(&stream) {
        return Err(stream);
    }
    Ok(BarQualityState {
        stream,
        bars: Vec::new(),
        last_event_sequence: Nat::zero(),
    })
}
fn same_bar_identity(first: &QualityBar, second: &QualityBar) -> bool {
    if first.stream != second.stream {
        return false;
    }
    match first.slot.cmp_exact(&second.slot) {
        Ordering::Equal => true,
        _ => false,
    }
}
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
fn copy_bar(bar: &QualityBar) -> QualityBar {
    QualityBar {
        stream: BarStreamIdentity {
            market: bar.stream.market.clone(),
            instrument: bar.stream.instrument.clone(),
            timeframe: bar.stream.timeframe.clone(),
        },
        slot: bar.slot.clone(),
        open: bar.open.clone(),
        high: bar.high.clone(),
        low: bar.low.clone(),
        close: bar.close.clone(),
        volume: bar.volume.clone(),
        turnover: match &bar.turnover {
            Some(value) => Some(value.clone()),
            None => None,
        },
        known_at: bar.known_at.clone(),
        closed: bar.closed,
    }
}
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
fn classify_append(
    mut state: BarQualityState,
    event_sequence: Nat,
    bar: QualityBar,
) -> QualityGateDecision {
    match event_sequence.cmp_exact(&state.last_event_sequence.add(&Nat::from_u64(1))) {
        Ordering::Equal => {}
        _ => {
            return blocked(
                state,
                QualityFormalResult::FactConflict,
                QualityClassification::EventSequenceConflict,
            );
        }
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
        match bar.slot.cmp_exact(&last.slot) {
            Ordering::Greater => {}
            _ => {
                if same_bar_identity(last, &bar) && same_bar_payload(last, &bar) {
                    state.last_event_sequence = event_sequence;
                    {};
                    {};
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
    }
    let material = if bar.closed {
        Some(copy_bar(&bar))
    } else {
        None
    };
    state.bars.push(bar);
    state.last_event_sequence = event_sequence;
    {};
    {};
    accepted(state, QualityAcceptance::AppendedBar, material)
}
fn classify_update_active_tail(
    mut state: BarQualityState,
    event_sequence: Nat,
    bar: QualityBar,
) -> QualityGateDecision {
    match event_sequence.cmp_exact(&state.last_event_sequence.add(&Nat::from_u64(1))) {
        Ordering::Equal => {}
        _ => {
            return blocked(
                state,
                QualityFormalResult::FactConflict,
                QualityClassification::EventSequenceConflict,
            );
        }
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
    if bar.known_at.cmp_exact(&last.known_at) == Ordering::Less {
        return blocked(
            state,
            QualityFormalResult::FactConflict,
            QualityClassification::InvalidBarPayload,
        );
    }
    let closing = bar.closed;
    let material = if closing { Some(copy_bar(&bar)) } else { None };
    state.bars.pop();
    state.bars.push(bar);
    state.last_event_sequence = event_sequence;
    {};
    {};
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
