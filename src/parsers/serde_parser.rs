use std::sync::LazyLock;

use serde::Deserialize;

use crate::{
    ZzParseOptions,
    domain::transaction::{ZzTx, ZzTxType, ZzTxTypeDiscriminants},
    parsers::csv_parser::{CsvParserResult, CsvZzTxParserTrait},
};

#[derive(Debug, Deserialize)]
pub struct ZzTxSerde<'a> {
    r#type: Option<ZzTxTypeDiscriminants>,
    #[serde(rename = "client")]
    client_id: Option<u16>,
    #[serde(rename = "tx")]
    tx_id: Option<u32>,
    amount: Option<&'a str>,
}

impl ZzTxSerde<'_> {
    pub fn to_zztx(self, parse_options: &ZzParseOptions) -> CsvParserResult {
        let Some(r#type) = self.r#type else {
            return CsvParserResult::MissingRequiredField;
        };
        let Some(client_id) = self.client_id else {
            return CsvParserResult::MissingRequiredField;
        };
        let Some(tx_id) = self.tx_id else {
            return CsvParserResult::MissingRequiredField;
        };
        let amount = if let Some(amount) = self.amount {
            let Ok(amount) = crate::parsers::nom::zz_amount::parse_zzamount(parse_options, amount)
                .map(|(_, res)| res)
            else {
                return CsvParserResult::Failed;
            };
            Some(amount)
        } else {
            None
        };

        let build_tx = move |r#type: ZzTxType| ZzTx {
            r#type,
            client_id,
            tx_id,
        };

        match (r#type, amount) {
            (ZzTxTypeDiscriminants::Deposit, Some(amount)) => {
                CsvParserResult::Parsed(build_tx(ZzTxType::Deposit(amount)))
            }
            (ZzTxTypeDiscriminants::Withdrawal, Some(amount)) => {
                CsvParserResult::Parsed(build_tx(ZzTxType::Withdrawal(amount)))
            }
            (ZzTxTypeDiscriminants::Dispute, None) => {
                CsvParserResult::Parsed(build_tx(ZzTxType::Dispute))
            }
            (ZzTxTypeDiscriminants::Resolve, None) => {
                CsvParserResult::Parsed(build_tx(ZzTxType::Resolve))
            }
            (ZzTxTypeDiscriminants::Chargeback, None) => {
                CsvParserResult::Parsed(build_tx(ZzTxType::Chargeback))
            }
            (ZzTxTypeDiscriminants::Deposit, None) | (ZzTxTypeDiscriminants::Withdrawal, None) => {
                CsvParserResult::MissingRequiredField
            }
            (ZzTxTypeDiscriminants::Dispute, Some(_)) => {
                CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Dispute))
            }
            (ZzTxTypeDiscriminants::Resolve, Some(_)) => {
                CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Resolve))
            }
            (ZzTxTypeDiscriminants::Chargeback, Some(_)) => {
                CsvParserResult::ContainsExcessiveFields(build_tx(ZzTxType::Chargeback))
            }
        }
    }
}

#[derive(Default)]
pub struct CsvZzTxParserSerdeImpl {
    raw_record: csv::StringRecord,
}

static HEADERS_RECORD: LazyLock<csv::StringRecord> = LazyLock::new(|| {
    let mut record = csv::StringRecord::new();
    record.push_field("type");
    record.push_field("client");
    record.push_field("tx");
    record.push_field("amount");
    record
});

impl CsvZzTxParserTrait for CsvZzTxParserSerdeImpl {
    fn deserialize_headers(
        &mut self,
        _parse_options: &crate::ZzParseOptions,
        header: &str,
    ) -> bool {
        let mut rdr = csv::Reader::from_reader(header.as_bytes());
        rdr.headers()
            .is_ok_and(|headers| headers == &*HEADERS_RECORD)
    }

    fn deserialize_row(&mut self, parse_options: &ZzParseOptions, row: &str) -> CsvParserResult {
        let mut rdr = csv::Reader::from_reader(row.as_bytes());
        if rdr.read_record(&mut self.raw_record).is_err() {
            return CsvParserResult::Failed;
        }

        let Ok(record) = self
            .raw_record
            .deserialize::<'_, ZzTxSerde>(Some(&HEADERS_RECORD))
        else {
            return CsvParserResult::Failed;
        };

        record.to_zztx(parse_options)
    }
}
