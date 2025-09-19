use serde::Serialize;

/// Writes a csv to the writer (W)
///
/// # Errors
///
/// Failed to write the csv
pub fn write_csv_client_balance_sheet<'a, Input, W, S>(
    sheet: Input,
    w: W,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: Serialize + 'a,
    Input: Iterator<Item = &'a S>,
    W: std::io::Write,
{
    let mut wtr = csv::Writer::from_writer(w);

    for balance in sheet {
        wtr.serialize(balance)?;
    }
    wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{common::zz_amount::ZzIAmount, domain::client_balance::ZzClientBalance};

    fn make_iamount(val: i64) -> ZzIAmount {
        ZzIAmount::new(val.into(), 0).unwrap()
    }

    use super::*;

    #[test]
    fn test_write_csv_client_balance_sheet() {
        let balances = [
            ZzClientBalance {
                client_id: 1,
                available: make_iamount(100),
                held: make_iamount(50),
                total: make_iamount(150),
                locked: false,
            },
            ZzClientBalance {
                client_id: 2,
                available: make_iamount(200),
                held: make_iamount(0),
                total: make_iamount(200),
                locked: true,
            },
        ];

        let mut output = Vec::new();
        write_csv_client_balance_sheet(balances.iter(), &mut output).unwrap();

        let csv_str = String::from_utf8(output).unwrap();
        let expected_lines: Vec<&str> = vec![
            "client,available,held,total,locked",
            "1,100,50,150,false",
            "2,200,0,200,true",
        ];

        for line in expected_lines {
            assert!(csv_str.contains(line));
        }
    }
}
