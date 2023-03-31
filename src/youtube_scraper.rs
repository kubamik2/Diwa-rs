use std::time::Duration;
use reqwest::Client;
use scraper::{ html::Html, selector::Selector };
use nom::{ IResult, bytes::complete::take_until };
use crate::MiniMetadata;

fn parse_quotes(input: &str) -> IResult<&str, &str> {
    let res = take_until("\"")(input);
    match res {
        Ok((remainder, _)) => { 
            take_until("\"")(remainder.split_at(1).1)
        },
        e @ Err(_) => e
    }
}

fn parse_bracket_quotes(input: &str) -> IResult<&str, &str> {
    let res = take_until("\"")(input);
    match res {
        Ok((remainder, _)) => { 
            take_until("\"}")(remainder.split_at(1).1)
        },
        e @ Err(_) => e
    }
}

fn parse_colon(input: &str) -> IResult<&str, &str> {
    take_until(":")(input).map(|result| (result.0.split_at(1).1, result.1))
}

fn parse_title(input: &str) -> IResult<&str, String> {
    let result: IResult<&str, &str> = take_until("\"title\":")(input);
    match result {
        Ok((remainder, _)) => {
            match take_until("\"text\":")(remainder) {
                Ok((remainder, _)) => {
                    match parse_colon(remainder) {
                        Ok((remainder, _)) => {
                            parse_bracket_quotes(remainder).map(|(a, b)| (a, b.replace("\\\"", "\"")))
                        },
                        Err(err) => Err(err)
                    }
                },
                Err(err) => Err(err)
            }
        },
        Err(err) => Err(err)
    }
}

fn parse_video_id(input: &str) -> IResult<&str, &str> {
    let result = take_until("\"videoId\":")(input);
    match result {
        Ok((remainder, _)) => {
            match parse_colon(remainder) {
                Ok((remainder, _)) => {
                    parse_quotes(remainder)
                },
                Err(err) => Err(err)
            }
        },
        Err(err) => Err(err)
    }
}

fn parse_duration_text(input: &str) -> IResult<&str, &str> {
    let result = take_until("\"lengthText\":")(input);
    match result {
        Ok((remainder, _)) => {
            match take_until("\"simpleText\":")(remainder) {
                Ok((remainder, _)) => {
                    match parse_colon(remainder) {
                        Ok((remainder, _)) => {
                            parse_quotes(remainder)
                        },
                        Err(err) => Err(err)
                    }
                },
                Err(err) => Err(err)
            }
        },
        Err(err) => Err(err)
    }
}

fn string_to_duration(input: &str) -> Duration {
    let mut time_sections = input.split(":").collect::<Vec<&str>>();
    let mut seconds: u64 = 0;
    let mut multiplier = 1;
    for time_section in time_sections.iter().rev() {
        if let Ok(time_section) = time_section.parse::<u64>() {
            seconds += time_section * multiplier;
        }
        multiplier *= 60;
    }
    
    Duration::from_secs(seconds)
}

pub async fn search(query: &str) -> (Option<String>, Option<String>, Option<Duration>) {
    let formatted_query = query.replace(" ", "+");
    let client = Client::new();
    let response = client.get(format!("https://www.youtube.com/results?search_query={}", formatted_query)).send().await.unwrap().text().await.unwrap();
    let doc = Html::parse_document(&response);
    
    let mut title: Option<String> = None;
    let mut video_id: Option<String> = None;
    let mut duration: Option<Duration> = None;
    for i in doc.select(&Selector::parse("script").unwrap()) {
        let mut html = i.inner_html();
        if html.contains("var ytInitialData = ") { 
            if let Ok((remainder, matched)) = parse_video_id(&html) {
                video_id = Some(matched.to_owned());
                html = remainder.to_owned();
            }
            if let Ok((_, matched)) = parse_title(&html) {
                title = Some(matched.to_owned());
            }
            if let Ok((_, matched)) = parse_duration_text(&html) {
                duration = Some(string_to_duration(matched));
            }
            break;
        }
    }
    (title, video_id, duration)
}