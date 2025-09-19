use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, multispace0},
    combinator::{eof, map_res, opt, recognize},
    sequence::terminated,
};

use crate::{
    ZzParseOptions,
    domain::transaction::{ZzTx, ZzTxType},
    parsers::{csv_parser::CsvParserResult, nom::zz_amount::parse_zzamount_u},
};

fn wrap_field<'a, P: Parser<&'a str, Error = nom::error::Error<&'a str>>>(
    parser: P,
    parse_options: &ZzParseOptions,
) -> impl FnOnce(&'a str) -> IResult<&'a str, Option<P::Output>> {
    move |input: &str| {
        macro_rules! terminated_parser {
            ($parser:expr) => {
                terminated($parser, alt((tag(","), eof))).parse(input)?
            };
        }

        let (input, res) = if parse_options.dont_trim_spaces {
            terminated_parser!(opt(parser))
        } else {
            let (input, (_, res, _)) = terminated_parser!((multispace0, opt(parser), multispace0));
            (input, res)
        };

        Ok((input, res))
    }
}

pub fn parse_zztx_csv_headers<'a>(
    parse_options: &ZzParseOptions,
    input: &'a str,
) -> IResult<&'a str, ()> {
    let (input, _) = wrap_field(tag("type"), parse_options)(input)?;
    let (input, _) = wrap_field(tag("client"), parse_options)(input)?;
    let (input, _) = wrap_field(tag("tx"), parse_options)(input)?;
    let (input, _) = wrap_field(tag("amount"), parse_options)(input)?;

    Ok((input, ()))
}

/// Parses a ZzTx from a csv row
///
/// initial_input: is a string that should start with the row
/// expect_eof: is the end of input actually the EOF of the parsed csv
///
/// # Errors
///
/// If expect_eof == false and the starting row is not terminated by \n
/// Malformatted row
pub fn parse_zztx_csv<'a>(
    parse_options: &ZzParseOptions,
    input: &'a str,
) -> IResult<&'a str, CsvParserResult> {
    fn parse_u16(input: &str) -> IResult<&str, u16> {
        map_res(digit1, str::parse::<u16>).parse(input)
    }
    fn parse_u32(input: &str) -> IResult<&str, u32> {
        map_res(digit1, str::parse::<u32>).parse(input)
    }

    let tx_type_parser = wrap_field(
        alt((
            tag("deposit"),
            tag("withdrawal"),
            tag("dispute"),
            tag("resolve"),
            tag("chargeback"),
        )),
        parse_options,
    );
    let client_id_parser = wrap_field(parse_u16, parse_options);
    let tx_id_parser = wrap_field(parse_u32, parse_options);

    let zz_amount_parser = wrap_field(
        map_res(recognize((digit1, opt((char('.'), digit1)))), |s: &str| {
            let (_, amt) = parse_zzamount_u(parse_options, s)?;
            Ok::<_, nom::Err<nom::error::Error<&str>>>(amt)
        }),
        parse_options,
    );

    // tx type
    let (input, tx_type_str) = tx_type_parser(input)?;
    let Some(tx_type_str) = tx_type_str else {
        return Ok((input, CsvParserResult::MissingRequiredField));
    };

    // client id
    let (input, client_id) = client_id_parser(input)?;
    let Some(client_id) = client_id else {
        return Ok((input, CsvParserResult::MissingRequiredField));
    };

    // tx id
    let (input, tx_id) = tx_id_parser(input)?;
    let Some(tx_id) = tx_id else {
        return Ok((input, CsvParserResult::MissingRequiredField));
    };

    // amount
    let (input, zz_amount) = zz_amount_parser(input)?;

    let build_tx = move |r#type: ZzTxType| ZzTx {
        r#type,
        client_id,
        tx_id,
    };

    let res = match (tx_type_str, zz_amount) {
        ("deposit", Some(amount)) => CsvParserResult::Parsed(build_tx(ZzTxType::Deposit(amount))),
        ("withdrawal", Some(amount)) => {
            CsvParserResult::Parsed(build_tx(ZzTxType::Withdrawal(amount)))
        }
        ("dispute", None) => CsvParserResult::Parsed(build_tx(ZzTxType::Dispute)),
        ("resolve", None) => CsvParserResult::Parsed(build_tx(ZzTxType::Resolve)),
        ("chargeback", None) => CsvParserResult::Parsed(build_tx(ZzTxType::Chargeback)),
        ("deposit", None) | ("withdrawal", None) => {
            return Ok((input, CsvParserResult::MissingRequiredField));
        }
        ("dispute", Some(_)) => {
            CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Dispute))
        }
        ("resolve", Some(_)) => {
            CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Resolve))
        }
        ("chargeback", Some(_)) => {
            CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Chargeback))
        }
        _ => unreachable!("tx_type_parser guards the possible values"),
    };

    Ok((input, res))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transaction::{ZzTxSerializeCsv, ZzTxType};
    use fake::{Fake, Faker};

    #[test]
    fn test_parse_zztx_happy_path() {
        let opts = &ZzParseOptions::default();

        let (_, ctrl) = parse_zztx_csv(opts, "deposit,1,10,50").unwrap();
        match ctrl {
            CsvParserResult::Parsed(tx) => match tx.r#type {
                ZzTxType::Deposit(amount) => assert_eq!(amount.to_string(), "50"),
                _ => panic!("Expected Deposit"),
            },
            _ => panic!("Expected Parsed"),
        }

        let (_, ctrl) = parse_zztx_csv(opts, "withdrawal,2,20,30").unwrap();
        match ctrl {
            CsvParserResult::Parsed(tx) => match tx.r#type {
                ZzTxType::Withdrawal(amount) => assert_eq!(amount.to_string(), "30"),
                _ => panic!("Expected Withdraw"),
            },
            _ => panic!("Expected Parsed"),
        }

        let (_, ctrl) = parse_zztx_csv(opts, "dispute,3,30,").unwrap();
        match ctrl {
            CsvParserResult::Parsed(tx) => assert!(matches!(tx.r#type, ZzTxType::Dispute)),
            _ => panic!("Expected Parsed"),
        }
    }

    #[test]
    fn test_missing_field_behavior() {
        let opts = &mut ZzParseOptions::default();

        let (_, ctrl) = parse_zztx_csv(opts, "deposit,1,10").unwrap();
        assert!(matches!(ctrl, CsvParserResult::MissingRequiredField));
    }

    #[test]
    fn test_excessive_field_behavior() {
        let opts = &mut ZzParseOptions::default();

        // Dispute should not have amount → Fail
        let (_, ctrl) = parse_zztx_csv(opts, "dispute,1,42,999").unwrap();
        assert_eq!(
            ctrl,
            CsvParserResult::ContainsExcessiveFields(ZzTx {
                r#type: ZzTxType::Dispute,
                client_id: 1,
                tx_id: 42
            })
        );
    }

    #[test]
    fn test_invalid_or_garbage() {
        let opts = &ZzParseOptions::default();

        // Unknown transaction type → fail
        let res = parse_zztx_csv(opts, "foobar,1,2,3");
        assert!(res.is_err());

        // Trailing garbage → fail
        let res = parse_zztx_csv(opts, "deposit,1,2,30xxx");
        assert!(res.is_err());
    }

    #[test]
    fn test_with_spaces_variations() {
        let opts = &ZzParseOptions::default();

        let (_, ctrl) = parse_zztx_csv(opts, "deposit ,   42 ,  99 ,   1000").unwrap();
        match ctrl {
            CsvParserResult::Parsed(tx) => {
                assert_eq!(tx.client_id, 42);
                assert_eq!(tx.tx_id, 99);
                match tx.r#type {
                    ZzTxType::Deposit(amount) => assert_eq!(amount.to_string(), "1000"),
                    _ => panic!("Expected Deposit"),
                }
            }
            _ => panic!("Expected Parsed"),
        }
    }

    #[test]
    fn test_fuzz_roundtrip() {
        let opts = &ZzParseOptions::default();

        for _ in 0..50 {
            let tx: ZzTx = Faker.fake();

            // serialize to CSV
            let line = format!("{}", ZzTxSerializeCsv(tx.clone()));

            // parse back
            let (_, ctrl) = parse_zztx_csv(opts, &line).unwrap();
            match ctrl {
                CsvParserResult::Parsed(parsed) => assert_eq!(tx, parsed),
                _ => panic!("Expected Parsed"),
            }
        }
    }
}
