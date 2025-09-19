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

#[test]
fn test_simple_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/simple").unwrap())
}

#[test]
fn test_no_headers_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/no_headers").unwrap())
}

#[test]
fn test_spaces_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/spaces").unwrap())
}

#[test]
fn test_chargeback_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/chargeback").unwrap())
}

#[test]
fn test_1_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/1").unwrap())
}

#[test]
fn test_2_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/2").unwrap())
}

#[test]
fn test_3_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/3").unwrap())
}

#[test]
fn test_4_case() {
    run_test_case(&PathBuf::from_str("tests/test_cases/4").unwrap())
}
