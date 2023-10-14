use std::collections::VecDeque;

pub struct BufList {
    cap: usize,
    list: VecDeque<Vec<u8>>,
}

impl BufList {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            cap,
            list: Default::default(),
        }
    }

    pub fn push(&mut self, buf: Vec<u8>) {
        if self.list.len() >= self.cap {
            self.list.pop_front();
        }
        self.list.push_back(buf);
    }

    pub fn peek(&self) -> Vec<u8> {
        let cap = self.list.iter().map(|b| b.len()).sum();
        let mut buf = Vec::with_capacity(cap);
        for avail_buf in self.list.iter() {
            buf.extend(avail_buf);
        }
        buf
    }

    #[allow(dead_code)]
    pub fn consume(&mut self) -> Vec<u8> {
        let cap = self.list.iter().map(|b| b.len()).sum();
        let mut buf = Vec::with_capacity(cap);
        while let Some(avail_buf) = self.list.pop_front() {
            buf.extend(avail_buf);
        }
        buf
    }
}

impl Default for BufList {
    fn default() -> Self {
        Self::with_capacity(8)
    }
}
