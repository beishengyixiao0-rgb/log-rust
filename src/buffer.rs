use parking_lot::Mutex;
use std::sync::Arc;

// 缓冲区配置常量（与 C++ 原版保持一致）
const BUFFER_DEFAULT_SIZE: usize = 1 * 1024 * 1024; // 1MB
const BUFFER_INCREMENT_SIZE: usize = 1 * 1024 * 1024; // 1MB
const BUFFER_THRESHOLD_SIZE: usize = 10 * 1024 * 1024; // 10MB

/// 简单的日志消息字节缓冲区
///
/// 这是一个带有读/写索引的环形缓冲区结构
pub struct Buffer {
    data: Vec<u8>,
    reader_idx: usize,
    writer_idx: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    /// 创建新缓冲区
    pub fn new() -> Self {
        Buffer {
            data: vec![0u8; BUFFER_DEFAULT_SIZE],
            reader_idx: 0,
            writer_idx: 0,
        }
    }

    /// 创建指定容量的缓冲区
    pub fn with_capacity(capacity: usize) -> Self {
        Buffer {
            data: vec![0u8; capacity],
            reader_idx: 0,
            writer_idx: 0,
        }
    }

    /// 检查缓冲区是否为空
    pub fn is_empty(&self) -> bool {
        self.reader_idx == self.writer_idx
    }

    /// 获取可读数据大小
    pub fn readable_size(&self) -> usize {
        self.writer_idx - self.reader_idx
    }

    /// 获取可写空间大小
    pub fn writable_size(&self) -> usize {
        self.data.len() - self.writer_idx
    }

    /// 重置缓冲区
    pub fn reset(&mut self) {
        self.reader_idx = 0;
        self.writer_idx = 0;
    }

    /// 与另一个缓冲区交换内容
    pub fn swap(&mut self, other: &mut Buffer) {
        std::mem::swap(&mut self.data, &mut other.data);
        std::mem::swap(&mut self.reader_idx, &mut other.reader_idx);
        std::mem::swap(&mut self.writer_idx, &mut other.writer_idx);
    }

    /// 推送数据到缓冲区
    pub fn push(&mut self, data: &[u8]) {
        self.ensure_capacity(data.len());
        self.data[self.writer_idx..self.writer_idx + data.len()].copy_from_slice(data);
        self.writer_idx += data.len();
    }

    /// 推送字符串到缓冲区
    pub fn push_str(&mut self, s: &str) {
        self.push(s.as_bytes());
    }

    /// 获取可读数据为切片
    pub fn as_slice(&self) -> &[u8] {
        &self.data[self.reader_idx..self.writer_idx]
    }

    /// 从缓冲区消耗 `len` 字节
    pub fn consume(&mut self, len: usize) {
        self.reader_idx += len;
        assert!(
            self.reader_idx <= self.writer_idx,
            "Reader index exceeded writer index"
        );
    }

    /// 获取数据为字符串（用于消费）
    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_slice()).to_string()
    }

    // 确保有足够的容量
    fn ensure_capacity(&mut self, len: usize) {
        if len <= self.writable_size() {
            return;
        }

        let new_capacity = if self.data.len() < BUFFER_THRESHOLD_SIZE {
            self.data.len() * 2 + len
        } else {
            self.data.len() + BUFFER_INCREMENT_SIZE + len
        };

        self.data.resize(new_capacity, 0);
    }
}

/// 线程安全的共享缓冲区包装器
#[derive(Clone)]
pub struct SharedBuffer {
    inner: Arc<Mutex<Buffer>>,
}

impl SharedBuffer {
    /// 创建新的共享缓冲区
    pub fn new() -> Self {
        SharedBuffer {
            inner: Arc::new(Mutex::new(Buffer::new())),
        }
    }

    /// 推送数据
    pub fn push(&self, data: &[u8]) {
        self.inner.lock().push(data);
    }

    /// 推送字符串
    pub fn push_str(&self, s: &str) {
        self.inner.lock().push_str(s);
    }

    /// 与另一个共享缓冲区交换
    pub fn swap_with(&self, other: &SharedBuffer) {
        let mut self_buf = self.inner.lock();
        let mut other_buf = other.inner.lock();
        self_buf.swap(&mut other_buf);
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    /// 获取可读大小
    pub fn readable_size(&self) -> usize {
        self.inner.lock().readable_size()
    }

    /// 在锁内操作缓冲区
    pub fn with_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Buffer) -> R,
    {
        let mut buf = self.inner.lock();
        f(&mut buf)
    }
}

impl Default for SharedBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_buffer_push_read() {
        let mut buf = Buffer::new();
        buf.push_str("hello");
        assert_eq!(buf.readable_size(), 5);
        assert_eq!(buf.as_slice(), b"hello");
    }

    #[test]
    fn test_buffer_consume() {
        let mut buf = Buffer::new();
        buf.push_str("hello world");
        buf.consume(6);
        assert_eq!(buf.as_slice(), b"world");
    }

    #[test]
    fn test_buffer_swap() {
        let mut buf1 = Buffer::new();
        let mut buf2 = Buffer::new();
        buf1.push_str("test");
        buf1.swap(&mut buf2);
        assert_eq!(buf2.as_slice(), b"test");
        assert!(buf1.is_empty());
    }
}
