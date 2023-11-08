use std::collections::VecDeque;

pub struct LogBuffer(VecDeque<u8>);

impl LogBuffer {
    pub fn with_capacity(cap: usize) -> Self {
        Self(VecDeque::with_capacity(cap))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn append(&mut self, buf: &[u8]) {
        let buf_len = buf.len();
        let cap = self.0.capacity();
        if buf_len >= cap {
            // The capacity is insufficient for new contents,
            // we must purge the whole buffer first.
            self.0.clear();
            let start = buf_len - cap;
            self.0.extend(&buf[start..buf_len]);
            return;
        }

        loop {
            let used_len = self.0.len();
            let remaining = cap - used_len;
            if remaining >= buf_len {
                self.0.extend(buf);
                return;
            }

            // Trim lines to make more room.
            while let Some(b) = self.0.pop_front() {
                if b == b'\n' {
                    break;
                }
            }
        }
    }

    pub fn with_buffers<F>(&self, mut f: F)
    where
        F: FnMut(&[u8]),
    {
        let slices = self.0.as_slices();
        f(slices.0);
        f(slices.1);
    }
}

impl Default for LogBuffer {
    fn default() -> Self {
        Self::with_capacity(4096)
    }
}

#[cfg(test)]
mod tests {
    use super::LogBuffer;

    fn assert_buf_eq(left: &LogBuffer, right: &[u8]) {
        let mut concat_buf: Vec<u8> = vec![];
        left.with_buffers(|buf| {
            concat_buf.extend(buf);
        });
        assert_eq!(&concat_buf, right);
    }

    #[test]
    fn test_append_and_read() {
        let mut buf = LogBuffer::default();
        buf.append(b"hello,");
        buf.append(b"world");
        assert_buf_eq(&buf, b"hello,world");
    }

    #[test]
    fn test_overwrite() {
        let mut buf = LogBuffer::with_capacity(8);
        buf.append(b"hello");
        buf.append(b"!");
        assert_buf_eq(&buf, b"hello!");

        buf.append(b"abcdefghijklmn");
        assert_buf_eq(&buf, b"ghijklmn");
    }

    #[test]
    fn test_trim_lines() {
        let mut buf = LogBuffer::with_capacity(16);
        buf.append(b"hello\nworld\n");
        buf.append(b"goodbye");
        assert_buf_eq(&buf, b"world\ngoodbye");

        buf = LogBuffer::with_capacity(16);
        buf.append(b"hello, world");
        buf.append(b"farewell!");
        assert_buf_eq(&buf, b"farewell!");
    }
}
