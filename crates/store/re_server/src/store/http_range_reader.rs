use std::io::{Read, Seek, SeekFrom};

use url::Url;

/// Blocking HTTP range reader for lazy RRD registration/loading.
///
/// The RRD footer and chunk provider APIs are synchronous `Read + Seek`, so this
/// adapter performs one HTTP range request per `read` at the current cursor.
pub struct HttpRangeReader {
    url: Url,
    agent: ureq::Agent,
    len: u64,
    pos: u64,
}

impl HttpRangeReader {
    pub fn new(url: Url) -> std::io::Result<Self> {
        let agent = ureq::Agent::new_with_defaults();
        let len = content_len(&agent, &url)?;
        Ok(Self {
            url,
            agent,
            len,
            pos: 0,
        })
    }
}

impl Read for HttpRangeReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.pos >= self.len {
            return Ok(0);
        }

        let requested_len = u64::try_from(buf.len()).map_err(std::io::Error::other)?;
        let end = self.pos.saturating_add(requested_len).min(self.len) - 1;
        let expected_len = usize::try_from(end - self.pos + 1).map_err(std::io::Error::other)?;

        let response = self
            .agent
            .get(self.url.as_str())
            .header("Range", &format!("bytes={}-{}", self.pos, end))
            .header("Accept-Encoding", "identity")
            .call()
            .map_err(to_io_error)?;

        if response.status() != 206 {
            return Err(std::io::Error::other(format!(
                "HTTP range request for {} returned {}; expected 206 Partial Content",
                self.url,
                response.status()
            )));
        }

        if let Some(content_range) = response.headers().get("Content-Range") {
            let expected = format!("bytes {}-{}/{}", self.pos, end, self.len);
            if content_range.to_str().ok() != Some(expected.as_str()) {
                return Err(std::io::Error::other(format!(
                    "HTTP Content-Range mismatch for {}: expected {expected}, got {:?}",
                    self.url, content_range
                )));
            }
        }

        let mut body = response.into_body();
        let bytes = body.read_to_vec().map_err(std::io::Error::other)?;
        if bytes.len() != expected_len {
            return Err(std::io::Error::other(format!(
                "HTTP range response for {} returned {} bytes; expected {expected_len}",
                self.url,
                bytes.len()
            )));
        }

        buf[..expected_len].copy_from_slice(&bytes);
        self.pos += u64::try_from(expected_len).map_err(std::io::Error::other)?;
        Ok(expected_len)
    }
}

impl Seek for HttpRangeReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.len) + i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.pos) + i128::from(offset),
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cannot seek before start of HTTP object",
            ));
        }

        self.pos = u64::try_from(new_pos).map_err(std::io::Error::other)?;
        Ok(self.pos)
    }
}

fn content_len(agent: &ureq::Agent, url: &Url) -> std::io::Result<u64> {
    let response = agent.head(url.as_str()).call().map_err(to_io_error)?;
    let len = response
        .headers()
        .get("Content-Length")
        .ok_or_else(|| {
            std::io::Error::other(format!("HTTP response missing Content-Length: {url}"))
        })?
        .to_str()
        .map_err(std::io::Error::other)?
        .parse::<u64>()
        .map_err(std::io::Error::other)?;

    if let Some(accept_ranges) = response.headers().get("Accept-Ranges")
        && accept_ranges
            .to_str()
            .is_ok_and(|value| value.eq_ignore_ascii_case("bytes"))
    {
        return Ok(len);
    }

    // Some CDNs omit Accept-Ranges on HEAD but still support Range on GET.
    let probe = agent
        .get(url.as_str())
        .header("Range", "bytes=0-0")
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(to_io_error)?;
    if probe.status() != 206 {
        return Err(std::io::Error::other(format!(
            "HTTP source does not support byte ranges: {url}"
        )));
    }

    Ok(len)
}

fn to_io_error(err: ureq::Error) -> std::io::Error {
    std::io::Error::other(err)
}
