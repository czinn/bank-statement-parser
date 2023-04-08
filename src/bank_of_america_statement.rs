use std::path::Path;

use chrono::{naive::NaiveDate as Date, Datelike, Month};
use pdf_extract::extract_text;

use nom::{
    bytes::complete::{is_a, tag, take_until},
    character::complete::{
        alpha1, anychar, char, digit1, i32, multispace0, multispace1, u32,
    },
    combinator::{map, map_opt, map_res, opt, peek},
    multi::{many1, many_till, separated_list1},
    sequence::{delimited, preceded, separated_pair, terminated},
    error::{Error, ErrorKind},
    IResult,
};

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
    posting_date: Date,
    description: String,
    reference_number: String,
    account_number: String,
    amount: i32,
}

#[derive(Debug)]
pub struct BankOfAmericaStatement {
    account_number: String,
    start_date: Date,
    end_date: Date,
    transactions: Vec<Transaction>,
    total_interest: i32,
}

fn account_number(input: &str) -> IResult<&str, String> {
    map(is_a("0123456789 "), |x: &str| x.to_string())(input)
}

fn month_word(input: &str) -> IResult<&str, Month> {
    map_res(alpha1, |x: &str| x.parse::<Month>())(input)
}

fn month_word_day(input: &str) -> IResult<&str, (Month, u32)> {
    separated_pair(month_word, multispace1, u32)(input)
}

fn month_day(input: &str) -> IResult<&str, (u32, u32)> {
    separated_pair(u32, char('/'), u32)(input)
}

fn infer_year(month: u32, day: u32, start_date: Date) -> Option<Date> {
    let year = if month < start_date.month() {
        start_date.year() + 1
    } else {
        start_date.year()
    };
    Date::from_ymd_opt(year, month, day)
}

fn dollar_amount(input: &str) -> IResult<&str, i32> {
    let (input, negate) = opt(char('-'))(input)?;
    let (input, _) = opt(char('$'))(input)?;
    let (input, dollars_strs) = separated_list1(char(','), digit1)(input)?;
    let (input, cents_str) = preceded(char('.'), digit1)(input)?;
    let cents = cents_str.parse::<i32>().unwrap();
    let dollars = (dollars_strs.into_iter().collect::<String>())
        .parse::<i32>()
        .unwrap();
    let abs_amount = dollars * 100 + cents;
    let amount = if negate.is_some() {
        -abs_amount
    } else {
        abs_amount
    };
    Ok((input, amount))
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

fn take_until_including(t: &str) -> impl Fn(&str) -> IResult<&str, ()> + '_ {
    move |input| {
        let (input, _) = take_until(t)(input)?;
        let (input, _) = tag(t)(input)?;
        Ok((input, ()))
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
        terminated(
            take_until_including("FOR THIS PERIOD"),
            multispace1,
        ),
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

fn parse_statement(input: &str) -> IResult<&str, BankOfAmericaStatement> {
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

    Ok((
        input,
        BankOfAmericaStatement {
            account_number,
            start_date,
            end_date,
            transactions,
            total_interest,
        },
    ))
}

impl StatementFormat for BankOfAmericaStatement {
    fn parse_file(path: &Path) -> Self {
        let pdf_text = extract_text(&path).unwrap();
        println!("{}", pdf_text);
        let (_, statement) = parse_statement(pdf_text.as_str()).unwrap();
        statement
    }
}
