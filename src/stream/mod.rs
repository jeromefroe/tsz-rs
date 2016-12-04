use std::{error, fmt};

use Bit;

#[derive(Debug, PartialEq)]
pub enum Error {
    EOF,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::EOF => write!(f, "Encountered the end of the stream"),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::EOF => "Encountered the end of the stream",
        }
    }
}

pub trait Read {
    fn read_bit(&mut self) -> Result<Bit, Error>;
    fn read_byte(&mut self) -> Result<u8, Error>;
    fn read_bits(&mut self, mut num_bits: u32) -> Result<u64, Error>;

    fn peak_bits(&mut self, num_bits: u32) -> Result<u64, Error>;
}

pub trait Write {
    fn write_bit(&mut self, bit: Bit);
    fn write_byte(&mut self, byte: u8);
    fn write_bits(&mut self, num_bits: u64, num_bits: u32);

    fn close(self) -> Box<[u8]>;
}

pub mod buffered_write;
pub use self::buffered_write::BufferedWriter;

pub mod buffered_read;
pub use self::buffered_read::BufferedReader;