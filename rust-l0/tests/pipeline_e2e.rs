//! 质量门与完整 L0 管线的组合回放；输出可重复检查的逐事件轨迹。
//!
//! 重构前固定失败面：活动 Bar 泄漏、更新重复发布、拒绝时改写账本、
//! 重复闭合 Bar 重放、镜像方向偏差、连续多段追加丢失或改写历史。

mod common;

use chan_l0::input::{BarStreamEvent, BarStreamIdentity, MarketDirection, QualityBar};
use chan_l0::lowest::{CompleteBoundary, advance_pipeline, new_pipeline_state};
use chan_l0::quality::{
    QualityClassification, QualityGateDecision, apply_quality_event, empty_bar_quality_state,
};
use chrono::{DateTime, TimeDelta, Utc};
use std::fmt::Write;
use std::path::Path;

fn time(value: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(value, 0).unwrap()
}

fn stream() -> BarStreamIdentity {
    BarStreamIdentity {
        market: "SYNTHETIC".into(),
        instrument: "L0_REFACTOR_E2E".into(),
        timeframe: TimeDelta::seconds(10),
    }
}

fn bar(index: usize, price: i64, mirror: bool, phase: usize) -> QualityBar {
    let (low, high) = if mirror {
        (-price - 1, -price)
    } else {
        (price, price + 1)
    };
    QualityBar {
        stream: stream(),
        slot: time(index as i64 * 10),
        open: low as f64,
        high: high as f64,
        low: low as f64,
        close: high as f64,
        volume: phase as u64 + 1,
        turnover: None,
        known_at: time(index as i64 * 10 + [0, 5, 10][phase]),
        closed: phase == 2,
    }
}

#[test]
fn quality_events_reach_multiple_frozen_lowest_movements() {
    let prices = [
        -5, -6, -3, 1, 5, 9, 7, 4, 1, -1, 3, 7, 11, 14, 12, 9, 6, 4, 6, 8, 9, 10, 8, 5, 2, 0, 4, 8,
        12, 16, 14, 12,
    ];
    for (name, price_scale, time_shift) in [
        ("pipeline-trace", 1.0, TimeDelta::zero()),
        (
            "pipeline-fractional-pre-epoch",
            0.125,
            TimeDelta::seconds(-3600) + TimeDelta::nanoseconds(123_456_789),
        ),
    ] {
        let make_bar = |index, price, mirror, phase| {
            let mut value = bar(index, price, mirror, phase);
            value.slot += time_shift;
            value.known_at += time_shift;
            value.open *= price_scale;
            value.high *= price_scale;
            value.low *= price_scale;
            value.close *= price_scale;
            value
        };
        let mut trace = String::new();
        for mirror in [false, true] {
            let label = if mirror { "down" } else { "up" };
            let mut quality = empty_bar_quality_state(stream()).unwrap();
            let mut pipeline = new_pipeline_state(
                stream(),
                CompleteBoundary {
                    root_point: 1,
                    initial_inclusion_direction: if mirror {
                        MarketDirection::Downward
                    } else {
                        MarketDirection::Upward
                    },
                    known_at: time(10) + time_shift,
                },
            );
            let mut sequence = 0;
            let mut prior_movements = Vec::new();
            for cycle in 0..4 {
                for (offset, price) in prices.iter().enumerate() {
                    let index = cycle * prices.len() + offset;
                    let price = price + cycle as i64 * 3;
                    for phase in 0..3 {
                        sequence += 1;
                        let payload = make_bar(index, price, mirror, phase);
                        let event = if phase == 0 {
                            BarStreamEvent::Append {
                                event_sequence: sequence,
                                bar: payload,
                            }
                        } else {
                            BarStreamEvent::UpdateActiveTail {
                                event_sequence: sequence,
                                bar: payload,
                            }
                        };
                        let QualityGateDecision::GateAccepted {
                            next,
                            structure_material,
                            ..
                        } = apply_quality_event(quality, event)
                        else {
                            panic!("valid event was rejected: {label}/{index}/{phase}");
                        };
                        quality = next;
                        assert_eq!(structure_material.is_some(), phase == 2);
                        let mut emitted = 0;
                        if let Some(material) = structure_material {
                            let (next, delta) = advance_pipeline(pipeline, &material).unwrap();
                            emitted = delta.lowest_movements.len();
                            assert_eq!(emitted, delta.segments.len());
                            let movements: Vec<_> = next
                                .lowest_movements
                                .iter()
                                .map(|m| format!("{m:?}"))
                                .collect();
                            assert_eq!(&movements[..prior_movements.len()], &prior_movements);
                            assert_eq!(movements.len(), prior_movements.len() + emitted);
                            prior_movements = movements;
                            for movement in &delta.lowest_movements {
                                writeln!(
                                    trace,
                                    "{label}|L0|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                                    movement.start_endpoint.market_order,
                                    movement.start_endpoint.open_time.to_rfc3339(),
                                    movement.start_endpoint.price,
                                    movement.end_endpoint.market_order,
                                    movement.end_endpoint.open_time.to_rfc3339(),
                                    movement.end_endpoint.price,
                                    movement.full_range.low,
                                    movement.full_range.high,
                                    movement.known_at.to_rfc3339()
                                )
                                .unwrap();
                            }
                            pipeline = next;
                        }
                        writeln!(
                            trace,
                            "{label}|{index}|{phase}|{sequence}|{}|{}|{}|{}|{}|{emitted}",
                            quality.bars.len(),
                            pipeline.morphology.f1.combined.len(),
                            pipeline.morphology.f1.confirmed.len(),
                            pipeline.morphology.stroke.confirmed.len(),
                            pipeline.lowest_movements.len()
                        )
                        .unwrap();

                        // 活动尾存在时不能追加下一根；拒绝不能消耗事件序号或改写账本。
                        if phase == 1 && index % 7 == 0 {
                            let before = format!("{quality:?}");
                            let event = BarStreamEvent::Append {
                                event_sequence: sequence + 1,
                                bar: make_bar(index + 1, price, mirror, 2),
                            };
                            let QualityGateDecision::GateBlocked {
                                preserved,
                                classification,
                                ..
                            } = apply_quality_event(quality, event)
                            else {
                                panic!("append over active tail was accepted");
                            };
                            assert_eq!(
                                classification,
                                QualityClassification::ActiveTailRequiresUpdate
                            );
                            assert_eq!(format!("{preserved:?}"), before);
                            writeln!(trace, "{label}|{index}|blocked-active-tail").unwrap();
                            quality = preserved;
                        }
                    }
                    // 闭合尾的幂等重送只前进事件序号，不能再次进入结构管线。
                    sequence += 1;
                    let duplicate = BarStreamEvent::Append {
                        event_sequence: sequence,
                        bar: make_bar(index, price, mirror, 2),
                    };
                    let QualityGateDecision::GateAccepted {
                        next,
                        structure_material,
                        ..
                    } = apply_quality_event(quality, duplicate)
                    else {
                        panic!("idempotent duplicate was rejected");
                    };
                    assert!(structure_material.is_none());
                    assert_eq!(next.bars.len(), index + 1);
                    quality = next;
                }
            }
            assert!(
                pipeline.lowest_movements.len() >= 3,
                "scenario must reach multiple segments"
            );
            assert_eq!(
                pipeline.segment.completed.len(),
                pipeline.lowest_movements.len()
            );
        }
        let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("target/e2e/{name}.txt"));
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::write(&artifact, &trace).unwrap();
        // 该组合基线录自重构前的 54c2ded；不是新增的 Dafny oracle。
        let expected = common::oracle_lines(include_str!("fixtures/pipeline_e2e.txt"), "pipeline")
            .into_iter()
            .map(|line| {
                let mut columns: Vec<_> = line.split('|').map(str::to_owned).collect();
                if columns[1] == "L0" {
                    for index in [3, 6, 10] {
                        let value = DateTime::parse_from_rfc3339(&columns[index])
                            .unwrap()
                            .with_timezone(&Utc);
                        columns[index] = (value + time_shift).to_rfc3339();
                    }
                    for index in [4, 7, 8, 9] {
                        columns[index] =
                            (columns[index].parse::<f64>().unwrap() * price_scale).to_string();
                    }
                }
                columns.join("|")
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(trace, expected);
    }
}

/// 原生数值的失败面：闭合时间加法溢出、事件序号耗尽、极值价格被窄化。
#[test]
fn native_limits_are_checked_before_structure_publication() {
    let mut trace = String::new();
    let mut input = bar(0, 0, false, 2);
    input.slot = DateTime::<Utc>::MAX_UTC - TimeDelta::seconds(10);
    input.known_at = DateTime::<Utc>::MAX_UTC;
    input.low = f64::MIN;
    input.high = f64::MAX;
    input.volume = u64::MAX;
    input.turnover = Some(u64::MAX);
    let initial = empty_bar_quality_state(stream()).unwrap();
    let accepted = apply_quality_event(
        initial,
        BarStreamEvent::Append {
            event_sequence: 1,
            bar: input.clone(),
        },
    );
    let QualityGateDecision::GateAccepted {
        structure_material: Some(material),
        ..
    } = accepted
    else {
        panic!("representable extreme Bar rejected");
    };
    let pipeline = new_pipeline_state(
        stream(),
        CompleteBoundary {
            root_point: 0,
            initial_inclusion_direction: MarketDirection::Upward,
            known_at: DateTime::<Utc>::MAX_UTC,
        },
    );
    let (pipeline, delta) = advance_pipeline(pipeline, &material).unwrap();
    assert_eq!(pipeline.morphology.f1.combined[0].low, f64::MIN);
    assert_eq!(pipeline.morphology.f1.combined[0].high, f64::MAX);
    assert!(delta.lowest_movements.is_empty());
    writeln!(
        trace,
        "extremes|{}|{}|{}|accepted",
        material.low, material.high, material.known_at
    )
    .unwrap();

    // 每个价格字段都必须拒绝非有限值，不能只依赖大小比较。
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in ["open", "high", "low", "close"] {
            let mut invalid_bar = bar(0, 0, false, 2);
            match field {
                "open" => invalid_bar.open = invalid,
                "high" => invalid_bar.high = invalid,
                "low" => invalid_bar.low = invalid,
                "close" => invalid_bar.close = invalid,
                _ => unreachable!(),
            }
            let quality = empty_bar_quality_state(stream()).unwrap();
            let before = quality.clone();
            let QualityGateDecision::GateBlocked {
                preserved,
                classification,
                ..
            } = apply_quality_event(
                quality,
                BarStreamEvent::Append {
                    event_sequence: 1,
                    bar: invalid_bar,
                },
            )
            else {
                panic!("non-finite price accepted: {field}={invalid}")
            };
            assert_eq!(preserved, before);
            assert_eq!(classification, QualityClassification::InvalidBarPayload);
            writeln!(
                trace,
                "non-finite|{field}|{invalid}|rejected|state-preserved"
            )
            .unwrap();
        }
    }

    // 时间恰好达到上界可接受；再前进一个单位必须拒绝，而不是回绕。
    input.slot += TimeDelta::seconds(1);
    let quality = empty_bar_quality_state(stream()).unwrap();
    let before = quality.clone();
    let QualityGateDecision::GateBlocked {
        preserved,
        classification,
        ..
    } = apply_quality_event(
        quality,
        BarStreamEvent::Append {
            event_sequence: 1,
            bar: input,
        },
    )
    else {
        panic!("overflowing close time accepted")
    };
    assert_eq!(preserved, before);
    assert_eq!(classification, QualityClassification::InvalidBarPayload);
    writeln!(trace, "close-time-overflow|rejected|state-preserved").unwrap();

    // MAX 是最后一个合法事件序号；追加和活动尾更新均不得回绕到零。
    for timeframe in [TimeDelta::zero(), TimeDelta::seconds(-1)] {
        let invalid_stream = BarStreamIdentity {
            timeframe,
            ..stream()
        };
        assert!(empty_bar_quality_state(invalid_stream).is_err());
        writeln!(trace, "invalid-timeframe|{timeframe}|rejected").unwrap();
    }
    for update in [false, true] {
        let quality = chan_l0::quality::BarQualityState {
            stream: stream(),
            bars: vec![bar(0, 0, false, 0)],
            last_event_sequence: u64::MAX - 1,
        };
        let final_event = BarStreamEvent::UpdateActiveTail {
            event_sequence: u64::MAX,
            bar: bar(0, 0, false, 1),
        };
        let QualityGateDecision::GateAccepted {
            next,
            structure_material: None,
            ..
        } = apply_quality_event(quality, final_event)
        else {
            panic!("last representable event rejected");
        };
        let before = next.clone();
        let event = if update {
            BarStreamEvent::UpdateActiveTail {
                event_sequence: 0,
                bar: bar(0, 0, false, 2),
            }
        } else {
            BarStreamEvent::Append {
                event_sequence: 0,
                bar: bar(1, 0, false, 2),
            }
        };
        let QualityGateDecision::GateBlocked {
            preserved,
            classification,
            ..
        } = apply_quality_event(next, event)
        else {
            panic!("event sequence wrapped");
        };
        assert_eq!(preserved, before);
        assert_eq!(classification, QualityClassification::EventSequenceConflict);
        writeln!(
            trace,
            "sequence-overflow|update={update}|rejected|state-preserved"
        )
        .unwrap();
    }
    let artifact = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/e2e/native-limits.txt");
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::write(artifact, trace).unwrap();
}
