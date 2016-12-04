use DataPoint;

pub trait Encode {
    fn encode(&mut self, dp: DataPoint);
    fn close(self) -> Box<[u8]>;
}

pub mod std_encoder;