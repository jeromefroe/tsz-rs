use std::boxed::Box;

use Bit;
use stream::{Error, Read};

#[derive(Debug)]
pub struct BufferedReader {
    bytes: Vec<u8>, // internal buffer of bytes
    index: usize, // index into bytes
    pos: u32, // position in the byte we are currenlty reading
}

impl BufferedReader {
    pub fn new(bytes: Box<[u8]>) -> Self {
        BufferedReader {
            bytes: bytes.into_vec(),
            index: 0,
            pos: 0,
        }
    }

    fn get_byte(&mut self) -> Result<u8, Error> {
        self.bytes.get(self.index).map(|byte| *byte).ok_or(Error::EOF)
    }
}

impl Read for BufferedReader {
    fn read_bit(&mut self) -> Result<Bit, Error> {
        if self.pos == 8 {
            self.index += 1;
            self.pos = 0;
        }

        let byte = self.get_byte()?;

        // let byte = match self.get_byte() {
        //     Some(byte) => byte,
        //     None => return Err(Error::EOF),
        // };

        let bit = if byte & 1u8.wrapping_shl(7 - self.pos) == 0 {
            Bit::Zero
        } else {
            Bit::One
        };

        self.pos += 1;

        Ok(bit)
    }

    fn read_byte(&mut self) -> Result<u8, Error> {
        if self.pos == 0 {
            self.pos += 8;
            return self.get_byte();
        }

        if self.pos == 8 {
            self.index += 1;
            return self.get_byte();
        }

        let mut byte = 0;
        let mut b = self.get_byte()?;

        byte = byte | (b.wrapping_shl(self.pos));

        self.index += 1;
        b = self.get_byte()?;

        // b = match self.get_byte() {
        //     Some(b) => b,
        //     None => return None,
        // };

        byte = byte | (b.wrapping_shr(8 - self.pos));

        Ok(byte)
    }

    fn read_bits(&mut self, mut num_bits: u32) -> Result<u64, Error> {
        // can't read more than 64 bits into a u64
        if num_bits > 64 {
            num_bits = 64;
        }

        let mut bits: u64 = 0;
        while num_bits >= 8 {
            let byte = self.read_byte().map(|byte| byte as u64)?;
            bits = bits.wrapping_shl(8) | byte;
            num_bits -= 8;
        }

        while num_bits > 0 {
            // match self.read_bit() {
            //     Some(bit) => {
            //         bits = bits.wrapping_shl(1) | bit.to_u64();
            //     }
            //     None => return None,
            // };
            self.read_bit().map(|bit| bits = bits.wrapping_shl(1) | bit.to_u64())?;

            num_bits -= 1;
        }

        Ok(bits)
    }

    fn peak_bits(&mut self, num_bits: u32) -> Result<u64, Error> {
        // save the current index and pos so we can reset them after calling `read_bits`
        let index = self.index;
        let pos = self.pos;

        // let bits = match self.read_bits(num_bits) {
        //     Some(bits) => bits,
        //     None => return None,
        // };
        let bits = self.read_bits(num_bits)?;

        self.index = index;
        self.pos = pos;

        Ok(bits)
    }
}

#[cfg(test)]
mod tests {
    use Bit;
    use stream::{Error, Read};
    use super::BufferedReader;

    #[test]
    fn read_bit() {
        let bytes = vec![0b01101100, 0b11101001];
        let mut b = BufferedReader::new(bytes.into_boxed_slice());

        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);

        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bit().unwrap(), Bit::One);

        assert_eq!(b.read_bit().err().unwrap(), Error::EOF);
    }

    #[test]
    fn read_byte() {
        let bytes = vec![100, 25, 0, 240, 240];
        let mut b = BufferedReader::new(bytes.into_boxed_slice());

        assert_eq!(b.read_byte().unwrap(), 100);
        assert_eq!(b.read_byte().unwrap(), 25);
        assert_eq!(b.read_byte().unwrap(), 0);

        // read some individual bits we can test `read_byte` when the position in the
        // byte we are currently reading is non-zero
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);
        assert_eq!(b.read_bit().unwrap(), Bit::One);

        assert_eq!(b.read_byte().unwrap(), 15);

        assert_eq!(b.read_byte().err().unwrap(), Error::EOF);
    }

    #[test]
    fn read_bits() {
        let bytes = vec![0b01010111, 0b00011101, 0b11110101, 0b00010100];
        let mut b = BufferedReader::new(bytes.into_boxed_slice());

        assert_eq!(b.read_bits(3).unwrap(), 0b010);
        assert_eq!(b.read_bits(1).unwrap(), 0b1);
        assert_eq!(b.read_bits(20).unwrap(), 0b01110001110111110101);
        assert_eq!(b.read_bits(8).unwrap(), 0b00010100);
        assert_eq!(b.read_bits(4).err().unwrap(), Error::EOF);
    }

    #[test]
    fn read_mixed() {
        let bytes = vec![0b01101101, 0b01101101];
        let mut b = BufferedReader::new(bytes.into_boxed_slice());

        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bits(3).unwrap(), 0b110);
        assert_eq!(b.read_byte().unwrap(), 0b11010110);
        assert_eq!(b.read_bits(2).unwrap(), 0b11);
        assert_eq!(b.read_bit().unwrap(), Bit::Zero);
        assert_eq!(b.read_bits(1).unwrap(), 0b1);
        assert_eq!(b.read_bit().err().unwrap(), Error::EOF);
    }

    #[test]
    fn peak_bits() {
        let bytes = vec![0b01010111, 0b00011101, 0b11110101, 0b00010100];
        let mut b = BufferedReader::new(bytes.into_boxed_slice());

        assert_eq!(b.peak_bits(1).unwrap(), 0b0);
        assert_eq!(b.peak_bits(4).unwrap(), 0b0101);
        assert_eq!(b.peak_bits(8).unwrap(), 0b01010111);
        assert_eq!(b.peak_bits(20).unwrap(), 0b01010111000111011111);

        // read some individual bits we can test `peak_bits` when the position in the
        // byte we are currently reading is non-zero
        assert_eq!(b.read_bits(12).unwrap(), 0b010101110001);

        assert_eq!(b.peak_bits(1).unwrap(), 0b1);
        assert_eq!(b.peak_bits(4).unwrap(), 0b1101);
        assert_eq!(b.peak_bits(8).unwrap(), 0b11011111);
        assert_eq!(b.peak_bits(20).unwrap(), 0b11011111010100010100);

        assert_eq!(b.peak_bits(22).err().unwrap(), Error::EOF);
    }
}