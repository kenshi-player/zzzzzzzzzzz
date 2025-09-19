use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::take_while_m_n,
    character::complete::{char, digit1},
    combinator::opt,
    sequence::preceded,
};
use num_bigint::{BigInt, BigUint};

use crate::{
    ZzParseOptions,
    common::zz_amount::{IntFromBytes, ZzIAmount, ZzUAmount},
};

/// nom parser for ZzIAmount
pub fn parse_zzamount_i<'a>(
    parse_options: &ZzParseOptions,
    initial_input: &'a str,
) -> IResult<&'a str, ZzIAmount> {
    let (input, (int, decimal)) = parse_zzamount_inner::<BigInt>(parse_options, initial_input)?;

    Ok((
        input,
        ZzIAmount::new(int, decimal).expect("Parser above guarantees only 4 digits"),
    ))
}

/// nom parser for ZzUAmount
pub fn parse_zzamount_u<'a>(
    parse_options: &ZzParseOptions,
    initial_input: &'a str,
) -> IResult<&'a str, ZzUAmount> {
    let (input, (int, decimal)) = parse_zzamount_inner::<BigUint>(parse_options, initial_input)?;

    Ok((
        input,
        ZzUAmount::new(int, decimal).expect("Parser above guarantees only 4 digits"),
    ))
}

fn parse_zzamount_inner<'a, Int: IntFromBytes>(
    parse_options: &ZzParseOptions,
    initial_input: &'a str,
) -> IResult<&'a str, (Int, u32)> {
    let (input, sign) = opt(alt((char('+'), char('-')))).parse(initial_input)?;

    // Parse integer part
    let (input, int_str) =
        take_while_m_n(1, (parse_options.zz_amount_max_size + 1) as _, |c: char| {
            c.is_ascii_digit()
        })(input)?;
    if int_str.len() > parse_options.zz_amount_max_size as _ {
        return Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TooLarge,
        )));
    }

    // Parse optional decimal part
    let (input, decimal_opt) = opt(preceded(char('.'), digit1)).parse(input)?;

    let mut int = Int::parse_bytes(int_str.as_bytes(), 10).unwrap();
    if let Some('-') = sign {
        int = int.unary().ok_or({
            nom::Err::Failure(nom::error::Error {
                input: initial_input,
                code: nom::error::ErrorKind::Digit,
            })
        })?;
    }

    let decimal = if let Some(d) = decimal_opt {
        let d_val: u32 = d
            .split_at_checked(4)
            .map(|(truncated, _)| truncated.to_string())
            .unwrap_or(format!("{d:0<4}"))
            .parse()
            .expect("Decimal part uses digit1 filter");
        d_val
    } else {
        0
    };

    Ok((input, (int, decimal)))
}

#[cfg(test)]
mod tests {
    use crate::common::zz_amount::ZzUAmount;

    use super::*;
    use fake::{Fake, Faker};

    #[test]
    fn test_parse_integer() {
        let opts = ZzParseOptions::default();
        let (_, amt) = parse_zzamount_i(&opts, "123").unwrap();
        assert_eq!(amt.to_string(), "123");
    }

    #[test]
    fn test_parse_integer_negative() {
        let opts = ZzParseOptions::default();
        let (_, amt) = parse_zzamount_i(&opts, "-456").unwrap();
        assert_eq!(amt.to_string(), "-456");
    }

    #[test]
    fn test_parse_decimal() {
        let opts = ZzParseOptions::default();
        let (_, amt) = parse_zzamount_i(&opts, "123.7890").unwrap();
        assert_eq!(amt.to_string(), "123.7890");
    }

    #[test]
    fn test_parse_negative_decimal() {
        let opts = ZzParseOptions::default();
        let (_, amt) = parse_zzamount_i(&opts, "-456.0123").unwrap();
        assert_eq!(amt.to_string(), "-456.0123");
    }

    #[test]
    fn test_uint_fails_on_negative() {
        let opts = ZzParseOptions::default();
        assert!(parse_zzamount_u(&opts, "-456").is_err());
        assert!(parse_zzamount_u(&opts, "-456.0123").is_err());
    }

    #[test]
    fn test_parse_decimal_truncation() {
        let opts = ZzParseOptions::default();
        let (_, amt) = parse_zzamount_u(&opts, "1.123456").unwrap();
        // Only first 4 digits of decimal are kept
        assert_eq!(amt.to_string(), "1.1234");
    }

    #[test]
    fn test_parse_invalid_decimal_truncates() {
        let opts = ZzParseOptions::default();
        // Parser takes only first 4 decimal digits, ignores rest
        let (_, mut amt) = parse_zzamount_u(&opts, "12345.99999").unwrap();
        assert_eq!(amt.inner_mut().clone() % 10_000u32, (9999u32).into());
    }

    #[test]
    fn test_integer_size_limit_respected() {
        let opts = ZzParseOptions {
            zz_amount_max_size: 5,
            ..Default::default()
        };

        // Exactly 5 digits works
        let (_, amt) = parse_zzamount_u(&opts, "12345").unwrap();
        assert_eq!(amt.to_string(), "12345");

        // More than 5 digits fails
        let result = parse_zzamount_u(&opts, "123456");
        assert!(result.is_err(), "Expected error for exceeding max digits");
    }

    #[test]
    fn test_large_limit_allows_big_integers() {
        let opts = ZzParseOptions {
            zz_amount_max_size: 50,
            ..Default::default()
        };

        let big_num = "9".repeat(50);
        let (_, amt) = parse_zzamount_u(&opts, &big_num).unwrap();
        assert_eq!(amt.to_string(), big_num);
    }

    #[test]
    fn test_zzamount_fuzz() {
        let opts = ZzParseOptions::default();

        for _ in 0..500 {
            let amount: ZzUAmount = Faker.fake();
            let amount_ser = format!("{amount}");
            let (_, amount_de) = parse_zzamount_u(&opts, &amount_ser).unwrap();

            assert_eq!(amount, amount_de);
        }

        for _ in 0..500 {
            let amount: ZzIAmount = Faker.fake();
            let amount_ser = format!("{amount}");
            let (_, amount_de) = parse_zzamount_i(&opts, &amount_ser).unwrap();

            assert_eq!(amount, amount_de);
        }
    }

    #[test]
    fn test_zzamount_decimal() {
        let cases = [
            (ZzUAmount::new(0u32.into(), 1).unwrap(), "0.0001", "0.0001"),
            (ZzUAmount::new(0u32.into(), 10).unwrap(), "0.001", "0.0010"),
            (ZzUAmount::new(0u32.into(), 100).unwrap(), "0.01", "0.0100"),
            (ZzUAmount::new(0u32.into(), 1000).unwrap(), "0.1", "0.1000"),
        ];

        let opts = ZzParseOptions::default();

        for (amount, input, output) in cases {
            assert_eq!(amount.to_string(), output);

            let (_, amount_de) = parse_zzamount_u(&opts, input).unwrap();
            assert_eq!(amount, amount_de);
        }
    }
}
