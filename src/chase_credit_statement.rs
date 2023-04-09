use std::path::Path;

use chrono::naive::NaiveDate as Date;
use nom::{
    branch::alt,
    bytes::complete::{tag},
    character::complete::{
        anychar, digit1, multispace0, multispace1, newline, not_line_ending,
    },
    combinator::{cond, peek, recognize, value},
    multi::{many0, many1_count, many_till},
    sequence::{delimited, preceded, separated_pair},
    IResult,
};

use crate::common_parsers::*;
use crate::pdftotext::pdftotext;
use crate::statement_format::StatementFormat;

#[derive(Debug, Copy, Clone)]
pub enum TransactionType {
    Credit,
    Purchase,
    Fee,
}

#[derive(Debug)]
pub struct Transaction {
    type_: TransactionType,
    date: Date,
    description: String,
    amount: i32,
}

#[derive(Debug)]
pub struct ChaseCreditStatement {
    account_number: String,
    start_date: Date,
    end_date: Date,
    start_balance: i32,
    end_balance: i32,
    transactions: Vec<Transaction>,
    total_interest: i32,
}

fn transaction(
    start_date: &Date,
    transaction_type: TransactionType,
) -> impl Fn(&str) -> IResult<&str, Transaction> + '_ {
    move |input| {
        let (input, (month, day)) = preceded(tag("  "), month_day)(input)?;
        let date = infer_year(month, day, *start_date).unwrap();
        let (input, _) = multispace1(input)?;
        let (input, (description_chars, amount)) =
            many_till(anychar, delimited(multispace0, dollar_amount, newline))(input)?;
        let (input, (additional_desc, _)) = many_till(
            delimited(multispace0, not_line_ending, newline),
            peek(alt((
                value((), preceded(tag("  "), month_day)),
                value((), newline),
            ))),
        )(input)?;
        let (input, _) = cond(additional_desc.len() > 0, newline)(input)?;
        let mut description: String = description_chars.into_iter().collect();
        additional_desc.into_iter().for_each(|s| {
            description += "\n";
            description += s
        });
        Ok((
            input,
            Transaction {
                type_: transaction_type,
                date,
                description,
                amount,
            },
        ))
    }
}

fn transaction_section<'a>(
    input: &'a str,
    start_date: &Date,
    section_header: &str,
    transaction_type: TransactionType,
) -> IResult<&'a str, Vec<Transaction>> {
    let (input, ()) = take_until_including(section_header)(input)?;
    let (input, _) = tag("\n\n")(input)?;
    let (input, transactions) = many0(transaction(start_date, transaction_type))(input)?;
    Ok((input, transactions))
}

fn parse_statement(input: &str) -> IResult<&str, ChaseCreditStatement> {
    let (input, ()) = take_until_including("ACCOUNT SUMMARY")(input)?;
    let (input, ()) = take_until_including("Account Number: ")(input)?;
    let (input, account_number) = recognize(many1_count(preceded(multispace0, digit1)))(input)?;
    let (input, _) = take_until_including("Previous Balance")(input)?;
    let (input, start_balance) = preceded(multispace0, dollar_amount)(input)?;
    let (input, ()) = take_until_including("New Balance")(input)?;
    let (input, end_balance) = preceded(multispace0, dollar_amount)(input)?;
    let (input, _) = delimited(multispace0, tag("Opening/Closing Date"), multispace0)(input)?;
    let (input, (start_date, end_date)) =
        separated_pair(month_day_year, tag(" - "), month_day_year)(input)?;
    let (input, ()) = take_until_including("ACCOUNT ACTIVITY")(input)?;

    let (input, mut transactions) = transaction_section(
        input,
        &start_date,
        "PAYMENTS AND OTHER CREDITS",
        TransactionType::Credit,
    )?;

    let (input, purchases) =
        transaction_section(input, &start_date, "PURCHASE", TransactionType::Purchase)?;
    transactions.extend(purchases.into_iter());

    Ok((
        input,
        ChaseCreditStatement {
            account_number: account_number.into(),
            start_date,
            end_date,
            start_balance,
            end_balance,
            transactions,
            // TODO: Find total interest
            total_interest: 0,
        },
    ))
}

impl StatementFormat for ChaseCreditStatement {
    fn parse_file(path: &Path) -> Self {
        let pdf_text = pdftotext(&path, true).unwrap();
        println!("{}", pdf_text);
        let (_, statement) = parse_statement(pdf_text.as_str()).unwrap();
        statement
    }
}
