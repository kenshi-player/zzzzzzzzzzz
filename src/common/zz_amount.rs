use std::{
    fmt::Display,
    ops::{Add, Sub},
};

use fake::Faker;
use num_bigint::{BigInt, BigUint, Sign};
use serde::Serialize;

/// Adaptor trait to allow creating a DRY generic ZzAmount<Int>
#[doc(hidden)]
pub trait IntFromBytes: Add + Sub + Serialize + Sized + Display {
    fn parse_bytes(buf: &[u8], radix: u32) -> Option<Self>;
    fn unary(&self) -> Option<Self>;
}

impl IntFromBytes for BigInt {
    fn parse_bytes(buf: &[u8], radix: u32) -> Option<Self> {
        BigInt::parse_bytes(buf, radix)
    }
    fn unary(&self) -> Option<Self> {
        Some(-self)
    }
}

impl IntFromBytes for BigUint {
    fn parse_bytes(buf: &[u8], radix: u32) -> Option<Self> {
        BigUint::parse_bytes(buf, radix)
    }
    fn unary(&self) -> Option<Self> {
        None
    }
}

/// A simple struct implementation for the use case of unbounded integer part and up to 4 digits of
/// precision for decimal.
///
/// FIXME(perf): the methods for this struct were built on demand (e.g. some only exist for ZzIAmount) and the lack of
/// flexibility of this struct for handling references is causing excessive copying in the project.
#[derive(Debug, Clone, PartialEq)]
pub struct ZzAmount<Int: IntFromBytes> {
    integer: Int,
    decimal: u32,
}

pub type ZzUAmount = ZzAmount<BigUint>;
pub type ZzIAmount = ZzAmount<BigInt>;

impl<Int: IntFromBytes> ZzAmount<Int> {
    /// Returns Some(...) if decimal is a value between 0..10000, returns None otherwise
    pub fn new(integer: Int, decimal: u32) -> Option<Self> {
        Self::validate_inner(&decimal).then(|| Self { integer, decimal })
    }

    pub fn decimal(&self) -> u32 {
        self.decimal
    }

    /// Validates if decimal is between 0..10000
    pub fn validate(&self) -> bool {
        Self::validate_inner(&self.decimal)
    }

    fn validate_inner(decimal: &u32) -> bool {
        (0..10000).contains(decimal)
    }
}

impl ZzUAmount {
    pub fn to_i_amount(self) -> ZzIAmount {
        ZzIAmount {
            integer: num_bigint::BigInt::from_biguint(num_bigint::Sign::Plus, self.integer),
            decimal: self.decimal,
        }
    }
}

impl ZzIAmount {
    pub fn unary(self) -> Self {
        Self {
            integer: -self.integer,
            decimal: self.decimal,
        }
    }

    pub fn zero() -> Self {
        Self {
            integer: 0.into(),
            decimal: 0,
        }
    }

    pub fn add(&mut self, other: &ZzIAmount) {
        self.decimal += other.decimal;
        if self.validate() {
            self.integer += &other.integer;
        } else {
            self.decimal -= 10_000;
            self.integer += &other.integer + 1;
        }
    }

    pub fn sub(&mut self, other: &ZzIAmount) {
        if self.decimal < other.decimal {
            self.decimal += 10_000 - other.decimal;
            self.integer -= &other.integer + 1;
        } else {
            self.decimal -= other.decimal;
            self.integer -= &other.integer;
        }
    }

    pub fn greater_eq_than(&self, other: ZzUAmount) -> bool {
        if self.integer.sign() != Sign::Minus {
            let other_int: BigInt = other.integer.into();
            self.integer >= other_int
                || (self.integer == other_int && self.decimal >= other.decimal)
        } else {
            false
        }
    }
}

impl<Int: IntFromBytes> std::fmt::Display for ZzAmount<Int> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.decimal == 0 {
            write!(f, "{}", self.integer)
        } else {
            write!(f, "{}.{:0>4}", self.integer, self.decimal)
        }
    }
}

impl<Int: IntFromBytes> Serialize for ZzAmount<Int> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl fake::Dummy<Faker> for ZzAmount<BigInt> {
    fn dummy_with_rng<R: fake::Rng + ?Sized>(_config: &Faker, rng: &mut R) -> Self {
        use num_bigint::ToBigInt;

        let integer: i128 = rng.random();
        let decimal: u32 = rng.random_range(0..10000);

        Self {
            integer: integer.to_bigint().unwrap(),
            decimal,
        }
    }
}

impl fake::Dummy<Faker> for ZzAmount<BigUint> {
    fn dummy_with_rng<R: fake::Rng + ?Sized>(_config: &Faker, rng: &mut R) -> Self {
        use num_bigint::ToBigUint;

        let integer: u128 = rng.random();
        let decimal: u32 = rng.random_range(0..10000);

        Self {
            integer: integer.to_biguint().unwrap(),
            decimal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::ToBigInt;

    #[test]
    fn test_display_positive_integer() {
        let amt = ZzAmount::new(123.to_bigint().unwrap(), 0).unwrap();
        assert_eq!(amt.to_string(), "123");
    }

    #[test]
    fn test_display_positive_decimal() {
        let amt = ZzAmount::new(123.to_bigint().unwrap(), 4567).unwrap();
        assert_eq!(amt.to_string(), "123.4567");
    }

    #[test]
    fn test_display_negative_integer() {
        let amt = ZzAmount::new((-789).to_bigint().unwrap(), 0).unwrap();
        assert_eq!(amt.to_string(), "-789");
    }

    #[test]
    fn test_display_negative_decimal() {
        let amt = ZzAmount::new((-789).to_bigint().unwrap(), 1234).unwrap();
        assert_eq!(amt.to_string(), "-789.1234");
    }

    #[test]
    fn test_invalid_decimal() {
        assert!(ZzAmount::new(1.to_bigint().unwrap(), 10000).is_none());
    }

    // ---------- Tests for add and sub ----------
    #[test]
    fn test_add_without_carry() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 2000).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 3000).unwrap();

        a.add(&b);

        // 2000 + 3000 = 5000 < 10000, no carry
        assert_eq!(a.integer, 15.to_bigint().unwrap());
        assert_eq!(a.decimal, 5000);
    }

    #[test]
    fn test_add_with_carry() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 7000).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 4000).unwrap();

        a.add(&b);

        // 7000 + 4000 = 11000 -> carry 1 to integer, decimal = 1000
        assert_eq!(a.integer, 16.to_bigint().unwrap());
        assert_eq!(a.decimal, 1000);
    }

    #[test]
    fn test_add_with_exact_carry_boundary() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 6000).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 4000).unwrap();

        a.add(&b);

        // 6000 + 4000 = 10000 -> carry 1, decimal = 0
        assert_eq!(a.integer, 16.to_bigint().unwrap());
        assert_eq!(a.decimal, 0);
    }

    #[test]
    fn test_sub_without_borrow() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 5000).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 3000).unwrap();

        a.sub(&b);

        // 5000 - 3000 = 2000, integer = 10 - 5 = 5
        assert_eq!(a.integer, 5.to_bigint().unwrap());
        assert_eq!(a.decimal, 2000);
    }

    #[test]
    fn test_sub_with_borrow() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 2000).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 3000).unwrap();

        a.sub(&b);

        // 2000 < 3000, borrow: decimal = 2000 + 10000 - 3000 = 9000
        // integer = 10 - (5 + 1) = 4
        assert_eq!(a.integer, 4.to_bigint().unwrap());
        assert_eq!(a.decimal, 9000);
    }

    #[test]
    fn test_sub_with_exact_borrow_boundary() {
        let mut a = ZzIAmount::new(10.to_bigint().unwrap(), 0).unwrap();
        let b = ZzIAmount::new(5.to_bigint().unwrap(), 1).unwrap();

        a.sub(&b);

        // borrow: decimal = 0 + 10000 - 1 = 9999
        // integer = 10 - (5 + 1) = 4
        assert_eq!(a.integer, 4.to_bigint().unwrap());
        assert_eq!(a.decimal, 9999);
    }

    #[test]
    fn test_add_and_sub_inverse_relationship() {
        let a = ZzIAmount::new(10.to_bigint().unwrap(), 5000).unwrap();
        let b = ZzIAmount::new(3.to_bigint().unwrap(), 2500).unwrap();

        let mut c = a.clone();
        c.add(&b);
        c.sub(&b);

        assert_eq!(c.integer, a.integer);
        assert_eq!(c.decimal, a.decimal);
    }
}
