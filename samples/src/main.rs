use std::path::Path;

use clap::{Parser, ValueEnum};
use pdf_extract::extract_text;

use bank_statement_parser::bank_of_america_credit_statement::BankOfAmericaCreditStatement;
use bank_statement_parser::bank_of_america_debit_statement::BankOfAmericaDebitStatement;
use bank_statement_parser::chase_credit_statement::ChaseCreditStatement;
use bank_statement_parser::statement_format::StatementFormat;

#[derive(ValueEnum, Debug, Clone, Copy)]
enum StatementType {
    BoaCredit,
    BoaDebit,
    ChaseCredit,
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    filename: String,
    #[arg(value_enum, short)]
    type_: StatementType,
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();
    let path = Path::new(&args.filename);
    if args.verbose {
        let pdf_text = extract_text(&path).unwrap();
        println!("{}", pdf_text);
    }

    match args.type_ {
        StatementType::BoaCredit => {
            let statement = BankOfAmericaCreditStatement::parse_file(&path);
            println!("{:?}", statement);
        },
        StatementType::BoaDebit => {
            let statement = BankOfAmericaDebitStatement::parse_file(&path);
            println!("{:?}", statement);
        },
        StatementType::ChaseCredit => {
            let statement = ChaseCreditStatement::parse_file(&path);
            println!("{:?}", statement);
        },
    }
}
