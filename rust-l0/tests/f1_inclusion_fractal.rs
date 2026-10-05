// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::f1::fractal::{advance_f1, shape_at};
use chan_l0::f1::inclusion::single_combined;
use chan_l0::f1::{CombinedBar, F1State, FractalKind, FractalPoint, RawBar, new_f1_state};
use chan_l0::input::{BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::number::{Int, Nat};

fn n(value: u64) -> Nat {
    Nat::from_u64(value)
}
fn i(value: i64) -> Int {
    Int::from_i64(value)
}
fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "F1".into(),
        timeframe: n(1),
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
        volume: n(1),
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
        value.first_slot.decimal(),
        value.last_slot.decimal(),
        value.low.decimal(),
        value.high.decimal(),
        dir_code(&value.direction),
        value.formed_known_at.decimal(),
        value.known_at.decimal(),
        value.low_slot.decimal(),
        value.high_slot.decimal()
    )
}
fn point_text(value: &FractalPoint, confirmed_known_at: &Nat) -> String {
    format!(
        "{{{},{},{},{}}}",
        kind_code(&value.kind),
        value.center_index.decimal(),
        value.detected_known_at.decimal(),
        confirmed_known_at.decimal()
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
    include_str!("fixtures/f1_inclusion_fractal.txt")
        .lines()
        .map(str::to_owned)
        .collect()
}
fn compare_with_runner(expected: &[String], actual: &[String]) {
    assert_eq!(expected, actual, "frozen executable oracle mismatch");
}

#[test]
fn incremental_inclusion_fractal_matches_frozen_oracle() {
    let expected = oracle_lines();
    assert_eq!(
        expected.len(),
        26,
        "Dafny fixture omitted an event or strict-boundary case"
    );
    let mut actual = Vec::new();
    for (name, initial, bars) in fixtures() {
        let mut state = new_f1_state(stream(), initial);
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
                assert!(
                    combined
                        .formed_known_at
                        .cmp_exact(&combined.known_at)
                        .is_le()
                );
                assert!(combined.first_slot.cmp_exact(&combined.low_slot).is_le());
                assert!(combined.low_slot.cmp_exact(&combined.last_slot).is_le());
                assert!(combined.first_slot.cmp_exact(&combined.high_slot).is_le());
                assert!(combined.high_slot.cmp_exact(&combined.last_slot).is_le());
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
                first_slot: left.first_slot.clone(),
                last_slot: left.last_slot.clone(),
                low: left.low.clone(),
                high: left.high.clone(),
                direction: MarketDirection::Upward,
                formed_known_at: left.formed_known_at.clone(),
                known_at: left.known_at.clone(),
                low_slot: left.low_slot.clone(),
                high_slot: left.high_slot.clone(),
            },
            CombinedBar {
                first_slot: middle.first_slot.clone(),
                last_slot: middle.last_slot.clone(),
                low: middle.low.clone(),
                high: middle.high.clone(),
                direction: MarketDirection::Upward,
                formed_known_at: middle.formed_known_at.clone(),
                known_at: middle.known_at.clone(),
                low_slot: middle.low_slot.clone(),
                high_slot: middle.high_slot.clone(),
            },
            right,
        ];
        let top = matches!(shape_at(&bars, 1), Some(FractalKind::Top));
        let bottom = matches!(shape_at(&bars, 1), Some(FractalKind::Bottom));
        actual.push(format!("{name}|T{top}|B{bottom}"));
    }
    compare_with_runner(&expected, &actual);
}
