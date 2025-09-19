use crate::parsers::csv_parser::{CsvParserResult, CsvZzTxParserTrait};

pub mod tx_parser;
pub mod zz_amount;

pub struct CsvZzTxParserNomImpl;

impl CsvZzTxParserTrait for CsvZzTxParserNomImpl {
    fn deserialize_headers(&mut self, parse_options: &crate::ZzParseOptions, header: &str) -> bool {
        tx_parser::parse_zztx_csv_headers(parse_options, header).is_ok()
    }

    fn deserialize_row(
        &mut self,
        parse_options: &crate::ZzParseOptions,
        row: &str,
    ) -> CsvParserResult {
        tx_parser::parse_zztx_csv(parse_options, row)
            .map(|(_, res)| res)
            .unwrap_or(CsvParserResult::Failed)
    }
}
