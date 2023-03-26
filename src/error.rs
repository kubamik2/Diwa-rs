use std::fmt::Display;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
pub enum LibError {
    MissingValue {value: String, line: u32, file: String},
    UrlError {description: String, line: u32, file: String}
}

impl Display for LibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return match *self {
            Self::MissingValue {ref value, ref line, ref file} => {
                write!(f, "{}:{} Missing Value: `{}`", file, line, value)
            },
            Self::UrlError { ref description, ref line, ref file } => {
                write!(f, "{}:{} Url Error: `{}`", file, line, description)
            }
        };
    }
}

impl std::error::Error for LibError {
    fn description(&self) -> &str {
        return match *self {
            Self::MissingValue {ref value, ref line, ref file} => {
                value
            },
            Self::UrlError { ref description, ref line, ref file } => {
                description
            }
        };
    }
}

#[macro_export] 
macro_rules! missing_value {
    ($x: expr) => {
        crate::error::LibError::MissingValue {value: $x.to_owned(), line: line!(), file: file!().into()}
    }
}

#[macro_export] 
macro_rules! url_error {
    ($x: expr) => {
        crate::error::LibError::UrlError {description: $x.to_owned(), line: line!(), file: file!().into()}
    }
}

// TODO: Complete this error
#[derive(Debug)]
pub enum VoiceError {
    DifferentChannel {line: u32, file: String}
}

impl Display for VoiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        return match *self {
            Self::DifferentChannel {ref line, ref file} => write!(f, "{}:{} Different channel", file, line)
        }
    }
}

impl std::error::Error for VoiceError {
    fn description(&self) -> &str {
        return match *self {
            Self::DifferentChannel {line: _, file: _} => "Different channel"
        }
    }
}

#[macro_export] 
macro_rules! different_channel {
    () => {
        crate::error::VoiceError::DifferentChannel {line: line!(), file: file!().into()}
    }
}