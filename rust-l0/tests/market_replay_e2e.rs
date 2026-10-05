//! 固定真实OHLC片段通过正式质量门和F1管线，检查镜像、历史冻结与完整价格范围。
//! 来源、输入摘要及时间解释见 evidence/market-fixture.json；不以参考库输出为oracle。

use chan_l0::f1::feature::SegmentDirection;
use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{CompleteBoundary, LowestMovement, advance_pipeline, new_pipeline_state};
use chan_l0::quality::{QualityGateDecision, apply_quality_event, empty_bar_quality_state};
use chrono::{NaiveDateTime, TimeDelta};
use std::fmt::Write;

fn mirrored(movement: &LowestMovement) -> LowestMovement {
    let mut value = *movement;
    value.start_endpoint.price = -movement.start_endpoint.price;
    value.end_endpoint.price = -movement.end_endpoint.price;
    value.full_range.low = -movement.full_range.high;
    value.full_range.high = -movement.full_range.low;
    value.direction = match movement.direction {
        SegmentDirection::Up => SegmentDirection::Down,
        SegmentDirection::Down => SegmentDirection::Up,
    };
    value
}

#[test]
fn market_bars_preserve_mirror_prefixes_and_actual_ranges() {
    let stream = BarStreamIdentity {
        market: "REFERENCE_FIXTURE".into(),
        instrument: "OHLC".into(),
        timeframe: TimeDelta::minutes(1),
    };
    let bars: Vec<_> = include_str!("fixtures/market_1m_4320.csv")
        .lines()
        .skip(1)
        .map(|line| {
            let row: Vec<_> = line.split(',').collect();
            let slot = NaiveDateTime::parse_from_str(row[0], "%Y-%m-%d %H:%M:%S")
                .unwrap()
                .and_utc();
            QualityBar {
                stream: stream.clone(),
                slot,
                open: row[1].parse().unwrap(),
                high: row[2].parse().unwrap(),
                low: row[3].parse().unwrap(),
                close: row[4].parse().unwrap(),
                volume: 1,
                turnover: None,
                known_at: slot + TimeDelta::minutes(1),
                closed: true,
            }
        })
        .collect();
    assert_eq!(bars.len(), 4320);
    let mut trace = String::new();
    let mut outputs = Vec::new();
    let mut errors = Vec::new();
    for mirror in [false, true] {
        let mut quality = empty_bar_quality_state(stream.clone()).unwrap();
        let mut pipeline = new_pipeline_state(
            stream.clone(),
            CompleteBoundary {
                root_point: 10,
                initial_inclusion_direction: if mirror {
                    MarketDirection::Downward
                } else {
                    MarketDirection::Upward
                },
                known_at: bars[10].slot,
            },
        );
        for (index, source) in bars.iter().enumerate() {
            let mut bar = source.clone();
            if mirror {
                bar.open = -source.open;
                bar.high = -source.low;
                bar.low = -source.high;
                bar.close = -source.close;
            }
            let QualityGateDecision::GateAccepted {
                next,
                structure_material: Some(material),
                ..
            } = apply_quality_event(
                quality,
                BarStreamEvent::Append {
                    event_sequence: index as u64 + 1,
                    bar,
                },
            )
            else {
                panic!("valid market Bar rejected at {index}");
            };
            quality = next;
            let previous = pipeline.lowest_movements.clone();
            let (next, delta) = advance_pipeline(pipeline, &material).unwrap();
            assert_eq!(&next.lowest_movements[..previous.len()], previous);
            for movement in &delta.lowest_movements {
                let raw: Vec<_> = bars
                    .iter()
                    .filter(|bar| {
                        movement.start_endpoint.open_time <= bar.slot
                            && bar.slot <= movement.end_endpoint.open_time
                    })
                    .collect();
                let raw_low = raw.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
                let raw_high = raw.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
                let (low, high) = if mirror {
                    (-raw_high, -raw_low)
                } else {
                    (raw_low, raw_high)
                };
                if movement.full_range.low != low || movement.full_range.high != high {
                    errors.push(format!(
                        "{mirror}|{index}|expected_raw=[{low},{high}]|{movement:?}"
                    ));
                }
                assert_eq!(movement.known_at, material.known_at);
                assert!(movement.is_valid());
                writeln!(trace, "{mirror}|{index}|L0|{movement:?}|raw=[{low},{high}]").unwrap();
            }
            writeln!(
                trace,
                "{mirror}|{index}|combined={}|fractals={}|strokes={}|segments={}|l0={}",
                next.morphology.f1.combined.len(),
                next.morphology.f1.confirmed.len(),
                next.morphology.stroke.confirmed.len(),
                next.segment.completed.len(),
                next.lowest_movements.len()
            )
            .unwrap();
            pipeline = next;
        }
        assert!(
            pipeline.lowest_movements.len() >= 20,
            "fixture must cover a sustained segment chain"
        );
        outputs.push(pipeline.lowest_movements);
    }
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/e2e");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("f1-market-4320.txt"), trace).unwrap();
    assert_eq!(
        outputs[0].iter().map(mirrored).collect::<Vec<_>>(),
        outputs[1]
    );
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
