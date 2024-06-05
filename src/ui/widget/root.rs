//! Root widget, it always exists along with RSVIM, as a root container for all other widgets.

use crate::geo::pos::{IPos, UPos};
use crate::geo::size::Size;
use crate::ui::term::Terminal;
use crate::ui::widget::{ChildWidgetsRw, Widget, WidgetRw};
use crate::uuid;
use std::sync::{Arc, RwLock};

pub struct RootWidget {
  id: usize,
  offset: IPos,
  abs_offset: UPos,
  size: Size,
  visible: bool,
  enabled: bool,
  children: ChildWidgetsRw,
}

impl RootWidget {
  pub fn new(size: Size) -> Self {
    RootWidget {
      id: uuid::next(),
      offset: IPos::new(0, 0),
      abs_offset: UPos::new(0, 0),
      size,
      visible: true,
      enabled: true,
      children: Arc::new(RwLock::new(vec![])),
    }
  }
}

impl Widget for RootWidget {
  fn id(&self) -> usize {
    self.id
  }

  fn offset(&self) -> IPos {
    self.offset
  }

  fn set_offset(&mut self, _: IPos) {}

  fn abs_offset(&self) -> UPos {
    self.abs_offset
  }

  fn size(&self) -> Size {
    self.size
  }

  fn set_size(&mut self, value: Size) {
    self.size = value;
  }

  fn zindex(&self) -> usize {
    0
  }

  fn set_zindex(&mut self, _: usize) {}

  fn visible(&self) -> bool {
    self.visible
  }

  fn set_visible(&mut self, value: bool) {
    self.visible = value;
  }

  fn enabled(&self) -> bool {
    self.enabled
  }

  fn set_enabled(&mut self, value: bool) {
    self.enabled = value;
  }

  fn parent(&self) -> Option<WidgetRw> {
    None
  }

  fn set_parent(&mut self, _: Option<WidgetRw>) {
    unimplemented!();
  }

  fn children(&self) -> ChildWidgetsRw {
    self.children.clone()
  }

  fn find_children(&self, _id: usize) -> Option<WidgetRw> {
    None
  }

  fn find_direct_children(&self, _id: usize) -> Option<WidgetRw> {
    None
  }

  fn draw(&self, _terminal: &Terminal) {
    todo!();
  }
}
