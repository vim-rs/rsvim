//! Vim buffers.

use crate::defaults::grapheme::AsciiControlCodeFormatter;
// use crate::evloop::msg::WorkerToMasterMessage;
use crate::res::IoResult;

// Re-export
pub use crate::buf::opt::{BufferLocalOptions, FileEncoding};

use ascii::AsciiChar;
use compact_str::CompactString;
use parking_lot::RwLock;
use path_absolutize::Absolutize;
use ropey::iter::Lines;
use ropey::{Rope, RopeBuilder, RopeSlice};
use std::collections::{BTreeMap, HashMap};
use std::convert::From;
use std::fs::Metadata;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};
use std::time::Instant;
// use tokio::sync::mpsc::Sender;
use tracing::debug;
use unicode_width::UnicodeWidthChar;

pub mod opt;

/// Buffer ID.
pub type BufferId = i32;

/// Next unique buffer ID.
///
/// NOTE: Start form 1.
pub fn next_buffer_id() -> BufferId {
  static VALUE: AtomicI32 = AtomicI32::new(1);
  VALUE.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
/// The Vim buffer's status.
pub enum BufferStatus {
  INIT,    // After created.
  LOADING, // Loading text content from disk file.
  SAVING,  // Saving buffer content to disk file.
  SYNCED,  // Synced content with file system.
  CHANGED, // Buffer content has been modified.
}

#[derive(Debug)]
/// The Vim buffer, it is the in-memory texts mapping to the filesystem.
///
/// It contains several internal data:
/// 1. File name that associated with filesystem.
/// 2. File contents.
/// 3. File metadata.
///
/// To stable and avoid data racing issues, all file IO operations are made in pure blocking and
/// single-threading manner. And buffer also provide a set of APIs that serves as middle-level
/// primitives which can be used to implement high-level Vim ex commands, etc.
pub struct Buffer {
  id: BufferId,
  rope: Rope,
  options: BufferLocalOptions,
  filename: Option<PathBuf>,
  absolute_filename: Option<PathBuf>,
  metadata: Option<Metadata>,
  last_sync_time: Option<Instant>,
  // worker_send_to_master: Sender<WorkerToMasterMessage>,
}

pub type BufferArc = Arc<RwLock<Buffer>>;
pub type BufferWk = Weak<RwLock<Buffer>>;

impl Buffer {
  /// NOTE: This API should not be used to create new buffer, please use
  /// [`BuffersManager`](BuffersManager) APIs to manage buffer instances.
  pub fn _new(
    rope: Rope,
    options: BufferLocalOptions,
    filename: Option<PathBuf>,
    absolute_filename: Option<PathBuf>,
    metadata: Option<Metadata>,
    last_sync_time: Option<Instant>,
  ) -> Self {
    Self {
      id: next_buffer_id(),
      rope,
      options,
      filename,
      absolute_filename,
      metadata,
      last_sync_time,
    }
  }

  /// NOTE: This API should not be used to create new buffer, please use
  /// [`BuffersManager`](BuffersManager) APIs to manage buffer instances.
  pub fn _new_empty(options: BufferLocalOptions) -> Self {
    Self {
      id: next_buffer_id(),
      rope: Rope::new(),
      options,
      filename: None,
      absolute_filename: None,
      metadata: None,
      last_sync_time: None,
    }
  }

  pub fn to_arc(b: Buffer) -> BufferArc {
    Arc::new(RwLock::new(b))
  }

  pub fn id(&self) -> BufferId {
    self.id
  }

  pub fn filename(&self) -> &Option<PathBuf> {
    &self.filename
  }

  pub fn set_filename(&mut self, filename: Option<PathBuf>) {
    self.filename = filename;
  }

  pub fn absolute_filename(&self) -> &Option<PathBuf> {
    &self.absolute_filename
  }

  pub fn set_absolute_filename(&mut self, absolute_filename: Option<PathBuf>) {
    self.absolute_filename = absolute_filename;
  }

  pub fn metadata(&self) -> &Option<Metadata> {
    &self.metadata
  }

  pub fn set_metadata(&mut self, metadata: Option<Metadata>) {
    self.metadata = metadata;
  }

  pub fn last_sync_time(&self) -> &Option<Instant> {
    &self.last_sync_time
  }

  pub fn set_last_sync_time(&mut self, last_sync_time: Option<Instant>) {
    self.last_sync_time = last_sync_time;
  }

  // pub fn status(&self) -> BufferStatus {
  //   BufferStatus::INIT
  // }

  // pub fn worker_send_to_master(&self) -> &Sender<WorkerToMasterMessage> {
  //   &self.worker_send_to_master
  // }
}

// Unicode {
impl Buffer {
  /// Get the display width for a unicode `char`.
  pub fn char_width(&self, c: char) -> usize {
    if c.is_ascii_control() {
      let ac = AsciiChar::from_ascii(c).unwrap();
      match ac {
        AsciiChar::Tab => self.tab_stop() as usize,
        AsciiChar::LineFeed | AsciiChar::CarriageReturn => 0,
        _ => {
          let ascii_formatter = AsciiControlCodeFormatter::from(ac);
          format!("{}", ascii_formatter).len()
        }
      }
    } else {
      UnicodeWidthChar::width_cjk(c).unwrap()
    }
  }

  /// Get the printable cell symbol and its display width.
  pub fn char_symbol(&self, c: char) -> (CompactString, usize) {
    let width = self.char_width(c);
    if c.is_ascii_control() {
      let ac = AsciiChar::from_ascii(c).unwrap();
      match ac {
        AsciiChar::Tab => (
          CompactString::from(" ".repeat(self.tab_stop() as usize)),
          width,
        ),
        AsciiChar::LineFeed | AsciiChar::CarriageReturn => (CompactString::new(""), width),
        _ => {
          let ascii_formatter = AsciiControlCodeFormatter::from(ac);
          (CompactString::from(format!("{}", ascii_formatter)), width)
        }
      }
    } else {
      (CompactString::from(c.to_string()), width)
    }
  }

  /// Get the display width for a unicode `str`.
  pub fn str_width(&self, s: &str) -> usize {
    s.chars().map(|c| self.char_width(c)).sum()
  }

  /// Get the printable cell symbols and the display width for a unicode `str`.
  pub fn str_symbols(&self, s: &str) -> (CompactString, usize) {
    s.chars().map(|c| self.char_symbol(c)).fold(
      (CompactString::with_capacity(s.len()), 0_usize),
      |(mut init_symbol, init_width), (mut symbol, width)| {
        init_symbol.push_str(symbol.as_mut_str());
        (init_symbol, init_width + width)
      },
    )
  }
}
// Unicode }

// Rope {
impl Buffer {
  /// Alias to method [`Rope::get_line`](Rope::get_line).
  pub fn get_line(&self, line_idx: usize) -> Option<RopeSlice> {
    self.rope.get_line(line_idx)
  }

  /// Alias to method [`Rope::get_lines_at`](Rope::get_lines_at).
  pub fn get_lines_at(&self, line_idx: usize) -> Option<Lines> {
    self.rope.get_lines_at(line_idx)
  }

  /// Alias to method [`Rope::lines`](Rope::lines).
  pub fn lines(&self) -> Lines {
    self.rope.lines()
  }

  /// Alias to method [`Rope::write_to`](Rope::write_to).
  pub fn write_to<T: std::io::Write>(&self, writer: T) -> std::io::Result<()> {
    self.rope.write_to(writer)
  }

  /// Alias to method [`Rope::append`](Rope::append).
  pub fn append(&mut self, other: Rope) {
    self.rope.append(other)
  }
}
// Rope }

// Options {
impl Buffer {
  pub fn options(&self) -> &BufferLocalOptions {
    &self.options
  }

  pub fn set_options(&mut self, options: &BufferLocalOptions) {
    self.options = options.clone();
  }

  pub fn tab_stop(&self) -> u16 {
    self.options.tab_stop()
  }

  pub fn set_tab_stop(&mut self, value: u16) {
    self.options.set_tab_stop(value);
  }
}
// Options }

impl PartialEq for Buffer {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Eq for Buffer {}

#[derive(Debug, Clone)]
/// The manager for all normal (file) buffers.
///
/// NOTE: A buffer has its unique filepath (on filesystem), and there is at most 1 unnamed buffer.
pub struct BuffersManager {
  // Buffers collection
  buffers: BTreeMap<BufferId, BufferArc>,

  // Buffers maps by absolute file path.
  buffers_by_path: HashMap<Option<PathBuf>, BufferArc>,

  // Local options for buffers.
  local_options: BufferLocalOptions,
}

impl BuffersManager {
  pub fn new() -> Self {
    BuffersManager {
      buffers: BTreeMap::new(),
      buffers_by_path: HashMap::new(),
      local_options: BufferLocalOptions::default(),
    }
  }

  pub fn to_arc(b: BuffersManager) -> BuffersManagerArc {
    Arc::new(RwLock::new(b))
  }

  /// Open a file with a newly created buffer.
  ///
  /// The file name must be unique and not existed, there are two use cases:
  /// 1. If the file exists on filesystem, the buffer will read the file contents into buffer.
  /// 2. If the file doesn't exist, the buffer will be empty but only set the file name.
  ///
  /// Returns
  ///
  /// It returns the buffer ID if the buffer created successfully, also the reading operations must
  /// be successful if the file exists on filesystem.
  /// Otherwise it returns [`BufferErr`](crate::res::BufferErr) that indicates the error.
  ///
  /// Panics
  ///
  /// If the file name already exists.
  ///
  /// NOTE: This is a primitive API.
  pub fn new_file_buffer(&mut self, filename: &Path) -> IoResult<BufferId> {
    let abs_filename = match filename.absolutize() {
      Ok(abs_filename) => abs_filename.to_path_buf(),
      Err(e) => {
        debug!("Failed to absolutize filepath {:?}:{:?}", filename, e);
        return Err(e);
      }
    };

    assert!(!self
      .buffers_by_path
      .contains_key(&Some(abs_filename.clone())));

    let existed = match std::fs::exists(abs_filename.clone()) {
      Ok(existed) => existed,
      Err(e) => {
        debug!("Failed to detect file {:?}:{:?}", filename, e);
        return Err(e);
      }
    };

    let buf = if existed {
      match self.edit_file(filename, &abs_filename) {
        Ok(buf) => buf,
        Err(e) => {
          return Err(e);
        }
      }
    } else {
      Buffer::_new(
        Rope::new(),
        self.local_options().clone(),
        Some(filename.to_path_buf()),
        Some(abs_filename.clone()),
        None,
        None,
      )
    };

    let buf_id = buf.id();
    let buf = Buffer::to_arc(buf);
    self.buffers.insert(buf_id, buf.clone());
    self.buffers_by_path.insert(Some(abs_filename), buf);
    Ok(buf_id)
  }

  /// Create new empty buffer without file name.
  ///
  /// The file name of this buffer is empty, i.e. the buffer is unnamed.
  ///
  /// Returns
  ///
  /// It returns the buffer ID if there is no other unnamed buffers.
  ///
  /// Panics
  ///
  /// If there is already other unnamed buffers.
  ///
  /// NOTE: This is a primitive API.
  pub fn new_empty_buffer(&mut self) -> BufferId {
    assert!(!self.buffers_by_path.contains_key(&None));

    let buf = Buffer::_new(
      Rope::new(),
      self.local_options().clone(),
      None,
      None,
      None,
      None,
    );
    let buf_id = buf.id();
    let buf = Buffer::to_arc(buf);
    self.buffers.insert(buf_id, buf.clone());
    self.buffers_by_path.insert(None, buf);
    buf_id
  }
}

// Primitive APIs {

impl BuffersManager {
  fn into_rope(&self, buf: &[u8], bufsize: usize) -> Rope {
    let bufstr = self.into_str(buf, bufsize);
    let mut block = RopeBuilder::new();
    block.append(&bufstr.to_owned());
    block.finish()
  }

  fn into_str(&self, buf: &[u8], bufsize: usize) -> String {
    let fencoding = self.local_options().file_encoding();
    match fencoding {
      FileEncoding::Utf8 => String::from_utf8_lossy(&buf[0..bufsize]).into_owned(),
    }
  }

  // Implementation for [new_buffer_edit_file](new_buffer_edit_file).
  fn edit_file(&self, filename: &Path, absolute_filename: &Path) -> IoResult<Buffer> {
    match std::fs::File::open(filename) {
      Ok(fp) => {
        let metadata = match fp.metadata() {
          Ok(metadata) => metadata,
          Err(e) => {
            debug!("Failed to fetch metadata from file {:?}:{:?}", filename, e);
            return Err(e);
          }
        };
        let mut buf: Vec<u8> = Vec::new();
        let mut reader = std::io::BufReader::new(fp);
        let bytes = match reader.read_to_end(&mut buf) {
          Ok(bytes) => bytes,
          Err(e) => {
            debug!("Failed to read file {:?}:{:?}", filename, e);
            return Err(e);
          }
        };
        assert!(bytes == buf.len());

        Ok(Buffer::_new(
          self.into_rope(&buf, buf.len()),
          self.local_options().clone(),
          Some(filename.to_path_buf()),
          Some(absolute_filename.to_path_buf()),
          Some(metadata),
          Some(Instant::now()),
        ))
      }
      Err(e) => {
        debug!("Failed to open file {:?}:{:?}", filename, e);
        Err(e)
      }
    }
  }
}

// Primitive APIs }

// BTreeMap {
impl BuffersManager {
  pub fn is_empty(&self) -> bool {
    self.buffers.is_empty()
  }

  pub fn len(&self) -> usize {
    self.buffers.len()
  }

  pub fn remove(&mut self, id: &BufferId) -> Option<BufferArc> {
    self.buffers.remove(id)
  }

  pub fn get(&self, id: &BufferId) -> Option<&BufferArc> {
    self.buffers.get(id)
  }

  pub fn contains_key(&self, id: &BufferId) -> bool {
    self.buffers.contains_key(id)
  }

  pub fn keys(&self) -> BuffersManagerKeys {
    self.buffers.keys()
  }

  pub fn values(&self) -> BuffersManagerValues {
    self.buffers.values()
  }

  pub fn iter(&self) -> BuffersManagerIter {
    self.buffers.iter()
  }

  pub fn first_key_value(&self) -> Option<(&BufferId, &BufferArc)> {
    self.buffers.first_key_value()
  }

  pub fn last_key_value(&self) -> Option<(&BufferId, &BufferArc)> {
    self.buffers.last_key_value()
  }
}
// BTreeMap }

impl Default for BuffersManager {
  fn default() -> Self {
    BuffersManager::new()
  }
}

// Options {
impl BuffersManager {
  pub fn local_options(&self) -> &BufferLocalOptions {
    &self.local_options
  }

  pub fn set_local_options(&mut self, options: &BufferLocalOptions) {
    self.local_options = options.clone();
  }
}
// Options }

pub type BuffersManagerArc = Arc<RwLock<BuffersManager>>;
pub type BuffersManagerWk = Weak<RwLock<BuffersManager>>;
pub type BuffersManagerKeys<'a> = std::collections::btree_map::Keys<'a, BufferId, BufferArc>;
pub type BuffersManagerValues<'a> = std::collections::btree_map::Values<'a, BufferId, BufferArc>;
pub type BuffersManagerIter<'a> = std::collections::btree_map::Iter<'a, BufferId, BufferArc>;

#[cfg(test)]
mod tests {
  use super::*;
  // use std::fs::File;
  // use tempfile::tempfile;
  // use tokio::sync::mpsc::Receiver;

  // fn make_channel() -> (
  //   Sender<WorkerToMasterMessage>,
  //   Receiver<WorkerToMasterMessage>,
  // ) {
  //   tokio::sync::mpsc::channel(1)
  // }

  // #[test]
  // fn buffer_from1() {
  //   let (sender, _) = make_channel();
  //
  //   let r1 = Rope::from_str("Hello");
  //   let buf1 = Buffer::_from_rope(sender.clone(), r1);
  //   let tmp1 = tempfile().unwrap();
  //   buf1.write_to(tmp1).unwrap();
  //
  //   let r2 = Rope::from_reader(File::open("Cargo.toml").unwrap()).unwrap();
  //   let buf2 = Buffer::_from_rope(sender, r2);
  //   let tmp2 = tempfile().unwrap();
  //   buf2.write_to(tmp2).unwrap();
  // }
  //
  // #[test]
  // fn buffer_from2() {
  //   let (sender, _) = make_channel();
  //
  //   let mut builder1 = RopeBuilder::new();
  //   builder1.append("Hello");
  //   builder1.append("World");
  //   let buf1 = Buffer::_from_rope_builder(sender, builder1);
  //   let tmp1 = tempfile().unwrap();
  //   buf1.write_to(tmp1).unwrap();
  // }

  #[test]
  fn next_buffer_id1() {
    assert!(next_buffer_id() > 0);
  }

  // #[test]
  // fn buffer_unicode_width1() {
  //   let (sender, _) = make_channel();
  //
  //   let b1 = Buffer::_from_rope_builder(sender, RopeBuilder::new());
  //   assert_eq!(b1.char_width('A'), 1);
  //   assert_eq!(b1.char_symbol('A'), (CompactString::new("A"), 1));
  //   assert_eq!(b1.str_width("ABCDEFG"), 7);
  //   assert_eq!(
  //     b1.str_symbols("ABCDEFG"),
  //     (CompactString::new("ABCDEFG"), 7)
  //   );
  // }
}
