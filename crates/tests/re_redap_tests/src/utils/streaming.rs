use tonic::codec::DecodeBuf;

struct AdapterDecoder<T>(Vec<T>);

impl<T> AdapterDecoder<T> {
    pub fn new(mut items: Vec<T>) -> Self {
        // we pop them in reverse order
        items.reverse();
        Self(items)
    }
}

impl<T> tonic::codec::Decoder for AdapterDecoder<T> {
    type Item = T;
    type Error = tonic::Status;

    fn decode(&mut self, _src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.0.pop())
    }
}

/// Utility to turn an iterator of requests into a streaming request that grpc handlers understand.
pub fn make_streaming_request<T: Send + Sync + 'static>(
    requests: impl IntoIterator<Item = T>,
) -> tonic::Request<tonic::Streaming<T>> {
    let items: Vec<T> = requests.into_iter().collect();

    // Create properly framed but empty gRPC messages
    // Each message: [compression_flag: 1 byte][length: 4 bytes][data: 0 bytes]
    let mut body = Vec::new();
    for _ in 0..items.len() {
        body.push(0u8); // compression flag = 0 (uncompressed)
        body.extend_from_slice(&[0, 0, 0, 0]); // message length = 0 (big-endian)
    }

    tonic::Request::new(tonic::Streaming::new_request(
        AdapterDecoder::new(items),
        String::from_utf8(body).unwrap(),
        None,
        None,
    ))
}
