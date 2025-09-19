# zzzzzzzzzzz

A project about parsing csv's and handling disputes...

All quotes to the spec will be wrapped like < quote >

## Objective

Read csv file with information about client transactions and calculate the final balance of the clients. Each row of the input csv represents a transaction where the index maps to a monotonically increasing unit of time.

obs: ordering here is of great importance since event processing depends on the current balance of a client (which also depends on events)

## Project structure

- `src/domains` the code that defines the business logic + some code that is "close" to similar structs
- `src/parsers/csv_parser.rs` the file that defines the io_loop and calls domain/parsers (this is what would normally be called the handler)
- `src/parsers` code about parsing the csv files
- `src/common` code that define more core logic (that don't pertain to a specific domain) but do follow rules of the spec (precision of up to 4 decimal digits)
- `tests/test_cases.rs` integration test that test against csv files in `tests/test_cases`
- `src/lib.rs` configuration declaration and a straightforward implementation of the final effect of the program (output to stdout)

### Naming

As a matter of taste, I usually name the public structs and functions in an identifiable way for the crate. Because this is the zzzzzzzzzzz crate, some types have Zz in them

### Testing

I use a lot of AI to help create unit tests and do static assertions. So expect some repeated code in those tests.

For integration test, I took care to create test cases that actually touch the edge cases. The `tests/test_cases.rs` has some comments explaining each test

## IO

The input is the path to a csv file and the output is the contents of a csv file.

### Input shape

```rust
/// Fields are ordered the same as the csv cols
struct Input {
    r#type: TxType,
    // renames to client
    client_id: u16,
    // renames to tx
    tx_id: u32,
    /// ZzAmount is a special struct that brings precision to the extent that
    /// is necessary
    amount: Option<ZzAmount>,
}
```

obs: this is not the input struct

### Output shape

```rust
/// Fields are ordered the same as the csv cols
struct Output {
    // renames to client
    client_id: u16,
    available: ZzAmount,
    held: ZzAmount,
    total: ZzAmount,
    locked: bool,
}
```

### ZzAmount

From <You can assume a precision of four places past the decimal and should output values with the same level of precision.> and because all operations to ZzAmount are addition or subtraction, we'll handle precision when parsing and drop all numbers after the 4th decimal (always round down).

### Transaction types

- Deposit: increases client's amount (amount is required)
- Withdraw: decreases client's amount (amount is required)
- Dispute: puts the funds of a deposit's amount into a held fund (amount is optional)
- Resolve: puts the funds of a disputed deposit's amount back to available fund (amount is optional)
- Chargeback: removes disputed deposit from held fund (amount is optional)

### State transitions

| State    | Deposit        | Withdrawal (sufficient) | Withdrawal (insufficient) | Dispute                                     | Resolve                    | Chargeback                     |
| -------- | -------------- | ----------------------- | ------------------------- | ------------------------------------------- | -------------------------- | ------------------------------ |
| Normal   | +available     | -available              | no change                 | -available, +held → Disputed                | n/a                        | n/a                            |
| Disputed | +available     | -available              | no change                 | -available, +held (or no change if invalid) | +available, -held → Normal | -held, account locked → Locked |
| Locked   | account locked | account locked          | account locked            | account locked                              | account locked             | account locked                 |


## Assumptions

From the overall spec, we'll assume that errors pertaining to the system (bad tx_id, for exmaple) will be ignored. But serialization will by default fail (no comments over serialization handling in the spec, I also added some configuration over this behavior). This default assumption maintains the behavior like <If the tx specified by the dispute doesn't exist you can ignore it> and <If the tx ... isn't under dispute you can ignore the resolve>.

From the description of dispute: <The transaction shouldn't be reversed yet but the associated funds should be held. This means that the clients available funds should DECREASE by the amount disputed, their held funds should INCREASE by the amount disputed, while their total funds should remain the same.> the words increase and decrease are used to reference the available fund and the way they are used represent a reversed Deposit.

From the Overview section and how partner error is mentioned only for Dispute, Resolve, and Chargeback. Deposit and Withdraw will be considered as an "internal" API. I'm arguing this because a partner wouldn't have the transaction id for a deposit before it happened, so this will also be considered partner error.

1. If a transaction exists but the client's id is wrong in the event, it'll be ignored (keeping consistent that bad partner requests are ignored)
2. Serialization will be strict (can't have excessive fields or missing requried), it'll fail the program
3. If a dispute references a withdrawal, the dispute will be ignored
4. If a dispute references a transaction that happened later, the dispute will be ignored
5. A client's amount may be negative (deposit -> withdraw -> dispute -> chargeback)
6. Because withdrawals and deposits are considered internal, I suppose they respect id uniquess
7. If a dispute is resolved, it may be disputed again

## Design decisions

### Parsing

It was recommended the use of csv + serde. While that combination is fine, I found it hard to use it when handling segmentation between read() calls. I decided to use nom after facing issues with segmentation. I did attempt to later create a trait and allow the user to pick either nom or serde but I couldn't make the serde impl work :/

### Error handling

I did a poor job here. For now it's mostly just panics and the program lacks observability (a lot of places that IMO should generate a backtrace won't because they don't panic immediately). I started with creating error enums, adding backtraces etc. but overall mapping all errors was making me not converge.

### Efficiency

Because we're dealing with files (we know the size of the file beforehand and can read any section of it) we could optimize the workflow by reading the file in parellel, this is a first implementation so I decided on just using a big buffer and `std::fs::FileExt::read_at()`

If we were dealing with concurrent TCP streams, the requirements would change because we'd need to define how the events are oredered now. Because you can parse a csv like a stream divided per lines, I can at least say that waiting for the complete file is waste of compute (worker will idle when it could've already parsed and processed parts of the csv

### Crates

1. nom ("Parsing" section explains)
2. clap: even though we have only 1 input in he spec, I'll use clap for extensibility and to allow fast edge case customization (ignoring vs. failing parsing)
3. serde: for the output
4. csv: for the output
5. num-bigint: for implementation of ZzAmount (allows for arbitrary big integers)
6. strum and serde_plain: help DRY some code
7. fake: simple fuzz tests

## AI usage

Here's the list of prompts and a summary of conclusion of each prompt

1. Fastest way to read a file in Linux, considering the data of the file needs to be read serially. This is a big file containing chronological events. I want to know if there's any modern tech to do this
    - Conclusion: read() or io_uring
2. It's a program that will process some data and output it to std and I imagine syscall context switching will be the bottleneck here because the processing itself is just summarizing data
    - Conclusion: io_uring might be overkill (yeah I don't want to set that up right now), read() with a big buffer should be a cleaner option
3. From the PDF, create a mermaid graph that describes the possible states of a client's bank and the effects of each transaction type. You may consider the states of the client as: Normal, Disputed, Locked. And the transitions are the transaction types with variations depending if have sufficient funds etc. I expect each transaction arrow to have the following content: <TxType>, [<+/-/account locked> <held>/<available>] Where TxType are one of the 5 types of transactions The array/account locked will be one of these: [+available] [-available] [+available, -held] [-available, +held] [-held, account locked] All possible transactions should be mapped so Normal must have withdraw, deposit, dispute Disputed must have all Locked must have Nothing
    - Conclusion: I failed to make the mermaid graph and ended up creating a table
4. From the same PDF, WDYT about these assumptions? (pasted the 5 first assumption list without the explanation before)
    - Conclusion: It questioned about assumption 5
5. But about 5, the overview talks about the very case of people doing chargeback to scam. So I think having negative balance is the logical step
    - Conclusion: looks good
6. Did you at any point in the assumptions conversation try to appease me? If so, please be blunt about my assumptions
    - Conclusion: assumption 2 might be bad because the spec doesn't talk about serialization errors specifically. As I mentioned in my reasoning about assumptions, I'll keep this one but with ChatGPT also raising it, I'll make it configurable so it'll be ignored if necessary
7. Asked ChatGPT to write nom parser for ZzAmount (and pasted zz_amount module)
    - Conclusion: it wrote a parser that didn't compile but had the correct intention, after fixing it I asked next prompt
8. Write tests for the ZzAmount implementations
    - Conclusion: good tests!
9. Asked about assumptions 6. 7. (I added them later)
    - Conclusion: looks good
10. Write unit tests for the transaction file functions
    - Conclusion: looks good
11. Now also add tests for the add and sub functions, testing all cases (considering the carry case for add and decimal smaller for sub)
    - Conclusion: looks good
12. Implement parse_zztx
    - Conclusion: minimal patching, looks good 
13. Now create tests like this for parse_zztx: Happy path, Incomplete, Fuzz test using fake (you can add a serialization implementation for the struct) Different spaces
    - Conclusion: I had to implement the serialization and rewrite the fuzz test and check if Incomplete returns incomplete error kind instead of just error
14. I patched the parse_zztx_csv function making it now consider that the caller always sends a &str terminated by eof. This makes me seek twice the string but saves a lot of complexity from the problem. Please rewrite the test adding edge cases that are relevant now (test if CsvParserControl is respected based on ZzParseOptions)
    - Conclusion: looks good
15. Also add tests omitting different fields and checking if it'll skip on missing fields
    - Conclusion: looks good
15. Given the spec and my assumptions, create test cases
    - Conclusion: added them as 1, 2 ...
