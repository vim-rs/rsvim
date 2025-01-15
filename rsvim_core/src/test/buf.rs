//! Buffer utils for testing.

use crate::buf::{Buffer, BufferArc, BufferLocalOptions};

use ropey::{Rope, RopeBuilder};
use std::fs::File;
use std::io::BufReader;

/// Create rope from filename.
pub fn make_rope_from_file(filename: String) -> Rope {
  Rope::from_reader(BufReader::new(File::open(filename).unwrap())).unwrap()
}

/// Create rope from lines.
pub fn make_rope_from_lines(lines: Vec<&str>) -> Rope {
  let mut rb: RopeBuilder = RopeBuilder::new();
  for line in lines.iter() {
    rb.append(line);
  }
  rb.finish()
}

/// Create buffer from filename.
pub fn make_buffer_from_file(filename: String) -> BufferArc {
  let rp = make_rope_from_file(filename);
  let bf = Buffer::_new(rp, BufferLocalOptions::default(), None, None, None, None);
  Buffer::to_arc(bf)
}

/// Create buffer from lines.
pub fn make_buffer_from_lines(lines: Vec<&str>) -> BufferArc {
  let rp = make_rope_from_lines(lines);
  let buf = Buffer::_new(rp, BufferLocalOptions::default(), None, None, None, None);
  Buffer::to_arc(buf)
}

/// Create empty buffer.
pub fn make_empty_buffer() -> BufferArc {
  let buf = Buffer::_new_empty(BufferLocalOptions::default());
  Buffer::to_arc(buf)
}
