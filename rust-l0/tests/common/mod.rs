//! 只在期望侧转换历史 oracle 的时间表示；不改写原始冻结文件。
//!
//! 历史场景中的时间整数在本测试中显式映射为 Unix 秒。
//! 实际输出使用 RFC 3339，保留 UTC 时区及亚秒精度。

use chrono::{DateTime, Utc};

fn time(value: &str) -> String {
    DateTime::<Utc>::from_timestamp(
        value.parse().expect("oracle time must be integral seconds"),
        0,
    )
    .expect("oracle time must fit UTC DateTime")
    .to_rfc3339()
}

fn fields(text: &str, positions: &[usize]) -> String {
    let mut parts: Vec<_> = text.split(',').map(str::to_owned).collect();
    for &position in positions {
        parts[position] = time(&parts[position]);
    }
    parts.join(",")
}

fn records(text: &str, positions: &[usize]) -> String {
    let mut result = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let end = rest[start..].find('}').unwrap() + start;
        result.push_str(&rest[..=start]);
        result.push_str(&fields(&rest[start + 1..end], positions));
        result.push('}');
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    result
}

/// 按各冻结输出的字段布局，将旧时间整数转换为 RFC 3339。
pub fn oracle_lines(text: &str, schema: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            if schema == "pipeline" {
                let mut parts: Vec<_> = line.split('|').map(str::to_owned).collect();
                if parts[1] == "L0" {
                    for position in [3, 6, 10] {
                        parts[position] = time(&parts[position]);
                    }
                }
                return parts.join("|");
            }
            // q20 超出当前数值和日期范围，原始证据保留，但不纳入新合同对照。
            if schema == "quality" && line.starts_with("q20|") {
                return line.to_owned();
            }
            line.split('|')
                .enumerate()
                .map(|(index, part)| {
                    if index == 0 {
                        return part.to_owned();
                    }
                    match schema {
                        "quality" => records(part, &[3, 10]),
                        "fractal" if part.starts_with('C') => records(part, &[0, 1, 5, 6, 7, 8]),
                        "fractal" if part.starts_with('P') || part.starts_with('F') => {
                            records(part, &[2, 3])
                        }
                        "stroke" if part.starts_with('S') => records(part, &[3, 4, 6]),
                        "stroke" if part.starts_with("AB") => {
                            format!("AB{}", fields(&part[2..], &[3, 4]))
                        }
                        "segment" if line.starts_with("fixed.") => {
                            if part.starts_with('S') {
                                records(part, &[6, 7, 8])
                            } else if part.starts_with('M') {
                                records(part, &[2, 5, 10])
                            } else if part.starts_with('A') && part != "A-" {
                                format!("A{}", fields(&part[1..], &[4]))
                            } else {
                                part.to_owned()
                            }
                        }
                        "segment" if line.starts_with("gap.") || line.starts_with("nogap.") => {
                            if let Some(values) = part.strip_prefix('R') {
                                format!("R{}", fields(values, &[1, 2]))
                            } else if part.starts_with("WA") || part.starts_with("WS") {
                                let end = part[2..]
                                    .find(|c: char| !c.is_ascii_digit())
                                    .map_or(part.len(), |n| n + 2);
                                format!("{}{}{}", &part[..2], time(&part[2..end]), &part[end..])
                            } else {
                                part.to_owned()
                            }
                        }
                        _ => part.to_owned(),
                    }
                })
                .collect::<Vec<_>>()
                .join("|")
        })
        .collect()
}
