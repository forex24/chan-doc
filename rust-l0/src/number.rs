//! Exact signed and nonnegative integers, decimal conversion and arithmetic.

use num_bigint::BigInt;
use num_traits::{Signed, Zero};
use std::cmp::Ordering;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Int(Arc<BigInt>);
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Nat(Arc<BigInt>);
#[derive(Debug, PartialEq, Eq)]
pub enum NumberError {
    NegativeNatural,
    ZeroDivisor,
    InvalidDecimal,
}
impl Clone for Int {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Clone for Nat {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Int {
    pub fn zero() -> Self {
        Self(Arc::new(BigInt::zero()))
    }
    pub fn from_i64(value: i64) -> Self {
        Self(Arc::new(BigInt::from(value)))
    }
    pub fn from_decimal(value: &str) -> Result<Self, NumberError> {
        if !decimal_syntax(value) {
            return Err(NumberError::InvalidDecimal);
        }
        value
            .parse::<BigInt>()
            .map(|v| Self(Arc::new(v)))
            .map_err(|_| NumberError::InvalidDecimal)
    }
    pub fn decimal(&self) -> String {
        self.0.to_string()
    }
    pub fn cmp_exact(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
    pub fn add(&self, other: &Self) -> Self {
        Self(Arc::new(self.0.as_ref() + other.0.as_ref()))
    }
    pub fn sub(&self, other: &Self) -> Self {
        Self(Arc::new(self.0.as_ref() - other.0.as_ref()))
    }
    pub fn mul(&self, other: &Self) -> Self {
        Self(Arc::new(self.0.as_ref() * other.0.as_ref()))
    }
    pub fn neg(&self) -> Self {
        Self(Arc::new(-self.0.as_ref()))
    }
    pub fn abs(&self) -> Self {
        Self(Arc::new(self.0.abs()))
    }
    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }
    pub fn as_nat(&self) -> Result<Nat, NumberError> {
        if self.0.is_negative() {
            Err(NumberError::NegativeNatural)
        } else {
            Ok(Nat(self.0.clone()))
        }
    }
    #[doc = " Euclidean division, for either divisor sign."]
    pub fn div_rem(&self, divisor: &Self) -> Result<(Self, Nat), NumberError> {
        if divisor.0.is_zero() {
            return Err(NumberError::ZeroDivisor);
        }
        let mut q = self.0.as_ref() / divisor.0.as_ref();
        let mut r = self.0.as_ref() % divisor.0.as_ref();
        if r.is_negative() {
            r += divisor.0.abs();
            if divisor.0.is_positive() {
                q -= 1;
            } else {
                q += 1;
            }
        }
        Ok((Self(Arc::new(q)), Nat(Arc::new(r))))
    }
    pub fn div(&self, divisor: &Self) -> Result<Self, NumberError> {
        match self.div_rem(divisor) {
            Ok((q, _r)) => {
                {}
                Ok(q)
            }
            Err(e) => Err(e),
        }
    }
    pub fn rem(&self, divisor: &Self) -> Result<Nat, NumberError> {
        self.div_rem(divisor).map(|(_, r)| r)
    }
    pub fn rounded_quotient(&self, divisor: &Nat) -> Result<Self, NumberError> {
        if divisor.0.is_zero() {
            return Err(NumberError::ZeroDivisor);
        }
        let half = divisor.0.as_ref() / BigInt::from(2);
        let magnitude: BigInt = (self.0.abs() + half) / divisor.0.as_ref();
        Ok(Self(Arc::new(if self.0.is_negative() {
            -magnitude
        } else {
            magnitude
        })))
    }
}
impl Nat {
    pub fn zero() -> Self {
        Self(Arc::new(BigInt::zero()))
    }
    pub fn from_u64(value: u64) -> Self {
        Self(Arc::new(BigInt::from(value)))
    }
    pub fn from_decimal(value: &str) -> Result<Self, NumberError> {
        if !decimal_syntax(value) {
            return Err(NumberError::InvalidDecimal);
        }
        let raw = value
            .parse::<BigInt>()
            .map_err(|_| NumberError::InvalidDecimal)?;
        if raw.is_negative() {
            Err(NumberError::NegativeNatural)
        } else {
            Ok(Self(Arc::new(raw)))
        }
    }
    pub fn new(value: Int) -> Result<Self, NumberError> {
        if value.0.is_negative() {
            Err(NumberError::NegativeNatural)
        } else {
            Ok(Self(value.0))
        }
    }
    pub fn as_int(&self) -> Int {
        Int(self.0.clone())
    }
    pub fn decimal(&self) -> String {
        self.0.to_string()
    }
    pub fn cmp_exact(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
    pub fn add(&self, other: &Self) -> Self {
        Self(Arc::new(self.0.as_ref() + other.0.as_ref()))
    }
    pub fn mul(&self, other: &Self) -> Self {
        Self(Arc::new(self.0.as_ref() * other.0.as_ref()))
    }
    pub fn sub(&self, other: &Self) -> Result<Self, NumberError> {
        if self.0 < other.0 {
            Err(NumberError::NegativeNatural)
        } else {
            Ok(Self(Arc::new(self.0.as_ref() - other.0.as_ref())))
        }
    }
    pub fn div_rem(&self, divisor: &Self) -> Result<(Self, Self), NumberError> {
        if divisor.0.is_zero() {
            return Err(NumberError::ZeroDivisor);
        }
        let q = self.0.as_ref() / divisor.0.as_ref();
        let r = self.0.as_ref() % divisor.0.as_ref();
        Ok((Self(Arc::new(q)), Self(Arc::new(r))))
    }
}
fn decimal_syntax(s: &str) -> bool {
    let bytes = s.as_bytes();
    let start = if bytes.first() == Some(&b'-') { 1 } else { 0 };
    bytes.len() > start && bytes[start..].iter().all(u8::is_ascii_digit)
}
