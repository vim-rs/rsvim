//! Display width index (char-wise) for each unicode char in vim buffer.

use crate::buf::opt::BufferLocalOptions;
use crate::buf::unicode;
use ropey::RopeSlice;

use std::collections::BTreeMap;
// use tracing::trace;

#[derive(Debug, Default, Clone)]
/// Display width index (char-wise) for each unicode char in vim buffer. For each line, the
/// char/column index starts from 0.
///
/// This structure is actually a prefix-sum tree structure. For example now we have a line:
///
/// ```text
///                           25
/// 0      7       15       25|
/// |      |       |         ||
/// This is<--HT-->an example.\n
/// |      |                 ||
/// 0      7                18|
///                           19
/// ```
///
/// Here we have some facts:
/// 1. The first char (`T`) index is 0, the display width of char range `[0,0]` is 1.
/// 2. The char (`.`) before the last char index is 18, the display width of char range `[0,18]` is
///    25, there's a tab char (`\t`) which display width is 8 cells.
/// 3. The last char (`\n`) index is 19, the display width of char range `[0,19]` is also 25,
///    because the last char display width is 0 cells.
///
/// Here we have below terms:
/// - **Prefix (Display) Width**: the display width from the first char to current char, inclusive
///   on both side.
pub struct ColIndex {
  // Char index maps to its prefix display width.
  char2width: Vec<usize>,

  // Prefix display width maps to the right-most char index, i.e. the reversed mapping of
  // `char2width`.
  //
  // NOTE:
  // 1. Some unicodes use more than 1 cells, thus the keys/widths could be non-continuous.
  // 2. Some unicodes use 0 cells (such as LF, CR), thus multiple char index could have same width.
  //    In such case, the width maps to the right-most char index, i.e. try to cover wider char
  //    index range.
  width2char: BTreeMap<usize, usize>,
}

impl ColIndex {
  /// Create new index.
  pub fn new() -> Self {
    Self {
      char2width: Vec::new(),
      width2char: BTreeMap::new(),
    }
  }

  #[cfg(not(debug_assertions))]
  pub fn _internal_check(&self) {}

  #[cfg(debug_assertions)]
  pub fn _internal_check(&self) {
    // Check length.
    assert!(self.char2width.len() >= self.width2char.len());

    // Check indexing.
    let mut last_width: Option<usize> = None;
    for (i, w) in self.char2width.iter().enumerate() {
      match last_width {
        Some(last_width1) => {
          assert!(*w >= last_width1);
        }
        None => { /* Skip */ }
      }
      last_width = Some(*w);
      assert!(self.width2char.contains_key(w));
      let c = self.width2char[w];
      // trace!("char2width[{i}]:{w:?}, width2char[{w}]:{c:?}");
      assert!(i <= c);
    }
  }

  // Build cache beyond the bound by `char_idx` or `width`.
  fn _build_cache(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx_bound: Option<usize>,
    width_bound: Option<usize>,
  ) {
    let n = rope_line.len_chars();
    debug_assert!(n > 0);

    let start_idx = self.char2width.len();
    let mut prefix: usize = if start_idx == 0 {
      0_usize
    } else {
      self.char2width[start_idx - 1]
    };

    let mut rope_chars = rope_line.chars_at(start_idx);
    for i in start_idx..n {
      let c = rope_chars.next().unwrap();
      prefix += unicode::char_width(options, c);

      // Update `char2width`
      self.char2width.push(prefix);

      // Update `width2char`
      let c = self.char2width.len() - 1;
      debug_assert_eq!(i, c);
      match self.width2char.get(&prefix) {
        Some(c1) => {
          if *c1 < c {
            self.width2char.insert(prefix, c);
          }
        }
        None => {
          self.width2char.insert(prefix, c);
        }
      }

      if let Some(char_idx) = char_idx_bound {
        if i > char_idx {
          return;
        }
      }
      if let Some(width) = width_bound {
        if prefix > width {
          return;
        }
      }
    }
  }

  // Build cache until `char_idx`.
  fn _build_cache_until_char_idx(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx: usize,
  ) {
    // Line is empty, no need to build.
    if rope_line.len_chars() == 0 {
      return;
    }

    // If not cached.
    if char_idx >= self.char2width.len() {
      self._build_cache(options, rope_line, Some(char_idx), None);
    }
  }

  /// Get the prefix display width in char index range `[0,char_idx)`, left-inclusive and
  /// right-exclusive.
  ///
  /// NOTE: This is equivalent to `width_until(char_idx-1)`.
  ///
  /// # Return
  ///
  /// 1. It returns 0 if `char_idx <= 0`.
  /// 2. It returns the prefix display width if `char_idx` is inside the line.
  /// 3. It returns the whole display width of the line if `char_idx` is greater than the line
  ///    length.
  pub fn width_before(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx: usize,
  ) -> usize {
    self._build_cache_until_char_idx(options, rope_line, char_idx);
    self._internal_check();

    if char_idx == 0 || self.char2width.is_empty() {
      assert_eq!((rope_line.len_chars() == 0), self.char2width.is_empty());
      0
    } else {
      assert!(!self.char2width.is_empty());
      assert!(rope_line.len_chars() > 0);
      if char_idx - 1 < self.char2width.len() {
        // Find width from the cache.
        self.char2width[char_idx - 1]
      } else {
        // If outside of the cache, returns the whole width.
        self.char2width[self.char2width.len() - 1]
      }
    }
  }

  /// Get the prefix display width in char index range `[0,char_idx]`, both sides are inclusive.
  ///
  /// NOTE: This is equivalent to `width_before(char_idx+1)`.
  ///
  /// # Return
  ///
  /// 1. It returns 0 if the line length is 0, i.e. the line itself is empty.
  /// 2. It returns the prefix display width if `char_idx` is inside the line.
  /// 3. It returns the whole display width of the line if `char_idx` is greater than or equal to
  ///    the line length.
  pub fn width_until(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx: usize,
  ) -> usize {
    self._build_cache_until_char_idx(options, rope_line, char_idx);
    self._internal_check();

    if self.char2width.is_empty() {
      assert!(rope_line.len_chars() == 0);
      0
    } else {
      assert!(!self.char2width.is_empty());
      assert!(rope_line.len_chars() > 0);
      if char_idx < self.char2width.len() {
        // Find width from the cache.
        self.char2width[char_idx]
      } else {
        // If outside of the cache, returns the whole width.
        self.char2width[self.char2width.len() - 1]
      }
    }
  }

  // Build cache until specified `width`.
  fn _build_cache_until_width(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    width: usize,
  ) {
    // Line is empty, no need to build.
    if rope_line.len_chars() == 0 {
      return;
    }

    // If not cached.
    match self.width2char.last_key_value() {
      Some((last_width, _last_char_idx)) => {
        if width > *last_width {
          self._build_cache(options, rope_line, None, Some(width));
        }
      }
      None => self._build_cache(options, rope_line, None, Some(width)),
    }
  }

  /// Get the right-most char index which the width is less than the specified width.
  ///
  /// Note:
  /// 1. The specified width is exclusive, i.e. the returned char index's width is always less than
  ///    the specified width, but cannot be greater than or equal to it.
  /// 2. For all the char indexes which the width is less, it returns the right-most char index.
  ///
  /// # Return
  ///
  /// 1. It returns None if the line length is 0, i.e. the line itself is empty, or there's no such
  ///    char.
  /// 2. It returns the right-most char index if `width` is inside the line.
  /// 3. It returns the last char index of the line if `width` is greater than or equal to
  ///    the line's whole display width.
  pub fn char_before(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    width: usize,
  ) -> Option<usize> {
    self._build_cache_until_width(options, rope_line, width);
    self._internal_check();

    if width == 0 || self.width2char.is_empty() {
      assert_eq!((rope_line.len_chars() == 0), self.width2char.is_empty());
      None
    } else {
      assert!(!self.width2char.is_empty());
      assert!(rope_line.len_chars() > 0);
      let (last_width, last_char_idx) = self.width2char.last_key_value().unwrap();
      if width <= *last_width {
        for w in (1..width).rev() {
          match self.width2char.get(&w) {
            Some(c) => {
              return Some(*c);
            }
            None => { /* Skip */ }
          }
        }
        // Not exist.
        None
      } else {
        Some(*last_char_idx)
      }
    }
  }

  /// Get the right-most char index which the width is greater than or equal to the specified
  /// width.
  ///
  /// Note:
  /// 1. The specified width is inclusive, i.e. the returned char index's width is greater than or
  ///    equal to the specified width, but cannot be less than it.
  /// 2. For all the char indexes which the width is greater or equal, it returns the right-most
  ///    char index.
  ///
  /// # Return
  ///
  /// 1. It returns None if the line length is 0, i.e. the line itself is empty, or there's no such
  ///    char.
  /// 2. It returns the right-most char index if `width` is inside the line.
  /// 3. It returns the last char index of the line if `width` is greater than or equal to
  ///    the line's whole display width.
  pub fn char_until(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    width: usize,
  ) -> Option<usize> {
    self._build_cache_until_width(options, rope_line, width);
    self._internal_check();

    if width == 0 || self.width2char.is_empty() {
      assert_eq!((rope_line.len_chars() == 0), self.width2char.is_empty());
      None
    } else {
      assert!(!self.width2char.is_empty());
      assert!(rope_line.len_chars() > 0);
      let (last_width, last_char_idx) = self.width2char.last_key_value().unwrap();
      if width <= *last_width {
        for w in (1..=width).rev() {
          match self.width2char.get(&w) {
            Some(c) => {
              return Some(*c);
            }
            None => { /* Skip */ }
          }
        }
        // Not exist.
        None
      } else {
        Some(*last_char_idx)
      }
    }
  }

  /// Truncate cache by char index.
  pub fn truncate_by_char(&mut self, char_idx: usize) {
    self._internal_check();

    if self.char2width.is_empty() || self.width2char.is_empty() {
      debug_assert_eq!(self.char2width.is_empty(), self.width2char.is_empty());
    } else if char_idx < self.char2width.len() {
      self.char2width.truncate(char_idx);
      self.width2char.retain(|&_w, &mut c| c < char_idx);
    }
  }

  /// Truncate cache by width.
  pub fn truncate_by_width(&mut self, width: usize) {
    self._internal_check();

    if self.char2width.is_empty() || self.width2char.is_empty() {
      debug_assert_eq!(self.char2width.is_empty(), self.width2char.is_empty());
    } else {
      let (last_width, _last_char_idx) = self.width2char.last_key_value().unwrap();
      if width <= *last_width {
        for w in (1..=width).rev() {
          match self.width2char.get(&w) {
            Some(c) => {
              self.truncate_by_char(*c);
              return;
            }
            None => { /* Skip */ }
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::test::buf::make_rope_from_lines;
  #[allow(dead_code)]
  use crate::test::log::init as test_log_init;

  use ropey::Rope;
  use tracing::info;

  fn assert_width_until(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    actual: &mut ColIndex,
    expect: &[usize],
  ) {
    for (i, e) in expect.iter().enumerate() {
      let a = actual.width_until(options, rope_line, i);
      info!("width_until:{i} actual:{a:?}, expect:{e:?}");
      assert_eq!(a, *e);
    }
  }

  fn assert_width_until_rev(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    actual: &mut ColIndex,
    expect: &[(usize, usize)],
  ) {
    for (e, i) in expect.iter() {
      let a = actual.width_until(options, rope_line, *i);
      info!("width_until:{i}, actual:{a:?}, expect:{e:?}");
      assert_eq!(a, *e);
    }
  }

  fn assert_width_before(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    actual: &mut ColIndex,
    expect: &[usize],
  ) {
    for (i, e) in expect.iter().enumerate() {
      let a = actual.width_before(options, rope_line, i);
      info!("width_before:{i} actual:{a:?}, expect:{e:?}");
      assert_eq!(a, *e);
    }
  }

  fn assert_width_before_rev(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    actual: &mut ColIndex,
    expect: &[(usize, usize)],
  ) {
    for (e, i) in expect.iter() {
      let a = actual.width_before(options, rope_line, *i);
      info!("width_before:{i}, actual:{a:?}, expect:{e:?}");
      assert_eq!(a, *e);
    }
  }

  #[test]
  fn width1() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["Hello,\tRSVIM!\n"]);
    let mut actual = ColIndex::new();

    let expect: Vec<usize> =
      [(1..=6).collect(), (14..=20).collect(), vec![20, 20, 20, 20]].concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [(0..=6).collect(), (14..=20).collect(), vec![20, 20, 20]].concat();
    assert_width_before(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width2() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["This is a quite simple and small test lines.\n"]);
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 5), 5);
    assert_eq!(actual.width_until(&options, &rope.line(0), 43), 44);

    let expect: Vec<usize> = [(1..=44).collect(), vec![44, 44, 44, 44]].concat();

    let expect1: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [(0..=44).collect(), vec![44, 44, 44]].concat();

    let expect1: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_before(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width3() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["But still\tit\\包含了好几种东西we want to test:\n"]);
    let mut actual = ColIndex::new();

    let expect: Vec<usize> = [
      (1..=9).collect(),
      (17..=20).collect(),
      (22..=29)
        .scan(22, |state, i| {
          let diff: usize = i - *state;
          Some(*state + 2 * diff)
        })
        .collect(),
      (37..=52).collect(),
      vec![52, 52, 52, 52],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [
      (0..=9).collect(),
      (17..=20).collect(),
      (22..=29)
        .scan(22, |state, i| {
          let diff: usize = i - *state;
          Some(*state + 2 * diff)
        })
        .collect(),
      (37..=52).collect(),
      vec![52, 52, 52],
    ]
    .concat();
    assert_width_before(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width4() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["  1. When the\r"]);
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 11), 11);
    assert_eq!(actual.width_until(&options, &rope.line(0), 10), 11);

    let expect: Vec<usize> = [(1..=13).collect(), vec![13, 13, 13, 13]].concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [(0..=13).collect(), vec![13, 13, 13]].concat();
    assert_width_before(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width5() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec![
      "一行文本小到可以放入一个窗口中，那么line-wrap和word-wrap选项就不会影响排版。\n",
    ]);
    let mut actual = ColIndex::new();

    let expect: Vec<usize> = [
      (1..=18).map(|i| i * 2).collect(),
      (37..=45).collect(),
      vec![47],
      (48..=56).collect(),
      (58..=67)
        .scan(58, |state, i| {
          let diff: usize = i - *state;
          Some(*state + 2 * diff)
        })
        .collect(),
      vec![76, 76, 76, 76],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [
      (0..=18).map(|i| i * 2).collect(),
      (37..=45).collect(),
      vec![47],
      (48..=56).collect(),
      (58..=67)
        .scan(58, |state, i| {
          let diff: usize = i - *state;
          Some(*state + 2 * diff)
        })
        .collect(),
      vec![76, 76, 76, 76],
    ]
    .concat();
    assert_width_before(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width6() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec![
      "\t\t2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
    ]);
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 1), 8);
    assert_eq!(actual.width_before(&options, &rope.line(0), 2), 16);
    assert_eq!(actual.width_until(&options, &rope.line(0), 2), 17);

    let expect: Vec<usize> = [vec![8, 16], (17..=129).collect(), vec![129, 129, 129, 129]].concat();

    let expect1: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_until_rev(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<usize> = [vec![0, 8, 16], (17..=129).collect(), vec![129, 129, 129]].concat();

    let expect1: Vec<(usize, usize)> = expect
      .iter()
      .enumerate()
      .map(|(i, e)| (*e, i))
      .rev()
      .collect();
    assert_width_before_rev(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_before(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width7() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = Rope::new();
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 10), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 10), 0);

    let rope = make_rope_from_lines(vec![]);
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 10), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 10), 0);

    let rope = make_rope_from_lines(vec![""]);
    let mut actual = ColIndex::new();

    assert_eq!(actual.width_before(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_before(&options, &rope.line(0), 10), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 0), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 1), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 2), 0);
    assert_eq!(actual.width_until(&options, &rope.line(0), 10), 0);
  }

  fn assert_char(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    widx: &mut ColIndex,
    expect_before: &[(usize, Option<usize>)],
    expect_until: &[(usize, Option<usize>)],
  ) {
    for (w, i) in expect_before.iter() {
      assert_eq!(widx.char_before(options, rope_line, *w), *i);
      if i.is_some() {
        assert!(widx.width_until(options, rope_line, i.unwrap()) < *w);
      }
    }
    for (w, i) in expect_until.iter() {
      assert_eq!(widx.char_until(options, rope_line, *w), *i);
      if i.is_some() {
        assert!(widx.width_until(options, rope_line, i.unwrap()) <= *w);
      }
    }
  }

  #[test]
  fn char1() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["This is a quite\t简单而且很小的test\tlines.\n"]);
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, None),
      (5, Some(3)),
      (10, Some(8)),
      (15, Some(13)),
      (16, Some(14)),
      (17, Some(14)),
      (22, Some(14)),
      (23, Some(14)),
      (24, Some(15)),
      (25, Some(15)),
      (26, Some(16)),
      (27, Some(16)),
      (28, Some(17)),
    ];

    let expect_until: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, Some(0)),
      (5, Some(4)),
      (10, Some(9)),
      (15, Some(14)),
      (16, Some(14)),
      (17, Some(14)),
      (22, Some(14)),
      (23, Some(15)),
      (24, Some(15)),
      (25, Some(16)),
      (26, Some(16)),
      (27, Some(17)),
      (28, Some(17)),
      (29, Some(18)),
    ];
    assert_char(
      &options,
      &rope.line(0),
      &mut widx,
      &expect_before,
      &expect_until,
    );
  }

  #[test]
  fn char2() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = Rope::new();
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];

    let expect_until: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];
    assert_char(
      &options,
      &rope.line(0),
      &mut widx,
      &expect_before,
      &expect_until,
    );

    let rope = make_rope_from_lines(vec![]);
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];

    let expect_until: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];
    assert_char(
      &options,
      &rope.line(0),
      &mut widx,
      &expect_before,
      &expect_until,
    );

    let rope = make_rope_from_lines(vec![""]);
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];

    let expect_until: Vec<(usize, Option<usize>)> =
      vec![(0, None), (1, None), (5, None), (10, None)];
    assert_char(
      &options,
      &rope.line(0),
      &mut widx,
      &expect_before,
      &expect_until,
    );

    let rope = make_rope_from_lines(vec!["\t"]);
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, None),
      (5, None),
      (7, None),
      (8, None),
      (9, Some(0)),
      (10, Some(0)),
    ];

    let expect_until: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, None),
      (5, None),
      (7, None),
      (8, Some(0)),
      (9, Some(0)),
      (10, Some(0)),
    ];
    assert_char(
      &options,
      &rope.line(0),
      &mut widx,
      &expect_before,
      &expect_until,
    );
  }

  #[test]
  fn truncate1() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["Hello,\tRSVIM!\n"]);
    let mut widx = ColIndex::new();

    let expect: Vec<usize> =
      [(1..=6).collect(), (14..=20).collect(), vec![20, 20, 20, 20]].concat();
    for (i, e) in expect.iter().enumerate() {
      let actual = widx.width_until(&options, &rope.line(0), i);
      assert_eq!(actual, *e);
      widx.truncate_by_char(i);
    }

    let expect: Vec<usize> = [(0..=6).collect(), (14..=20).collect(), vec![20, 20, 20]].concat();
    for (i, e) in expect.iter().enumerate() {
      let a = widx.width_before(&options, &rope.line(0), i);
      assert_eq!(a, *e);
      widx.truncate_by_char(i);
    }

    let rope = make_rope_from_lines(vec!["This is a quite\t简单而且很小的test\tlines.\n"]);
    let mut widx = ColIndex::new();

    let expect_before: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, None),
      (5, Some(3)),
      (10, Some(8)),
      (15, Some(13)),
      (16, Some(14)),
      (17, Some(14)),
      (22, Some(14)),
      (23, Some(14)),
      (24, Some(15)),
      (25, Some(15)),
      (26, Some(16)),
      (27, Some(16)),
      (28, Some(17)),
    ];

    let expect_until: Vec<(usize, Option<usize>)> = vec![
      (0, None),
      (1, Some(0)),
      (5, Some(4)),
      (10, Some(9)),
      (15, Some(14)),
      (16, Some(14)),
      (17, Some(14)),
      (22, Some(14)),
      (23, Some(15)),
      (24, Some(15)),
      (25, Some(16)),
      (26, Some(16)),
      (27, Some(17)),
      (28, Some(17)),
      (29, Some(18)),
    ];
    for (w, i) in expect_before.iter() {
      assert_eq!(widx.char_before(&options, &rope.line(0), *w), *i);
      widx.truncate_by_width(*w);
    }
    for (w, i) in expect_until.iter() {
      assert_eq!(widx.char_until(&options, &rope.line(0), *w), *i);
      widx.truncate_by_width(*w);
    }
  }
}
