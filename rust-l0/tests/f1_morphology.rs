// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::f1::morphology::{
    MorphologyState, advance_morphology, fold_morphology, new_morphology_state,
};
use chan_l0::f1::stroke::{ConfirmedStroke, StrokePhase};
use chan_l0::input::{BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::number::{Int, Nat};

fn nat(value: u64) -> Nat {
    Nat::from_u64(value)
}
fn int(value: i64) -> Int {
    Int::from_i64(value)
}
fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "EXAMPLE".into(),
        instrument: "F1".into(),
        timeframe: nat(1),
    }
}
fn bar(slot: u64, price: i64) -> QualityBar {
    QualityBar {
        stream: stream(),
        slot: nat(slot),
        open: int(price),
        high: int(price + 2),
        low: int(price),
        close: int(price + 2),
        volume: nat(1),
        turnover: None,
        known_at: nat(slot + 1),
        closed: true,
    }
}
fn fixture() -> Vec<(&'static str, Vec<i64>)> {
    vec![
        (
            "wave",
            vec![
                10, 9, 8, 9, 10, 11, 12, 13, 12, 11, 10, 9, 8, 9, 10, 11, 12, 13, 12, 11, 10,
            ],
        ),
        (
            "short",
            vec![10, 9, 8, 9, 10, 9, 7, 8, 9, 10, 9, 7, 8, 9, 10],
        ),
        (
            "tail",
            vec![
                10, 9, 8, 9, 10, 11, 12, 13, 12, 11, 14, 15, 14, 13, 16, 17, 16, 15, 14, 13, 12,
                11, 8, 9, 10, 11,
            ],
        ),
    ]
}
fn stroke_text(s: &ConfirmedStroke) -> String {
    format!(
        "{{{},{},{},{},{},{},{}}}",
        s.geometry.begin_index,
        s.geometry.formation_end_index,
        s.geometry.end_index,
        s.formed_known_at.decimal(),
        s.current_known_at.decimal(),
        s.confirmed_by_point_index,
        s.confirmed_known_at.decimal()
    )
}
fn render(name: &str, step: usize, state: &MorphologyState) -> String {
    let mut text = format!(
        "{name}.{step}|P{}|S{}",
        state.f1.confirmed.len(),
        state.stroke.confirmed.len()
    );
    for item in &state.stroke.confirmed {
        text.push_str(&stroke_text(item));
    }
    text.push_str("|A");
    match &state.stroke.phase {
        StrokePhase::AwaitingFirstPoint => text.push('-'),
        StrokePhase::SeekingFirstStroke { anchor_point_index } => {
            text.push_str(&format!("Q{anchor_point_index}"))
        }
        StrokePhase::BuildingStroke { active, pending } => {
            text.push_str(&format!(
                "B{},{},{},{},{}",
                active.geometry.begin_index,
                active.geometry.formation_end_index,
                active.geometry.end_index,
                active.formed_known_at.decimal(),
                active.current_known_at.decimal()
            ));
            match pending {
                None => text.push_str(",N"),
                Some(value) => text.push_str(&format!(",P{}", value.confirming_point_index)),
            }
        }
    }
    text.push_str(&format!(
        "|D{}|C{}",
        state.stroke.detected_point_count, state.stroke.confirmed_point_count
    ));
    text
}
fn oracle_lines() -> Vec<String> {
    include_str!("fixtures/f1_morphology.txt")
        .lines()
        .map(str::to_owned)
        .collect()
}
fn compare_with_runner(expected: &[String], actual: &[String]) {
    assert_eq!(expected, actual, "frozen executable oracle mismatch");
}

#[test]
fn incremental_strokes_match_frozen_oracle_and_chunking() {
    let expected = oracle_lines();
    assert_eq!(expected.len(), 62);
    let mut actual = Vec::new();
    let mut replacements = 0;
    let mut saw_equal = false;
    let mut saw_confirmation = false;
    for (name, prices) in fixture() {
        let bars: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, p)| bar(i as u64 + 1, *p))
            .collect();
        let mut state = new_morphology_state(stream(), MarketDirection::Upward);
        for (index, input) in bars.iter().enumerate() {
            let previous = state
                .stroke
                .confirmed
                .iter()
                .map(stroke_text)
                .collect::<Vec<_>>();
            let previous_end = match &state.stroke.phase {
                StrokePhase::BuildingStroke { active, .. } => Some(active.geometry.end_index),
                _ => None,
            };
            let previous_anchor = match &state.stroke.phase {
                StrokePhase::SeekingFirstStroke { anchor_point_index } => Some(*anchor_point_index),
                _ => None,
            };
            let previous_points = state.f1.confirmed.len();
            let (next, delta) = advance_morphology(state, input).unwrap();
            assert_eq!(next.stroke.detected_point_count, next.f1.confirmed.len());
            assert_eq!(next.stroke.confirmed_point_count, next.f1.confirmed.len());
            assert_eq!(
                next.stroke
                    .confirmed
                    .iter()
                    .take(previous.len())
                    .map(stroke_text)
                    .collect::<Vec<_>>(),
                previous
            );
            assert!(next.stroke.confirmed.len() <= previous.len() + 1);
            assert_eq!(
                delta.confirmed_stroke.as_ref().map(stroke_text),
                (next.stroke.confirmed.len() > previous.len())
                    .then(|| stroke_text(next.stroke.confirmed.last().unwrap()))
            );
            if delta.confirmed_stroke.is_some() {
                saw_confirmation = true;
            }
            if let (Some(before), StrokePhase::BuildingStroke { active, .. }) =
                (previous_end, &next.stroke.phase)
            {
                if active.geometry.end_index > before
                    && active.geometry.formation_end_index < active.geometry.end_index
                {
                    replacements += 1;
                }
            }
            if name == "short"
                && next.f1.confirmed.len() > previous_points
                && previous_anchor == Some(2)
            {
                if let StrokePhase::SeekingFirstStroke {
                    anchor_point_index: 2,
                } = &next.stroke.phase
                {
                    let previous_low = &next.f1.combined[6].low;
                    let current_low = &next.f1.combined[11].low;
                    if previous_low == current_low {
                        saw_equal = true;
                    }
                }
            }
            for stroke in &next.stroke.confirmed {
                assert!(stroke.geometry.begin_index < stroke.geometry.formation_end_index);
                assert!(stroke.geometry.formation_end_index <= stroke.geometry.end_index);
                assert!(stroke.geometry.end_index < next.f1.confirmed.len());
                assert!(stroke.confirmed_by_point_index < next.f1.confirmed.len());
                assert_ne!(
                    next.f1.confirmed[stroke.geometry.begin_index].point.kind,
                    next.f1.confirmed[stroke.geometry.formation_end_index]
                        .point
                        .kind
                );
            }
            for pair in next.stroke.confirmed.windows(2) {
                assert_eq!(pair[0].geometry.end_index, pair[1].geometry.begin_index);
                assert_ne!(
                    next.f1.confirmed[pair[0].geometry.begin_index].point.kind,
                    next.f1.confirmed[pair[1].geometry.begin_index].point.kind
                );
            }
            if let StrokePhase::BuildingStroke { active, pending } = &next.stroke.phase {
                assert!(
                    pending.is_none(),
                    "closed-material transaction left a pending reverse"
                );
                assert_eq!(active.geometry.begin_index, active.begin_point_index);
                assert!(active.geometry.begin_index < active.geometry.formation_end_index);
                assert!(active.geometry.formation_end_index <= active.geometry.end_index);
                assert!(active.geometry.end_index < next.f1.confirmed.len());
                assert_ne!(
                    next.f1.confirmed[active.geometry.begin_index].point.kind,
                    next.f1.confirmed[active.geometry.formation_end_index]
                        .point
                        .kind
                );
                if let Some(last) = next.stroke.confirmed.last() {
                    assert_eq!(last.geometry.end_index, active.geometry.begin_index);
                }
            }
            actual.push(render(name, index + 1, &next));
            state = next;
        }
        let before = render(name, 999, &state);
        let (preserved, _) = advance_morphology(state, bars.last().unwrap()).unwrap_err();
        assert_eq!(
            render(name, 999, &preserved),
            before,
            "duplicate changed state"
        );
        let mut active = bar(bars.len() as u64 + 1, 10);
        active.closed = false;
        let (preserved, _) = advance_morphology(preserved, &active).unwrap_err();
        assert_eq!(
            render(name, 999, &preserved),
            before,
            "active bar changed state"
        );
        let (chunked, _) = fold_morphology(
            new_morphology_state(stream(), MarketDirection::Upward),
            &bars[..bars.len() / 2],
        )
        .unwrap();
        let (chunked, _) = fold_morphology(chunked, &bars[bars.len() / 2..]).unwrap();
        assert_eq!(
            render(name, 999, &chunked),
            before,
            "chunking changed state"
        );
    }
    assert!(saw_confirmation);
    assert!(
        replacements >= 2,
        "fixture did not exercise multiple active tail replacements"
    );
    assert!(
        saw_equal,
        "fixture did not exercise equal endpoint retention"
    );
    compare_with_runner(&expected, &actual);
}
