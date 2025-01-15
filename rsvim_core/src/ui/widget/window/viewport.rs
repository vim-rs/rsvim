//! Buffer viewport on a window.

use crate::buf::BufferWk;
use crate::cart::U16Rect;
//use crate::envar;
//use crate::rlock;
use crate::ui::widget::window::ViewportOptions;

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::{Arc, Weak};
// use tracing::trace;

pub mod sync;

#[derive(Debug, Clone)]
/// The row viewport in a buffer line.
pub struct RowViewport {
  start_dcol_idx: usize,
  end_dcol_idx: usize,
  start_char_idx: usize,
  end_char_idx: usize,
  char2dcolumns: BTreeMap<usize, (usize, usize)>,
}

impl RowViewport {
  /// Make new instance.
  pub fn new(
    dcol_idx_range: Range<usize>,
    char_idx_range: Range<usize>,
    char2dcolumns: &BTreeMap<usize, (usize, usize)>,
  ) -> Self {
    Self {
      start_dcol_idx: dcol_idx_range.start,
      end_dcol_idx: dcol_idx_range.end,
      start_char_idx: char_idx_range.start,
      end_char_idx: char_idx_range.end,
      char2dcolumns: char2dcolumns.clone(),
    }
  }

  /// Get the chars length (count) on the row of the line.
  pub fn chars_length(&self) -> usize {
    self.end_char_idx - self.start_char_idx
  }

  /// Get the chars display width on the row of the line.
  pub fn chars_width(&self) -> usize {
    self.end_dcol_idx - self.start_dcol_idx
  }

  /// Get start display column index (in the buffer) for current row, starts from 0.
  ///
  /// NOTE: For the term _**display column**_, please see [`Viewport`].
  pub fn start_dcol_idx(&self) -> usize {
    self.start_dcol_idx
  }

  /// Get end display column index (in the buffer) for current row.
  ///
  /// NOTE: The start and end indexes are left-inclusive and right-exclusive.
  pub fn end_dcol_idx(&self) -> usize {
    self.end_dcol_idx
  }

  /// First (fully displayed) char index in current row.
  ///
  /// NOTE: The start and end indexes are left-inclusive and right-exclusive.
  pub fn start_char_idx(&self) -> usize {
    self.start_char_idx
  }

  /// Get end (next to the fully displayed) char index in current row.
  ///
  /// NOTE:
  /// The char index is based on the line of the buffer, not based on the whole buffer.
  /// The start and end indexes are left-inclusive and right-exclusive.
  pub fn end_char_idx(&self) -> usize {
    self.end_char_idx
  }

  /// Maps from each char index to its (start and end) display column indexes in current row,
  /// starts from 0. The value's 0 slot is `start_dcolumn`, 1 slot is `end_dcolumn`.
  pub fn char2dcolumns(&self) -> &BTreeMap<usize, (usize, usize)> {
    &self.char2dcolumns
  }
}

#[derive(Debug, Clone)]
/// The buffer line viewport in a buffer.
pub struct LineViewport {
  rows: BTreeMap<u16, RowViewport>,
  start_filled_columns: usize,
  end_filled_columns: usize,
}

impl LineViewport {
  /// Make new instance.
  pub fn new(
    rows: BTreeMap<u16, RowViewport>,
    start_filled_columns: usize,
    end_filled_columns: usize,
  ) -> Self {
    Self {
      rows,
      start_filled_columns,
      end_filled_columns,
    }
  }

  /// Maps from row index (based on the window) to a row in the buffer line, starts from 0.
  pub fn rows(&self) -> &BTreeMap<u16, RowViewport> {
    &self.rows
  }

  /// Get extra filled columns at the beginning of the line.
  ///
  /// For most cases, this value should be zero. But when the first char (indicate by
  /// `start_char_idx`) doesn't show at the first column of the row, and meanwhile the cells width
  /// is not enough for the previous character.
  ///
  /// For example:
  ///
  /// ```text
  ///              Column index in viewport -> 0   3
  ///                                          |   |
  /// 0         10        20        30    36   40  |  <- Column index in the buffer
  /// |         |         |         |     |    |   |
  /// 0         10        20        30    36   |   37  <- Char index in the buffer
  /// |         |         |         |     |    |   |
  ///                                         |---------------------|
  /// This is the beginning of the buffer.<--H|T-->But it begins to |show at here.
  /// The second line is really short!        |                     |
  /// Too short to show in viewport, luckily t|he third line is ok. |
  ///                                         |---------------------|
  /// ```
  ///
  /// The example shows the first char `B` starts at column index 3 in the viewport, and its
  /// previous char `<--HT-->` uses 8 cells width so cannot fully shows in the viewport.
  ///
  /// In this case, the variable `start_filled_columns` is 4, `start_dcolumn` is 40,
  /// `start_char_idx` is 37.
  pub fn start_filled_columns(&self) -> usize {
    self.start_filled_columns
  }

  /// Get extra filled columns at the end of the row, see:
  /// [`start_filled_columns`](LineViewport::start_filled_columns).
  pub fn end_filled_columns(&self) -> usize {
    self.end_filled_columns
  }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// The cursor viewport to maintain the positions.
///
/// As explained in [`Viewport`], ASCII control codes and other unicode chars can use 0 or more
/// cells when displayed in terminal, thus when cursor moves on the window/buffer, it needs to
/// always stay on the left most cell of a unicode char. Thus this viewport maintains the cursor
/// positions by taking consideration of both terminal cell position and buffer unicode char
/// position.
///
/// NOTE: It is not a must that a window/buffer has a cursor inside it. But once it has, we will
/// always maintain this position information for it.
pub struct CursorViewport {
  // Display column index.
  start_dcol_idx: usize,
  end_dcol_idx: usize,
  // Char index.
  char_idx: usize,
  // Row index.
  row_idx: u16,
  // Line index.
  line_idx: usize,
}

impl CursorViewport {
  /// Make new instance.
  pub fn new(dcol_idx_range: Range<usize>, char_idx: usize, row_idx: u16, line_idx: usize) -> Self {
    Self {
      start_dcol_idx: dcol_idx_range.start,
      end_dcol_idx: dcol_idx_range.end,
      char_idx,
      row_idx,
      line_idx,
    }
  }

  /// Get start display column index, starts from 0.
  pub fn start_dcol_idx(&self) -> usize {
    self.start_dcol_idx
  }

  /// Get end display column index, starts from 0.
  pub fn end_dcol_idx(&self) -> usize {
    self.end_dcol_idx
  }

  /// Get char index, starts from 0.
  pub fn char_idx(&self) -> usize {
    self.char_idx
  }

  /// Get the row index, starts from 0.
  pub fn row_idx(&self) -> u16 {
    self.row_idx
  }

  /// Get the line index, starts from 0.
  pub fn line_idx(&self) -> usize {
    self.line_idx
  }
}

#[derive(Debug, Clone)]
// spellchecker:off
/// The viewport for a buffer.
///
/// There are several factors affecting the final display effects when a window showing a buffer:
///
/// 1. Window (display) options (such as ['wrap'](crate::defaults::win),
///    ['line-break'](crate::defaults::win)), they decide how does the line in the buffer renders
///    in the window.
/// 2. ASCII control codes and unicode, they decide how does a character renders in the window.
///
/// ## Case-1: Line-wrap and word-wrap
///
/// ### Case-1.1: Wrap is disabled
///
/// When 'wrap' option is `false`, and there's one very long line which is longer than the width
/// of the window. The viewport has to truncate the line, and only shows part of it. For example:
///
/// Example-1
///
/// ```text
/// |-------------------------------------|
/// |This is the beginning of the very lon|g line, which only shows the beginning part.
/// |-------------------------------------|
/// ```
///
/// Example-2
///
/// ```text
///                                           |---------------------------------------|
/// This is the beginning of the very long lin|e, which only shows the beginning part.|
/// This is the short line, it's not shown.   |                                       |
/// This is the second very long line, which s|till shows in the viewport.            |
///                                           |---------------------------------------|
/// ```
///
/// Example-1 only shows the beginning of the line, and example-2 only shows the ending of the
/// line (with 'wrap' option is `false`, the second line is too short to show in viewport).
///
/// ### Case-1.2: Wrap is enabled
///
/// When 'wrap' option is `true`, the long line will take multiple rows and try to use the whole
/// window to render all of it, while still been truncated if it's just too long to show within the
/// whole window. For example:
///
/// Example-3
///
/// ```text
/// |-------------------------------------|
/// |This is the beginning of the very lon|
/// |g line, which only shows the beginnin|
/// |g part.                              |
/// |-------------------------------------|
/// ```
///
/// Example-4
///
/// ```text
///  This is the beginning of the very lon
/// |-------------------------------------|
/// |g line, which only shows the beginnin|
/// |g part? No, even it sets `wrap=true`,|
/// | it is still been truncated because t|
/// |-------------------------------------|
///  he whole window cannot render it.
/// ```
///
/// Example-3 shows `wrap=true` can help a window renders the very long line if the window itself
/// is big enough. And example-4 shows the very long line will still be truncated if it's too long.
///
/// ## Case-2: ASCII control codes and unicodes
///
/// Most characters use 1 cell width in the terminal, such as alphabets (A-Z), numbers (0-9) and
/// punctuations. But ASCII control codes can use 2 or more cells width. For example:
///
/// ### Case-2.1: ASCII control codes
///
/// Example-5
///
/// ```text
/// ^@^A^B^C^D^E^F^G^H<--HT-->
/// ^K^L^M^N^O^P^Q^R^S^T^U^V^W^X^Y^Z^[^\^]^^^_
/// ```
///
/// Example-5 shows how ASCII control codes render in Vim. Most of them use 2 cells width, except
/// horizontal tab (HT, 9) renders as 8 spaces (shows as `<--HT-->` in the example), and line feed
/// (LF, 10) renders as empty, i.e. 0 cell width, it just starts a new line.
///
/// Some unicode, especially Chinese/Japanese/Korean, can use 2 cells width as well. For example:
///
/// Example-6
///
/// ```text
/// 你好，Vim！
/// こんにちは、Vim！
/// 안녕 Vim!
/// ```
///
/// ### Case-2.2: Unicode and CJK
///
/// ## Case-3: All together
///
/// Taking all above scenarios into consideration, when rendering a buffer in a viewport, some edge
/// cases are:
///
/// Example-7
///
/// ```text
///                                  31(LF)
/// 0  3 4                   24    30|
/// |  | |                   |      ||
///     |---------------------|
/// <--H|T-->This is the first| line.                 <- Line-1
/// This| is the second line. |                       <- Line-2
/// This| is the third line, 它 有一点点长。          <- Line-3
///     |---------------------|
/// |  |                     | |          |||
/// 0  3                     24|         36||
///                            25         37|
///                                         38(LF)
/// ```
///
/// Example-7 shows a use case with 'wrap' option is `false`, at the beginning of the first line in
/// the viewport, the horizontal tab (`<--HT-->`, use 8 cells width) cannot been fully rendered in
/// viewport. And at the end of the last line in the viewport, the Chinese character (`它`, use 2
/// cells width) also cannot been fully rendered in viewport.
///
/// For the _**display column**_ in the buffer, it's based on the display width of the unicode
/// character, not the char index in the buffer. In example-7, line-1, the start display column is
/// 4 (the `T` in the `<--HT-->`, inclusive), the end display column is 25 (the ` ` whitespace
/// after the `t`, exclusive). In line-3, the start display column is 4 (the ` ` whitespace after
/// the `s`, inclusive), the end display column is 25 (the right half part in the `它` character,
/// exclusive).
///
/// Another hidden rule is:
/// 1. When 'wrap' option is `false`, there can be multiple lines start from non-zero columns in
///    the buffer, or end at non-ending position of the line. Just like example-7, there are 3
///    lines and they all start from column 4, and for line-1 and line-3 they don't end at their
///    ending position of the line.
/// 2. When 'wrap' option is `true`, there will be at most 1 line start from non-zero columns, or
///    end at non-ending position of the line in the buffer. For example:
///
/// Example-8
///
/// ```text
/// 0  3 4  <- Column index in the line of the buffer.
/// |  | |
///     |---------------------|
/// <--H|T-->This is the first|
///     | line. It is quite lo|
///     |ng and even cannot <-|-HT--> be fully rendered in viewport.
///     |---------------------|
///      |                 ||
///      46               64|   <- Column index in the line of the buffer.
///                         65
/// ```
///
/// The example-8 shows a very long line that cannot be fully rendered in the viewport, both start
/// and ending parts are been truncated. But there will not be 2 lines that are both truncated,
/// because with 'wrap' options is `true`, if one line is too long to render, only the one line
/// will show in the viewport. If one line is not too lone and there could be another line, then at
/// least for the first line, it can be fully rendered in the viewport, and it must starts from the
/// first char of the line (as 'wrap' option requires this rendering behavior).
///
/// But what if there're characters cannot been rendered in 2nd row end in example-8? for example:
///
/// Example-9
///
/// ```text
/// 0  3 4  <- Column index in the line of the buffer.
/// |  | |
///     |---------------------|
/// <--H|T-->This is the first|
///     | line. It is quite<--|
///     |HT-->long and even ca|nnot <--HT--> be fully rendered in viewport.
///     |---------------------|
///      |    |              |
///      43   44             59   <- Column index in the line of the buffer.
/// ```
///
/// The above example is a variant from example-8, in the 2nd row end, the tab (`<--HT-->`, uses 8
/// cells width) char cannot be fully rendered. Vim doesn't allow such cases, i.e. when 'wrap'
/// option is `true`, the non-fully rendering only happens at the beginning (top left corner) of
/// viewport, and at the end (bottom right corner) of viewport. It cannot happens inside the
/// viewport. Vim forces the tab char to render in the next row. When 'wrap' option is `false`,
/// each row handles exactly one line on their own, including the non-fully rendering.
///
/// Example-10
///
/// ```text
/// 0  3 4  <- Column index in the line of the buffer.
/// |  | |
///     |---------------------|
/// <--H|T-->This is the first|
///     | line. It is quite___|
///     |<--HT-->long and even| cannot <--HT--> be fully rendered in viewport.
///     |---------------------|
///      |       |           |
///      43      44          56   <- Column index in the line of the buffer.
/// ```
///
/// The above example is a variant from example-9, when 'wrap' is `true`, in the 2nd row end, the 3
/// cells cannot place the tab char, so move it to the next row.
///
/// Finally, some most important anchors (in a viewport) are:
///
/// - `start_line`: The start line (inclusive) of the buffer, it is the first line shows at the top
///   row of the viewport.
/// - `start_dcolumn`: The start display column (inclusive) of the buffer, it is the the first cell
///   of a line displayed in the viewport.
/// - `start_filled_columns`: The filled columns at the beginning of the row in the viewport, it is
///   only useful when the first char in a line doesn't show at the first column of the top row in
///   the viewport (because the previous char cannot be fully placed within these cells).
/// - `end_line`: The end line (exclusive) of the buffer, it is next to the last line at the bottom
///   row of the viewport.
/// - `end_dcolumn`: The end display column (exclusive) of the buffer, it is next to the last cell
///   of a line displayed in the viewport.
/// - `end_filled_columns`: The filled columns at the end of the row in the viewport, it is only
///   useful when the last char in a line doesn't show at the last column at the bottom row in the
///   viewport (because the following char cannot be fully placed within these cells).
///
/// NOTE: The _**display column**_ in the buffer is the characters displayed column index, not the
/// char index of the buffer, not the cell column of the viewport/window. It's named `dcolumn`
/// (short for `displayed_column`).
///
/// When rendering a buffer, viewport will need to go through each lines and characters in the
/// buffer to ensure how it display. It can starts from 4 corners:
///
/// 1. Start from top left corner.
/// 2. Start from top right corner.
/// 3. Start from bottom left corner.
/// 4. Start from bottom right corner.
// spellchecker:on
#[allow(dead_code)]
pub struct Viewport {
  // Options.
  options: ViewportOptions,

  // Buffer.
  buffer: BufferWk,

  // Actual shape.
  actual_shape: U16Rect,

  // Start line index in the buffer, starts from 0.
  start_line_idx: usize,

  // End line index in the buffer.
  end_line_idx: usize,

  // Maps from buffer line index to its displayed rows in the window.
  lines: BTreeMap<usize, LineViewport>,

  // Cursor position (if has).
  cursor: CursorViewport,
}

pub type ViewportArc = Arc<RwLock<Viewport>>;
pub type ViewportWk = Weak<RwLock<Viewport>>;

impl Viewport {
  /// Make new instance.
  pub fn new(options: &ViewportOptions, buffer: BufferWk, actual_shape: &U16Rect) -> Self {
    // By default the viewport start from the first line, i.e. starts from 0.
    let (line_idx_range, lines) = sync::from_top_left(options, buffer.clone(), actual_shape, 0, 0);
    let cursor = if line_idx_range.is_empty() {
      assert!(lines.is_empty());
      CursorViewport::new(0..1, 0, 0, 0)
    } else {
      assert!(!lines.is_empty());
      // trace!(
      //   "lines.len:{:?} line_range.len:{:?}",
      //   lines.len(),
      //   line_range.len()
      // );
      assert!(lines.len() == line_idx_range.len());
      assert!(lines.first_key_value().is_some());
      assert!(lines.last_key_value().is_some());
      // trace!(
      //   "lines.first_key_value:{:?} line_range.start_line:{:?}",
      //   lines.first_key_value().unwrap(),
      //   line_range.start_line()
      // );
      assert!(*lines.first_key_value().unwrap().0 == line_idx_range.start_line_idx());
      // trace!(
      //   "lines.last_key_value:{:?} line_range.end_line:{:?}",
      //   lines.last_key_value().unwrap(),
      //   line_range.end_line()
      // );
      assert!(line_idx_range.end_line_idx() > 0);
      assert!(*lines.last_key_value().unwrap().0 == line_idx_range.end_line_idx() - 1);
      let first_line = lines.first_key_value().unwrap();
      let line_idx = *first_line.0;
      let first_line = first_line.1;
      if first_line.rows().is_empty() {
        CursorViewport::new(0..1, 0, 0, 0)
      } else {
        let first_row = first_line.rows().first_key_value().unwrap();
        let row_idx = *first_row.0;
        let first_row = first_row.1;
        let char_idx = first_row.start_char_idx();
        let (start_dcolumn, end_dcolumn) = first_row.char2dcolumns().get(&char_idx).unwrap();
        CursorViewport::new(*start_dcolumn..*end_dcolumn, char_idx, row_idx, line_idx)
      }
    };

    Viewport {
      options: *options,
      buffer,
      actual_shape: *actual_shape,
      start_line_idx: line_idx_range.start_line_idx(),
      end_line_idx: line_idx_range.end_line_idx(),
      lines,
      cursor,
    }
  }

  /// Convert struct to Arc pointer.
  pub fn to_arc(v: Viewport) -> ViewportArc {
    Arc::new(RwLock::new(v))
  }

  #[cfg(not(debug_assertions))]
  fn _internal_check(&self) {}

  #[cfg(debug_assertions)]
  fn _internal_check(&self) {
    assert_eq!(
      self.end_line_idx <= self.start_line_idx,
      self.lines.is_empty()
    );
    assert!(self.lines.first_key_value().is_some());
    assert_eq!(
      *self.lines.first_key_value().unwrap().0,
      self.start_line_idx
    );
    assert!(self.lines.last_key_value().is_some());
    assert_eq!(
      *self.lines.last_key_value().unwrap().0,
      self.end_line_idx - 1
    );
    let mut last_line_idx: Option<usize> = None;
    let mut last_row_idx: Option<u16> = None;
    for (line_idx, line_viewport) in self.lines.iter() {
      match last_line_idx {
        Some(last_line_idx1) => assert_eq!(last_line_idx1 + 1, *line_idx),
        None => { /* Skip */ }
      }
      last_line_idx = Some(*line_idx);
      let mut last_char_idx: Option<usize> = None;
      let mut last_dcolumn_idx: Option<usize> = None;
      for (row_idx, row_viewport) in line_viewport.rows() {
        match last_row_idx {
          Some(last_row_idx1) => assert_eq!(last_row_idx1 + 1, *row_idx),
          None => { /* Skip */ }
        }
        last_row_idx = Some(*row_idx);
        assert!(row_viewport.char2dcolumns().first_key_value().is_some());
        assert_eq!(
          *row_viewport.char2dcolumns().first_key_value().unwrap().0,
          row_viewport.start_char_idx()
        );
        assert_eq!(
          row_viewport.char2dcolumns().first_key_value().unwrap().1 .0,
          row_viewport.start_dcol_idx()
        );
        assert!(row_viewport.char2dcolumns().last_key_value().is_some());
        assert_eq!(
          *row_viewport.char2dcolumns().last_key_value().unwrap().0,
          row_viewport.end_char_idx() - 1
        );
        assert_eq!(
          row_viewport.char2dcolumns().last_key_value().unwrap().1 .1,
          row_viewport.end_dcol_idx()
        );
        for (char_idx, dcolumn_idx) in row_viewport.char2dcolumns().iter() {
          match last_char_idx {
            Some(last_char_idx1) => assert_eq!(last_char_idx1 + 1, *char_idx),
            None => { /* Skip */ }
          }
          last_char_idx = Some(*char_idx);
          match last_dcolumn_idx {
            Some(last_dcolumn_idx1) => {
              assert_eq!(dcolumn_idx.0, last_dcolumn_idx1);
            }
            None => { /* Skip */ }
          }
          last_dcolumn_idx = Some(dcolumn_idx.1);
        }
      }
    }
  }

  /// Get start line index in the buffer, starts from 0.
  pub fn start_line_idx(&self) -> usize {
    self._internal_check();
    self.start_line_idx
  }

  /// Get end line index in the buffer.
  pub fn end_line_idx(&self) -> usize {
    self._internal_check();
    self.end_line_idx
  }

  /// Get viewport information by lines.
  pub fn lines(&self) -> &BTreeMap<usize, LineViewport> {
    self._internal_check();
    &self.lines
  }

  /// Whether viewport is empty.
  pub fn is_empty(&self) -> bool {
    self._internal_check();
    self.lines.is_empty()
  }

  /// Get cursor viewport information.
  pub fn cursor(&self) -> &CursorViewport {
    self._internal_check();
    &self.cursor
  }

  /// Set cursor viewport information.
  pub fn set_cursor(&mut self, cursor: CursorViewport) {
    self.cursor = cursor;
  }

  /// Sync from top-left corner, i.e. `start_line` and `start_dcolumn`.
  pub fn sync_from_top_left(&mut self, start_line: usize, start_dcolumn: usize) {
    let (line_idx_range, lines) = sync::from_top_left(
      &self.options,
      self.buffer.clone(),
      &self.actual_shape,
      start_line,
      start_dcolumn,
    );
    self.start_line_idx = line_idx_range.start_line_idx();
    self.end_line_idx = line_idx_range.end_line_idx();
    self.lines = lines;
  }
}

//#[derive(Debug, Clone, Copy)]
// /// The vertical offset for viewport/cursor move up/down.
//pub enum ViewportVerticalOffset {
//  Up(usize),
//  Down(usize),
//}
//
//#[derive(Debug, Clone, Copy)]
// /// The horizontal offset for viewport/cursor move left/right.
//pub enum ViewportHorizontalOffset {
//  Left(usize),
//  Right(usize),
//}
//
// // Cursor Movement {
//impl Viewport {
//  /// Detect whether current viewport contains a specific line (vertical).
//  pub fn contains_line(&self, line_idx: usize) -> bool {
//    if line_idx < self.start_line_idx() {
//      false
//    } else if line_idx >= self.end_line_idx() {
//      false
//    } else {
//      true
//    }
//  }
//
//  /// Calculate how to move the viewport so that it can contain the specific line.
//  ///
//  /// It returns the start line index for the viewport, which indicates the viewport that starts
//  /// from this line index can contain the specific line, and the movement of the viewport is
//  /// minimized.
//  ///
//  /// It doesn't panic if the line doesn't exist in the buffer, but it try to move to it.
//  ///
//  /// # Returns
//  ///
//  /// It returns `None` if the specific line is already inside current viewport, i.e. the viewport
//  /// doesn't need to move.
//  ///
//  /// It returns the start line index which indicates the new viewport should start from.
//  pub fn vertical_displacement(&self, line_idx: usize) -> Option<usize> {
//    if line_idx < self.start_line_idx() {
//      Some(line_idx)
//    } else if line_idx >= self.end_line_idx() {
//      let height = self.actual_shape.height() as usize;
//      let buf_lines_len = rlock!(self.buffer.upgrade().unwrap()).len_lines();
//      let line_idx = std::cmp::min(buf_lines_len, line_idx);
//      if line_idx >= height {
//        Some(line_idx - height)
//      } else {
//        Some(0_usize)
//      }
//    } else {
//      None
//    }
//  }
//
//  /// Detect whether current viewport contains a specific char (horizontal).
//  pub fn contains_char(&self, char_idx: usize) -> bool {
//    if char_idx < self.start_line_idx() {
//      false
//    } else if char_idx >= self.end_line_idx() {
//      false
//    } else {
//      true
//    }
//  }
//
//  /// Calculate how to move the viewport so that it can contain the specific line.
//  ///
//  /// It doesn't panic if the line doesn't exist in the buffer, but it try to move to it.
//  ///
//  /// # Returns
//  ///
//  /// It returns None if the line is already inside current viewport, i.e. the viewport
//  /// doesn't need to move.
//  ///
//  /// It returns a offset, which indicates the movement is either up or down.
//  pub fn get_horizontal_displacement(&self, line_idx: usize) -> Option<ViewportHorizontalOffset> {
//    if line_idx < self.start_line_idx() {
//      Some(ViewportHorizontalOffset::Left(
//        self.start_line_idx() - line_idx,
//      ))
//    } else if line_idx >= self.end_line_idx() {
//      Some(ViewportHorizontalOffset::Right(
//        line_idx - self.end_line_idx(),
//      ))
//    } else {
//      None
//    }
//  }
//
//  /// Get relative location based on current cursor viewport, this is useful when implementing
//  /// cursor movement.
//  ///
//  /// There are several scenarios when moving cursor:
//  /// - The cursor is still inside of current viewport after movement.
//  /// - The cursor goes outside of current viewport after movement. Thus the viewport needs to be
//  ///   adjusted as well, and cursor actually remains at the same position (based on terminal).
//  pub fn cursor_relative_position(&self, offset: ViewportVerticalOffset) -> CursorViewport {
//    // If current viewport is empty, don't move.
//    if self.is_empty() {
//      return self.cursor;
//    }
//
//    let cur = &self.cursor;
//    match offset {
//      ViewportVerticalOffset::Up(n) => {
//        let n = n as usize;
//        // Inside.
//        if cur.line_idx() >= self.start_line_idx() + n {
//          let line_idx = cur.line_idx() - n;
//          self._cursor_vertical_relative_position(line_idx)
//        } else {
//          // Outside.
//          unimplemented!();
//        }
//      }
//      ViewportVerticalOffset::Down(n) => {
//        let n = n as usize;
//        // Inside.
//        if cur.line_idx() + n < self.end_line_idx() {
//          let line_idx = cur.line_idx() + n;
//          self._cursor_vertical_relative_position(line_idx)
//        } else {
//          // Outside.
//          unimplemented!();
//        }
//      }
//      ViewportVerticalOffset::Left(n) => {
//        let n = n as usize;
//        // Inside.
//        if cur.line_idx() >= self.start_line_idx() + n {
//          let line_idx = cur.line_idx() - n;
//          let line_viewport = self.lines.get(&line_idx).unwrap();
//          for (row, row_viewport) in line_viewport.rows().iter() {
//            if row_viewport.start_char_idx() >= cur.char_idx()
//              && row_viewport.end_char_idx() < cur.char_idx()
//            {
//              let row_idx = *row;
//              let char_idx = cur.char_idx();
//              let dcol = row_viewport.char2dcolumns().get(&char_idx).unwrap();
//              let start_dcol_idx = dcol.0;
//              let end_dcol_idx = dcol.1;
//              return CursorViewport::new(
//                start_dcol_idx..end_dcol_idx,
//                char_idx,
//                row_idx,
//                line_idx,
//              );
//            }
//          }
//          unreachable!("Failed to find the char_idx for CursorViewport");
//        } else {
//          // Outside.
//          unimplemented!();
//        }
//      }
//      ViewportVerticalOffset::Right(n) => {
//        let n = n as usize;
//        // Inside.
//        if cur.line_idx() >= self.start_line_idx() + n {
//          let line_idx = cur.line_idx() - n;
//          let line_viewport = self.lines.get(&line_idx).unwrap();
//          for (row, row_viewport) in line_viewport.rows().iter() {
//            if row_viewport.start_char_idx() >= cur.char_idx()
//              && row_viewport.end_char_idx() < cur.char_idx()
//            {
//              let row_idx = *row;
//              let char_idx = cur.char_idx();
//              let dcol = row_viewport.char2dcolumns().get(&char_idx).unwrap();
//              let start_dcol_idx = dcol.0;
//              let end_dcol_idx = dcol.1;
//              return CursorViewport::new(
//                start_dcol_idx..end_dcol_idx,
//                char_idx,
//                row_idx,
//                line_idx,
//              );
//            }
//          }
//          unreachable!("Failed to find the char_idx for CursorViewport");
//        } else {
//          // Outside.
//          unimplemented!();
//        }
//      }
//    }
//  }
//
//  fn _cursor_vertical_relative_position(&self, line_idx: usize) -> CursorViewport {
//    let cur = &self.cursor;
//    let line_viewport = self.lines.get(&line_idx).unwrap();
//    for (row, row_viewport) in line_viewport.rows().iter() {
//      if row_viewport.start_char_idx() >= cur.char_idx()
//        && row_viewport.end_char_idx() < cur.char_idx()
//      {
//        let row_idx = *row;
//        let char_idx = cur.char_idx();
//        let dcol = row_viewport.char2dcolumns().get(&char_idx).unwrap();
//        let start_dcol_idx = dcol.0;
//        let end_dcol_idx = dcol.1;
//        return CursorViewport::new(start_dcol_idx..end_dcol_idx, char_idx, row_idx, line_idx);
//      }
//    }
//    unreachable!("Failed to find vertical relative position for CursorViewport")
//  }
//
//  fn _cursor_horizontal_relative_position(&self, line_idx: usize) -> CursorViewport {
//    let cur = &self.cursor;
//    let line_viewport = self.lines.get(&line_idx).unwrap();
//    for (row, row_viewport) in line_viewport.rows().iter() {
//      if row_viewport.start_char_idx() >= cur.char_idx()
//        && row_viewport.end_char_idx() < cur.char_idx()
//      {
//        let row_idx = *row;
//        let char_idx = cur.char_idx();
//        let dcol = row_viewport.char2dcolumns().get(&char_idx).unwrap();
//        let start_dcol_idx = dcol.0;
//        let end_dcol_idx = dcol.1;
//        return CursorViewport::new(start_dcol_idx..end_dcol_idx, char_idx, row_idx, line_idx);
//      }
//    }
//    unreachable!("Failed to find vertical relative position for CursorViewport")
//  }
//}
// // Cursor Movement }

impl Viewport {
  /// Get options.
  pub fn options(&self) -> &ViewportOptions {
    &self.options
  }

  /// Set options.
  pub fn set_options(&mut self, options: &ViewportOptions) {
    self.options = *options;
  }

  /// Get buffer.
  pub fn buffer(&self) -> BufferWk {
    self.buffer.clone()
  }

  /// Set buffer.
  pub fn set_buffer(&mut self, buffer: BufferWk) {
    self.buffer = buffer;
  }

  /// Get actual shape.
  pub fn actual_shape(&self) -> &U16Rect {
    &self.actual_shape
  }

  /// Set actual shape.
  pub fn set_actual_shape(&mut self, actual_shape: &U16Rect) {
    self.actual_shape = *actual_shape;
  }
}

// spellchecker:off
#[allow(unused_imports)]
#[cfg(test)]
mod tests {
  use super::*;

  use crate::buf::BufferArc;
  use crate::cart::{IRect, U16Size};
  use crate::envar;
  use crate::rlock;
  use crate::test::buf::{make_buffer_from_lines, make_empty_buffer};
  #[allow(dead_code)]
  use crate::test::log::init as test_log_init;
  use crate::ui::tree::internal::Inodeable;
  use crate::ui::tree::Tree;
  use crate::ui::widget::window::{Window, WindowLocalOptions};

  use compact_str::ToCompactString;
  use ropey::{Rope, RopeBuilder};
  use std::fs::File;
  use std::io::{BufReader, BufWriter};
  use std::sync::Arc;
  use std::sync::Once;
  use tracing::info;

  fn make_viewport_from_size(
    size: U16Size,
    buffer: BufferArc,
    window_options: &WindowLocalOptions,
  ) -> Viewport {
    let mut tree = Tree::new(size);
    tree.set_local_options(window_options);
    let window_shape = IRect::new((0, 0), (size.width() as isize, size.height() as isize));
    let window = Window::new(window_shape, Arc::downgrade(&buffer), tree.local_options());
    rlock!(window.viewport()).clone()
  }

  #[allow(clippy::too_many_arguments)]
  fn ensure_sync_from_top_left(
    buffer: BufferArc,
    actual: &Viewport,
    expect: &Vec<&str>,
    expect_start_line: usize,
    expect_end_line: usize,
    expect_start_fills: &BTreeMap<usize, usize>,
    expect_end_fills: &BTreeMap<usize, usize>,
  ) {
    info!(
      "actual start_line/end_line:{:?}/{:?}",
      actual.start_line_idx(),
      actual.end_line_idx()
    );
    for (k, v) in actual.lines().iter() {
      info!("actual-{:?}: {:?}", k, v);
    }
    info!("expect:{:?}", expect);

    assert_eq!(actual.start_line_idx(), expect_start_line);
    assert_eq!(actual.end_line_idx(), expect_end_line);
    if actual.lines().is_empty() {
      assert!(actual.end_line_idx() <= actual.start_line_idx());
    } else {
      let (first_line_idx, _first_line_viewport) = actual.lines().first_key_value().unwrap();
      let (last_line_idx, _last_line_viewport) = actual.lines().last_key_value().unwrap();
      assert_eq!(*first_line_idx, actual.start_line_idx());
      assert_eq!(*last_line_idx, actual.end_line_idx() - 1);
    }
    assert_eq!(
      actual.end_line_idx() - actual.start_line_idx(),
      actual.lines().len()
    );
    assert_eq!(
      actual.end_line_idx() - actual.start_line_idx(),
      expect_start_fills.len()
    );
    assert_eq!(
      actual.end_line_idx() - actual.start_line_idx(),
      expect_end_fills.len()
    );

    let buffer = buffer.read();
    let buflines = buffer.get_lines_at(actual.start_line_idx()).unwrap();
    let total_lines = expect_end_line - expect_start_line;

    for (l, line) in buflines.enumerate() {
      if l >= total_lines {
        break;
      }
      let actual_line_idx = l + expect_start_line;
      let line_viewport = actual.lines().get(&actual_line_idx).unwrap();

      info!(
        "l-{:?}, line_viewport:{:?}, actual_line_idx:{}, expect_start_fills:{:?}, expect_end_fills:{:?}",
        l, line_viewport, actual_line_idx, expect_start_fills, expect_end_fills
      );
      assert_eq!(
        line_viewport.start_filled_columns(),
        *expect_start_fills.get(&actual_line_idx).unwrap()
      );
      assert_eq!(
        line_viewport.end_filled_columns(),
        *expect_end_fills.get(&actual_line_idx).unwrap()
      );

      let rows = &line_viewport.rows();
      for (r, row) in rows.iter() {
        info!("r-{:?}, row:{:?}", r, row);
        assert_eq!(row.chars_length(), row.char2dcolumns().len());
        assert_eq!(
          row.start_char_idx(),
          *row.char2dcolumns().first_key_value().unwrap().0
        );
        assert_eq!(
          row.end_char_idx(),
          *row.char2dcolumns().last_key_value().unwrap().0 + 1
        );
        assert_eq!(
          row.start_dcol_idx(),
          row.char2dcolumns().first_key_value().unwrap().1 .0
        );
        assert_eq!(
          row.end_dcol_idx(),
          row.char2dcolumns().last_key_value().unwrap().1 .1
        );

        if r > rows.first_key_value().unwrap().0 {
          let prev_r = r - 1;
          let prev_row = rows.get(&prev_r).unwrap();
          info!(
            "row-{:?}, current row[{}]:{:?}, previous row[{}]:{:?}",
            r, r, row, prev_r, prev_row
          );
          assert_eq!(prev_row.end_dcol_idx(), row.start_dcol_idx());
        }
        if r < rows.last_key_value().unwrap().0 {
          let next_r = r + 1;
          let next_row = rows.get(&next_r).unwrap();
          info!(
            "row-{:?}, current row[{}]:{:?}, next row[{}]:{:?}",
            r, r, row, next_r, next_row
          );
          assert_eq!(next_row.start_dcol_idx(), row.end_dcol_idx());
        }

        let mut last_char_dcolumn: Option<usize> = None;
        let mut payload = String::new();
        for c_idx in row.start_char_idx()..row.end_char_idx() {
          let c = line.get_char(c_idx).unwrap();
          let c_width = buffer.char_width(c);
          let c_dcols = row.char2dcolumns().get(&c_idx).unwrap();
          assert_eq!(c_dcols.1 - c_dcols.0, c_width);
          if let Some(last_char_docl) = last_char_dcolumn {
            assert_eq!(last_char_docl, c_dcols.0);
          }
          payload.push(c);
          last_char_dcolumn = Some(c_dcols.1);
        }
        info!(
          "row-{:?}, actual:{:?}, expect:{:?}",
          r, payload, expect[*r as usize]
        );
        assert_eq!(payload, expect[*r as usize]);
        let total_width = payload.chars().map(|c| buffer.char_width(c)).sum::<usize>();
        assert_eq!(total_width, row.end_dcol_idx() - row.start_dcol_idx());
      }
    }
  }

  #[test]
  fn sync_from_top_left_nowrap1() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);

    let expect = vec![
      "Hello, RSV",
      "This is a ",
      "But still ",
      "  1. When ",
      "  2. When ",
      "     * The",
      "     * The",
      "",
    ];
    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![
      (0, 0),
      (1, 0),
      (2, 0),
      (3, 0),
      (4, 0),
      (5, 0),
      (6, 0),
      (7, 0),
    ]
    .into_iter()
    .collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      8,
      &expect_fills,
      &expect_fills,
    );
  }

  #[test]
  fn sync_from_top_left_nowrap2() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "But still it contains sever",
      "  1. When the line is small",
      "  2. When the line is too l",
      "     * The extra parts are ",
      "     * The extra parts are ",
      "",
    ];
    let size = U16Size::new(27, 15);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![
      (0, 0),
      (1, 0),
      (2, 0),
      (3, 0),
      (4, 0),
      (5, 0),
      (6, 0),
      (7, 0),
    ]
    .into_iter()
    .collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      8,
      &expect_fills,
      &expect_fills,
    );
  }

  #[test]
  fn sync_from_top_left_nowrap3() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and smal",
      "But still it contains several t",
      "  1. When the line is small eno",
      "  2. When the line is too long ",
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
      .into_iter()
      .collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      5,
      &expect_fills,
      &expect_fills,
    );
  }

  #[test]
  fn sync_from_top_left_nowrap4() {
    test_log_init();

    let buffer = make_empty_buffer();
    let expect = vec![""];

    let size = U16Size::new(20, 20);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      1,
      &expect_fills,
      &expect_fills,
    );
  }

  #[test]
  fn sync_from_top_left_nowrap5() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello,\tRSVIM!\n",
      "This\r",
      "is a quite\tsimple and small test lines.\n",
      "But still\\it\r",
      "contains\tseveral things we want to test:\n",
      "\t1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "\t2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "\t\t* The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "\t\t* The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello,", // 4 fills for '\t'
      "This\r",
      "is a quite",
      "But still\\",
      "contains", // 2 fills for '\t'
      "\t1.",
      "\t2.",
      "\t", // 2 fills for '\t'
      "\t", // 2 fills for '\t'
      "",
    ];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![
      (0, 0),
      (1, 0),
      (2, 0),
      (3, 0),
      (4, 0),
      (5, 0),
      (6, 0),
      (7, 0),
      (8, 0),
      (9, 0),
    ]
    .into_iter()
    .collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![
      (0, 4),
      (1, 0),
      (2, 0),
      (3, 0),
      (4, 2),
      (5, 0),
      (6, 0),
      (7, 2),
      (8, 2),
      (9, 0),
    ]
    .into_iter()
    .collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      10,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_nowrap6() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "你好，\tRSVIM！\n",
      "这是\ta quite 简单而且很小的测试文字内容行。\n",
      "But still\\it\t包含了好几种我们想测试的情况：\n",
      "\t1. 当那条线\tis small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line 特别长而无法完全 to put in a row of the window content widget, there're multiple cases:\n",
      "\t* The extra\tparts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "  * The extra parts\tare split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "你好，\tRSVIM！\n",
      "这是\ta quite 简单而",  // 1 fills for '且'
      "But still\\it\t包含了", // 1 fills for '好'
      "\t1. 当那条线\t",
      "  2. When the line 特别长而",
      "\t* The extra\t",
    ];

    let size = U16Size::new(27, 6);
    let options = WindowLocalOptions::builder().wrap(false).build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]
        .into_iter()
        .collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 1), (2, 1), (3, 0), (4, 0), (5, 0)]
        .into_iter()
        .collect();
    ensure_sync_from_top_left(
      buffer.clone(),
      &actual,
      &expect,
      0,
      6,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak1() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSV",
      "IM!\n",
      "This is a ",
      "quite simp",
      "le and sma",
      "ll test li",
      "nes.\n",
      "But still ",
      "it contain",
      "s several ",
    ];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    ensure_sync_from_top_left(buffer, &actual, &expect, 0, 3, &expect_fills, &expect_fills);
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak2() {
    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains sever",
      "al things we want to test:\n",
      "  1. When the line is small",
      " enough to completely put i",
      "nside a row of the window c",
      "ontent widget, then the lin",
      "e-wrap and word-wrap doesn'",
      "t affect the rendering.\n",
      "  2. When the line is too l",
      "ong to be completely put in",
      " a row of the window conten",
      "t widget, there're multiple",
      "",
    ];

    let size = U16Size::new(27, 15);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
      .into_iter()
      .collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
      .into_iter()
      .collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      5,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak3() {
    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and smal",
      "l test lines.\n",
      "But still it contains several t",
      "hings we want to test:\n",
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    ensure_sync_from_top_left(buffer, &actual, &expect, 0, 3, &expect_fills, &expect_fills);
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak4() {
    let buffer = make_empty_buffer();
    let expect = vec![""];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0)].into_iter().collect();
    ensure_sync_from_top_left(buffer, &actual, &expect, 0, 1, &expect_fills, &expect_fills);
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak5() {
    let buffer = make_buffer_from_lines(vec![
      "\t\t* The extra parts are\tsplit into the next\trow,\tif either line-wrap or word-wrap options are been set. If the extra\tparts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "\t\t* The extra par",
      "ts are\tsplit into the ne",
      "xt\trow,\tif either",
      " line-wrap or word-wrap options",
      " are been set. If the extra",
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 4)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      1,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak6() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "But still it contains several things we want to test:\n",
      "\t\t1. When\tthe line\tis small\tenough to\tcompletely put\tinside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
    ]);
    let expect = vec![
      "But still it contains several t",
      "hings we want to test:\n",
      "\t\t1. When\t",
      "the line\tis small",
      "\tenough to\tcomple",
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      2,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak7() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "But still it contains several things we want to test:\n",
      "\t\t1. When\tthe line\tis small\tenough\tto\tcompletely put\tinside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
    ]);
    let expect = vec![
      "But still it contains several t",
      "hings we want to test:\n",
      "\t\t1. When\t",
      "the line\tis small",
      "\tenough\tto", // 7 fills
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 7)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      2,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak8() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "但它仍然contains several things 我们想要测试的文字内容：\n",
      "\t第一，当一行文字内容太小了，然后可以完全的放进窗口的一行之中，那么行wrap和词wrap两个选项并不会影响渲染的最终效果。\n",
    ]);
    let expect = vec![
      "但它仍然contains several things",
      " 我们想要测试的文字内容：\n",
      "\t第一，当一行文字内容太",
      "小了，然后可以完全的放进窗口的",
      "一行之中，那么行wrap和词wrap两", // 1 fills
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      2,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_nolinebreak9() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "但它仍然contains several th\tings 我们想要测试的文字内容：\n",
      "\t第一，当一行文字内容太小了，然后可以完全的放进窗口的一行之中，那么行wrap和词wrap两个选项并不会影响渲染的最终效果。\n",
    ]);
    let expect = vec![
      "但它仍然contains several th",
      "\tings 我们想要测试的文字",
      "内容：\n",
      "\t第一，当一行文字内容太",
      "小了，然后可以完全的放进窗口的",
      "一行之中，那么行wrap和词wrap两", // 1 fills
    ];

    let size = U16Size::new(31, 5);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(false)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      2,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak1() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, ",
      "RSVIM!\n",
      "This is a ",
      "quite ",
      "simple and",
      " small ",
      "test lines",
      ".\n",
      "But still ",
      "it ",
    ];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      3,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak2() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is to\to long to be completely p\tut in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains ",
      "several things we want to ",
      "test:\n",
      "  1. When the line is small",
      " enough to completely put ",
      "inside a row of the window ",
      "content widget, then the ",
      "line-wrap and word-wrap ",
      "doesn't affect the ",
      "rendering.\n",
      "  2. When the line is to",
      "\to long to be ",
    ];

    let size = U16Size::new(27, 15);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
      .into_iter()
      .collect();
    let expect_end_fills: BTreeMap<usize, usize> = vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
      .into_iter()
      .collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      5,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak3() {
    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains several ",
      "things we want to test:\n",
      "  1. When the line is small ",
      "enough to completely put inside",
      " a row of the window content ",
      "widget, then the line-wrap and ",
      "word-wrap doesn't affect the ",
      "rendering.\n",
      "",
    ];

    let size = U16Size::new(31, 11);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak4() {
    test_log_init();

    let buffer = make_empty_buffer();
    let expect = vec![""];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_fills: BTreeMap<usize, usize> = vec![(0, 0)].into_iter().collect();
    ensure_sync_from_top_left(buffer, &actual, &expect, 0, 1, &expect_fills, &expect_fills);
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak5() {
    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains several ",
      "things we want to test:\n",
      "  1. When the line is small ",
      "enough to completely put inside",
      " a row of the window content ",
      "widget, then the line-wrap and ",
      "word-wrap doesn't affect the ",
    ];

    let size = U16Size::new(31, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak6() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "\t\t第一，当一行文本内容的长度足够短，短到可以完整的放入一个窗口（的一行）之中，那么基于行的换行和基于单词的换行两个选项都不会影响渲染的最终效果。\n",
      "\t\t第二，当一行内容文本的长度足够长，而无法放入窗口中，那么我们需要考虑很多种情况：\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains several ",
      "things we want to test:\n",
      "\t\t第一，当一行文",
      "本内容的长度足够短，短到可以完",
      "整的放入一个窗口（的一行）之中",
      "，那么基于行的换行和基于单词的",
      "换行两个选项都不会影响渲染的最",
    ];

    let size = U16Size::new(31, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak7() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "\t\t第一，当一行文本内容的长度足够短，短到可以完整的放入一个窗口（的一行）之中，那么基于行的换行和基于单词的换行两个选项都不会影响渲染的最终效果。\n",
      "\t\t第二，当一行内容文本的长度足够长，而无法放入窗口中，那么我们需要考虑很多种情况：\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and ",
      "small test lines.\n",
      "But still it contains several ",
      "things we want to test:\n",
      "\t\t第一，当一行文",
      "本内容的长度足够短，短到可以完",
      "整的放入一个窗口（的一行）之中",
      "，那么基于行的换行和基于单词的",
      "换行两个选项都不会影响渲染的最",
      "终效果。\n",
    ];

    let size = U16Size::new(31, 11);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak8() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple andsmalltestlineswithoutevenanewlinebreakbecausewewanttotesthowitwillhappensifthereisaverylongwordthatcannotbeenpplaceinsidearowofthewindowcontent.\n",
      "But still it contains several things we want to test:\n",
      "\t\t第一，当一行文本内容的长度足够短，短到可以完整的放入一个窗口（的一行）之中，那么基于行的换行和基于单词的换行两个选项都不会影响渲染的最终效果。\n",
      "\t\t第二，当一行内容文本的长度足够长，而无法放入窗口中，那么我们需要考虑很多种情况：\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!\n",
      "This is a quite simple ",
      "andsmalltestlineswithoutevenane",
      "wlinebreakbecausewewanttotestho",
      "witwillhappensifthereisaverylon",
      "gwordthatcannotbeenpplaceinside",
      "arowofthewindowcontent.\n",
      "But still it contains several ",
      "things we want to test:\n",
      "\t\t第一，当一行文",
      "本内容的长度足够短，短到可以完",
    ];

    let size = U16Size::new(31, 11);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak9() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, ",
      "RSVIM!\n",
      "This is a ",
      "quite ",
      "simple and",
      " small ",
      "test lines",
      ".\n",
      "But still ",
      "it ",
    ];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      3,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak10() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple and small test lines.\n",
      "But still it contai\tseveral things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, then the line-wrap and word-wrap doesn't affect the rendering.\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, ",
      "RSVIM!\n",
      "This is a ",
      "quite ",
      "simple and",
      " small ",
      "test lines",
      ".\n",
      "But still ",
      "it contai",
    ];

    let size = U16Size::new(10, 10);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      3,
      &expect_start_fills,
      &expect_end_fills,
    );
  }

  #[test]
  fn sync_from_top_left_wrap_linebreak11() {
    test_log_init();

    let buffer = make_buffer_from_lines(vec![
      "Hello, RSVIM!\n",
      "This is a quite simple andsmalltestlineswithoutevenanewlinebreakbecausewewanttotesthowitwillhappensifthereisaverylongwordthatcannotbeenpplaceinsidearowofthewindowcontent.\n",
      "But still it contains several things we want to test:\n",
      "  1. When the line is small enough to completely put inside a row of the window content widget, 那么行换行和单词换行选项都不会影响最终的渲染效果。\n",
      "  2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
      "     * The extra parts are been truncated if both line-wrap and word-wrap options are not set.\n",
      "     * The extra parts are split into the next row, if either line-wrap or word-wrap options are been set. If the extra parts are still too long to put in the next row, repeat this operation again and again. This operation also eats more rows in the window, thus it may contains less lines in the buffer.\n",
    ]);
    let expect = vec![
      "Hello, RSVIM!",
      "\n",
      "This is a ",
      "quite simple ",
      "andsmalltestl",
      "ineswithoutev",
      "enanewlinebre",
      "akbecausewewa",
      "nttotesthowit",
      "willhappensif",
      "thereisaveryl",
      "ongwordthatca",
      "nnotbeenpplac",
      "einsidearowof",
      "thewindowcont",
      "ent.\n",
      "But still it ",
      "contains ",
      "several ",
      "things we ",
      "want to test:",
      "\n",
      "  1. When the",
      " line is ",
      "small enough ",
      "to completely",
      " put inside a",
      " row of the ",
      "window ",
      "content ",
      "widget, 那么",
      "行换行和单词",
      "换行选项都不",
    ];

    let size = U16Size::new(13, 31);
    let options = WindowLocalOptions::builder()
      .wrap(true)
      .line_break(true)
      .build();
    let actual = make_viewport_from_size(size, buffer.clone(), &options);
    let expect_start_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 0)].into_iter().collect();
    let expect_end_fills: BTreeMap<usize, usize> =
      vec![(0, 0), (1, 0), (2, 0), (3, 1)].into_iter().collect();
    ensure_sync_from_top_left(
      buffer,
      &actual,
      &expect,
      0,
      4,
      &expect_start_fills,
      &expect_end_fills,
    );
  }
}
// spellchecker:on
