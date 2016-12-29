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

#[derive(Debug, PartialEq, Copy)]
pub struct DataPoint {
    time: u64, // time
    value: f64, // value
}

impl Clone for DataPoint {
    fn clone(&self) -> DataPoint {
        *self
    }
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

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::{DataPoint, Encode, Decode, StdEncoder, StdDecoder};
    use super::stream::{BufferedReader, BufferedWriter};
    use super::decode::Error;

    const DATA: &'static str = "1482892270,1.76
1482892280,7.78
1482892288,7.95
1482892292,5.53
1482892310,4.41
1482892323,5.30
1482892334,5.30
1482892341,2.92
1482892350,0.73
1482892360,-1.33
1482892370,-1.78
1482892390,-12.45
1482892401,-34.76
1482892490,78.9
1482892500,335.67
1482892800,12908.12
";

    #[test]
    fn integration_test() {
        let w = BufferedWriter::new();
        let mut encoder = StdEncoder::new(1482892260, w);

        let mut original_datapoints = Vec::new();

        for line in DATA.lines() {
            let substrings: Vec<&str> = line.split(",").collect();
            let t = substrings[0].parse::<u64>().unwrap();
            let v = substrings[1].parse::<f64>().unwrap();
            let dp = DataPoint::new(t, v);
            original_datapoints.push(dp);
        }

        for dp in &original_datapoints {
            encoder.encode(*dp);
        }

        let bytes = encoder.close();
        let r = BufferedReader::new(bytes);
        let mut decoder = StdDecoder::new(r);

        let mut new_datapoints = Vec::new();

        let mut done = false;
        loop {
            if done {
                break;
            }

            match decoder.next() {
                Ok(dp) => new_datapoints.push(dp),
                Err(err) => {
                    if err == Error::EndOfStream {
                        done = true;
                    } else {
                        panic!("Received an error from decoder: {:?}", err);
                    }
                }
            };
        }

        assert_eq!(original_datapoints, new_datapoints);
    }
}