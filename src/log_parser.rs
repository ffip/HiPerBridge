use nom::{bytes::complete::*, *};
fn parse_time(input: &str) -> IResult<&str, ()> {
    let (input, _year) = take_while1(char::is_dec_digit)(input)?;
    let (input, _) = tag("-")(input)?;
    let (input, _mouth) = take_while1(char::is_dec_digit)(input)?;
    let (input, _) = tag("-")(input)?;
    let (input, _day) = take_while1(char::is_dec_digit)(input)?;
    let (input, _) = tag(" ")(input)?;
    let (input, _hour) = take_while1(char::is_dec_digit)(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, _minture) = take_while1(char::is_dec_digit)(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, _second) = take_while1(char::is_dec_digit)(input)?;
    Ok((input, ()))
}

fn parse_ipv4(input: &str) -> IResult<&str, std::net::Ipv4Addr> {
    let (input, a) = take_while1(char::is_dec_digit)(input)?;
    let a = a.parse().unwrap();
    let (input, _) = tag(".")(input)?;
    let (input, b) = take_while1(char::is_dec_digit)(input)?;
    let b = b.parse().unwrap();
    let (input, _) = tag(".")(input)?;
    let (input, c) = take_while1(char::is_dec_digit)(input)?;
    let c = c.parse().unwrap();
    let (input, _) = tag(".")(input)?;
    let (input, d) = take_while1(char::is_dec_digit)(input)?;
    let d = d.parse().unwrap();
    Ok((input, std::net::Ipv4Addr::new(a, b, c, d)))
}

pub fn parse_log_line(input: &str) -> IResult<&str, &str> {
    let (input, _time) = parse_time(input)?;
    let (input, _) = tag(" ")(input)?;
    let (input, log_type) = take_while1(|x: char| !x.is_whitespace())(input)?;
    let (input, _) = tag(" ")(input)?;
    Ok((input, log_type))
}

fn parse_ipv4_line(input: &str) -> IResult<&str, std::net::Ipv4Addr> {
    let (input, _log_type) = parse_log_line(input)?;
    let (input, _) = tag("ipv4[")(input)?;
    let (input, ipv4) = parse_ipv4(input)?;
    let (input, _) = tag("]")(input)?;
    Ok((input, ipv4))
}

pub fn try_get_ipv4(line: &str) -> Option<std::net::Ipv4Addr> {
    parse_ipv4_line(line).map(|x| x.1).ok()
}
