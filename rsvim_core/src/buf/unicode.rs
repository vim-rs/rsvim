//! Unicode utils.

use crate::buf::opt::BufferLocalOptions;
use crate::defaults::grapheme::AsciiControlCodeFormatter;

use ascii::AsciiChar;
use compact_str::CompactString;
//use tracing::trace;
use unicode_width::UnicodeWidthChar;

/// Get the display width for a `char`, supports both ASCI control codes and unicode.
///
/// The char display width follows the
/// [Unicode Standard Annex #11](https://www.unicode.org/reports/tr11/), implemented with
/// [UnicodeWidthChar], there's another equivalent crate
/// [icu::properties::EastAsianWidth](https://docs.rs/icu/latest/icu/properties/maps/fn.east_asian_width.html#).
pub fn char_width(opt: &BufferLocalOptions, c: char) -> usize {
  if c.is_ascii_control() {
    let ac = AsciiChar::from_ascii(c).unwrap();
    match ac {
      AsciiChar::Tab => opt.tab_stop() as usize,
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
pub fn char_symbol(opt: &BufferLocalOptions, c: char) -> (CompactString, usize) {
  let width = char_width(opt, c);
  if c.is_ascii_control() {
    let ac = AsciiChar::from_ascii(c).unwrap();
    match ac {
      AsciiChar::Tab => (
        CompactString::from(" ".repeat(opt.tab_stop() as usize)),
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
pub fn str_width(opt: &BufferLocalOptions, s: &str) -> usize {
  s.chars().map(|c| char_width(opt, c)).sum()
}

/// Get the printable cell symbols and the display width for a unicode `str`.
pub fn str_symbols(opt: &BufferLocalOptions, s: &str) -> (CompactString, usize) {
  s.chars().map(|c| char_symbol(opt, c)).fold(
    (CompactString::with_capacity(s.len()), 0_usize),
    |(mut init_symbol, init_width), (mut symbol, width)| {
      init_symbol.push_str(symbol.as_mut_str());
      (init_symbol, init_width + width)
    },
  )
}
