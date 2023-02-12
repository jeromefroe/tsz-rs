use std::fmt;
use stream;
use DataPoint;

/// Error
///
/// Error encapsulates the potential errors that can be encountered when decoding data
#[derive(Debug, PartialEq)]
pub enum Error {
    Stream(stream::Error),
    InvalidInitialTimestamp,
    InvalidEndOfStream,
    EndOfStream,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Stream(ref err) => write!(f, "Stream error: {}", err),
            Error::InvalidInitialTimestamp => write!(f, "Failed to parse intitial timestamp"),
            Error::InvalidEndOfStream => write!(f, "Encountered invalid end of steam marker"),
            Error::EndOfStream => write!(f, "Encountered end of the stream"),
        }
    }
}

impl From<stream::Error> for Error {
    fn from(err: stream::Error) -> Error {
        Error::Stream(err)
    }
}

/// Decode
///
/// Decode is the trait used to encapsulate decoding `DataPoint`s
pub trait Decode {
    fn next(&mut self) -> Result<DataPoint, Error>;
}

pub mod std_decoder;
