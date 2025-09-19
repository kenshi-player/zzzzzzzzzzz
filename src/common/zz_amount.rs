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
/// precision for decimal. This is done by serializing/deserializing the struct into the big int
/// divided by 10_000
#[derive(Debug, Clone, PartialEq)]
pub struct ZzAmount<Int: IntFromBytes> {
    integer: Int,
}

pub type ZzUAmount = ZzAmount<BigUint>;
pub type ZzIAmount = ZzAmount<BigInt>;

impl<Int: IntFromBytes> ZzAmount<Int> {
    pub fn inner_mut(&mut self) -> &mut Int {
        &mut self.integer
    }
}

impl ZzUAmount {
    pub fn new(mut integer: BigUint, decimal: u32) -> Option<Self> {
        if decimal > 10_000 {
            return None;
        }

        integer *= 10_000u32;
        integer += decimal;

        Some(Self { integer })
    }

    pub fn to_i_amount(self) -> ZzIAmount {
        ZzIAmount {
            integer: num_bigint::BigInt::from_biguint(num_bigint::Sign::Plus, self.integer),
        }
    }
}

impl ZzIAmount {
    pub fn new(mut integer: BigInt, decimal: u32) -> Option<Self> {
        if decimal > 10_000 {
            return None;
        }

        integer *= 10_000;
        if integer.sign() == Sign::Minus {
            integer -= decimal;
        } else {
            integer += decimal;
        }

        Some(Self { integer })
    }

    pub fn unary(self) -> Self {
        Self {
            integer: -self.integer,
        }
    }

    pub fn zero() -> Self {
        Self { integer: 0.into() }
    }

    pub fn add(&mut self, other: &Self) {
        self.integer += &other.integer;
    }

    pub fn sub(&mut self, other: &Self) {
        self.integer -= &other.integer;
    }

    pub fn greater_eq_than(&self, other: ZzUAmount) -> bool {
        if self.integer.sign() != Sign::Minus {
            let other_int: BigInt = other.integer.into();
            self.integer >= other_int
        } else {
            false
        }
    }
}

impl std::fmt::Display for ZzAmount<BigInt> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (sign, abs_val) = if self.integer.sign() == Sign::Minus {
            ("-", -&self.integer)
        } else {
            ("", self.integer.clone())
        };

        let int = &abs_val / 10_000;
        let decimal: BigInt = &abs_val % 10_000;

        if decimal == BigInt::ZERO {
            write!(f, "{sign}{int}")
        } else {
            write!(f, "{sign}{int}.{decimal:0>4}")
        }
    }
}

impl std::fmt::Display for ZzAmount<BigUint> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let decimal = &self.integer % 10_000u32;
        let int = &self.integer / 10_000u32;

        if decimal == BigUint::ZERO {
            write!(f, "{int}")
        } else {
            write!(f, "{int}.{decimal:0>4}")
        }
    }
}

impl Serialize for ZzAmount<BigInt> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl Serialize for ZzAmount<BigUint> {
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

        Self {
            integer: integer.to_bigint().unwrap(),
        }
    }
}

impl fake::Dummy<Faker> for ZzAmount<BigUint> {
    fn dummy_with_rng<R: fake::Rng + ?Sized>(_config: &Faker, rng: &mut R) -> Self {
        use num_bigint::ToBigUint;

        let integer: u128 = rng.random();

        Self {
            integer: integer.to_biguint().unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::ToBigInt;

    fn amt_i_parts(int: i64, dec: u32) -> ZzIAmount {
        ZzIAmount::new(int.to_bigint().unwrap(), dec).unwrap()
    }

    #[test]
    fn test_display_positive_integer() {
        let amt = amt_i_parts(123, 0); // 123.0000
        assert_eq!(amt.to_string(), "123");
    }

    #[test]
    fn test_display_positive_decimal() {
        let amt = amt_i_parts(123, 4567); // 123.4567
        assert_eq!(amt.to_string(), "123.4567");
    }

    #[test]
    fn test_display_negative_integer() {
        let amt = amt_i_parts(-789, 0);
        assert_eq!(amt.to_string(), "-789");
    }

    #[test]
    fn test_display_negative_decimal() {
        let amt = amt_i_parts(-789, 1234);
        assert_eq!(amt.to_string(), "-789.1234");
    }

    #[test]
    fn test_add_simple() {
        let mut a = amt_i_parts(10, 0); // 10.0000
        let b = amt_i_parts(5, 2500); // 5.2500

        a.add(&b);
        assert_eq!(a.to_string(), "15.2500");
    }

    #[test]
    fn test_sub_simple() {
        let mut a = amt_i_parts(10, 5000); // 10.5000
        let b = amt_i_parts(5, 3000); // 5.3000

        a.sub(&b);
        assert_eq!(a.to_string(), "5.2000");
    }

    #[test]
    fn test_sub_resulting_in_negative() {
        let mut a = amt_i_parts(0, 1); // 0.0001
        let b = amt_i_parts(0, 2); // 0.0002

        a.sub(&b);
        assert_eq!(a.to_string(), "-0.0001");
    }

    #[test]
    fn test_add_and_sub_inverse() {
        let a = amt_i_parts(10, 1234); // 10.1234
        let b = amt_i_parts(3, 9876); // 3.9876

        let mut c = a.clone();
        c.add(&b);
        c.sub(&b);

        assert_eq!(c, a);
    }

    #[test]
    fn test_equal_numbers_results_in_zero() {
        let mut a = amt_i_parts(7, 7777);
        let b = amt_i_parts(7, 7777);

        a.sub(&b);
        assert_eq!(a, ZzIAmount::zero());
    }

    #[test]
    fn test_sub_zero_minus_smallest_fraction() {
        let mut a = amt_i_parts(0, 0); // 0
        let b = amt_i_parts(0, 1); // 0.0001

        a.sub(&b);
        assert_eq!(a.to_string(), "-0.0001");
    }
}
