//! Main event loop for TUI application.

#![allow(unused_imports, dead_code)]

use crate::cart::{IRect, Size, U16Rect, U16Size, URect};
use crate::geo_size_as;
use crate::ui::frame::CursorStyle;
use crate::ui::term::{Terminal, TerminalArc};
use crate::ui::tree::{Tree, TreeArc, TreeNode, TreeNodeArc};
use crate::ui::widget::{
  Cursor, RootContainer, Widget, WidgetValue, WindowContainer, WindowContent,
};
use crossterm::event::{
  DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture, Event,
  EventStream, KeyCode, KeyEventKind, KeyEventState, KeyModifiers,
};
use crossterm::{cursor as termcursor, queue, terminal};
use futures::StreamExt;
use geo::point;
use heed::types::U16;
use std::borrow::Borrow;
use std::io::{Result as IoResult, Write};
use std::sync::Arc;
use tracing::{debug, error};

pub struct EventLoop {
  screen: TerminalArc,
  tree: TreeArc,
}

impl EventLoop {
  pub async fn new() -> IoResult<Self> {
    let (cols, rows) = terminal::size()?;
    let screen_size = U16Size::new(cols, rows);
    let screen = Terminal::new(screen_size);
    let screen = Terminal::to_arc(screen);
    let mut tree = Tree::new(Arc::downgrade(&screen));
    debug!("new, screen size: {:?}", screen_size);

    let root_container = RootContainer::default();
    let root_container_shape = IRect::new(
      (0, 0),
      (screen_size.width() as isize, screen_size.height() as isize),
    );
    let root_container_node = TreeNode::new(
      None,
      WidgetValue::RootContainer(root_container),
      root_container_shape,
    );
    let root_container_node = TreeNode::to_arc(root_container_node);
    tree.insert(None, root_container_node.clone());

    let window_container = WindowContainer::default();
    let window_container_shape = IRect::new(
      (0, 0),
      (screen_size.width() as isize, screen_size.height() as isize),
    );
    let window_container_node = TreeNode::new(
      Some(Arc::downgrade(&root_container_node)),
      WidgetValue::WindowContainer(window_container),
      window_container_shape,
    );
    let window_container_node = TreeNode::to_arc(window_container_node);
    tree.insert(
      Some(root_container_node.clone()),
      window_container_node.clone(),
    );

    let window_content = WindowContent::default();
    let window_content_shape = IRect::new(
      (0, 0),
      (screen_size.width() as isize, screen_size.height() as isize),
    );
    let window_content_node = TreeNode::new(
      Some(Arc::downgrade(&window_container_node)),
      WidgetValue::WindowContent(window_content),
      window_content_shape,
    );
    let window_content_node = TreeNode::to_arc(window_content_node);
    tree.insert(
      Some(window_container_node.clone()),
      window_content_node.clone(),
    );

    let cursor = Cursor::default();
    let cursor_shape = IRect::new((0, 0), (1, 1));
    let cursor_node = TreeNode::new(
      Some(Arc::downgrade(&window_content_node)),
      WidgetValue::Cursor(cursor),
      cursor_shape,
    );
    let cursor_node = TreeNode::to_arc(cursor_node);
    tree.insert(Some(window_container_node.clone()), cursor_node.clone());

    debug!("new, built widget tree");

    Ok(EventLoop {
      screen,
      tree: Tree::to_arc(tree),
    })
  }

  pub async fn init(&self) -> IoResult<()> {
    let mut out = std::io::stdout();

    debug!("init, draw cursor");
    let screen_guard = self.screen.lock();
    let cursor = screen_guard.borrow().frame().cursor;
    if cursor.blinking {
      queue!(out, termcursor::EnableBlinking)?;
    } else {
      queue!(out, termcursor::DisableBlinking)?;
    }
    if cursor.hidden {
      queue!(out, termcursor::Hide)?;
    } else {
      queue!(out, termcursor::Show)?;
    }

    queue!(out, cursor.style)?;
    queue!(out, termcursor::MoveTo(cursor.pos.x(), cursor.pos.y()))?;

    out.flush()?;
    debug!("init, draw cursor - done");

    Ok(())
  }

  pub async fn run(&mut self) -> IoResult<()> {
    let mut reader = EventStream::new();
    loop {
      tokio::select! {
        polled_event = reader.next() => match polled_event {
          Some(Ok(event)) => {
            debug!("run, polled event: {:?}", event);
            if !self.accept(event).await {
                break;
            }
          },
          Some(Err(e)) => {
            debug!("run, error: {:?}", e);
            error!("Error: {:?}\r", e);
            break;
          },
          None => break,
        }
      }
    }
    Ok(())
  }

  pub async fn accept(&mut self, event: Event) -> bool {
    debug!("Event::{:?}", event);
    println!("Event:{:?}", event);

    // match event {
    //   Event::FocusGained => {}
    //   Event::FocusLost => {}
    //   Event::Key(key_event) => match key_event.kind {
    //     KeyEventKind::Press => {}
    //     KeyEventKind::Repeat => {}
    //     KeyEventKind::Release => {}
    //   },
    //   Event::Mouse(_mouse_event) => {}
    //   Event::Paste(ref _paste_string) => {}
    //   Event::Resize(_columns, _rows) => {}
    // }

    // if event == Event::Key(KeyCode::Char('c').into()) {
    //   println!("Curosr position: {:?}\r", termcursor::position());
    // }

    // quit loop
    if event == Event::Key(KeyCode::Esc.into()) {
      println!("ESC: {:?}\r", termcursor::position());
      return false;
    }

    // continue loop
    true
  }
}
