// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::f1::feature::{FeatureElement, PriceInterval, SegmentDirection, normalized_push};
use chan_l0::f1::segment::{
    PrimarySplitProbe, ProbeStage, ProbeStep, RecognitionReason, actual_break, advance_probe,
    first_stroke_directional_resolution, initial_three_direction_resolved,
};
use chan_l0::input::{BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{
    CompleteBoundary, F1PipelineState, PipelineError, advance_pipeline, new_pipeline_state,
};
use chan_l0::number::{Int, Nat};

fn nat(value: u64) -> Nat {
    Nat::from_u64(value)
}
fn int(value: i64) -> Int {
    Int::from_i64(value)
}
fn element(mirror: bool, source: usize, low: i64, high: i64) -> FeatureElement {
    let (low, high) = if mirror { (-high, -low) } else { (low, high) };
    FeatureElement {
        source_stroke_indices: vec![source],
        source_stroke_index: source,
        interval: PriceInterval {
            low: int(low),
            high: int(high),
        },
        extreme_source_stroke_index: source,
    }
}
fn direction(mirror: bool) -> SegmentDirection {
    if mirror {
        SegmentDirection::Down
    } else {
        SegmentDirection::Up
    }
}
fn probe(mirror: bool, left: (i64, i64), center: (i64, i64)) -> PrimarySplitProbe {
    PrimarySplitProbe {
        split_stroke_index: 2,
        primary: vec![
            element(mirror, 0, left.0, left.1),
            element(mirror, 2, center.0, center.1),
        ],
        second: Vec::new(),
        second_target: None,
        stage: ProbeStage::SeekingPrimary,
    }
}
fn waiting(step: ProbeStep) -> PrimarySplitProbe {
    match step {
        ProbeStep::Waiting(probe) => probe,
        _ => panic!("expected waiting split"),
    }
}
fn render(name: &str, step: usize, outcome: &ProbeStep) -> String {
    let mut text = format!("{name}.{step}|");
    match outcome {
        ProbeStep::Discarded => text.push('D'),
        ProbeStep::Waiting(probe) => {
            text.push('W');
            match &probe.stage {
                ProbeStage::SeekingPrimary => text.push('P'),
                ProbeStage::SeekingActualBreak { candidate_known_at } => {
                    text.push_str(&format!("A{}", candidate_known_at.decimal()))
                }
                ProbeStage::SeekingSecond {
                    candidate_known_at,
                    actual_break,
                } => {
                    text.push_str(&format!("S{}", candidate_known_at.decimal()));
                    match actual_break {
                        Some(value) => text.push_str(&format!(
                            "E{},{}",
                            value.center_source_stroke_index, value.observed_stroke_index
                        )),
                        None => text.push('N'),
                    }
                }
            }
            text.push_str(&format!(
                "|P{}|S{}|T",
                probe.primary.len(),
                probe.second.len()
            ));
            match probe.second_target {
                Some(index) => text.push_str(&index.to_string()),
                None => text.push('-'),
            }
        }
        ProbeStep::Recognized(value) => {
            let reason = match value.reason {
                RecognitionReason::ImmediateNoGap => "N",
                RecognitionReason::RepairedBySecondSequence => "G",
            };
            text.push_str(&format!(
                "R{},{},{},{}|E{},{}",
                value.split_stroke_index,
                value.candidate_known_at.decimal(),
                value.confirmed_known_at.decimal(),
                reason,
                value.actual_break.center_source_stroke_index,
                value.actual_break.observed_stroke_index
            ));
        }
    }
    text
}
fn oracle_lines() -> Vec<String> {
    include_str!("fixtures/f1_segment.txt")
        .lines()
        .map(str::to_owned)
        .collect()
}
fn compare_with_runner(expected: &[String], actual: &[String]) {
    assert_eq!(expected, actual, "frozen executable oracle mismatch");
}

#[test]
fn primary_split_probes_match_frozen_oracle() {
    let expected: Vec<_> = oracle_lines()
        .into_iter()
        .filter(|line| line.starts_with("nogap.") || line.starts_with("gap."))
        .collect();
    assert_eq!(expected.len(), 14);
    let mut actual = Vec::new();
    for mirror in [false, true] {
        let name = if mirror { "nogap.down" } else { "nogap.up" };
        let direction = direction(mirror);
        let right = element(mirror, 4, 5, 10);
        let p = probe(mirror, (7, 9), (6, 10));
        let step = advance_probe(p, &right, true, &direction, &nat(40), false);
        actual.push(render(name, 1, &step));
        let p = waiting(step);
        let step = advance_probe(
            p,
            &element(mirror, 6, 4, 9),
            true,
            &direction,
            &nat(60),
            true,
        );
        actual.push(render(name, 2, &step));
        let p = probe(mirror, (7, 9), (6, 10));
        let step = advance_probe(p, &right, true, &direction, &nat(40), true);
        actual.push(render(name, 3, &step));
    }
    for mirror in [false, true] {
        let name = if mirror { "gap.down" } else { "gap.up" };
        let direction = direction(mirror);
        let mut p = Some(probe(mirror, (0, 1), (3, 6)));
        let steps = [
            (4, 2, 5, true, 40, true),
            (5, 3, 5, false, 50, false),
            (7, 1, 4, false, 70, false),
            (9, 2, 5, false, 90, false),
        ];
        for (index, (source, low, high, opposes, known, actual_break)) in
            steps.into_iter().enumerate()
        {
            let step = advance_probe(
                p.take().unwrap(),
                &element(mirror, source, low, high),
                opposes,
                &direction,
                &nat(known),
                actual_break,
            );
            actual.push(render(name, index + 1, &step));
            if index < 3 {
                p = Some(waiting(step));
            }
        }
    }
    compare_with_runner(&expected, &actual);
}

fn direction_code(value: &SegmentDirection) -> usize {
    match value {
        SegmentDirection::Up => 0,
        SegmentDirection::Down => 1,
    }
}
fn feature_text(value: &FeatureElement) -> String {
    let mut text = format!(
        "({},{},{},{},",
        value.source_stroke_index,
        value.extreme_source_stroke_index,
        value.interval.low.decimal(),
        value.interval.high.decimal()
    );
    for source in &value.source_stroke_indices {
        text.push_str(&format!("{source},"));
    }
    text.push(')');
    text
}

#[test]
fn continuous_inclusion_keeps_full_sources_and_first_extreme() {
    let expected: Vec<_> = oracle_lines()
        .into_iter()
        .filter(|line| line.starts_with("feature."))
        .collect();
    assert_eq!(expected.len(), 10);
    let mut actual = Vec::new();
    for mirror in [false, true] {
        let name = if mirror { "feature.down" } else { "feature.up" };
        let direction = direction(mirror);
        let inputs = [
            (216, 124979, 125240),
            (218, 124817, 125798),
            (220, 125012, 125171),
            (222, 125073, 125248),
            (224, 124870, 125332),
        ];
        let mut primary = Vec::new();
        for (step, (source, low, high)) in inputs.into_iter().enumerate() {
            primary = normalized_push(primary, element(mirror, source, low, high), &direction, 218);
            let mut row = format!("{name}.{}|N{}", step + 1, primary.len());
            for feature in &primary {
                row.push_str(&feature_text(feature));
            }
            actual.push(row);
        }
        assert_eq!(primary.len(), 3);
        assert_eq!(primary[1].source_stroke_indices, [218, 220, 222]);
        assert_eq!(primary[1].extreme_source_stroke_index, 218);
    }
    compare_with_runner(&expected, &actual);
}

#[test]
fn exact_internal_break_rejections_match_owner() {
    let expected: Vec<_> = oracle_lines()
        .into_iter()
        .filter(|line| line.starts_with("reject."))
        .collect();
    assert_eq!(expected.len(), 6);
    let mut actual = Vec::new();
    for mirror in [false, true] {
        let name = if mirror { "down" } else { "up" };
        let interval = |low, high| element(mirror, 0, low, high).interval;
        let direction = direction(mirror);
        let previous = interval(4303901, 4312733);
        let first = interval(4292110, 4332503);
        let challenge = interval(4301375, 4318280);
        let beyond = interval(4290000, 4318280);
        let result = first_stroke_directional_resolution(&previous, &first, &challenge, &direction);
        actual.push(format!(
            "reject.first.{name}|{}",
            if result { "Y" } else { "N" }
        ));
        let result = first_stroke_directional_resolution(&previous, &first, &beyond, &direction);
        actual.push(format!(
            "reject.beyond.{name}|{}",
            if result { "Y" } else { "N" }
        ));
        let first = interval(264600, 265453);
        let third = interval(264945, 265426);
        let challenge = interval(264758, 265426);
        let result = initial_three_direction_resolved(&first, &third, &challenge, &direction);
        actual.push(format!(
            "reject.initial.{name}|{}",
            if result { "Y" } else { "N" }
        ));
    }
    compare_with_runner(&expected, &actual);
}
fn fixed_text(name: &str, step: usize, state: &F1PipelineState) -> String {
    let mut text = format!(
        "{name}.{step}|F{}|P{}|K{}|S{}",
        state.morphology.f1.combined.len(),
        state.morphology.f1.confirmed.len(),
        state.morphology.stroke.confirmed.len(),
        state.segment.completed.len()
    );
    for segment in &state.segment.completed {
        let reason = match segment.reason {
            RecognitionReason::ImmediateNoGap => "N",
            RecognitionReason::RepairedBySecondSequence => "G",
        };
        text.push_str(&format!(
            "{{{},{},{},{},{},{},{},{},{},{},{},{}",
            segment.ordinal,
            segment.begin_stroke_index,
            segment.end_stroke_index,
            direction_code(&segment.direction),
            segment.start_price.decimal(),
            segment.end_price.decimal(),
            segment.times.occurred_at.decimal(),
            segment.times.candidate_known_at.decimal(),
            segment.times.confirmed_at.decimal(),
            segment.price_interval.low.decimal(),
            segment.price_interval.high.decimal(),
            reason
        ));
        for element in &segment.elements {
            text.push_str(&feature_text(element));
        }
        text.push('}');
    }
    text.push_str(&format!("|M{}", state.lowest.completed.len()));
    for movement in &state.lowest.completed {
        let direction = match movement.key.direction {
            MarketDirection::Upward => 0,
            MarketDirection::Downward => 1,
            MarketDirection::NoNetDisplacement => 2,
        };
        text.push_str(&format!(
            "{{{},{},{},{},{},{},{},{},{},{},{},{}}}",
            movement.key.level_ordinal.decimal(),
            movement.key.start_endpoint.market_order.decimal(),
            movement.key.start_endpoint.open_time.decimal(),
            movement.key.start_endpoint.price.decimal(),
            movement.key.end_endpoint.market_order.decimal(),
            movement.key.end_endpoint.open_time.decimal(),
            movement.key.end_endpoint.price.decimal(),
            direction,
            movement.key.full_range.low.decimal(),
            movement.key.full_range.high.decimal(),
            movement.known_at.decimal(),
            movement.direct_materials.len()
        ));
    }
    text.push_str("|A");
    match &state.segment.pending {
        None => text.push('-'),
        Some(active) => text.push_str(&format!(
            "{},{},{},{},{}",
            active.begin_stroke_index,
            direction_code(&active.direction),
            active.primary.len(),
            active.probes.len(),
            active.known_at.decimal()
        )),
    }
    text.push_str(&format!("|C{}", state.segment.next_stroke_index));
    text
}

#[test]
fn fixed_history_reaches_segment_owner() {
    let expected: Vec<_> = oracle_lines()
        .into_iter()
        .filter(|line| line.starts_with("fixed.") || line.starts_with("break."))
        .collect();
    assert_eq!(expected.len(), 104);
    let prices = [
        -5, -6, -3, 1, 5, 9, 7, 4, 1, -1, 3, 7, 11, 14, 12, 9, 6, 4, 6, 8, 9, 10, 8, 5, 2, 0, 4, 8,
        12, 16, 14, 12,
    ];
    let mut actual = Vec::new();
    for mirror in [false, true] {
        let name = if mirror { "fixed.down" } else { "fixed.up" };
        let stream = BarStreamIdentity {
            market: "SYNTHETIC".into(),
            instrument: "W31_READY".into(),
            timeframe: nat(1),
        };
        let boundary = CompleteBoundary {
            root_point: nat(1),
            initial_inclusion_direction: if mirror {
                MarketDirection::Downward
            } else {
                MarketDirection::Upward
            },
            known_at: nat(1),
        };
        let mut state = new_pipeline_state(stream, boundary);
        let mut published = 0usize;
        let mut prior_segments = Vec::<String>::new();
        let mut prior_movements = Vec::<String>::new();
        for (index, price) in prices.into_iter().enumerate() {
            let (low, high) = if mirror {
                (-price - 1, -price)
            } else {
                (price, price + 1)
            };
            let input = QualityBar {
                stream: BarStreamIdentity {
                    market: "SYNTHETIC".into(),
                    instrument: "W31_READY".into(),
                    timeframe: nat(1),
                },
                slot: nat(index as u64),
                open: int(low),
                high: int(high),
                low: int(low),
                close: int(low),
                volume: nat(1),
                turnover: None,
                known_at: nat(index as u64 + 1),
                closed: true,
            };
            let (next, delta) = advance_pipeline(state, &input).unwrap();
            assert_eq!(delta.segments.len(), delta.lowest_movements.len());
            let segment_rows: Vec<_> = next
                .segment
                .completed
                .iter()
                .map(|s| format!("{s:?}"))
                .collect();
            let movement_rows: Vec<_> = next
                .lowest
                .completed
                .iter()
                .map(|m| format!("{m:?}"))
                .collect();
            assert_eq!(&segment_rows[..prior_segments.len()], &prior_segments);
            assert_eq!(&movement_rows[..prior_movements.len()], &prior_movements);
            assert_eq!(
                segment_rows.len(),
                prior_segments.len() + delta.segments.len()
            );
            assert_eq!(
                movement_rows.len(),
                prior_movements.len() + delta.lowest_movements.len()
            );
            for segment in &next.segment.completed {
                assert_ne!(
                    segment
                        .times
                        .occurred_at
                        .cmp_exact(&segment.times.confirmed_at),
                    std::cmp::Ordering::Greater
                );
                assert_ne!(
                    segment
                        .times
                        .candidate_known_at
                        .cmp_exact(&segment.times.confirmed_at),
                    std::cmp::Ordering::Greater
                );
                for element in &segment.elements {
                    assert_eq!(
                        element.source_stroke_indices.last(),
                        Some(&element.source_stroke_index)
                    );
                    assert!(
                        element
                            .source_stroke_indices
                            .contains(&element.extreme_source_stroke_index)
                    );
                    assert!(
                        element
                            .source_stroke_indices
                            .windows(2)
                            .all(|pair| pair[0] < pair[1])
                    );
                    assert!(
                        element
                            .source_stroke_indices
                            .iter()
                            .all(|&source| source < next.morphology.stroke.confirmed.len())
                    );
                }
            }
            prior_segments = segment_rows;
            prior_movements = movement_rows;
            published += delta.segments.len();
            if index == 31 {
                let count = next.morphology.stroke.confirmed.len();
                for split in 2..count {
                    for center_source in split..count {
                        for observed in center_source + 1..count {
                            for (direction_code, direction) in
                                [(0, SegmentDirection::Up), (1, SegmentDirection::Down)]
                            {
                                let result = actual_break(
                                    &next.morphology.f1,
                                    &next.morphology.stroke.confirmed,
                                    split,
                                    center_source,
                                    observed,
                                    &direction,
                                );
                                actual.push(format!("break.{}.{split}.{center_source}.{observed}.{direction_code}|{}",
                                    if mirror { "down" } else { "up" }, if result { "Y" } else { "N" }));
                            }
                        }
                    }
                }
            }
            actual.push(fixed_text(name, index + 1, &next));
            state = next;
        }
        assert_eq!(state.morphology.stroke.confirmed.len(), 6);
        assert_eq!(published, 1);
        assert_eq!(state.segment.completed.len(), state.lowest.completed.len());
    }
    assert!(
        actual
            .iter()
            .any(|row| row.starts_with("break.") && row.ends_with("|Y"))
    );
    assert!(
        actual
            .iter()
            .any(|row| row.starts_with("break.") && row.ends_with("|N"))
    );
    compare_with_runner(&expected, &actual);
}

#[test]
fn rejected_lowest_boundary_preserves_segment_for_retry() {
    let prices = [
        -5, -6, -3, 1, 5, 9, 7, 4, 1, -1, 3, 7, 11, 14, 12, 9, 6, 4, 6, 8, 9, 10, 8, 5, 2, 0, 4, 8,
        12, 16, 14, 12,
    ];
    let stream = BarStreamIdentity {
        market: "SYNTHETIC".into(),
        instrument: "W31_READY".into(),
        timeframe: nat(1),
    };
    let boundary = CompleteBoundary {
        root_point: nat(999),
        initial_inclusion_direction: MarketDirection::Upward,
        known_at: nat(1),
    };
    let mut state = new_pipeline_state(stream, boundary);
    let mut rejected = false;
    for (index, price) in prices.into_iter().enumerate() {
        let input = QualityBar {
            stream: BarStreamIdentity {
                market: "SYNTHETIC".into(),
                instrument: "W31_READY".into(),
                timeframe: nat(1),
            },
            slot: nat(index as u64),
            open: int(price),
            high: int(price + 1),
            low: int(price),
            close: int(price),
            volume: nat(1),
            turnover: None,
            known_at: nat(index as u64 + 1),
            closed: true,
        };
        let before = format!("{:?}", state.segment);
        match advance_pipeline(state, &input) {
            Ok((next, delta)) => {
                assert!(delta.segments.is_empty());
                state = next;
            }
            Err((preserved, PipelineError::LowestMovementInvalid)) => {
                assert_eq!(format!("{:?}", preserved.segment), before);
                assert!(preserved.lowest.completed.is_empty());
                rejected = true;
                break;
            }
            Err((_, reason)) => panic!("unexpected pipeline rejection: {reason:?}"),
        }
    }
    assert!(rejected);
}
