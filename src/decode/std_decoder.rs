use DataPoint;
use stream::Read;
use decode::{Decode, Error};

use ::{Bit, END_MARKER, END_MARKER_LEN};

#[derive(Debug)]
pub struct StdDecoder<T: Read> {
    n: u64, // number of timestamps decoded
    time: u64, // current time
    delta: u64, // current time delta
    value_bits: u64, // current float value as bits
    xor: u64, // current xor

    leading_zeroes: u32, // leading zeroes
    trailing_zeroes: u32, // trailing zeroes

    done: bool,

    r: T,
}

impl<T> StdDecoder<T>
    where T: Read
{
    pub fn new(r: T) -> Self {
        StdDecoder {
            n: 0,
            time: 0,
            delta: 0,
            value_bits: 0,
            xor: 0,
            leading_zeroes: 0,
            trailing_zeroes: 0,
            done: false,
            r: r,
        }
    }

    fn read_initial_timestamp(&mut self) -> Result<u64, Error> {
        self.r
            .read_bits(64)
            .map_err(|_| Error::InvalidInitialTimestamp)
            .map(|time| {
                self.time = time;
                time
            })
    }

    fn read_first_timestamp(&mut self) -> Result<u64, Error> {
        self.read_initial_timestamp()?;

        // sanity check to confirm that the stream contains more than just the initial timestamp
        let control_bit = self.r.peak_bits(1)?;
        if control_bit == 1 {
            return self.r
                .read_bits(END_MARKER_LEN)
                .map_err(|err| Error::Stream(err))
                .and_then(|marker| if marker == END_MARKER {
                    Err(Error::EndOfStream)
                } else {
                    Err(Error::InvalidEndOfStream)
                });
        }

        // stream contains datapoints so we can throw away the control bit
        self.r.read_bit()?;

        self.r
            .read_bits(14)
            .map(|delta| {
                self.delta = delta;
                self.time += delta;
            })?;

        Ok(self.time)
    }

    fn read_next_timestamp(&mut self) -> Result<u64, Error> {
        if self.n == 1 {
            return self.read_first_timestamp();
        }

        let mut control_bits = 0;
        for _ in 0..4 {
            let bit = self.r.read_bit()?;

            if bit == Bit::One {
                control_bits += 1;
            } else {
                break;
            }
        }

        let size = match control_bits {
            0 => {
                self.time += self.delta;
                return Ok(self.time);
            }
            1 => 7,
            2 => 9,
            3 => 12,
            4 => {
                return self.r
                    .read_bits(32)
                    .map_err(|err| Error::Stream(err))
                    .and_then(|dod| if dod == 0 {
                        Err(Error::EndOfStream)
                    } else {
                        Ok(dod)
                    });
            }
            _ => unreachable!(),
        };

        let mut dod = self.r.read_bits(size)?;

        // need to sign extend negative numbers
        if dod > (1 << (size - 1)) {
            let limit = 1 << size;
            dod = dod.wrapping_sub(limit) + limit
        }

        // by performing a wrapping_add we can ensure that negative numbers will be handled correctly
        self.delta = self.delta.wrapping_add(dod);
        self.time = self.time.wrapping_add(self.delta);

        Ok(self.time)
    }

    fn read_first_value(&mut self) -> Result<f64, Error> {
        self.r
            .read_bits(64)
            .map_err(|err| Error::Stream(err))
            .map(|bits| {
                self.value_bits = bits;
                self.value_bits as f64
            })
    }

    fn read_next_value(&mut self) -> Result<f64, Error> {
        if self.n == 1 {
            return self.read_first_value();
        }

        let contol_bit = self.r.read_bit()?;

        if contol_bit == Bit::Zero {
            return Ok(self.value_bits as f64);
        }

        let zeroes_bit = self.r.read_bit()?;

        if zeroes_bit == Bit::One {
            self.leading_zeroes = self.r.read_bits(6).map(|n| n as u32)?;
            let significant_digits = self.r.read_bits(6).map(|n| (n + 1) as u32)?;
            self.trailing_zeroes = 64 - self.leading_zeroes - significant_digits;
        }

        let size = 64 - self.leading_zeroes - self.trailing_zeroes;
        self.r
            .read_bits(size)
            .map_err(|err| Error::Stream(err))
            .map(|bits| {
                self.value_bits ^= bits << self.trailing_zeroes;
                self.value_bits as f64
            })
    }
}

impl<T> Decode for StdDecoder<T>
    where T: Read
{
    fn next(&mut self) -> Result<DataPoint, Error> {
        if self.done {
            return Err(Error::EndOfStream);
        }

        self.n += 1;
        let time = self.read_next_timestamp()
            .map_err(|err| {
                if err == Error::EndOfStream {
                    self.done = true;
                }
                err
            })?;
        let value = self.read_next_value()?;

        Ok(DataPoint::new(time, value))
    }
}

#[cfg(test)]
mod tests {
    use {DataPoint, Decode};
    use stream::BufferedReader;
    use decode::Error;
    use super::StdDecoder;

    #[test]
    fn create_new_decoder() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 240, 0, 0, 0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let mut decoder = StdDecoder::new(r);

        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }

    #[test]
    fn decode_datapoint() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 0, 0, 0, 0, 0, 0, 0, 3, 224, 0, 0,
                         0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let mut decoder = StdDecoder::new(r);

        let expected_datapoint = DataPoint::new(1482268055 + 10, 1.0);

        assert_eq!(decoder.next().unwrap(), expected_datapoint);
        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }

    #[test]
    fn decode_multiple_datapoints() {
        let bytes = vec![0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 0, 0, 0, 0, 0, 0, 0, 2, 64, 191,
                         129, 252, 0, 0, 0, 0];
        let r = BufferedReader::new(bytes.into_boxed_slice());
        let mut decoder = StdDecoder::new(r);

        let first_expected_datapoint = DataPoint::new(1482268055 + 10, 1.0);
        let second_expected_datapoint = DataPoint::new(1482268055 + 20, 1.0);
        let third_expected_datapoint = DataPoint::new(1482268055 + 32, 2.0);

        assert_eq!(decoder.next().unwrap(), first_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), second_expected_datapoint);
        assert_eq!(decoder.next().unwrap(), third_expected_datapoint);
        assert_eq!(decoder.next().err().unwrap(), Error::EndOfStream);
    }
}