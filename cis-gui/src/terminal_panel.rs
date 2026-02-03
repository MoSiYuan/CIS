//! # Terminal Panel
//!
//! Terminal emulation using Alacritty backend.
//! Supports both local and remote sessions.

use std::collections::VecDeque;

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
}

impl TerminalPanel {
    pub fn new(terminal_type: TerminalType) -> Self {
        Self {
            terminal_type,
            scrollback: VecDeque::with_capacity(10000),
            cursor_x: 0,
            cursor_y: 0,
        }
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
    
    /// Handle input
    pub fn input(&mut self, _data: &[u8]) {
        // TODO: Send to PTY
    }
    
    /// Resize terminal
    pub fn resize(&mut self, _cols: u16, _rows: u16) {
        // TODO: Send resize to PTY
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
