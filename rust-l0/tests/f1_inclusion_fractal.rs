mod common;

use chrono::{DateTime, TimeDelta, Utc};
// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::f1::fractal::{advance_f1, shape_at};
use chan_l0::f1::inclusion::single_combined;
use chan_l0::f1::{
    CombinedBar, F1InputError, F1State, FractalKind, FractalPoint, RawBar, new_f1_state,
};
use chan_l0::input::{BarStreamIdentity, MarketDirection, QualityBar};

fn n(value: u64) -> DateTime<Utc> {
    DateTime::from_timestamp(value as i64, 0).unwrap()
}
fn i(value: i64) -> f64 {
    value as f64
}
fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "F1".into(),
        timeframe: TimeDelta::seconds(1),
    }
}
fn bar(slot: u64, low: i64, high: i64) -> QualityBar {
    QualityBar {
        stream: stream(),
        slot: n(slot),
        open: i(low),
        high: i(high),
        low: i(low),
        close: i(high),
        volume: 1,
        turnover: None,
        known_at: n(slot + 1),
        closed: true,
    }
}
fn dir_code(value: &MarketDirection) -> u8 {
    match value {
        MarketDirection::Upward => 0,
        MarketDirection::Downward => 1,
        MarketDirection::NoNetDisplacement => 2,
    }
}
fn kind_code(value: &FractalKind) -> u8 {
    match value {
        FractalKind::Top => 0,
        FractalKind::Bottom => 1,
    }
}
fn combined_text(value: &CombinedBar) -> String {
    format!(
        "{{{},{},{},{},{},{},{},{},{}}}",
        value.first_slot.to_rfc3339(),
        value.last_slot.to_rfc3339(),
        value.low,
        value.high,
        dir_code(&value.direction),
        value.formed_known_at.to_rfc3339(),
        value.known_at.to_rfc3339(),
        value.low_slot.to_rfc3339(),
        value.high_slot.to_rfc3339()
    )
}
fn point_text(value: &FractalPoint, confirmed_known_at: &DateTime<Utc>) -> String {
    format!(
        "{{{},{},{},{}}}",
        kind_code(&value.kind),
        value.center_index,
        value.detected_known_at.to_rfc3339(),
        confirmed_known_at.to_rfc3339()
    )
}
fn render(name: &str, step: usize, state: &F1State) -> String {
    let mut result = format!("{name}.{step}|C{}", state.combined.len());
    for combined in &state.combined {
        result.push_str(&combined_text(combined));
    }
    result.push_str(&format!("|P{}", state.confirmed.len()));
    for point in &state.confirmed {
        result.push_str(&point_text(&point.point, &point.confirmed_known_at));
    }
    result.push_str("|F");
    match &state.forming {
        Some(point) => result.push_str(&point_text(point, &n(0))),
        None => result.push('-'),
    }
    result
}
fn fixtures() -> Vec<(&'static str, MarketDirection, Vec<QualityBar>)> {
    vec![
        (
            "u01",
            MarketDirection::Upward,
            vec![
                bar(1, 10, 20),
                bar(2, 12, 18),
                bar(3, 13, 19),
                bar(4, 16, 25),
                bar(5, 15, 24),
                bar(6, 10, 21),
            ],
        ),
        (
            "u02",
            MarketDirection::Downward,
            vec![bar(1, 10, 20), bar(2, 12, 18), bar(3, 9, 19), bar(4, 8, 17)],
        ),
        (
            "u03",
            MarketDirection::Upward,
            vec![
                bar(1, 10, 20),
                bar(2, 12, 22),
                bar(3, 11, 21),
                bar(4, 12, 20),
                bar(5, 9, 19),
            ],
        ),
        (
            "u04",
            MarketDirection::Downward,
            vec![
                bar(1, -20, -10),
                bar(2, -22, -12),
                bar(3, -21, -11),
                bar(4, -20, -12),
                bar(5, -19, -9),
            ],
        ),
        (
            "u05",
            MarketDirection::NoNetDisplacement,
            vec![bar(1, 10, 20), bar(2, 12, 18), bar(3, 9, 19), bar(4, 8, 17)],
        ),
    ]
}

fn oracle_lines() -> Vec<String> {
    common::oracle_lines(include_str!("fixtures/f1_inclusion_fractal.txt"), "fractal")
}
fn compare_with_runner(expected: &[String], actual: &[String]) {
    assert_eq!(expected, actual, "frozen executable oracle mismatch");
}

#[test]
fn incremental_inclusion_fractal_matches_frozen_oracle() {
    // u05把未知包含方向当成向下，已由正式左边界合同撤销；历史观测仍原样保留。
    let expected: Vec<_> = oracle_lines()
        .into_iter()
        .filter(|line| !line.starts_with("u05."))
        .collect();
    assert_eq!(
        expected.len(),
        22,
        "Dafny fixture omitted an event or strict-boundary case"
    );
    let mut actual = Vec::new();
    for (name, initial, bars) in fixtures() {
        let mut state = new_f1_state(stream(), initial);
        if initial == MarketDirection::NoNetDisplacement {
            for input in bars {
                let (preserved, reason) = advance_f1(state.clone(), &input).unwrap_err();
                assert_eq!(preserved, state);
                assert_eq!(reason, F1InputError::InvalidInitialDirection);
            }
            continue;
        }
        for (index, input) in bars.iter().enumerate() {
            let old_combined: Vec<String> = state.combined.iter().map(combined_text).collect();
            let old_confirmed: Vec<String> = state
                .confirmed
                .iter()
                .map(|point| point_text(&point.point, &point.confirmed_known_at))
                .collect();
            let old_forming = state.forming.as_ref().map(|point| point_text(point, &n(0)));
            let (next, delta) = advance_f1(state, input).unwrap();
            assert!(
                next.combined.len() == old_combined.len()
                    || next.combined.len() == old_combined.len() + 1
            );
            for (before, after) in old_combined
                .iter()
                .take(old_combined.len().saturating_sub(1))
                .zip(next.combined.iter())
            {
                assert_eq!(before, &combined_text(after));
            }
            assert_eq!(
                next.confirmed
                    .iter()
                    .take(old_confirmed.len())
                    .map(|point| point_text(&point.point, &point.confirmed_known_at))
                    .collect::<Vec<_>>(),
                old_confirmed
            );
            assert_eq!(
                delta.finalized_combined.is_some(),
                !old_combined.is_empty() && next.combined.len() > old_combined.len()
            );
            assert_eq!(
                delta.confirmed_fractal.is_some(),
                next.confirmed.len() > old_confirmed.len()
            );
            if let Some(finalized) = &delta.finalized_combined {
                assert_eq!(Some(combined_text(finalized)), old_combined.last().cloned());
            }
            if let Some(confirmed) = &delta.confirmed_fractal {
                assert_eq!(
                    point_text(&confirmed.point, &confirmed.confirmed_known_at),
                    point_text(
                        &next.confirmed.last().unwrap().point,
                        &next.confirmed.last().unwrap().confirmed_known_at
                    )
                );
            }
            assert_eq!(
                delta.forming.as_ref().map(|point| point_text(point, &n(0))),
                next.forming.as_ref().map(|point| point_text(point, &n(0)))
            );
            if delta.confirmed_fractal.is_some() {
                assert!(old_forming.is_some());
            }
            assert_eq!(
                delta.tail_updated,
                !old_combined.is_empty() && next.combined.len() == old_combined.len()
            );
            for combined in &next.combined {
                assert!(combined.formed_known_at.cmp(&combined.known_at).is_le());
                assert!(combined.first_slot.cmp(&combined.low_slot).is_le());
                assert!(combined.low_slot.cmp(&combined.last_slot).is_le());
                assert!(combined.first_slot.cmp(&combined.high_slot).is_le());
                assert!(combined.high_slot.cmp(&combined.last_slot).is_le());
            }
            actual.push(render(name, index + 1, &next));
            state = next;
        }
        let mut active = bar(100, 0, 1);
        active.closed = false;
        let before = render(name, 999, &state);
        let (preserved, _) = advance_f1(state, &active).unwrap_err();
        assert_eq!(
            render(name, 999, &preserved),
            before,
            "active Bar changed F1 state"
        );
    }
    let left = single_combined(
        &RawBar {
            slot: n(1),
            low: i(10),
            high: i(20),
            known_at: n(2),
        },
        &MarketDirection::Upward,
    );
    let middle = single_combined(
        &RawBar {
            slot: n(2),
            low: i(12),
            high: i(22),
            known_at: n(3),
        },
        &MarketDirection::Upward,
    );
    let equal_high = single_combined(
        &RawBar {
            slot: n(3),
            low: i(11),
            high: i(22),
            known_at: n(4),
        },
        &MarketDirection::Downward,
    );
    let equal_low = single_combined(
        &RawBar {
            slot: n(3),
            low: i(12),
            high: i(21),
            known_at: n(4),
        },
        &MarketDirection::Downward,
    );
    for (name, right) in [("s01", equal_high), ("s02", equal_low)] {
        let bars = vec![
            CombinedBar {
                first_slot: left.first_slot,
                last_slot: left.last_slot,
                low: left.low,
                high: left.high,
                direction: MarketDirection::Upward,
                formed_known_at: left.formed_known_at,
                known_at: left.known_at,
                low_slot: left.low_slot,
                high_slot: left.high_slot,
            },
            CombinedBar {
                first_slot: middle.first_slot,
                last_slot: middle.last_slot,
                low: middle.low,
                high: middle.high,
                direction: MarketDirection::Upward,
                formed_known_at: middle.formed_known_at,
                known_at: middle.known_at,
                low_slot: middle.low_slot,
                high_slot: middle.high_slot,
            },
            right,
        ];
        let top = matches!(shape_at(&bars, 1), Some(FractalKind::Top));
        let bottom = matches!(shape_at(&bars, 1), Some(FractalKind::Bottom));
        actual.push(format!("{name}|T{top}|B{bottom}"));
    }
    compare_with_runner(&expected, &actual);
}
