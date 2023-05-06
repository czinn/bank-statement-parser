use std::path::Path;

use chrono::naive::NaiveDate as Date;
use nom::{
    bytes::complete::{is_a, tag},
    character::complete::{anychar, digit1, i32, multispace0, multispace1},
    combinator::{map, map_opt, opt, peek},
    error::{Error, ErrorKind},
    multi::{many1, many_till},
    sequence::{delimited, preceded, separated_pair, terminated},
    IResult,
};
use pdf_extract::extract_text;

use crate::common_parsers::*;
use crate::statement_format::StatementFormat;

#[derive(Debug, Copy, Clone)]
pub enum TransactionType {
    Credit,
    Purchase,
    Fee,
}

#[derive(Debug)]
pub struct Transaction {
    pub type_: TransactionType,
    pub date: Date,
    pub posting_date: Date,
    pub description: String,
    pub reference_number: String,
    pub account_number: String,
    pub amount: i32,
}

#[derive(Debug)]
pub struct BankOfAmericaCreditStatement {
    pub account_number: String,
    pub start_date: Date,
    pub end_date: Date,
    pub start_balance: i32,
    pub end_balance: i32,
    pub transactions: Vec<Transaction>,
    pub total_interest: i32,
}

fn account_number(input: &str) -> IResult<&str, String> {
    map(is_a("0123456789 "), |x: &str| x.to_string())(input)
}

fn transaction(
    start_date: Date,
    account_number: &str,
    transaction_type: TransactionType,
) -> impl Fn(&str) -> IResult<&str, Transaction> + '_ {
    move |input| {
        let (input, date) =
            map_opt(month_day, |(month, day)| infer_year(month, day, start_date))(input)?;
        let (input, _) = multispace1(input)?;
        let (input, posting_date) =
            map_opt(month_day, |(month, day)| infer_year(month, day, start_date))(input)?;
        let (input, _) = multispace1(input)?;
        let (input, (description_chars, (reference_number, account_number))) = many_till(
            anychar,
            preceded(
                multispace1,
                separated_pair(digit1, multispace1, tag(account_number)),
            ),
        )(input)?;
        let (input, amount) = preceded(multispace1, dollar_amount)(input)?;
        let (input, _) = tag("\n\n")(input)?;
        Ok((
            input,
            Transaction {
                type_: transaction_type,
                date,
                posting_date,
                description: description_chars.into_iter().collect(),
                reference_number: reference_number.into(),
                account_number: account_number.into(),
                amount,
            },
        ))
    }
}

fn transaction_section<'a>(
    input: &'a str,
    start_date: Date,
    account_number: &str,
    section_header: &str,
    transaction_type: TransactionType,
) -> IResult<&'a str, Vec<Transaction>> {
    let (input, ()) = take_until_including(section_header)(input)?;
    let (input, transactions) =
        many1(transaction(start_date, account_number, transaction_type))(input)?;
    let (input, total) = preceded(
        terminated(take_until_including("FOR THIS PERIOD"), multispace1),
        dollar_amount,
    )(input)?;
    let (input, _) = tag("\n\n")(input)?;
    // Check the total
    let computed_total: i32 = transactions.iter().map(|t| t.amount).sum();
    if computed_total != total {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Verify)));
    }
    Ok((input, transactions))
}

fn parse_statement(input: &str) -> IResult<&str, BankOfAmericaCreditStatement> {
    let (input, ()) = take_until_including("Account# ")(input)?;
    let (input, account_number) = account_number(input)?;
    let (input, _) = multispace0(input)?;
    let (input, ((start_month, start_day), (end_month, end_day))) =
        separated_pair(month_word_day, tag(" - "), month_word_day)(input)?;
    let (input, end_year) = delimited(tag(", "), i32, multispace0)(input)?;

    let start_year = if start_month.number_from_month() > end_month.number_from_month() {
        end_year - 1
    } else {
        end_year
    };
    let start_date =
        Date::from_ymd_opt(start_year, start_month.number_from_month(), start_day).unwrap();
    let end_date = Date::from_ymd_opt(end_year, end_month.number_from_month(), end_day).unwrap();

    let (input, ()) = take_until_including("Previous Balance ")(input)?;
    let (input, start_balance) = dollar_amount(input)?;
    let (input, ()) = take_until_including("New Balance Total ")(input)?;
    let (input, end_balance) = dollar_amount(input)?;

    let (input, mut transactions) = transaction_section(
        input,
        start_date,
        &account_number[account_number.len() - 4..],
        "Payments and Other Credits\n\n",
        TransactionType::Credit,
    )?;

    let (input, purchases) = transaction_section(
        input,
        start_date,
        &account_number[account_number.len() - 4..],
        "Purchases and Adjustments\n\n",
        TransactionType::Purchase,
    )?;
    transactions.extend(purchases.into_iter());

    let (input, fees_present) = peek(opt(tag("Fees\n\n")))(input)?;
    let (input, fees) = if fees_present.is_some() {
        transaction_section(
            input,
            start_date,
            &account_number[account_number.len() - 4..],
            "Fees\n\n",
            TransactionType::Fee,
        )?
    } else {
        (input, Vec::new())
    };
    transactions.extend(fees.into_iter());

    let (input, total_interest) = preceded(
        terminated(
            take_until_including("TOTAL INTEREST CHARGED FOR THIS PERIOD"),
            multispace1,
        ),
        dollar_amount,
    )(input)?;

    let computed_total = transactions.iter().map(|t| t.amount).sum::<i32>() + total_interest;
    if end_balance - start_balance != computed_total {
        return Err(nom::Err::Error(Error::new(input, ErrorKind::Verify)));
    }

    Ok((
        input,
        BankOfAmericaCreditStatement {
            account_number,
            start_date,
            end_date,
            start_balance,
            end_balance,
            transactions,
            total_interest,
        },
    ))
}

impl StatementFormat for BankOfAmericaCreditStatement {
    fn parse_file(path: &Path) -> Self {
        let pdf_text = extract_text(&path).unwrap();
        let (_, statement) = parse_statement(pdf_text.as_str()).unwrap();
        statement
    }
}
