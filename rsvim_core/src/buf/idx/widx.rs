//! Display width index (line-wise) for each unicode char in vim buffer.

use crate::buf::opt::BufferLocalOptions;
use crate::buf::unicode;
use ropey::RopeSlice;

use std::collections::BTreeMap;
// use tracing::trace;

#[derive(Debug, Default, Clone)]
/// Display width index (line-wise) for each unicode char in vim buffer. For each line, the
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
pub struct BufWindex {
  // Char index maps to its prefix display width.
  char2width: Vec<usize>,

  // Prefix display width maps to the right-most char index, i.e. the reversed mapping of
  // `char2width`.
  //
  // NOTE:
  // 1. Some unicodes could use more than 1 cells, thus the widths could be non-continuous.
  // 2. Some unicodes could use 0 cells (i.e. line-feed), multiple char index could have the same
  //    width. In such case, the width point to the larger char index and try to cover wider char
  //    index range.
  width2char: BTreeMap<usize, usize>,
}

impl BufWindex {
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

  /// Get the prefix display width starts from the first char 0 until the specified char. Note the
  /// specified char's width is included.
  ///
  /// NOTE: This is equivalent to `width_between(0..=char_idx)`.
  ///
  /// # Return
  ///
  /// It returns the prefix display width if `char_idx` is inside the index.
  /// It returns `None` if the `char_idx` is out of index range.
  pub fn width_until(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx: usize,
  ) -> Option<usize> {
    // If not cached.
    if char_idx >= self.char2width.len() {
      // If this char exists in the rope line, build the cache.
      if char_idx < rope_line.len_chars() {
        let start_idx = self.char2width.len();
        let mut prefix_width: usize = if start_idx == 0 {
          0_usize
        } else {
          self.char2width[start_idx - 1]
        };
        let mut rope_chars = rope_line.chars().skip(start_idx);
        for _i in start_idx..=char_idx {
          let c = rope_chars.next().unwrap();
          prefix_width += unicode::char_width(options, c);

          // Update `char2width`
          self.char2width.push(prefix_width);

          // Update `width2char`
          let w = prefix_width;
          let c = self.char2width.len() - 1;
          if self.width2char.contains_key(&w) {
            if self.width2char[&w] < c {
              self.width2char.insert(w, c);
            }
          } else {
            self.width2char.insert(w, c);
          }
        }
      }
    }

    self._internal_check();

    if char_idx < self.char2width.len() {
      // Find width from the cache.
      Some(self.char2width[char_idx])
    } else {
      // If this char index doesn't exist in the cache, it is just not existed.
      None
    }
  }

  /// Get the display width in the inclusive range, i.e. `[a, b]`.
  ///
  /// # Return
  ///
  /// It returns the display width of the `char_idx_range` if the range is inside the index.
  /// It returns `None` if the `char_idx_range` is out of index range.
  pub fn width_between(
    &mut self,
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    char_idx_range: std::ops::RangeInclusive<usize>,
  ) -> Option<usize> {
    self._internal_check();
    let c_start = *char_idx_range.start();
    let c_last = *char_idx_range.end();
    let w_start = self.width_until(options, rope_line, c_start);
    let w_last = self.width_until(options, rope_line, c_last);
    match (w_start, w_last) {
      (Some(start_width), Some(last_width)) => {
        assert!(start_width <= last_width);
        Some(last_width - start_width)
      }
      _ => None,
    }
  }

  /// Get the first char index which width is greater or equal than the specified width.
  ///
  /// Here the *greater or equal than* indicates that:
  /// 1. If the width is exactly the width on a char index, it returns the char index.
  /// 2. Otherwise, it returns the first char which width is greater than it.
  ///
  /// # Return
  ///
  /// It returns the first char index if the `width` is inside the index.
  /// It returns `None` if the `width` is out of the index range.
  pub fn char_at(&self, _width: usize) -> Option<usize> {
    unimplemented!();
  }

  /// Set/update a specified char's width, and re-calculate all display width since this char.
  ///
  /// NOTE: This operation is `O(N)`, where `N` is the chars count of current line.
  pub fn set_width_at(&mut self, _char_idx: usize, _width: usize) {
    unimplemented!();
  }

  /// Set/update a range of chars and their width, and re-calculate all display width since the first
  /// char in the range.
  ///
  /// NOTE: This operation is `O(N)`, where `N` is the chars count of current line.
  ///
  /// # Panics
  ///
  /// It panics if the provided parameter `char2width` keys are not continuous, i.e. the chars
  /// index must be continuous.
  pub fn set_width_between(&mut self, _widths: &BTreeMap<usize, usize>) {
    unimplemented!();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  use crate::test::buf::make_rope_from_lines;
  #[allow(dead_code)]
  use crate::test::log::init as test_log_init;

  use tracing::info;

  fn assert_width_until(
    options: &BufferLocalOptions,
    rope_line: &RopeSlice,
    actual: &mut BufWindex,
    expect: &Vec<(usize, Option<usize>)>,
  ) {
    for (i, e) in expect.iter().enumerate() {
      let a = actual.width_until(options, rope_line, i);
      info!("actual[{i}]:{a:?}, expect[{i}]:{e:?}");
      assert_eq!(a, e.clone());
    }
  }

  #[test]
  fn width_until1() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["Hello,\tRSVIM!\n"]);
    let mut actual = BufWindex::new();

    // 1-6, 14-20, 20
    let expect: Vec<(usize, Option<usize>)> = [
      (0..=5).map(|i| (i, Some(i + 1))).collect(),
      (6..=12).map(|i| (i + 8, Some(i))).collect(),
      vec![(13, Some(20)), (14, None), (15, None), (16, None)],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<(usize, Option<usize>)> = expect
      .iter()
      .filter(|e| e.1.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width_until2() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["This is a quite simple and small test lines.\n"]);
    let mut actual = BufWindex::new();

    assert_eq!(actual.width_until(&options, &rope.line(0), 43), Some(44));

    // 1-44
    let expect: Vec<Option<usize>> = [
      (1..=44).map(|i| Some(i)).collect(),
      vec![Some(44), None, None, None],
    ]
    .concat();

    let expect1: Vec<Option<usize>> = expect
      .iter()
      .filter(|e| e.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width_until3() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["But still\tit\\包含了好几种东西we want to test:\n"]);
    let mut actual = BufWindex::new();

    // 0-8, 16-18, 19-33, 35-51, 51
    let expect: Vec<Option<usize>> = [
      (1..=9).map(|i| Some(i)).collect(),
      (17..=20).map(|i| Some(i)).collect(),
      (22..=29)
        .scan(22, |state, i| {
          let diff: usize = i - *state;
          Some(Some(*state + 2 * diff))
        })
        .collect(),
      (37..=53).map(|i| Some(i)).collect(),
      vec![Some(53), None, None, None],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<Option<usize>> = expect
      .iter()
      .filter(|e| e.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width_until4() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec!["  1. When the\r"]);
    let mut actual = BufWindex::new();

    assert_eq!(actual.width_until(&options, &rope.line(0), 10), Some(11));

    // 0-12, 12
    let expect: Vec<Option<usize>> = [
      (0..=12).map(|i| Some(i)).collect(),
      vec![Some(12), None, None, None],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<Option<usize>> = expect
      .iter()
      .filter(|e| e.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width_until5() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec![
      "一行文本小到可以放入一个窗口中，那么line-wrap和word-wrap选项就不会影响排版。\n",
    ]);
    let mut actual = BufWindex::new();
    // 0-35, 36-45, 46, 48-57, 58-77, 77
    let expect: Vec<Option<usize>> = [
      (0..=17).map(|i| Some(i * 2)).collect(),
      (36..=45).map(|i| Some(i)).collect(),
      vec![Some(46)],
      (48..=57).map(|i| Some(i)).collect(),
      (58..=67)
        .scan(58, |state, i| {
          let diff: usize = i - *state;
          Some(Some(*state + 2 * diff))
        })
        .collect(),
      vec![Some(77), None, None, None],
    ]
    .concat();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);

    let expect: Vec<Option<usize>> = expect
      .iter()
      .filter(|e| e.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }

  #[test]
  fn width_until6() {
    test_log_init();

    let options = BufferLocalOptions::default();
    let rope = make_rope_from_lines(vec![
      "\t\t2. When the line is too long to be completely put in a row of the window content widget, there're multiple cases:\n",
    ]);
    let mut actual = BufWindex::new();

    assert_eq!(actual.width_until(&options, &rope.line(0), 2), Some(17));

    // 0, 8, 16-129, 129
    let expect: Vec<Option<usize>> = [
      vec![Some(0), Some(8)],
      (16..=129).map(|i| Some(i)).collect(),
      vec![Some(129), None, None, None],
    ]
    .concat();

    let expect1: Vec<Option<usize>> = expect
      .iter()
      .filter(|e| e.is_some())
      .rev()
      .cloned()
      .collect();
    assert_width_until(&options, &rope.line(0), &mut actual, &expect1);

    assert_width_until(&options, &rope.line(0), &mut actual, &expect);
  }
}
