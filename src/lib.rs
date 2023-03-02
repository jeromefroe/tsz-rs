// MIT License

// Copyright (c) 2016 Jerome Froelich

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! A crate for time series compression based upon Facebook's white paper
//! [Gorilla: A Fast, Scalable, In-Memory Time Series Database](http://www.vldb.org/pvldb/vol8/p1816-teller.pdf).
//! `tsz` provides functionality for compressing a stream of `DataPoint`s, which are composed of a
//! time and value, into bytes, and decompressing a stream of bytes into `DataPoint`s.
//!
//! ## Example
//!
//! Below is a simple example of how to interact with `tsz` to encode and decode `DataPoint`s.
//!
//! ```rust,no_run
//! extern crate tsz;
//!
//! use std::vec::Vec;
//! use tsz::{DataPoint, Encode, Decode, StdEncoder, StdDecoder};
//! use tsz::stream::{BufferedReader, BufferedWriter};
//! use tsz::decode::Error;
//!
//! const DATA: &'static str = "1482892270,1.76
//! 1482892280,7.78
//! 1482892288,7.95
//! 1482892292,5.53
//! 1482892310,4.41
//! 1482892323,5.30
//! 1482892334,5.30
//! 1482892341,2.92
//! 1482892350,0.73
//! 1482892360,-1.33
//! 1482892370,-1.78
//! 1482892390,-12.45
//! 1482892401,-34.76
//! 1482892490,78.9
//! 1482892500,335.67
//! 1482892800,12908.12
//! ";
//!
//! fn main() {
//!     let w = BufferedWriter::new();
//!
//!     // 1482892260 is the Unix timestamp of the start of the stream
//!     let mut encoder = StdEncoder::new(1482892260, w);
//!
//!     let mut actual_datapoints = Vec::new();
//!
//!     for line in DATA.lines() {
//!         let substrings: Vec<&str> = line.split(",").collect();
//!         let t = substrings[0].parse::<u64>().unwrap();
//!         let v = substrings[1].parse::<f64>().unwrap();
//!         let dp = DataPoint::new(t, v);
//!         actual_datapoints.push(dp);
//!     }
//!
//!     for dp in &actual_datapoints {
//!         encoder.encode(*dp);
//!     }
//!
//!     let bytes = encoder.close();
//!     let r = BufferedReader::new(bytes);
//!     let mut decoder = StdDecoder::new(r);
//!
//!     let mut expected_datapoints = Vec::new();
//!
//!     let mut done = false;
//!     loop {
//!         if done {
//!             break;
//!         }
//!
//!         match decoder.next() {
//!             Ok(dp) => expected_datapoints.push(dp),
//!             Err(err) => {
//!                 if err == Error::EndOfStream {
//!                     done = true;
//!                 } else {
//!                     panic!("Received an error from decoder: {:?}", err);
//!                 }
//!             }
//!         };
//!     }
//!
//!     println!("actual datapoints: {:?}", actual_datapoints);
//!     println!("expected datapoints: {:?}", expected_datapoints);
//! }
//! ```

use std::cmp::Ordering;

/// Bit
///
/// An enum used to represent a single bit, can be either `Zero` or `One`.
#[derive(Debug, PartialEq)]
pub enum Bit {
    Zero,
    One,
}

impl Bit {
    /// Convert a bit to u64, so `Zero` becomes 0 and `One` becomes 1.
    pub fn to_u64(&self) -> u64 {
        match self {
            Bit::Zero => 0,
            Bit::One => 1,
        }
    }
}

/// DataPoint
///
/// Struct used to represent a single datapoint. Consists of a time and value.
#[derive(Debug, Copy, serde::Deserialize, serde::Serialize)]
pub struct DataPoint {
    time: u64,
    value: f64,
}

impl Clone for DataPoint {
    fn clone(&self) -> DataPoint {
        *self
    }
}

impl DataPoint {
    // Create a new DataPoint from a time and value.
    pub fn new(time: u64, value: f64) -> Self {
        DataPoint { time, value }
    }

    // Get the time for this DataPoint.
    pub fn get_time(&self) -> u64 {
        self.time
    }

    // Get the value for this DataPoint.
    pub fn get_value(&self) -> f64 {
        self.value
    }
}

impl PartialEq for DataPoint {
    #[inline]
    fn eq(&self, other: &DataPoint) -> bool {
        if self.time == other.time {
            if self.value.is_nan() {
                return other.value.is_nan();
            } else {
                return self.value == other.value;
            }
        }
        false
    }
}

impl Eq for DataPoint {}

impl Ord for DataPoint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for DataPoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub mod stream;

pub mod encode;
pub use self::encode::std_encoder::StdEncoder;
pub use self::encode::Encode;

pub mod decode;
pub use self::decode::std_decoder::StdDecoder;
pub use self::decode::Decode;

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::decode::Error;
    use super::stream::{BufferedReader, BufferedWriter};
    use super::{DataPoint, Decode, Encode, StdDecoder, StdEncoder};

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

    #[test]
    fn data_point_ordering_test() {
        let dp_1 = DataPoint::new(20, 2.0);
        let dp_2 = DataPoint::new(10, 3.0);
        let dp_3 = DataPoint::new(10, 3.0);

        // The ordering of data points is based on time, so dp_2 will be less than dp_1.
        assert!(dp_2 < dp_1);

        // Data points are equal if their time and values are equal.
        assert!(dp_2 == dp_3 && dp_1 != dp_2);

        // Data points with NaN values are equal if their times are equal.
        let dp_4 = DataPoint::new(10, f64::NAN);
        let dp_5 = DataPoint::new(10, f64::NAN);
        assert!(dp_4 == dp_5);
    }
}
