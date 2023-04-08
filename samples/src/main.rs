use std::path::Path;

use clap::Parser;

use bank_statement_parser::bank_of_america_statement::BankOfAmericaStatement;
use bank_statement_parser::statement_format::StatementFormat;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(short, long)]
    filename: String,
}

fn main() {
    let args = Args::parse();
    let statement = BankOfAmericaStatement::parse_file(&Path::new(&args.filename));
    println!("{:?}", statement);
}
