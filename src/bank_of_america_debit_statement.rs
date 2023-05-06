use std::path::Path;

use chrono::naive::NaiveDate as Date;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{anychar, digit1, multispace0, multispace1},
    combinator::{opt, peek, recognize},
    error::{Error, ErrorKind},
    multi::{many0, many1_count, many_till},
    sequence::{delimited, preceded},
    IResult,
};
use pdf_extract::extract_text;

use crate::common_parsers::*;
use crate::statement_format::StatementFormat;

#[derive(Debug, Copy, Clone)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Fee,
}

#[derive(Debug)]
pub struct Transaction {
    pub type_: TransactionType,
    pub date: Date,
    pub description: String,
    pub amount: i32,
}

#[derive(Debug)]
pub struct BankOfAmericaDebitStatement {
    pub account_number: String,
    pub start_date: Date,
    pub end_date: Date,
    pub start_balance: i32,
    pub end_balance: i32,
    pub transactions: Vec<Transaction>,
}

fn dollar_amount_and_date_or_footer_follows(
    section_footer: &str,
) -> impl Fn(&str) -> IResult<&str, i32> + '_ {
    move |input| {
        let (input, amount) = preceded(multispace0, dollar_amount)(input)?;
        let (input, _) = peek(preceded(
            multispace0,
            alt((recognize(month_day_year), tag(section_footer))),
        ))(input)?;
        Ok((input, amount))
    }
}

fn transaction(
    section_footer: &str,
    transaction_type: TransactionType,
) -> impl Fn(&str) -> IResult<&str, Transaction> + '_ {
    move |input| {
        let (input, date) = month_day_year(input)?;
        let (input, _) = multispace1(input)?;
        let (input, (description_chars, amount)) = many_till(
            anychar,
            dollar_amount_and_date_or_footer_follows(section_footer),
        )(input)?;
        let (input, _) = multispace1(input)?;
        Ok((
            input,
            Transaction {
                type_: transaction_type,
                date,
                description: description_chars.into_iter().collect(),
                amount,
            },
        ))
    }
}

fn transaction_section<'a>(
    input: &'a str,
    section_header: &str,
    section_footer: &str,
    transaction_type: TransactionType,
) -> IResult<&'a str, Vec<Transaction>> {
    let (input, ()) = take_until_including(section_header)(input)?;
    let (input, _) = delimited(
        multispace0,
        alt((
            tag("Date Description Amount"),
            tag("Date Transaction description Amount"),
        )),
        multispace0,
    )(input)?;
    let (input, transactions) = many0(transaction(section_footer, transaction_type))(input)?;
    let (input, _) = tag(section_footer)(input)?;
    let (input, total) = preceded(multispace1, dollar_amount)(input)?;
    // Check the total
    let computed_total: i32 = transactions.iter().map(|t| t.amount).sum();
    if computed_total != total {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Verify)));
    }
    Ok((input, transactions))
}

fn parse_statement(input: &str) -> IResult<&str, BankOfAmericaDebitStatement> {
    let (input, ()) = take_until_including("Account number:")(input)?;
    let (input, account_number) = recognize(many1_count(preceded(multispace0, digit1)))(input)?;

    let (input, ()) = take_until_including("Beginning balance on ")(input)?;
    let (input, start_date) = month_word_day_year(input)?;
    let (input, start_balance) = preceded(multispace0, dollar_amount)(input)?;

    let (input, ()) = take_until_including("Ending balance on ")(input)?;
    let (input, end_date) = month_word_day_year(input)?;
    let (input, end_balance) = preceded(multispace0, dollar_amount)(input)?;

    let (input, mut transactions) = transaction_section(
        input,
        "Deposits and other additions",
        "Total deposits and other additions",
        TransactionType::Deposit,
    )?;

    let (input, withdrawals) = transaction_section(
        input,
        "Withdrawals and other subtractions",
        "Total withdrawals and other subtractions",
        TransactionType::Withdrawal,
    )?;
    transactions.extend(withdrawals.into_iter());

    let (input, fees_present) = peek(opt(take_until("Service fees")))(input)?;
    let (input, fees) = if fees_present.is_some() {
        transaction_section(
            input,
            "Service fees",
            "Total service fees",
            TransactionType::Withdrawal,
        )?
    } else {
        (input, Vec::new())
    };
    transactions.extend(fees.into_iter());

    let computed_total: i32 = transactions.iter().map(|t| t.amount).sum();
    if end_balance - start_balance != computed_total {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Verify)));
    }

    Ok((
        input,
        BankOfAmericaDebitStatement {
            account_number: account_number.into(),
            start_date,
            end_date,
            start_balance,
            end_balance,
            transactions,
        },
    ))
}

impl StatementFormat for BankOfAmericaDebitStatement {
    fn parse_file(path: &Path) -> Self {
        let pdf_text = extract_text(&path).unwrap();
        let (_, statement) = parse_statement(pdf_text.as_str()).unwrap();
        statement
    }
}
