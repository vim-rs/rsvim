//! Frame for terminal rendering.

#![allow(dead_code)]

use std::ops::Range;
use std::vec::Splice;

use crate::cart::{U16Size, UPos};
use crate::ui::canvas::frame::cell::Cell;
use crate::ui::canvas::frame::cursor::Cursor;

pub mod cell;
pub mod cursor;

#[derive(Debug, Clone)]
/// Logical frame for the canvas.
///
/// When UI widget tree drawing on the canvas, it actually draws on the current frame. Then the
/// canvas will diff the changes made by UI tree, and only print the changes to hardware device.
pub struct Frame {
  size: U16Size,
  cells: Vec<Cell>,
  cursor: Cursor,

  /// Indicate which part of the frame is dirty, i.e. it's been drawn by the UI widget tree. When
  /// rendering to the hardware device, only dirty parts will be printed.
  dirty_cells: Vec<Range<usize>>,
  dirty_cursor: bool,
}

impl Frame {
  /// Make new frame.
  pub fn new(size: U16Size, cursor: Cursor) -> Self {
    let n = size.height() as usize * size.width() as usize;
    Frame {
      size,
      cells: vec![Cell::default(); n],
      cursor,
      dirty_cells: vec![], // When first create, it's not dirty.
      dirty_cursor: false,
    }
  }

  /// Get current frame size.
  pub fn size(&self) -> U16Size {
    self.size
  }

  /// Set current frame size.
  pub fn set_size(&mut self, size: U16Size) -> U16Size {
    let old_size = self.size;
    self.size = size;
    old_size
  }

  /// Get a cell.
  pub fn cell(&self, pos: UPos) -> &Cell {
    &self.cells[pos.x() * pos.y()]
  }

  /// Set a cell.
  pub fn set_cell(&mut self, pos: UPos, cell: Cell) -> Cell {
    let index = pos.x() * pos.y();
    let old = self.cells[index].clone();
    self.cells[index] = cell;
    self.dirty_cells.push(index..(index + 1));
    old
  }

  /// Get n continuously cells, start from position.
  pub fn cells(&self, pos: UPos, n: usize) -> &[Cell] {
    let start_at = pos.x() * pos.y();
    let end_at = start_at + n;
    &self.cells[start_at..end_at]
  }

  /// Set continuously cells, start from position.
  /// Returns n old cells.
  pub fn set_cells(
    &mut self,
    pos: UPos,
    cells: Vec<Cell>,
  ) -> Splice<'_, <Vec<Cell> as IntoIterator>::IntoIter> {
    let start_at = pos.x() * pos.y();
    let end_at = start_at + cells.len();
    self.dirty_cells.push(start_at..end_at);
    self.cells.splice(start_at..end_at, cells)
  }

  /// Get dirty cells.
  pub fn dirty_cells(&self) -> &Vec<Range<usize>> {
    &self.dirty_cells
  }

  /// Get cursor.
  pub fn cursor(&self) -> &Cursor {
    &self.cursor
  }

  /// Set cursor.
  pub fn set_cursor(&mut self, cursor: Cursor) {
    if self.cursor != cursor {
      self.cursor = cursor;
      self.dirty_cursor = true;
    }
  }

  /// Whether cursor is dirty.
  pub fn dirty_cursor(&self) -> bool {
    self.dirty_cursor
  }

  /// Reset/clean all dirty components.
  ///
  /// Note: This method should be called after each frame been flushed to terminal device.
  pub fn reset_dirty(&mut self) {
    self.dirty_cells = vec![];
    self.dirty_cursor = false;
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn new1() {
    let sz = U16Size::new(2, 1);
    let f = Frame::new(sz, Cursor::default());
    assert_eq!(f.size.width, 2);
    assert_eq!(f.size.height, 1);
    assert_eq!(
      f.cells.len(),
      f.size.height as usize * f.size.width as usize
    );
    for c in f.cells.iter() {
      assert_eq!(c.symbol(), Cell::default().symbol());
    }
  }

  #[test]
  fn set_cells1() {
    let sz = U16Size::new(10, 10);
    let _f = Frame::new(sz, Cursor::default());
  }
}
