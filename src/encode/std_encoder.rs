use std::mem;

use encode::Encode;
use stream::Write;
use {Bit, DataPoint};

// END_MARKER relies on the fact that when we encode the delta of delta for a number that requires
// more than 12 bits we write four control bits 1111 followed by the 32 bits of the value. Since
// encoding assumes the value is greater than 12 bits, we can store the value 0 to signal the end
// of the stream

/// END_MARKER is a special bit sequence used to indicate the end of the stream
pub const END_MARKER: u64 = 0b1111_0000_0000_0000_0000_0000_0000_0000_0000;

/// END_MARKER_LEN is the length, in bits, of END_MARKER
pub const END_MARKER_LEN: u32 = 36;

/// StdEncoder
///
/// StdEncoder is used to encode `DataPoint`s
#[derive(Debug)]
pub struct StdEncoder<T: Write> {
    time: u64,       // current time
    delta: u64,      // current time delta
    value_bits: u64, // current float value as bits

    // store the number of leading and trailing zeroes in the current xor as u32 so we
    // don't have to do any conversions after calling `leading_zeros` and `trailing_zeros`
    leading_zeroes: u32,
    trailing_zeroes: u32,

    first: bool, // will next DataPoint be the first DataPoint encoded

    w: T,
}

impl<T> StdEncoder<T>
where
    T: Write,
{
    /// new creates a new StdEncoder whose starting timestamp is `start` and writes its encoded
    /// bytes to `w`
    pub fn new(start: u64, w: T) -> Self {
        let mut e = StdEncoder {
            time: start,
            delta: 0,
            value_bits: 0,
            leading_zeroes: 64,  // 64 is an initial sentinel value
            trailing_zeroes: 64, // 64 is an intitial sentinel value
            first: true,
            w,
        };

        // write timestamp header
        e.w.write_bits(start, 64);

        e
    }

    fn write_first(&mut self, time: u64, value_bits: u64) {
        self.delta = time - self.time;
        self.time = time;
        self.value_bits = value_bits;

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

    fn write_next_timestamp(&mut self, time: u64) {
        let delta = time - self.time; // current delta
        let dod = delta.wrapping_sub(self.delta) as i32; // delta of delta

        // store the delta of delta using variable length encoding
        #[cfg_attr(feature = "cargo-clippy", allow(match_overlapping_arm))]
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

    fn write_next_value(&mut self, value_bits: u64) {
        let xor = value_bits ^ self.value_bits;
        self.value_bits = value_bits;

        if xor == 0 {
            // if xor with previous value is zero just store single zero bit
            self.w.write_bit(Bit::Zero);
        } else {
            self.w.write_bit(Bit::One);

            let leading_zeroes = xor.leading_zeros();
            let trailing_zeroes = xor.trailing_zeros();

            if leading_zeroes >= self.leading_zeroes && trailing_zeroes >= self.trailing_zeroes {
                // if the number of leading and trailing zeroes in this xor are >= the leading and
                // trailing zeroes in the previous xor then we only need to store a control bit and
                // the significant digits of this xor
                self.w.write_bit(Bit::Zero);
                self.w.write_bits(
                    xor.wrapping_shr(self.trailing_zeroes),
                    64 - self.leading_zeroes - self.trailing_zeroes,
                );
            } else {
                // if the number of leading and trailing zeroes in this xor are not less than the
                // leading and trailing zeroes in the previous xor then we store a control bit and
                // use 6 bits to store the number of leading zeroes and 6 bits to store the number
                // of significant digits before storing the significant digits themselves

                self.w.write_bit(Bit::One);
                self.w.write_bits(u64::from(leading_zeroes), 6);

                // if significant_digits is 64 we cannot encode it using 6 bits, however since
                // significant_digits is guaranteed to be at least 1 we can subtract 1 to ensure
                // significant_digits can always be expressed with 6 bits or less
                let significant_digits = 64 - leading_zeroes - trailing_zeroes;
                self.w.write_bits(u64::from(significant_digits - 1), 6);
                self.w
                    .write_bits(xor.wrapping_shr(trailing_zeroes), significant_digits);

                // finally we need to update the number of leading and trailing zeroes
                self.leading_zeroes = leading_zeroes;
                self.trailing_zeroes = trailing_zeroes;
            }
        }
    }
}

impl<T> Encode for StdEncoder<T>
where
    T: Write,
{
    fn encode(&mut self, dp: DataPoint) {
        let value_bits = unsafe { mem::transmute::<f64, u64>(dp.value) };

        if self.first {
            self.write_first(dp.time, value_bits);
            self.first = false;
            return;
        }

        self.write_next_timestamp(dp.time);
        self.write_next_value(value_bits)
    }

    fn close(mut self) -> Box<[u8]> {
        self.w.write_bits(END_MARKER, 36);
        self.w.close()
    }
}

#[cfg(test)]
mod tests {
    use super::StdEncoder;
    use encode::Encode;
    use stream::BufferedWriter;
    use DataPoint;

    #[test]
    fn create_new_encoder() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let e = StdEncoder::new(start_time, w);

        let bytes = e.close();
        let expected_bytes: [u8; 13] = [0, 0, 0, 0, 88, 89, 157, 151, 240, 0, 0, 0, 0];

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn encode_datapoint() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let mut e = StdEncoder::new(start_time, w);

        let d1 = DataPoint::new(1482268055 + 10, 1.24);

        e.encode(d1);

        let bytes = e.close();
        let expected_bytes: [u8; 23] = [
            0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 127, 231, 174, 20, 122, 225, 71, 175, 224, 0, 0,
            0, 0,
        ];

        assert_eq!(bytes[..], expected_bytes[..]);
    }

    #[test]
    fn encode_multiple_datapoints() {
        let w = BufferedWriter::new();
        let start_time = 1482268055; // 2016-12-20T21:07:35+00:00
        let mut e = StdEncoder::new(start_time, w);

        let d1 = DataPoint::new(1482268055 + 10, 1.24);

        e.encode(d1);

        let d2 = DataPoint::new(1482268055 + 20, 1.98);

        let d3 = DataPoint::new(1482268055 + 32, 2.37);
        let d4 = DataPoint::new(1482268055 + 44, -7.41);
        let d5 = DataPoint::new(1482268055 + 52, 103.50);

        e.encode(d2);
        e.encode(d3);
        e.encode(d4);
        e.encode(d5);

        let bytes = e.close();
        let expected_bytes: [u8; 61] = [
            0, 0, 0, 0, 88, 89, 157, 151, 0, 20, 127, 231, 174, 20, 122, 225, 71, 174, 204, 207,
            30, 71, 145, 228, 121, 30, 96, 88, 61, 255, 253, 91, 214, 245, 189, 111, 91, 3, 232, 1,
            245, 97, 88, 86, 21, 133, 55, 202, 1, 17, 15, 92, 40, 245, 194, 151, 128, 0, 0, 0, 0,
        ];

        assert_eq!(bytes[..], expected_bytes[..]);
    }
}
