use fallocate::{fallocate, Mode as FallocateMode};
use std::fs::File;
use std::io::{self, Write};

pub struct PunchCat {
    keep_size_shl: u8,
    punch_size_shl: u8,

    // Up to this position is sparse
    sparse_offset: u64,
    
    // Up to this position has been written
    written_offset: u64,

    backing: File,
}

impl Write for PunchCat {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = try!(self.backing.write(buf));

        self.written_offset += written as u64;

        if let Some((wo, len)) = self.punch_helper() {
            let falloc_mode = FallocateMode::punch_hole();

             try!(fallocate(&mut self.backing, falloc_mode, wo as i64, len as i64));
             self.sparse_offset = wo + len;
        }

        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.backing.flush()
    }
}

impl PunchCat {
    pub fn new(keep_shl: u8, punch_shl: u8, backing: File) -> PunchCat {
        PunchCat {
            keep_size_shl: keep_shl,
            punch_size_shl: punch_shl,

            sparse_offset: 0,
            written_offset: 0,

            backing: backing,
        }
    }

    fn punch_helper(&self) -> Option<(u64, u64)> {
        let keep_size = 1 << self.keep_size_shl;

        if self.written_offset <= self.sparse_offset {
            return None;
        }

        let storage_online = self.written_offset - self.sparse_offset;
        if storage_online < keep_size {
            return None;
        }

        let punch_len = (storage_online - keep_size) >> self.punch_size_shl;
        if punch_len == 0 {
            return None;
        }

        let punch_len_bytes = punch_len << self.punch_size_shl;
        Some((self.sparse_offset as u64, punch_len_bytes as u64))
    }
}