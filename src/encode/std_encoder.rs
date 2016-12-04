use {Bit, DataPoint};
use encode::Encode;
use stream::Write;

// END_MARKER relies on the fact that when we encode the delta of delta for a number that requires
// more than 12 bits we write four control bits 1111 followed by the 32 bits of the value. Since
// encoding assumes the value is greater than 12 bits, we can store the value 0 to signal the end
// of the stream
pub const END_MARKER: u64 = 0b111100000000000000000000000000000000;
pub const END_MARKER_LEN: u32 = 36;

#[derive(Debug)]
pub struct StdEncoder<T: Write> {
    time: u64, // current time
    delta: u64, // current time delta
    value_bits: u64, // current float value as bits

    // store the number of leading and trailing zeroes in the current xor as u32 so we
    // don't have to do any conversions after calling `leading_zeros` and `trailing_zeros`
    leading_zeroes: u32,
    trailing_zeroes: u32,

    first: bool, // have we written the first datapoint yet

    w: T,
}

impl<T> StdEncoder<T>
    where T: Write
{
    pub fn new(start: u64, w: T) -> Self {
        let mut e = StdEncoder {
            time: start,
            delta: 0,
            value_bits: 0,
            leading_zeroes: 0,
            trailing_zeroes: 0,
            first: false,
            w: w,
        };

        // write timestamp header
        e.w.write_bits(start, 64);

        e
    }

    fn write_first(&mut self, dp: DataPoint) {
        let start = self.time;
        self.time = dp.time;
        self.value_bits = dp.value as u64;
        self.delta = self.time - start;

        // write one control bit so we can distinguish a stream which contains only an initial
        // timestamp, this assumes the first bit of the END_MARKER is 1
        self.w.write_bit(Bit::Zero);

        // store the first delta with 14 bits which is enough to span just over 4 hours
        // if one wanted to use a window larger than 4 hours this size would increase
        self.w.write_bits(self.delta, 14);

        // store the first value exactly
        self.w.write_bits(self.value_bits, 64);

        self.first = true
    }

    fn write_next(&mut self, dp: DataPoint) {

        self.write_next_timestamp(dp.time);
        self.write_next_value(dp.value)
    }

    fn write_next_timestamp(&mut self, time: u64) {
        let delta = time - self.time; // current delta
        let dod = delta.wrapping_sub(self.delta) as i32; // delta of delta

        // store the delta of delta using variable length encoding
        match dod {
            0 => {
                self.w.write_bit(Bit::Zero);
            }
            -63...64 => {
                self.w.write_bits(0b10, 2);
                self.w.write_bits(dod as u64, 7);
            }
            -255...256 => {
                self.w.write_bits(0b110, 3);
                self.w.write_bits(dod as u64, 9);
            }
            -2047...2048 => {
                self.w.write_bits(0b1110, 4);
                self.w.write_bits(dod as u64, 12);
            }
            _ => {
                self.w.write_bits(0b1111, 4);
                self.w.write_bits(dod as u64, 32);
            }
        }

        self.delta = delta;
        self.time = time;
    }

    fn write_next_value(&mut self, value: f64) {
        let value_bits = value as u64;
        let xor = value_bits ^ self.value_bits;

        if xor == 0 {
            // if xor with previous value is zero just store single zero bit
            self.w.write_bit(Bit::Zero);
        } else {
            self.w.write_bit(Bit::One);

            let leading_zeroes = xor.leading_zeros();
            let trailing_zeroes = xor.trailing_zeros();

            if leading_zeroes < self.leading_zeroes && trailing_zeroes < self.trailing_zeroes {
                // if the number of leading and trailing zeroes in this xor are less than the
                // leading and trailing zeroes in the previous xor then we only need to store
                // a control bit and the significant digits of this xor
                self.w.write_bit(Bit::Zero);
                self.w.write_bits(xor.wrapping_shl(trailing_zeroes),
                                  64 - leading_zeroes - trailing_zeroes);
            } else {

                // if the number of leading and trailing zeroes in this xor are not less than the
                // leading and trailing zeroes in the previous xor then we store a control bit and
                // use 6 bits to store the number of leading zeroes and 6 bits to store the number
                // of significant digits before storing the significant digits themselves

                self.w.write_bit(Bit::One);
                self.w.write_bits(leading_zeroes as u64, 6);

                // if significant_digits is 64 we cannot encode it using 6 bits, however since
                // significant_digits is guaranteed to be at least 1 we can subtract 1 to ensure
                // significant_digits can always be expressed with 6 bits or less
                let significant_digits = 64 - leading_zeroes - trailing_zeroes;
                self.w.write_bits((significant_digits - 1) as u64, 6);
                self.w.write_bits(xor.wrapping_shl(trailing_zeroes), significant_digits);

                // finally we need to update the number of leading and trailing zeroes
                self.leading_zeroes = leading_zeroes;
                self.trailing_zeroes = trailing_zeroes;
            }

        }
    }
}

impl<T> Encode for StdEncoder<T>
    where T: Write
{
    fn encode(&mut self, dp: DataPoint) {
        if !self.first {
            return self.write_first(dp);
        }

        self.write_next(dp);
    }

    fn close(mut self) -> Box<[u8]> {
        self.w.write_bits(END_MARKER, 36);
        self.w.close()
    }
}

#[cfg(test)]
mod tests {
    use DataPoint;
    use encode::Encode;
    use stream::BufferedWriter;
    use super::StdEncoder;

    #[test]
    fn create_new_encoder() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let e = StdEncoder::new(start_time, w);

        let bytes = e.close();

        // 1482268055 = 00000000 00000000 00000000 00000000 01011000 01011001 10011101 10010111
        //            =     0        0        0        0       88       89       157     151
        // END_MARKER = 11110000 00000000 00000000 00000000 0000
        //            =    240       0        0        0       0
        let expected_bytes: [u8; 13] = [0, 0, 0, 0, 88, 89, 157, 151, 240, 0, 0, 0, 0];

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn encode_one_datapoint() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let mut e = StdEncoder::new(start_time, w);

        let d1 = DataPoint::new(1482268055 + 10, 1.0);

        e.encode(d1);

        let bytes = e.close();

        // write control bit => 0
        // write first delta (10) using 14 bits => 00000000 001010
        // write first value (1.0) using 64 bits => 00000000 00000000 00000000 00000000 00000000 00000000 00000000 00000001
        // write end marker using 36 bits => 11110000 00000000 00000000 00000000 0000
        //
        // bits written: 00000000 00010100 00000000 00000000 00000000 00000000 00000000 00000000
        //         =        0        20        0        0        0       0         0        0
        //               00000000 00000011 11100000 00000000 00000000 00000000 000
        //                   0        3       224       0        0       0         0
        let expected_bytes: [u8; 23] = [0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 0, 0, 0, 0, 0, 0, 0,
                                        3, 224, 0, 0, 0, 0];

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn encode_multiple_datapoints() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let mut e = StdEncoder::new(start_time, w);

        let d1 = DataPoint::new(1482268055 + 10, 1.0);

        e.encode(d1);

        let d2 = DataPoint::new(1482268055 + 20, 1.0);

        let d3 = DataPoint::new(1482268055 + 32, 2.0);

        e.encode(d2);
        e.encode(d3);

        let bytes = e.close();

        // write delta of delta (0) with 1 bit => 0
        // write xor of values (0) with 1 bit => 0
        // write delta of delta (2) with 9 bits => 1 00 000010
        // write xor of values (3) with 16 bits => 1 1 111110 000001 11
        // write end marker using 36 bits => 11110000 00000000 00000000 00000000 0000
        //
        // bits written: 0 01000000 10111111 10000001 11111100 00000000 00000000 00000000 000000
        //          =         64      191       129      252        0        0        0       0

        let expected_bytes: [u8; 26] = [0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 0, 0, 0, 0, 0, 0, 0,
                                        2, 64, 191, 129, 252, 0, 0, 0, 0];

        assert_eq!(bytes[..], expected_bytes[..]);
    }
}