//! # Terminal Panel
//!
//! Terminal emulation using Alacritty backend.
//! Supports both local and remote sessions.

use std::collections::VecDeque;

/// PTY (Pseudo Terminal) handle for sending input and resize events
#[derive(Debug)]
pub struct PtyHandle {
    /// Input sender channel
    input_tx: Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>,
    /// Resize sender channel
    resize_tx: Option<tokio::sync::mpsc::UnboundedSender<(u16, u16)>>,
}

impl PtyHandle {
    pub fn new() -> Self {
        Self {
            input_tx: None,
            resize_tx: None,
        }
    }
    
    pub fn with_channels(
        input_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
        resize_tx: tokio::sync::mpsc::UnboundedSender<(u16, u16)>,
    ) -> Self {
        Self {
            input_tx: Some(input_tx),
            resize_tx: Some(resize_tx),
        }
    }
    
    /// Send input data to PTY
    pub fn send_input(&self, data: &[u8]) {
        if let Some(ref tx) = self.input_tx {
            let _ = tx.send(data.to_vec());
        }
    }
    
    /// Send resize event to PTY
    pub fn send_resize(&self, cols: u16, rows: u16) {
        if let Some(ref tx) = self.resize_tx {
            let _ = tx.send((cols, rows));
        }
    }
}

impl Default for PtyHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal session type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalType {
    Local,
    Remote { session_id: usize },
}

/// Terminal panel
pub struct TerminalPanel {
    terminal_type: TerminalType,
    scrollback: VecDeque<String>,
    cursor_x: usize,
    cursor_y: usize,
    /// PTY handle for I/O operations
    pty_handle: PtyHandle,
    /// Terminal size (cols, rows)
    size: (u16, u16),
}

impl TerminalPanel {
    pub fn new(terminal_type: TerminalType) -> Self {
        Self {
            terminal_type,
            scrollback: VecDeque::with_capacity(10000),
            cursor_x: 0,
            cursor_y: 0,
            pty_handle: PtyHandle::new(),
            size: (80, 24), // Default terminal size
        }
    }
    
    /// Create terminal panel with PTY handle
    pub fn with_pty(terminal_type: TerminalType, pty_handle: PtyHandle) -> Self {
        Self {
            terminal_type,
            scrollback: VecDeque::with_capacity(10000),
            cursor_x: 0,
            cursor_y: 0,
            pty_handle,
            size: (80, 24),
        }
    }
    
    /// Set PTY handle
    pub fn set_pty_handle(&mut self, pty_handle: PtyHandle) {
        self.pty_handle = pty_handle;
    }
    
    /// Get terminal size
    pub fn size(&self) -> (u16, u16) {
        self.size
    }
    
    pub fn local() -> Self {
        Self::new(TerminalType::Local)
    }
    
    pub fn remote(session_id: usize) -> Self {
        Self::new(TerminalType::Remote { session_id })
    }
    
    /// Write output to terminal
    pub fn write(&mut self, data: &str) {
        for line in data.lines() {
            self.scrollback.push_back(line.to_string());
        }
        
        // Trim scrollback
        while self.scrollback.len() > 10000 {
            self.scrollback.pop_front();
        }
    }
    
    /// Get visible lines
    pub fn visible_lines(&self, _height: usize) -> Vec<&str> {
        self.scrollback.iter()
            .map(|s| s.as_str())
            .collect()
    }
    
    /// Handle input - sends data to PTY
    pub fn input(&mut self, data: &[u8]) {
        // Send input to PTY
        self.pty_handle.send_input(data);
        
        // Also echo to local scrollback for feedback
        if let Ok(text) = std::str::from_utf8(data) {
            for line in text.lines() {
                self.scrollback.push_back(format!("> {}", line));
            }
        }
        
        // Trim scrollback
        while self.scrollback.len() > 10000 {
            self.scrollback.pop_front();
        }
    }
    
    /// Resize terminal - sends resize event to PTY
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.size = (cols, rows);
        self.pty_handle.send_resize(cols, rows);
    }
    
    /// Clear screen
    pub fn clear(&mut self) {
        self.scrollback.clear();
    }
}

/// Terminal multiplexer (tabs)
pub struct TerminalMux {
    terminals: Vec<TerminalPanel>,
    active: usize,
}

impl TerminalMux {
    pub fn new() -> Self {
        Self {
            terminals: vec![TerminalPanel::local()],
            active: 0,
        }
    }
    
    pub fn add_terminal(&mut self, panel: TerminalPanel) -> usize {
        self.terminals.push(panel);
        self.terminals.len() - 1
    }
    
    pub fn switch_to(&mut self, idx: usize) {
        if idx < self.terminals.len() {
            self.active = idx;
        }
    }
    
    pub fn active(&self) -> &TerminalPanel {
        &self.terminals[self.active]
    }
    
    pub fn active_mut(&mut self) -> &mut TerminalPanel {
        &mut self.terminals[self.active]
    }
    
    pub fn close(&mut self, idx: usize) {
        if self.terminals.len() > 1 && idx < self.terminals.len() {
            self.terminals.remove(idx);
            if self.active >= idx && self.active > 0 {
                self.active -= 1;
            }
        }
    }
}

impl Default for TerminalMux {
    fn default() -> Self {
        Self::new()
    }
}
