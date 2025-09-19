use clap::Parser;
use zzzzzzzzzzz::{ZzProcessCsvInput, process_csv};

pub fn main() {
    process_csv(&ZzProcessCsvInput::parse());
}
