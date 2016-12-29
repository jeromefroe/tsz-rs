#TSZ

A crate for time series compression based upon Facebook's white paper
[Gorilla: A Fast, Scalable, In-Memory Time Series Database](http://www.vldb.org/pvldb/vol8/p1816-teller.pdf).
Provides functionality for compressing a stream of `DataPoint`s, which are composed of a time and
value, into bytes, and decompressing a stream of bytes into `DataPoint`s.

## Example

Below is a simple example of how to interact with `tsz` to encode and decode `DataPoint`s.

```rust
extern crate tsz;

use std::vec::Vec;
use tsz::{DataPoint, Encode, Decode, StdEncoder, StdDecoder};
use tsz::stream::{BufferedReader, BufferedWriter};
use tsz::decode::Error;

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

fn main() {
    let w = BufferedWriter::new();

    // 1482892260 is the Unix timestamp of the start of the stream
    let mut encoder = StdEncoder::new(1482892260, w);

    let mut actual_datapoints = Vec::new();

    for line in DATA.lines() {
        let substrings: Vec<&str> = line.split(",").collect();
        let t = substrings[0].parse::<u64>().unwrap();
        let v = substrings[1].parse::<f64>().unwrap();
        let dp = DataPoint::new(t, v);
        actual_datapoints.push(dp);
    }

    for dp in &actual_datapoints {
        encoder.encode(*dp);
    }

    let bytes = encoder.close();
    let r = BufferedReader::new(bytes);
    let mut decoder = StdDecoder::new(r);

    let mut expected_datapoints = Vec::new();

    let mut done = false;
    loop {
        if done {
            break;
        }

        match decoder.next() {
            Ok(dp) => expected_datapoints.push(dp),
            Err(err) => {
                if err == Error::EndOfStream {
                    done = true;
                } else {
                    panic!("Received an error from decoder: {:?}", err);
                }
            }
        };
    }

    println!("actual datapoints: {:?}", actual_datapoints);
    println!("expected datapoints: {:?}", expected_datapoints);
}
```