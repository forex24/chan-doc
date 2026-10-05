mod common;

use chrono::{DateTime, TimeDelta, Utc};
// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::input::{BarStreamEvent, BarStreamIdentity, QualityBar};
use chan_l0::quality::{
    BarQualityState, QualityAcceptance, QualityClassification, QualityFormalResult,
    QualityGateDecision, apply_quality_event, empty_bar_quality_state,
};
fn n(value: u64) -> u64 {
    value
}
fn i(value: i64) -> f64 {
    value as f64
}
fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "NORMALIZED".into(),
        timeframe: TimeDelta::seconds(1),
    }
}
fn time(value: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(value as i64, 0).unwrap()
}
fn bar(stream: BarStreamIdentity, slot: u64, known_at: u64, closed: bool) -> QualityBar {
    QualityBar {
        stream,
        slot: time(slot),
        open: i(10),
        high: i(12),
        low: i(9),
        close: i(11),
        volume: n(1),
        turnover: None,
        known_at: time(known_at),
        closed,
    }
}
fn state(bars: &[QualityBar], seq: u64) -> BarQualityState {
    BarQualityState {
        stream: stream(),
        bars: bars.to_vec(),
        last_event_sequence: n(seq),
    }
}
fn append(seq: u64, b: &QualityBar) -> BarStreamEvent {
    BarStreamEvent::Append {
        event_sequence: n(seq),
        bar: b.clone(),
    }
}
fn update(seq: u64, b: &QualityBar) -> BarStreamEvent {
    BarStreamEvent::UpdateActiveTail {
        event_sequence: n(seq),
        bar: b.clone(),
    }
}
fn bar_text(b: &QualityBar) -> String {
    format!(
        "{{{},{},{},{},{},{},{},{},{},{},{},{}}}",
        b.stream.market,
        b.stream.instrument,
        b.stream.timeframe.num_seconds(),
        b.slot.to_rfc3339(),
        b.open,
        b.high,
        b.low,
        b.close,
        b.volume,
        b.turnover.as_ref().map_or("_".into(), u64::to_string),
        b.known_at.to_rfc3339(),
        b.closed
    )
}
fn result_code(v: &QualityFormalResult) -> u8 {
    match v {
        QualityFormalResult::NormalAcceptance => 0,
        QualityFormalResult::ExplicitRejection => 1,
        QualityFormalResult::FactConflict => 2,
    }
}
fn class_code(v: &QualityClassification) -> u8 {
    match v {
        QualityClassification::NoQualityAnomaly => 0,
        QualityClassification::OutOfOrderAppend => 1,
        QualityClassification::DuplicateIdentityDifferentPayload => 2,
        QualityClassification::ActiveTailAlreadyPublished => 3,
        QualityClassification::StreamIdentityMismatch => 4,
        QualityClassification::InvalidBarPayload => 5,
        QualityClassification::ActiveTailRequiresUpdate => 6,
        QualityClassification::EventSequenceConflict => 7,
    }
}
fn acceptance_code(v: &QualityAcceptance) -> u8 {
    match v {
        QualityAcceptance::AppendedBar => 0,
        QualityAcceptance::IdempotentDuplicate => 1,
        QualityAcceptance::UpdatedActiveTail => 2,
        QualityAcceptance::ClosedActiveTail => 3,
    }
}
fn render(name: &str, decision: &QualityGateDecision) -> String {
    let (prefix, next, material) = match decision {
        QualityGateDecision::GateAccepted {
            result,
            classification,
            acceptance,
            next,
            structure_material,
        } => (
            format!(
                "A|{}|{}|{}",
                result_code(result),
                class_code(classification),
                acceptance_code(acceptance)
            ),
            next,
            structure_material.as_ref(),
        ),
        QualityGateDecision::GateBlocked {
            result,
            classification,
            preserved,
        } => (
            format!("B|{}|{}|-", result_code(result), class_code(classification)),
            preserved,
            None,
        ),
    };
    let mut text = format!(
        "{name}|{prefix}|{}|{}|S",
        next.last_event_sequence,
        next.bars.len()
    );
    for b in &next.bars {
        text.push_str(&bar_text(b));
    }
    text.push_str(if material.is_some() { "|M1" } else { "|M0" });
    if let Some(b) = material {
        text.push_str(&bar_text(b));
    }
    text
}

fn cases() -> Vec<(String, BarQualityState, BarStreamEvent)> {
    let s = stream();
    let closed = bar(s.clone(), 1, 2, true);
    let active = bar(s.clone(), 1, 1, false);
    let active2 = bar(s.clone(), 1, 2, false);
    let other = BarStreamIdentity {
        market: "OTHER".into(),
        instrument: "NORMALIZED".into(),
        timeframe: TimeDelta::seconds(1),
    };
    let mut bad = closed.clone();
    bad.high = i(8);
    let mut other_bad = bad.clone();
    other_bad.stream = other;
    let mut changed_high = closed.clone();
    changed_high.high = i(13);
    let closed2 = bar(s.clone(), 2, 3, true);
    let active_last = bar(s.clone(), 2, 2, false);
    let mut cases = Vec::new();
    let mut case_number = 0;
    macro_rules! case {
        ($state:expr, $event:expr) => {{
            case_number += 1;
            cases.push((format!("q{case_number:02}"), $state, $event));
        }};
    }
    case!(state(&[], 0), append(1, &closed));
    case!(state(&[], 0), append(1, &active));
    case!(state(std::slice::from_ref(&closed), 1), append(2, &closed));
    case!(
        state(std::slice::from_ref(&closed), 1),
        append(2, &changed_high)
    );
    case!(
        state(std::slice::from_ref(&closed), 1),
        append(2, &bar(s.clone(), 0, 1, true))
    );
    case!(state(std::slice::from_ref(&active), 1), append(2, &active));
    case!(state(std::slice::from_ref(&active), 1), update(2, &active2));
    case!(state(std::slice::from_ref(&active), 1), update(2, &closed));
    case!(state(std::slice::from_ref(&active2), 1), update(2, &active));
    case!(state(&[], 0), update(1, &active));
    case!(
        state(std::slice::from_ref(&active), 1),
        update(2, &bar(s.clone(), 2, 2, false))
    );
    case!(state(std::slice::from_ref(&closed), 1), update(2, &closed));
    case!(state(&[], 0), append(2, &other_bad));
    case!(state(&[], 0), append(1, &other_bad));
    case!(state(&[], 0), append(1, &bad));
    case!(state(&[], 0), append(1, &bar(s.clone(), 1, 1, true)));
    case!(state(std::slice::from_ref(&active), 1), append(2, &bad));
    case!(
        state(std::slice::from_ref(&active), 1),
        append(2, &bar(s.clone(), 0, 1, true))
    );
    case!(
        state(std::slice::from_ref(&closed), 1),
        append(2, &bar(s.clone(), 1, 3, true))
    );
    // 原 q20 依赖超出 u64/i64 的值，退出当前数值合同；极限值见组合 E2E。
    case_number += 1;
    case!(state(&[], 0), append(1, &bar(s.clone(), 2, 1, false)));
    case!(
        state(std::slice::from_ref(&active), 1),
        update(2, &other_bad)
    );
    case!(state(&[], 0), update(1, &bad));
    case!(
        state(std::slice::from_ref(&active), 1),
        update(3, &bar(s.clone(), 2, 2, false))
    );
    case!(state(std::slice::from_ref(&closed), 1), append(2, &closed2));
    case!(
        state(&[closed.clone(), closed2.clone()], 2),
        append(3, &closed2)
    );
    case!(
        state(&[closed.clone(), active_last], 2),
        update(3, &closed2)
    );
    cases
}

#[test]
fn frozen_quality_oracle_and_state_invariants() {
    assert!(empty_bar_quality_state(stream()).is_ok());
    let mut invalid = stream();
    invalid.market.clear();
    assert!(empty_bar_quality_state(invalid).is_err());
    let stdout = include_str!("fixtures/quality.txt");
    let expected: Vec<String> = common::oracle_lines(stdout, "quality")
        .into_iter()
        .filter(|line| line.starts_with('q') && !line.starts_with("q20|"))
        .collect();
    assert_eq!(expected.len(), 26, "missing Dafny oracle cases: {stdout}");
    let actual_cases = cases();
    assert_eq!(actual_cases.len(), expected.len());
    for ((name, prior, event), oracle) in actual_cases.into_iter().zip(expected) {
        let old_bars: Vec<String> = prior.bars.iter().map(bar_text).collect();
        let old_seq = prior.last_event_sequence;
        let decision = apply_quality_event(prior, event);
        assert_eq!(render(&name, &decision), oracle, "case {name}");
        match decision {
            QualityGateDecision::GateBlocked { preserved, .. } => {
                assert_eq!(preserved.last_event_sequence, old_seq);
                assert_eq!(
                    preserved.bars.iter().map(bar_text).collect::<Vec<_>>(),
                    old_bars
                );
            }
            QualityGateDecision::GateAccepted {
                next,
                structure_material,
                ..
            } => {
                assert_eq!(next.last_event_sequence, old_seq + 1);
                assert!(structure_material.as_ref().is_none_or(|bar| bar.closed));
                assert!(
                    next.bars
                        .iter()
                        .take(next.bars.len().saturating_sub(1))
                        .all(|bar| bar.closed)
                );
                assert!(
                    next.bars
                        .windows(2)
                        .all(|pair| pair[0].slot.cmp(&pair[1].slot).is_lt())
                );
                for (before, after) in old_bars.iter().zip(next.bars.iter()) {
                    if before.ends_with("true}") {
                        assert_eq!(before, &bar_text(after));
                    }
                }
            }
        }
    }
}
