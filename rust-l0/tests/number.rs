// Migrated existing behavioral tests; expected observations captured from the original oracle.
use chan_l0::number::{Int, Nat, NumberError};
use std::cmp::Ordering;

#[test]
fn exact_large_values_and_signs() {
    let huge = Int::from_decimal("184467440737095516160000000000000000000").unwrap();
    let neg = huge.neg();
    assert_eq!(huge.add(&neg), Int::zero());
    assert_eq!(
        huge.sub(&neg).decimal(),
        "368934881474191032320000000000000000000"
    );
    assert_eq!(huge.mul(&huge).div(&huge).unwrap(), huge);
    assert_eq!(neg.cmp_exact(&Int::zero()), Ordering::Less);
    assert_eq!(neg.as_nat(), Err(NumberError::NegativeNatural));
    assert_eq!(
        Nat::from_decimal("18446744073709551616").unwrap().decimal(),
        "18446744073709551616"
    );
    assert_eq!(Nat::from_decimal("-1"), Err(NumberError::NegativeNatural));
    assert_eq!(Int::from_decimal("1.5"), Err(NumberError::InvalidDecimal));
}

#[test]
fn euclidean_division_all_signs() {
    for (a, b, q, r) in [
        ("7", "3", "2", "1"),
        ("-7", "3", "-3", "2"),
        ("7", "-3", "-2", "1"),
        ("-7", "-3", "3", "2"),
        ("0", "-3", "0", "0"),
    ] {
        let (actual_q, actual_r) = Int::from_decimal(a)
            .unwrap()
            .div_rem(&Int::from_decimal(b).unwrap())
            .unwrap();
        assert_eq!(actual_q.decimal(), q);
        assert_eq!(actual_r.decimal(), r);
    }
    assert_eq!(
        Int::from_i64(1).div(&Int::zero()),
        Err(NumberError::ZeroDivisor)
    );
}

#[test]
fn dafny_rounding_and_natural_subtraction() {
    let two = Nat::from_u64(2);
    for (x, rounded) in [(-5, -3), (-3, -2), (-1, -1), (0, 0), (1, 1), (3, 2), (5, 3)] {
        assert_eq!(
            Int::from_i64(x).rounded_quotient(&two).unwrap(),
            Int::from_i64(rounded)
        );
    }
    assert_eq!(
        Int::zero().rounded_quotient(&Nat::zero()),
        Err(NumberError::ZeroDivisor)
    );
    assert_eq!(
        Nat::from_u64(1).sub(&Nat::from_u64(2)),
        Err(NumberError::NegativeNatural)
    );
}

#[test]
fn decimal_surface_and_large_euclidean_identity() {
    for bad in ["", "-", "+1", " 1", "1 ", "1.0", "１", "--1"] {
        assert_eq!(Int::from_decimal(bad), Err(NumberError::InvalidDecimal));
    }
    let x = Int::from_decimal("-000184467440737095516160000000000000000").unwrap();
    assert_eq!(x.decimal(), "-184467440737095516160000000000000000");
    assert_eq!(Nat::from_decimal("-0000").unwrap(), Nat::zero());
    let d = Int::from_decimal("-10000000000000000000000000000000003").unwrap();
    let (q, r) = x.div_rem(&d).unwrap();
    assert_eq!(d.mul(&q).add(&r.as_int()), x);
    assert!(r.cmp_exact(&d.abs().as_nat().unwrap()) == Ordering::Less);
    assert_eq!(x.rem(&d).unwrap(), r);
    assert_eq!(x.div(&d).unwrap(), q);
}

#[test]
fn round_large_signed_values_without_float_or_narrowing() {
    let n = Int::from_decimal("184467440737095516160000000000000000001").unwrap();
    let d = Nat::from_u64(3);
    let p = n.rounded_quotient(&d).unwrap();
    let m = n.neg().rounded_quotient(&d).unwrap();
    assert_eq!(p.neg(), m);
    assert_eq!(p.mul(&Int::from_i64(3)).sub(&n).abs(), Int::from_i64(1));
}

#[test]
fn primitive_boundary_matches_small_integer_reference() {
    for a in -80i64..=80 {
        for b in -13i64..=13 {
            if b == 0 {
                continue;
            }
            let (q, r) = Int::from_i64(a).div_rem(&Int::from_i64(b)).unwrap();
            assert_eq!(q.decimal(), a.div_euclid(b).to_string());
            assert_eq!(r.decimal(), a.rem_euclid(b).to_string());
        }
        for divisor in 1i64..=13 {
            let d = Nat::from_u64(divisor as u64);
            let expected_magnitude = (a.abs() + divisor / 2) / divisor;
            let expected = if a < 0 {
                -expected_magnitude
            } else {
                expected_magnitude
            };
            assert_eq!(
                Int::from_i64(a).rounded_quotient(&d).unwrap(),
                Int::from_i64(expected)
            );
        }
    }
}
