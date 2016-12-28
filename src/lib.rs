#[derive(Debug, PartialEq)]
pub enum Bit {
    Zero,
    One,
}

impl Bit {
    pub fn to_u64(self) -> u64 {
        match self {
            Bit::Zero => 0,
            Bit::One => 1,
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct DataPoint {
    time: u64, // time
    value: f64, // value
}

impl DataPoint {
    pub fn new(time: u64, value: f64) -> Self {
        DataPoint {
            time: time,
            value: value,
        }
    }
}

pub mod stream;

pub mod encode;
pub use self::encode::Encode;
pub use self::encode::std_encoder::StdEncoder;

pub mod decode;
pub use self::decode::Decode;
pub use self::decode::std_decoder::StdDecoder;