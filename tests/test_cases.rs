use std::{
    io::Cursor,
    path::{Path, PathBuf},
    str::FromStr,
};

use zzzzzzzzzzz::{
    parsers::{csv_parser::csv_zztx_parser_streaming, nom::CsvZzTxParserNomImpl},
    utils::write_csv_client_balance_sheet,
};

fn run_test_case(test_dir_path: &Path) {
    let test_dir = std::fs::read_dir(test_dir_path).unwrap();

    let mut input = None;
    let mut output = None;

    for test_entry in test_dir {
        let test_entry = test_entry.unwrap();
        if !test_entry.file_type().unwrap().is_file() {
            continue;
        }

        match test_entry.file_name().to_str().unwrap() {
            "input.csv" => {
                input = Some(test_entry.path());
            }
            "output.csv" => {
                output = Some(test_entry.path());
            }
            _ => {}
        }
    }

    let input_file = input.expect("input file not found");
    let file = std::fs::File::open(&input_file).unwrap();

    let client_balance_map =
        csv_zztx_parser_streaming(&mut CsvZzTxParserNomImpl, &file, &Default::default());

    let mut res = vec![];
    let cursor = Cursor::new(&mut res);
    write_csv_client_balance_sheet(client_balance_map.iter().filter_map(|x| x.as_ref()), cursor)
        .unwrap();

    let output = output.expect("output file not found but csv was successfully produced");
    let v = std::fs::read(output).unwrap();

    assert_eq!(
        res,
        v,
        "\n{}\n!=\n{}",
        str::from_utf8(&res).unwrap(),
        str::from_utf8(&v).unwrap()
    );
}

macro_rules! test_case {
    ($($test_name:expr),+) => {
        paste::paste! {
        $(
            #[test]
            fn [<test_ $test_name _case>]() {
                run_test_case(&PathBuf::from_str(
                    concat!("tests/test_cases/", stringify!($test_name))
                ).unwrap())
            }
        )+
        }
    };
}

test_case!(
    // expected state changes
    deposit_withdraw,
    chargeback,
    resolve,
    // parsing
    no_headers,
    spaces,
    // edge cases
    // if a deposit is disputed/resolved many times will it work as expected?
    multi_dispute,
    // will a withdrawal not happen if not enough funds
    withdrawal_fail,
    // after a chargeback will a client be frozen (nothing can change its balance anymore)
    freeze_after_chargeback,
    // will many doing deposit_withdraw cause issues?
    many_clients_isolation,
    // are mistakes from the spec ignored?
    partner_mistakes_are_ignored,
    // AI generated
    1,
    2,
    3,
    4
);
