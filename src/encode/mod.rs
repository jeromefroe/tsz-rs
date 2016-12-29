use DataPoint;

/// Encode
///
/// Encode is the trait used to encode a stream of `DataPoint`s.
pub trait Encode {
    fn encode(&mut self, dp: DataPoint);
    fn close(self) -> Box<[u8]>;
}

pub mod std_encoder;