//! Canvas.

use crate::cart::{U16Pos, U16Size};

// Re-export
pub use crate::ui::canvas::frame::cell::Cell;
pub use crate::ui::canvas::frame::cursor::{
  cursor_style_eq, Cursor, CursorStyle, CursorStyleFormatter,
};
pub use crate::ui::canvas::frame::Frame;

use compact_str::ToCompactString;
use crossterm;
use geo::point;
use parking_lot::RwLock;
use std::fmt;
use std::fmt::Debug;
use std::slice::Iter;
use std::sync::Arc;
use tracing::trace;

pub mod frame;
pub mod internal;

#[derive(Debug, Clone)]
/// Logical canvas.
///
/// It manages both the current frame and the last frame as a screenshot, and internally uses a
/// diff-algorithm to compare the TUI changes, thus only flushing the changed parts to reduce IO
/// operations.
///
/// NOTE: APIs named without `prev_` are current frame, with `prev_` are for previous frame.
pub struct Canvas {
  frame: Frame,
  prev_frame: Frame,
}

pub type CanvasArc = Arc<RwLock<Canvas>>;

impl Canvas {
  /// Make new canvas with terminal actual size.
  pub fn new(size: U16Size) -> Self {
    Canvas {
      prev_frame: Frame::new(size, Cursor::default()),
      frame: Frame::new(size, Cursor::default()),
    }
  }

  /// Convert struct into smart pointer.
  pub fn to_arc(t: Canvas) -> CanvasArc {
    Arc::new(RwLock::new(t))
  }

  // Current frame {

  /// Get current frame.
  pub fn frame(&self) -> &Frame {
    &self.frame
  }

  /// Get mutable current frame.
  pub fn frame_mut(&mut self) -> &mut Frame {
    &mut self.frame
  }

  pub fn size(&self) -> U16Size {
    self.frame.size()
  }

  /// Get current frame cells.
  pub fn cells(&self) -> &Vec<Cell> {
    self.frame.get_cells()
  }

  /// Get current frame cursor.
  pub fn cursor(&self) -> &Cursor {
    self.frame.cursor()
  }

  // Current frame }

  // Previous frame {

  /// Get previous frame.
  pub fn prev_frame(&self) -> &Frame {
    &self.prev_frame
  }

  pub fn prev_size(&self) -> U16Size {
    self.prev_frame.size()
  }

  /// Get previous frame cells.
  pub fn prev_cells(&self) -> &Vec<Cell> {
    self.prev_frame.get_cells()
  }

  /// Get previous frame cells at specified range.
  pub fn prev_cells_at(&self, pos: U16Pos, n: usize) -> &[Cell] {
    self.prev_frame.get_cells_at(pos, n)
  }

  /// Get previous frame cursor.
  pub fn prev_cursor(&self) -> &Cursor {
    self.prev_frame.cursor()
  }

  // Previous frame }

  /// Get the shader commands that should print to the terminal device, it internally uses a
  /// diff-algorithm to reduce the outputs.
  pub fn shade(&mut self) -> Shader {
    let mut shader = Shader::new();

    // For cells, it needs extra save and restore cursor position
    let mut cells_shaders = self._shade_cells();
    let saved_cursor_pos = self.cursor().pos();
    shader.append(&mut cells_shaders);
    shader.push(ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(
      saved_cursor_pos.x(),
      saved_cursor_pos.y(),
    )));

    // For cursor
    let mut cursor_shaders = self._shade_cursor();
    shader.append(&mut cursor_shaders);

    // Finish shade.
    self._shade_done();

    shader
  }

  /// Shade done.
  pub fn _shade_done(&mut self) {
    // Save current frame.
    self.prev_frame = self.frame.clone();
    // Reset the `dirty` fields.
    self.frame.reset_dirty_rows();
  }

  /// Shade cursor and append results into shader vector.
  pub fn _shade_cursor(&mut self) -> Vec<ShaderCommand> {
    let cursor = self.frame.cursor();
    let prev_cursor = self.prev_frame.cursor();
    let mut shader = vec![];

    // If cursor is changed.
    if cursor != prev_cursor {
      if cursor.blinking() != prev_cursor.blinking() {
        if cursor.blinking() {
          shader.push(ShaderCommand::CursorEnableBlinking(
            crossterm::cursor::EnableBlinking,
          ));
        } else {
          shader.push(ShaderCommand::CursorDisableBlinking(
            crossterm::cursor::DisableBlinking,
          ));
        }
      }
      if cursor.hidden() != prev_cursor.hidden() {
        if cursor.hidden() {
          shader.push(ShaderCommand::CursorHide(crossterm::cursor::Hide));
        } else {
          shader.push(ShaderCommand::CursorShow(crossterm::cursor::Show));
        }
      }
      if !cursor_style_eq(&cursor.style(), &prev_cursor.style()) {
        shader.push(ShaderCommand::CursorSetCursorStyle(cursor.style()));
      }
      if cursor.pos() != prev_cursor.pos() {
        shader.push(ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(
          cursor.pos().x(),
          cursor.pos().y(),
        )));
      }
    }

    shader
  }

  /// Shade cells and append results into shader vector.
  pub fn _shade_cells(&mut self) -> Vec<ShaderCommand> {
    if self.size() == self.prev_size() {
      // When terminal size remains the same, use dirty-marks diff-algorithm.
      self._dirty_marks_diff()
    } else {
      // When terminal size remains the same, use brute-force diff-algorithm.
      self._brute_force_diff()
    }
  }

  /// Find next same cell in current row of frame. NOTE: row is y, col is x.
  ///
  /// # Returns
  ///
  /// 1. The column number if found the same cell, column number started from 0.
  /// 2. The end column index on the row if not found, i.e. the width of current frame.
  pub fn _next_same_cell_in_row(&self, row: u16, col: u16) -> u16 {
    let frame = self.frame();
    let prev_frame = self.prev_frame();

    let mut col_end_at = col;
    while col_end_at < frame.size().width() {
      let pos: U16Pos = point!(x: col_end_at, y: row);
      let cell2 = frame.get_cell(pos);
      let prev_cell2 = prev_frame.get_cell(pos);
      if cell2 == prev_cell2 {
        break;
      }
      col_end_at += 1;
    }
    col_end_at
  }

  pub fn _make_print_shaders(&self, row: u16, start_col: u16, end_col: u16) -> Vec<ShaderCommand> {
    let frame = self.frame();
    let mut shaders = Vec::new();

    assert!(end_col > start_col);
    let new_cells = frame.get_cells_at(
      point!(x: start_col, y: row),
      end_col as usize - start_col as usize,
    );
    let new_contents = new_cells
      .iter()
      .map(|c| {
        if c.symbol().is_empty() {
          " ".to_compact_string()
        } else {
          c.symbol().clone()
        }
      })
      .collect::<Vec<_>>()
      .join("");
    shaders.push(ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(
      start_col, row,
    )));
    shaders.push(ShaderCommand::StylePrintString(crossterm::style::Print(
      new_contents.to_string(),
    )));
    shaders
  }

  /// Brute force diff-algorithm, it iterates all cells on current frame, and compares with
  /// previous frame to find out the changed cells.
  ///
  /// This algorithm is useful when the whole terminal size is changed, and row/column based
  /// diff-algorithm becomes invalid.
  pub fn _brute_force_diff(&mut self) -> Vec<ShaderCommand> {
    let frame = self.frame();
    let size = self.size();
    let prev_frame = self.prev_frame();
    let _prev_size = self.prev_size();
    trace!("brute force diff, size:{:?}", size);

    let mut shaders = vec![];

    if !frame.zero_sized() {
      for row in 0..size.height() {
        let mut col = 0_u16;
        while col < size.width() {
          // Skip unchanged columns
          let pos: U16Pos = point!(x: col, y: row);
          let cell = frame.get_cell(pos);
          let prev_cell = prev_frame.get_cell(pos);
          if cell == prev_cell {
            col += 1;
            continue;
          }

          // Find the continuously changed parts by iterating over columns
          let col_end_at = self._next_same_cell_in_row(row, col);

          if col_end_at > col {
            let mut print_shaders = self._make_print_shaders(row, col, col_end_at);
            shaders.append(&mut print_shaders);
            col = col_end_at;
          }
        }
      }
    }

    shaders
  }

  /// Dirty marks diff-algorithm, it only iterates on the area that has been marked as dirty by UI
  /// widgets.
  ///
  /// This algorithm is more performant when the whole terminal size remains unchanged.
  pub fn _dirty_marks_diff(&mut self) -> Vec<ShaderCommand> {
    let frame = self.frame();
    let size = self.size();
    let prev_frame = self.prev_frame();
    let _prev_size = self.prev_size();
    trace!("dirty marks diff, size:{:?}", size);

    let mut shaders = vec![];

    if !frame.zero_sized() {
      for (row, dirty) in frame.dirty_rows().iter().enumerate() {
        if row < size.height() as usize && *dirty {
          let mut col = 0_u16;
          while col < size.width() {
            // Skip unchanged columns
            let pos: U16Pos = point!(x: col, y: row as u16);
            let cell = frame.get_cell(pos);
            let prev_cell = prev_frame.get_cell(pos);
            if cell == prev_cell {
              col += 1;
              continue;
            }

            // Find the continuously changed parts by iterating over columns
            let col_end_at = self._next_same_cell_in_row(row as u16, col);

            if col_end_at > col {
              let mut print_shaders = self._make_print_shaders(row as u16, col, col_end_at);
              shaders.append(&mut print_shaders);
              col = col_end_at;
            }
          }
        }
      }
    }

    shaders
  }
}

#[derive(Clone)]
/// Shader command enums.
///
/// All-in-one wrapper to wrap all the [`crossterm::Command`], thus helps to return the rendering
/// updates for the terminal.
pub enum ShaderCommand {
  CursorSetCursorStyle(crossterm::cursor::SetCursorStyle),
  CursorDisableBlinking(crossterm::cursor::DisableBlinking),
  CursorEnableBlinking(crossterm::cursor::EnableBlinking),
  CursorHide(crossterm::cursor::Hide),
  CursorMoveDown(crossterm::cursor::MoveDown),
  CursorMoveLeft(crossterm::cursor::MoveLeft),
  CursorMoveRight(crossterm::cursor::MoveRight),
  CursorMoveTo(crossterm::cursor::MoveTo),
  CursorMoveToColumn(crossterm::cursor::MoveToColumn),
  CursorMoveToNextLine(crossterm::cursor::MoveToNextLine),
  CursorMoveToPreviousLine(crossterm::cursor::MoveToPreviousLine),
  CursorMoveToRow(crossterm::cursor::MoveToRow),
  CursorMoveUp(crossterm::cursor::MoveUp),
  CursorRestorePosition(crossterm::cursor::RestorePosition),
  CursorSavePosition(crossterm::cursor::SavePosition),
  CursorShow(crossterm::cursor::Show),
  EventDisableBracketedPaste(crossterm::event::DisableBracketedPaste),
  EventDisableFocusChange(crossterm::event::DisableFocusChange),
  EventDisableMouseCapture(crossterm::event::DisableMouseCapture),
  EventEnableBracketedPaste(crossterm::event::EnableBracketedPaste),
  EventEnableFocusChange(crossterm::event::EnableFocusChange),
  EventEnableMouseCapture(crossterm::event::EnableMouseCapture),
  EventPopKeyboardEnhancementFlags(crossterm::event::PopKeyboardEnhancementFlags),
  EventPushKeyboardEnhancementFlags(crossterm::event::PushKeyboardEnhancementFlags),
  StyleResetColor(crossterm::style::ResetColor),
  StyleSetAttribute(crossterm::style::SetAttribute),
  StyleSetAttributes(crossterm::style::SetAttributes),
  StyleSetBackgroundColor(crossterm::style::SetBackgroundColor),
  StyleSetColors(crossterm::style::SetColors),
  StyleSetForegroundColor(crossterm::style::SetForegroundColor),
  StyleSetStyle(crossterm::style::SetStyle),
  StyleSetUnderlineColor(crossterm::style::SetUnderlineColor),
  StylePrintStyledContentString(crossterm::style::PrintStyledContent<String>),
  StylePrintString(crossterm::style::Print<String>),
  TerminalBeginSynchronizedUpdate(crossterm::terminal::BeginSynchronizedUpdate),
  TerminalClear(crossterm::terminal::Clear),
  TerminalDisableLineWrap(crossterm::terminal::DisableLineWrap),
  TerminalEnableLineWrap(crossterm::terminal::EnableLineWrap),
  TerminalEndSynchronizedUpdate(crossterm::terminal::EndSynchronizedUpdate),
  TerminalEnterAlternateScreen(crossterm::terminal::EnterAlternateScreen),
  TerminalLeaveAlternateScreen(crossterm::terminal::LeaveAlternateScreen),
  TerminalScrollDown(crossterm::terminal::ScrollDown),
  TerminalScrollUp(crossterm::terminal::ScrollUp),
  TerminalSetSize(crossterm::terminal::SetSize),
}

impl fmt::Debug for ShaderCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    let s = match self {
      ShaderCommand::CursorSetCursorStyle(command) => {
        format!(
          "CursorSetCursorStyle({:?})",
          CursorStyleFormatter::from(*command)
        )
      }
      ShaderCommand::CursorDisableBlinking(command) => {
        format!("CursorDisableBlinking({:?})", command)
      }
      ShaderCommand::CursorEnableBlinking(command) => {
        format!("CursorEnableBlinking({:?})", command)
      }
      ShaderCommand::CursorHide(command) => format!("CursorHide({:?})", command),
      ShaderCommand::CursorMoveDown(command) => {
        format!("CursorMoveDown({:?})", command)
      }
      ShaderCommand::CursorMoveLeft(command) => {
        format!("CursorMoveLeft({:?})", command)
      }
      ShaderCommand::CursorMoveRight(command) => {
        format!("CursorMoveRight({:?})", command)
      }
      ShaderCommand::CursorMoveTo(command) => format!("CursorMoveTo({:?})", command),
      ShaderCommand::CursorMoveToColumn(command) => {
        format!("CursorMoveToColumn({:?})", command)
      }
      ShaderCommand::CursorMoveToNextLine(command) => {
        format!("CursorMoveToNextLine({:?})", command)
      }
      ShaderCommand::CursorMoveToPreviousLine(command) => {
        format!("CursorMoveToPreviousLine({:?})", command)
      }
      ShaderCommand::CursorMoveToRow(command) => {
        format!("CursorMoveToRow({:?})", command)
      }
      ShaderCommand::CursorMoveUp(command) => format!("CursorMoveUp({:?})", command),
      ShaderCommand::CursorRestorePosition(command) => {
        format!("CursorRestorePosition({:?})", command)
      }
      ShaderCommand::CursorSavePosition(command) => {
        format!("CursorSavePosition({:?})", command)
      }
      ShaderCommand::CursorShow(command) => format!("CursorShow({:?})", command),
      ShaderCommand::EventDisableBracketedPaste(command) => {
        format!("EventDisableBracketedPaste({:?})", command)
      }
      ShaderCommand::EventDisableFocusChange(command) => {
        format!("EventDisableFocusChange({:?})", command)
      }
      ShaderCommand::EventDisableMouseCapture(command) => {
        format!("EventDisableMouseCapture({:?})", command)
      }
      ShaderCommand::EventEnableBracketedPaste(command) => {
        format!("EventEnableBracketedPaste({:?})", command)
      }
      ShaderCommand::EventEnableFocusChange(command) => {
        format!("EventEnableFocusChange({:?})", command)
      }
      ShaderCommand::EventEnableMouseCapture(command) => {
        format!("EventEnableMouseCapture({:?})", command)
      }
      ShaderCommand::EventPopKeyboardEnhancementFlags(command) => {
        format!("EventPopKeyboardEnhancementFlags({:?})", command)
      }
      ShaderCommand::EventPushKeyboardEnhancementFlags(command) => {
        format!("EventPushKeyboardEnhancementFlags({:?})", command)
      }
      ShaderCommand::StyleResetColor(command) => {
        format!("StyleResetColor({:?})", command)
      }
      ShaderCommand::StyleSetAttribute(command) => {
        format!("StyleSetAttribute({:?})", command)
      }
      ShaderCommand::StyleSetAttributes(command) => {
        format!("StyleSetAttributes({:?})", command)
      }
      ShaderCommand::StyleSetBackgroundColor(command) => {
        format!("StyleSetBackgroundColor({:?})", command)
      }
      ShaderCommand::StyleSetColors(command) => {
        format!("StyleSetColors({:?})", command)
      }
      ShaderCommand::StyleSetForegroundColor(command) => {
        format!("StyleSetForegroundColor({:?})", command)
      }
      ShaderCommand::StyleSetStyle(command) => {
        format!("StyleSetStyle({:?})", command)
      }
      ShaderCommand::StyleSetUnderlineColor(command) => {
        format!("StyleSetUnderlineColor({:?})", command)
      }
      ShaderCommand::StylePrintStyledContentString(command) => {
        format!("StylePrintStyledContentString({:?})", command)
      }
      ShaderCommand::StylePrintString(command) => {
        format!("StylePrintString({:?})", command)
      }
      ShaderCommand::TerminalBeginSynchronizedUpdate(command) => {
        format!("TerminalBeginSynchronizedUpdate({:?})", command)
      }
      ShaderCommand::TerminalClear(command) => {
        format!("TerminalClear({:?})", command)
      }
      ShaderCommand::TerminalDisableLineWrap(command) => {
        format!("TerminalDisableLineWrap({:?})", command)
      }
      ShaderCommand::TerminalEnableLineWrap(command) => {
        format!("TerminalEnableLineWrap({:?})", command)
      }
      ShaderCommand::TerminalEndSynchronizedUpdate(command) => {
        format!("TerminalEndSynchronizedUpdate({:?})", command)
      }
      ShaderCommand::TerminalEnterAlternateScreen(command) => {
        format!("TerminalEnterAlternateScreen({:?})", command)
      }
      ShaderCommand::TerminalLeaveAlternateScreen(command) => {
        format!("TerminalLeaveAlternateScreen({:?})", command)
      }
      ShaderCommand::TerminalScrollDown(command) => {
        format!("TerminalScrollDown({:?})", command)
      }
      ShaderCommand::TerminalScrollUp(command) => {
        format!("TerminalScrollUp({:?})", command)
      }
      ShaderCommand::TerminalSetSize(command) => {
        format!("TerminalSetSize({:?})", command)
      }
    };
    let s = format!("ShaderCommand::{}", s);
    f.debug_struct(&s).finish()
  }
}

#[derive(Debug, Default, Clone)]
/// The rendering updates on each draw, returns from [`Canvas::shade`] method.
///
/// It's simply a collection of [`ShaderCommand`].
pub struct Shader {
  commands: Vec<ShaderCommand>,
}

impl Shader {
  /// Make new shader.
  pub fn new() -> Self {
    Shader { commands: vec![] }
  }

  /// Push a shader command.
  pub fn push(&mut self, command: ShaderCommand) {
    self.commands.push(command)
  }

  /// Append a vector of shader commands.
  pub fn append(&mut self, commands: &mut Vec<ShaderCommand>) {
    self.commands.append(commands);
  }

  /// Get an iterator of the collection.
  pub fn iter(&self) -> Iter<ShaderCommand> {
    self.commands.iter()
  }
}

#[cfg(test)]
mod tests {
  use compact_str::CompactString;
  use std::sync::Once;
  use tracing::info;

  use crate::test::log::init as test_log_init;

  use super::*;

  static INIT: Once = Once::new();

  fn int2letter(i: u8) -> char {
    (i + 65) as char
  }

  #[test]
  fn new1() {
    let can = Canvas::new(U16Size::new(3, 4));
    assert_eq!(can.frame().size(), can.prev_frame().size());
    assert_eq!(*can.frame().cursor(), *can.prev_frame().cursor());
  }

  #[test]
  fn shader_command_debug1() {
    INIT.call_once(test_log_init);
    info!(
      "ShaderCommand::TerminalEndSynchronizedUpdate: {:?}",
      ShaderCommand::TerminalEndSynchronizedUpdate(crossterm::terminal::EndSynchronizedUpdate)
    );
    assert_eq!(
      format!(
        "{:?}",
        ShaderCommand::TerminalEndSynchronizedUpdate(crossterm::terminal::EndSynchronizedUpdate)
      ),
      "ShaderCommand::TerminalEndSynchronizedUpdate(EndSynchronizedUpdate)"
    );
    info!(
      "ShaderCommand::CursorSetCursorStyle(DefaultUserShape): {:?}",
      ShaderCommand::CursorSetCursorStyle(crossterm::cursor::SetCursorStyle::DefaultUserShape)
    );
    assert_eq!(
      format!(
        "{:?}",
        ShaderCommand::CursorSetCursorStyle(crossterm::cursor::SetCursorStyle::DefaultUserShape)
      ),
      "ShaderCommand::CursorSetCursorStyle(DefaultUserShape)"
    );
  }

  #[test]
  fn _shade_cursor1() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    let cursor1 = Cursor::default();
    can.frame_mut().set_cursor(cursor1);
    let actual1 = can._shade_cursor();
    can._shade_done();
    assert!(actual1.is_empty());

    let cursor2 = Cursor::new(point!(x:3, y:7), false, true, CursorStyle::BlinkingBar);
    can.frame_mut().set_cursor(cursor2);
    let actual2 = can._shade_cursor();
    can._shade_done();
    info!("actual2:{:?}", actual2);
    assert!(!actual2.is_empty());
    assert_eq!(actual2.len(), 4);
    assert_eq!(
      actual2
        .iter()
        .filter(
          |sh| if let ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(x, y)) = sh {
            *x == 3 && *y == 7
          } else {
            false
          }
        )
        .collect::<Vec<_>>()
        .len(),
      1
    );
    assert_eq!(
      actual2
        .iter()
        .filter(|sh| {
          matches!(
            sh,
            ShaderCommand::CursorDisableBlinking(crossterm::cursor::DisableBlinking)
          )
        })
        .collect::<Vec<_>>()
        .len(),
      1
    );
    assert_eq!(
      actual2
        .iter()
        .filter(|sh| { matches!(sh, ShaderCommand::CursorHide(crossterm::cursor::Hide)) })
        .collect::<Vec<_>>()
        .len(),
      1
    );
    assert_eq!(
      actual2
        .iter()
        .filter(|sh| {
          matches!(
            sh,
            ShaderCommand::CursorSetCursorStyle(crossterm::cursor::SetCursorStyle::BlinkingBar)
          )
        })
        .collect::<Vec<_>>()
        .len(),
      1
    );

    let cursor3 = Cursor::new(point!(x:4, y:5), true, true, CursorStyle::SteadyUnderScore);
    can.frame_mut().set_cursor(cursor3);
    let actual3 = can._shade_cursor();
    can._shade_done();
    info!("actual3:{:?}", actual3);
    assert_eq!(actual3.len(), 3);
    assert_eq!(
      actual3
        .iter()
        .filter(
          |sh| if let ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(x, y)) = sh {
            *x == 4 && *y == 5
          } else {
            false
          }
        )
        .collect::<Vec<_>>()
        .len(),
      1
    );
    assert_eq!(
      actual3
        .iter()
        .filter(|sh| {
          matches!(
            sh,
            ShaderCommand::CursorEnableBlinking(crossterm::cursor::EnableBlinking)
          )
        })
        .collect::<Vec<_>>()
        .len(),
      1
    );
    assert_eq!(
      actual3
        .iter()
        .filter(|sh| {
          matches!(
            sh,
            ShaderCommand::CursorSetCursorStyle(
              crossterm::cursor::SetCursorStyle::SteadyUnderScore
            )
          )
        })
        .collect::<Vec<_>>()
        .len(),
      1
    );
  }

  #[test]
  fn _next_same_cell_in_row1() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    can
      .frame_mut()
      .set_cells_at(point!(x:0,y:0), vec![Cell::with_char('A'); 20]);
    for i in 0..10 {
      let actual = can._next_same_cell_in_row(0, i);
      info!("1-{:?} actual:{:?}", i, actual);
      assert_eq!(actual, 10);
    }
    for i in 0..10 {
      let actual = can._next_same_cell_in_row(1, i);
      info!("2-{:?} actual:{:?}", i, actual);
      assert_eq!(actual, 10);
    }
  }

  #[test]
  fn _next_same_cell_in_row2() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    can.frame_mut().set_cells_at(
      point!(x:3,y:5),
      (0..9)
        .map(|i| Cell::with_char(int2letter(i)))
        .collect::<Vec<_>>(),
    );
    let chars = (0_u8..9_u8)
      .map(|i| int2letter(i).to_compact_string())
      .collect::<Vec<_>>();
    info!(
      "frame:{:?}",
      can
        .frame()
        .raw_symbols_with_placeholder(" ".to_compact_string())
        .iter()
        .map(|cs| cs
          .iter()
          .map(CompactString::to_string)
          .collect::<Vec<_>>()
          .join(""))
        .collect::<Vec<_>>()
    );
    for col in 0..10 {
      for row in 0..10 {
        let actual = can._next_same_cell_in_row(row, col);
        info!("row:{:?}, col:{:?}, actual:{:?}", row, col, actual);
        if !(5..7).contains(&row) {
          assert_eq!(actual, col);
        } else if row == 5 && (3..10).contains(&col) {
          assert_eq!(actual, 10);
          info!(
            "chars:{:?}, symbol:{:?}",
            chars,
            can.frame().get_cell(point!(x:col, y:row)).symbol()
          );
          assert!(chars.contains(can.frame().get_cell(point!(x:col, y:row)).symbol()));
        } else if row == 6 && (0..2).contains(&col) {
          assert_eq!(actual, 2);
          info!(
            "chars:{:?}, symbol:{:?}",
            chars,
            can.frame().get_cell(point!(x:col, y:row)).symbol()
          );
          assert!(chars.contains(can.frame().get_cell(point!(x:col, y:row)).symbol()));
        } else {
          assert_eq!(actual, col);
        }
      }
    }
  }

  #[test]
  fn _next_same_cell_in_row3() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    can.frame_mut().set_cells_at(
      point!(x:2,y:3),
      (0..4)
        .map(|i| Cell::with_char(int2letter(i)))
        .collect::<Vec<_>>(),
    );
    let mut char_index = 0_u8;
    info!(
      "frame:{:?}",
      can
        .frame()
        .raw_symbols_with_placeholder(" ".to_compact_string())
        .iter()
        .map(|cs| cs
          .iter()
          .map(CompactString::to_string)
          .collect::<Vec<_>>()
          .join(""))
        .collect::<Vec<_>>()
    );
    for col in 0..10 {
      for row in 0..10 {
        let actual = can._next_same_cell_in_row(row, col);
        info!("row:{:?}, col:{:?}, actual:{:?}", row, col, actual);
        if row != 3 {
          assert_eq!(actual, col);
        } else if (2..6).contains(&col) {
          assert_eq!(actual, 6);
          assert_eq!(
            int2letter(char_index).to_compact_string(),
            can.frame().get_cell(point!(x:col, y:row)).symbol()
          );
          char_index += 1;
        } else {
          assert_eq!(actual, col);
        }
      }
    }
  }

  #[test]
  fn _make_print_shader1() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    can.frame_mut().set_cells_at(
      point!(x:2,y:3),
      (0..4)
        .map(|i| Cell::with_char(int2letter(i)))
        .collect::<Vec<_>>(),
    );
    let col = 2;
    let row = 3;
    let col_end_at = can._next_same_cell_in_row(row, col);
    let shaders = can._make_print_shaders(row, col, col_end_at);
    info!("shader:{:?}", shaders);
    assert_eq!(shaders.len(), 2);
    assert!(matches!(
      shaders[0],
      ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(_, _))
    ));
    assert!(matches!(
      shaders[1],
      ShaderCommand::StylePrintString(crossterm::style::Print(_))
    ));
    if let ShaderCommand::StylePrintString(crossterm::style::Print(contents)) = &shaders[1] {
      assert_eq!(*contents, "ABCD".to_string());
    }
  }

  #[test]
  fn diff1() {
    INIT.call_once(test_log_init);
    let mut can = Canvas::new(U16Size::new(10, 10));

    can.frame_mut().set_cells_at(
      point!(x:2,y:3),
      (0..4)
        .map(|i| Cell::with_char(int2letter(i)))
        .collect::<Vec<_>>(),
    );
    let actual1 = can._dirty_marks_diff();
    let actual2 = can._brute_force_diff();
    info!("dirty marks:{:?}", actual1);
    info!("brute force:{:?}", actual2);
    assert_eq!(actual1.len(), 2);
    assert!(matches!(
      actual1[0],
      ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(_, _))
    ));
    assert!(matches!(
      actual1[1],
      ShaderCommand::StylePrintString(crossterm::style::Print(_))
    ));
    if let ShaderCommand::StylePrintString(crossterm::style::Print(contents)) = &actual1[1] {
      assert_eq!(*contents, "ABCD".to_string());
    }
    assert_eq!(actual2.len(), 2);
    assert!(matches!(
      actual2[0],
      ShaderCommand::CursorMoveTo(crossterm::cursor::MoveTo(_, _))
    ));
    assert!(matches!(
      actual2[1],
      ShaderCommand::StylePrintString(crossterm::style::Print(_))
    ));
    if let ShaderCommand::StylePrintString(crossterm::style::Print(contents)) = &actual2[1] {
      assert_eq!(*contents, "ABCD".to_string());
    }
  }
}
