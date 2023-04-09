use chrono::{naive::NaiveDate as Date, Datelike, Month};
use nom::{
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, char, digit1, i32, multispace0, multispace1, u32},
    combinator::{map_opt, map_res, opt},
    multi::separated_list0,
    sequence::{delimited, preceded, separated_pair, tuple},
    IResult,
};

pub fn month_word(input: &str) -> IResult<&str, Month> {
    map_res(alpha1, |x: &str| x.parse::<Month>())(input)
}

pub fn month_word_day(input: &str) -> IResult<&str, (Month, u32)> {
    separated_pair(month_word, multispace1, u32)(input)
}

pub fn month_day(input: &str) -> IResult<&str, (u32, u32)> {
    separated_pair(u32, char('/'), u32)(input)
}

pub fn month_day_year(input: &str) -> IResult<&str, Date> {
    let (input, (month, _, day, _, year)) = tuple((u32, char('/'), u32, char('/'), i32))(input)?;
    Ok((input, Date::from_ymd_opt(2000 + year, month, day).unwrap()))
}

pub fn infer_year(month: u32, day: u32, start_date: Date) -> Option<Date> {
    let year = if month < start_date.month() {
        start_date.year() + 1
    } else {
        start_date.year()
    };
    Date::from_ymd_opt(year, month, day)
}

pub fn month_word_day_year(input: &str) -> IResult<&str, Date> {
    let (input, (month, day)) = month_word_day(input)?;
    let (input, _) = delimited(multispace0, opt(char(',')), multispace0)(input)?;
    map_opt(i32, move |year| {
        Date::from_ymd_opt(year, month.number_from_month(), day)
    })(input)
}

pub fn dollar_amount(input: &str) -> IResult<&str, i32> {
    let (input, negate) = opt(char('-'))(input)?;
    let (input, _) = opt(char('+'))(input)?;
    let (input, _) = opt(char('$'))(input)?;
    let (input, dollars_strs) = separated_list0(char(','), digit1)(input)?;
    let (input, cents_str) = preceded(char('.'), digit1)(input)?;
    let cents = cents_str.parse::<i32>().unwrap();
    let dollars = if dollars_strs.len() > 0 {
        (dollars_strs.into_iter().collect::<String>())
            .parse::<i32>()
            .unwrap()
    } else {
        0
    };
    let abs_amount = dollars * 100 + cents;
    let amount = if negate.is_some() {
        -abs_amount
    } else {
        abs_amount
    };
    Ok((input, amount))
}

pub fn take_until_including(t: &str) -> impl Fn(&str) -> IResult<&str, ()> + '_ {
    move |input| {
        let (input, _) = take_until(t)(input)?;
        let (input, _) = tag(t)(input)?;
        Ok((input, ()))
    }
}
