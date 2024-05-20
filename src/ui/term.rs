//! Backend terminal for receiving user inputs & canvas for UI rendering.

use crate::ui::geo::size::Size;
use crate::ui::term::buffer::Buffer;
use crossterm::event::{
  DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
};
use crossterm::event::{Event, EventStream, KeyCode};
use crossterm::{cursor, queue, terminal};
use futures::StreamExt;
use std::io::{Result, Write};
use tracing::debug;

pub mod buffer;
pub mod cell;

/// Backend terminal
pub struct Terminal {
  buf: Buffer,
  prev_buf: Buffer,
}

impl Terminal {
  pub async fn init() -> Result<Self> {
    if !terminal::is_raw_mode_enabled()? {
      terminal::enable_raw_mode()?;
    }

    let (cols, rows) = terminal::size()?;
    let size = Size::new(rows as usize, cols as usize);
    let mut out = std::io::stdout();

    queue!(out, EnableMouseCapture)?;
    queue!(out, EnableFocusChange)?;

    queue!(
      out,
      terminal::EnterAlternateScreen,
      terminal::Clear(terminal::ClearType::All),
      cursor::SetCursorStyle::BlinkingBlock,
      cursor::MoveTo(0, 0),
      cursor::Show,
    )?;

    out.flush()?;

    let t = Terminal {
      prev_buf: Buffer::new(size),
      buf: Buffer::new(size),
    };
    Ok(t)
  }

  pub async fn shutdown(&self) -> Result<()> {
    let mut out = self.out();
    queue!(
      out,
      DisableMouseCapture,
      DisableFocusChange,
      terminal::LeaveAlternateScreen,
    )?;

    out.flush()?;

    if terminal::is_raw_mode_enabled()? {
      terminal::disable_raw_mode()?;
    }

    Ok(())
  }

  pub async fn run(&mut self) -> Result<()> {
    let mut reader = EventStream::new();
    loop {
      tokio::select! {
        maybe_event = reader.next() => match maybe_event {
          Some(Ok(event)) => {
            println!("Event::{:?}\r", event);
            debug!("Event::{:?}", event);

            if event == Event::Key(KeyCode::Char('c').into()) {
              println!("Curosr position: {:?}\r", cursor::position());
            }

            if event == Event::Key(KeyCode::Esc.into()) {
              break;
            }
          }
          Some(Err(e)) => println!("Error: {:?}\r", e),
          None => break,
        }
      }
    }
    Ok(())
  }

  pub fn size(&self) -> Size {
    self.buf.size
  }

  pub fn prev_size(&self) -> Size {
    self.prev_buf.size
  }

  pub fn flush(&mut self) {
    self.prev_buf = self.buf.clone();
  }

  pub fn out(&self) -> std::io::Stdout {
    std::io::stdout()
  }

  pub fn err(&self) -> std::io::Stderr {
    std::io::stderr()
  }

  #[cfg(test)]
  pub fn new_from_size(size: Size) -> Self {
    Terminal {
      prev_buf: Buffer::new(size),
      buf: Buffer::new(size),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn should_equal_on_terminal_new() {
    let sz = Size::new(1, 2);
    let c1 = Terminal::new_from_size(sz);
    assert_eq!(c1.size(), c1.prev_size());
  }
}
