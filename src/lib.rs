#![feature(string_remove_matches, assert_matches)]

pub mod common;
pub mod domain;
pub mod parsers;
pub mod utils;
// pub(crate) mod utils;

use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::{io::stdout, num::NonZeroU8, path::PathBuf};

use crate::{
    parsers::{
        csv_parser::csv_zztx_parser_streaming, nom::CsvZzTxParserNomImpl,
        serde_parser::CsvZzTxParserSerdeImpl,
    },
    utils::write_csv_client_balance_sheet,
};

#[derive(Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ParsingStrictnessOptions {
    /// Disallows extra fields
    Fail,
    /// Allows row if parse error found but will ignore if error is irrecoverable (e.g. missing
    /// field)
    Allow,
    /// Ignores rows if parse error found
    Ignore,
}

/// The parser implementation that will be used
#[derive(Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum ParserImplOptions {
    /// Use nom to parse csv contents
    Nom,
    /// Use serde (and csv crate) to parse csv contents
    /// Obs: currently doesn't work
    Serde,
}

impl ParsingStrictnessOptions {
    pub fn fail(&self) -> bool {
        matches!(self, Self::Fail)
    }

    pub fn ignore(&self) -> bool {
        matches!(self, Self::Ignore)
    }

    pub fn allow(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

serde_plain::derive_display_from_serialize!(ParsingStrictnessOptions);
serde_plain::derive_display_from_serialize!(ParserImplOptions);

/// Input for the zzzzzzzzzzz program
#[derive(Parser)]
pub struct ZzProcessCsvInput {
    /// The relative path of the csv file
    file: PathBuf,
    #[clap(flatten)]
    parse_options: ZzParseOptions,
    #[arg(long, default_value_t = ParserImplOptions::Nom)]
    parser: ParserImplOptions,
    // #[clap(flatten)]
    // execute_options: ZzExecuteOptions,
}

#[derive(Clone, Parser)]
pub struct ZzParseOptions {
    /// The maximum size of the integer part of a decimal which can be parsed
    #[arg(short, long, default_value_t = 200)]
    zz_amount_max_size: u16,
    /// What to do if found a row with a missing field
    #[arg(long, default_value_t = ParsingStrictnessOptions::Fail)]
    on_missing_field: ParsingStrictnessOptions,
    /// What to do if found a row with a missing field
    #[arg(long, default_value_t = ParsingStrictnessOptions::Fail)]
    on_excessive_field: ParsingStrictnessOptions,
    /// What to do if some parse error happens
    #[arg(long, default_value_t = ParsingStrictnessOptions::Fail)]
    on_parse_error: ParsingStrictnessOptions,
    /// The maximum line width, anything over this will fail
    #[arg(long, default_value_t = 4096)]
    max_line_width: usize,
}

#[allow(dead_code)]
#[derive(Clone, Default, Parser)]
pub struct ZzExecuteOptions {
    /// The total size of each io buffer
    #[arg(short, long)]
    buffers_mb: Option<NonZeroU8>,
    /// The total threads that will be assigned to io (reading the file)
    #[arg(short, long)]
    io_threads: Option<NonZeroU8>,
    /// The total buffers that will be allocated in a ring buffer for IO
    #[arg(short, long)]
    total_buffers: Option<NonZeroU8>,
}

impl Default for ZzParseOptions {
    fn default() -> Self {
        Self {
            zz_amount_max_size: 200,
            on_missing_field: ParsingStrictnessOptions::Fail,
            on_excessive_field: ParsingStrictnessOptions::Fail,
            on_parse_error: ParsingStrictnessOptions::Fail,
            max_line_width: 4096,
        }
    }
}

/// Process a csv and write the resulting csv to stdout. This doesn't
pub fn process_csv(input: &ZzProcessCsvInput) {
    let file = std::fs::File::open(&input.file).unwrap();
    let client_balance_map = match input.parser {
        ParserImplOptions::Nom => {
            csv_zztx_parser_streaming(&mut CsvZzTxParserNomImpl, &file, &input.parse_options)
        }
        ParserImplOptions::Serde => csv_zztx_parser_streaming(
            &mut CsvZzTxParserSerdeImpl::default(),
            &file,
            &input.parse_options,
        ),
    };

    write_csv_client_balance_sheet(
        client_balance_map.iter().filter_map(|x| x.as_ref()),
        stdout(),
    )
    .unwrap()
}
