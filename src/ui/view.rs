use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::ui::rect::{IPos, Size, UPos};
use crate::ui::term::Terminal;

pub mod root_view;
pub mod window;

/// View
pub trait View {
  /// (Relative) x/y offset vased on parent view
  fn offset(&self) -> IPos;

  /// Absolute x/y offset based on terminal
  fn abs_offset(&self) -> UPos;

  /// Rectangle height/width
  fn size(&self) -> Size;

  /// Control arrange content layout when multiple views conflict on each other.
  fn zindex(&self) -> usize;

  /// Parent view of this view.
  /// Note: Root view doesn't have a parent view.
  fn parent(&self) -> Option<ViewWk>;

  /// Draw the view to canvas buffer.
  fn draw(&self, terminal: &Terminal);
}

pub type ViewRc = Rc<RefCell<dyn View>>;
pub type ViewWk = Weak<RefCell<dyn View>>;
