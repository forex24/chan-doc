// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::input::{BarStreamEvent, BarStreamIdentity, QualityBar};
use chan_l0::number::{Int, Nat};
use chan_l0::quality::{
    BarQualityState, QualityAcceptance, QualityClassification, QualityFormalResult,
    QualityGateDecision, apply_quality_event, empty_bar_quality_state,
};
fn n(value: u64) -> Nat {
    Nat::from_u64(value)
}
fn i(value: i64) -> Int {
    Int::from_i64(value)
}
fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "NORMALIZED".into(),
        timeframe: n(1),
    }
}
fn bar(stream: BarStreamIdentity, slot: u64, known_at: u64, closed: bool) -> QualityBar {
    QualityBar {
        stream,
        slot: n(slot),
        open: i(10),
        high: i(12),
        low: i(9),
        close: i(11),
        volume: n(1),
        turnover: None,
        known_at: n(known_at),
        closed,
    }
}
fn copy_stream(s: &BarStreamIdentity) -> BarStreamIdentity {
    BarStreamIdentity {
        market: s.market.clone(),
        instrument: s.instrument.clone(),
        timeframe: s.timeframe.clone(),
    }
}
fn copy_bar(b: &QualityBar) -> QualityBar {
    QualityBar {
        stream: copy_stream(&b.stream),
        slot: b.slot.clone(),
        open: b.open.clone(),
        high: b.high.clone(),
        low: b.low.clone(),
        close: b.close.clone(),
        volume: b.volume.clone(),
        turnover: b.turnover.clone(),
        known_at: b.known_at.clone(),
        closed: b.closed,
    }
}
fn state(bars: &[QualityBar], seq: u64) -> BarQualityState {
    BarQualityState {
        stream: stream(),
        bars: bars.iter().map(copy_bar).collect(),
        last_event_sequence: n(seq),
    }
}
fn append(seq: u64, b: &QualityBar) -> BarStreamEvent {
    BarStreamEvent::Append {
        event_sequence: n(seq),
        bar: copy_bar(b),
    }
}
fn update(seq: u64, b: &QualityBar) -> BarStreamEvent {
    BarStreamEvent::UpdateActiveTail {
        event_sequence: n(seq),
        bar: copy_bar(b),
    }
}
fn bar_text(b: &QualityBar) -> String {
    format!(
        "{{{},{},{},{},{},{},{},{},{},{},{},{}}}",
        b.stream.market,
        b.stream.instrument,
        b.stream.timeframe.decimal(),
        b.slot.decimal(),
        b.open.decimal(),
        b.high.decimal(),
        b.low.decimal(),
        b.close.decimal(),
        b.volume.decimal(),
        b.turnover.as_ref().map_or("_".into(), Nat::decimal),
        b.known_at.decimal(),
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
        next.last_event_sequence.decimal(),
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
    let closed = bar(copy_stream(&s), 1, 2, true);
    let active = bar(copy_stream(&s), 1, 1, false);
    let active2 = bar(copy_stream(&s), 1, 2, false);
    let other = BarStreamIdentity {
        market: "OTHER".into(),
        instrument: "NORMALIZED".into(),
        timeframe: n(1),
    };
    let mut bad = copy_bar(&closed);
    bad.high = i(8);
    let mut other_bad = copy_bar(&bad);
    other_bad.stream = other;
    let mut changed_high = copy_bar(&closed);
    changed_high.high = i(13);
    let huge_timeframe = Nat::from_decimal("18446744073709551617").unwrap();
    let huge_slot = Nat::from_decimal("18446744073709551619").unwrap();
    let huge = BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "NORMALIZED".into(),
        timeframe: huge_timeframe.clone(),
    };
    let huge_bar = QualityBar {
        stream: copy_stream(&huge),
        slot: huge_slot.clone(),
        open: Int::from_decimal("-1844674407370955161600000").unwrap(),
        high: Int::from_decimal("1844674407370955161600000").unwrap(),
        low: Int::from_decimal("-1844674407370955161600001").unwrap(),
        close: i(0),
        volume: Nat::from_decimal("1844674407370955161600000").unwrap(),
        turnover: Some(Nat::from_decimal("1844674407370955161600000").unwrap()),
        known_at: huge_slot.add(&huge_timeframe),
        closed: true,
    };
    let closed2 = bar(copy_stream(&s), 2, 3, true);
    let active_last = bar(copy_stream(&s), 2, 2, false);
    let mut cases = Vec::new();
    macro_rules! case {
        ($state:expr, $event:expr) => {{
            cases.push((format!("q{:02}", cases.len() + 1), $state, $event));
        }};
    }
    case!(state(&[], 0), append(1, &closed));
    case!(state(&[], 0), append(1, &active));
    case!(state(&[copy_bar(&closed)], 1), append(2, &closed));
    case!(state(&[copy_bar(&closed)], 1), append(2, &changed_high));
    case!(
        state(&[copy_bar(&closed)], 1),
        append(2, &bar(copy_stream(&s), 0, 1, true))
    );
    case!(state(&[copy_bar(&active)], 1), append(2, &active));
    case!(state(&[copy_bar(&active)], 1), update(2, &active2));
    case!(state(&[copy_bar(&active)], 1), update(2, &closed));
    case!(state(&[copy_bar(&active2)], 1), update(2, &active));
    case!(state(&[], 0), update(1, &active));
    case!(
        state(&[copy_bar(&active)], 1),
        update(2, &bar(copy_stream(&s), 2, 2, false))
    );
    case!(state(&[copy_bar(&closed)], 1), update(2, &closed));
    case!(state(&[], 0), append(2, &other_bad));
    case!(state(&[], 0), append(1, &other_bad));
    case!(state(&[], 0), append(1, &bad));
    case!(state(&[], 0), append(1, &bar(copy_stream(&s), 1, 1, true)));
    case!(state(&[copy_bar(&active)], 1), append(2, &bad));
    case!(
        state(&[copy_bar(&active)], 1),
        append(2, &bar(copy_stream(&s), 0, 1, true))
    );
    case!(
        state(&[copy_bar(&closed)], 1),
        append(2, &bar(copy_stream(&s), 1, 3, true))
    );
    case!(
        BarQualityState {
            stream: huge,
            bars: Vec::new(),
            last_event_sequence: n(0)
        },
        append(1, &huge_bar)
    );
    case!(state(&[], 0), append(1, &bar(copy_stream(&s), 2, 1, false)));
    case!(state(&[copy_bar(&active)], 1), update(2, &other_bad));
    case!(state(&[], 0), update(1, &bad));
    case!(
        state(&[copy_bar(&active)], 1),
        update(3, &bar(copy_stream(&s), 2, 2, false))
    );
    case!(state(&[copy_bar(&closed)], 1), append(2, &closed2));
    case!(
        state(&[copy_bar(&closed), copy_bar(&closed2)], 2),
        append(3, &closed2)
    );
    case!(
        state(&[copy_bar(&closed), active_last], 2),
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
    let expected: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with('q'))
        .collect();
    assert_eq!(expected.len(), 27, "missing Dafny oracle cases: {stdout}");
    let actual_cases = cases();
    assert_eq!(actual_cases.len(), expected.len());
    for ((name, prior, event), oracle) in actual_cases.into_iter().zip(expected) {
        let old_bars: Vec<String> = prior.bars.iter().map(bar_text).collect();
        let old_seq = prior.last_event_sequence.clone();
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
                assert_eq!(next.last_event_sequence, old_seq.add(&n(1)));
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
                        .all(|pair| pair[0].slot.cmp_exact(&pair[1].slot).is_lt())
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
