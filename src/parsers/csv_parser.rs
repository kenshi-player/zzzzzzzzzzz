use std::os::unix::fs::FileExt;

use crate::{
    ZzParseOptions,
    common::zz_amount::ZzIAmount,
    domain::{
        client_balance::ZzClientBalance,
        transaction::{TransactionHashMapImpl, TransactionMap, ZzTx},
    },
};

#[derive(Debug, PartialEq)]
pub enum CsvParserResult {
    Parsed(ZzTx),
    Failed,
    MissingRequiredField,
    ContainsExcessiveFields(ZzTx),
}

pub trait CsvZzTxParserTrait {
    /// If the header matches the expected ZzTx headers. This will be used to handle if the header
    /// is present or not.
    fn deserialize_headers(&mut self, parse_options: &crate::ZzParseOptions, header: &str) -> bool;
    /// Parse a row, the CsvParserControl
    fn deserialize_row(&mut self, parse_options: &ZzParseOptions, row: &str) -> CsvParserResult;
}

/// This is the main function for the current parsing loop.
///
/// If a csv file doesn't contain headers it'll still try to parse it as if it had headers
pub fn csv_zztx_parser_streaming<ZzTxParser: CsvZzTxParserTrait>(
    parser: &mut ZzTxParser,
    file: &std::fs::File,
    parse_options: &ZzParseOptions,
) -> Vec<Option<ZzClientBalance>> {
    let buf = &mut vec![0; 16 * 1024 * 1024];
    let mut offset = 0;
    // used to handle segmentation, it keeps the tail (last row) of the last read(). This is
    // necessary because we assume the parsers only parse full rows
    let mut tail = String::with_capacity(128);

    let mut tx_map = TransactionHashMapImpl::default();
    let mut client_balance_map = vec![None; u16::MAX as usize + 1];
    // used to keep track if having/not having headers was verified.
    let mut is_first = true;

    macro_rules! error_on_big_row {
        ($row:ident) => {
            if $row.len() > parse_options.max_line_width {
                panic!("Row too big");
            }
        };
    }

    let mut process_tx = |zztx: ZzTx| {
        let client_id = zztx.client_id;
        if let Some(effect) =
            tx_map.insert_transaction(zztx, client_balance_map[client_id as usize].as_ref())
        {
            // SAFETY: client_map is instantiated with enough entries to take any u16
            client_balance_map[client_id as usize]
                .get_or_insert_with(|| ZzClientBalance {
                    client_id,
                    available: ZzIAmount::zero(),
                    held: ZzIAmount::zero(),
                    total: ZzIAmount::zero(),
                    locked: false,
                })
                .process_tx_effect(effect);
        }
    };

    loop {
        let size = match file.read_at(buf, offset) {
            Ok(x) => x,
            Err(err) => panic!("{err}"),
        };
        if size == 0 {
            break;
        }
        offset += size as u64;

        let mut buf = str::from_utf8(&buf[..size]).unwrap();

        if is_first {
            let (first_row, rest) = buf.split_once('\n').unwrap_or((buf, ""));

            if first_row.len() == buf.len() {
                tail += first_row;
                error_on_big_row!(tail);

                continue;
            }

            if parser.deserialize_headers(parse_options, first_row) {
                buf = rest;
            }
            is_first = false;
        }

        let (segmented, rest) = buf.split_once('\n').unwrap_or((buf, ""));
        tail += segmented;

        let mut it = std::iter::once(segmented)
            .chain(rest.split('\n'))
            .peekable();

        while let Some(row) = it.next() {
            error_on_big_row!(row);
            if it.peek().is_none() {
                tail.clear();
                tail.push_str(row);

                break;
            }

            // because we already know there's a next element, this row is complete
            let zztx = match parser.deserialize_row(parse_options, row) {
                CsvParserResult::Parsed(zztx) => zztx,
                CsvParserResult::MissingRequiredField => {
                    if parse_options.on_missing_field.fail() {
                        panic!("Failed to parse csv. Row: {row}");
                    } else {
                        continue;
                    }
                }
                CsvParserResult::ContainsExcessiveFields(zztx) => {
                    match parse_options.on_excessive_field {
                        crate::ParsingStrictnessOptions::Fail => {
                            panic!("Failed to parse csv. Row: {row}")
                        }
                        crate::ParsingStrictnessOptions::Allow => zztx,
                        crate::ParsingStrictnessOptions::Ignore => continue,
                    }
                }
                CsvParserResult::Failed => {
                    if parse_options.on_parse_error.fail() {
                        panic!("Failed to parse csv. Row: {row}")
                    } else {
                        continue;
                    }
                }
            };

            process_tx(zztx);
        }
    }

    if !tail.is_empty() {
        error_on_big_row!(tail);
        match parser.deserialize_row(parse_options, &tail) {
            CsvParserResult::Parsed(zztx) => {
                process_tx(zztx);
            }
            CsvParserResult::ContainsExcessiveFields(zztx) => {
                match parse_options.on_excessive_field {
                    crate::ParsingStrictnessOptions::Fail => panic!("Failed to parse csv {tail}"),
                    crate::ParsingStrictnessOptions::Allow => process_tx(zztx),
                    crate::ParsingStrictnessOptions::Ignore => {}
                }
            }
            CsvParserResult::MissingRequiredField => {
                if parse_options.on_missing_field.fail() {
                    panic!("Failed to parse csv {tail}")
                }
            }
            CsvParserResult::Failed => {
                if parse_options.on_parse_error.fail() {
                    panic!("Failed to parse csv {tail}")
                }
            }
        }
    }

    for client in client_balance_map.iter_mut().flatten() {
        client.compute_total();
    }

    client_balance_map
}
