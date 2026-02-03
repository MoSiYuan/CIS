直接启动 `claude`（或 `cis` CLI）在 PTY 里，**真终端 + 真 GUI 按钮**。

## 1. 依赖（Cargo.toml）

```toml
[dependencies]
egui = "0.28"
eframe = { version = "0.28", features = ["default_fonts", "wgpu"] }
alacritty_terminal = "0.23"
portable-pty = "0.8"
crossbeam = "0.8"
libc = "0.2"  # 用于 PTY 设置

[target.'cfg(target_os = "macos")'.dependencies]
objc = "0.2"
```

## 2. 主代码（main.rs）

```rust
use alacritty_terminal::{
    config::Config,
    event::{Event, EventListener},
    grid::Dimensions,
    index::{Column, Line},
    term::{Term, TermMode},
    vte::ansi::Color as AnsiColor,
};
use eframe::egui;
use portable_pty::{CommandBuilder, NativePtySystem, PtyPair, PtySize, PtySystem};
use std::sync::{Arc, Mutex};

/// 终端 GUI，内部运行 claude/cis 等 CLI
pub struct TermGui {
    // PTY 状态
    pty_pair: Arc<Mutex<PtyPair>>,
    pty_writer: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    
    // 终端模拟器（alacritty 核心）
    term: Arc<Mutex<Term<EventProxy>>>,
    
    // 渲染缓存（从 Term 同步到此处）
    screen_buffer: Vec<Vec<TermCell>>,
    cursor: (usize, usize),
    
    // GUI 状态
    matrix_connected: bool,
    show_node_menu: bool,
    
    // 输入处理
    input_buffer: String,
    last_update: std::time::Instant,
}

#[derive(Clone, Default)]
struct TermCell {
    ch: char,
    fg: egui::Color32,
    bg: egui::Color32,
    flags: alacritty_terminal::grid::Flags,
}

impl TermGui {
    pub fn new() -> Self {
        // 1. 创建 PTY
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 40,
                cols: 100,
                pixel_width: 800,
                pixel_height: 600,
            })
            .expect("Failed to open PTY");
        
        // 2. 启动 claude（或 ./cis-core，或 /bin/bash）
        let cmd = CommandBuilder::new("claude"); // 或 "cis", "bash", "zsh"
        // 如果 claude 需要特定工作目录：
        // cmd.cwd("/path/to/project");
        let _child = pair.slave.spawn_command(cmd).expect("Failed to spawn");
        
        // 3. 初始化 alacritty Terminal（用于解析 VT100）
        let config = Config::default();
        let term = Term::new(&config, &(), alacritty_terminal::event::WindowSize::default(), EventProxy);
        
        let term_arc = Arc::new(Mutex::new(term));
        let pair_arc = Arc::new(Mutex::new(pair));
        
        // 4. 启动读取线程（PTY -> alacritty -> GUI）
        let term_clone = term_arc.clone();
        let mut reader = pair_arc.lock().unwrap().master.try_clone_reader().expect("Failed to clone reader");
        
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let mut term = term_clone.lock().unwrap();
                        // 关键：解析 ANSI/VT100 序列并更新内部屏幕状态
                        term.advance_bytes(&buf[..n]);
                        drop(term);
                        // 通知 GUI 刷新（简单方式：直接依赖 egui 的连续刷新）
                    }
                    Err(e) => {
                        eprintln!("PTY read error: {}", e);
                        break;
                    }
                }
            }
        });
        
        // 5. 获取写入句柄（GUI -> PTY）
        let writer = pair_arc.lock().unwrap().master.take_writer().expect("Failed to take writer");
        
        Self {
            pty_pair: pair_arc,
            pty_writer: Arc::new(Mutex::new(writer)),
            term: term_arc,
            screen_buffer: vec![],
            cursor: (0, 0),
            matrix_connected: false,
            show_node_menu: false,
            input_buffer: String::new(),
            last_update: std::time::Instant::now(),
        }
    }
    
    /// 同步 alacritty Term 状态到 egui 可渲染格式
    fn sync_screen(&mut self) {
        let term = self.term.lock().unwrap();
        let grid = term.grid();
        let rows = grid.screen_lines().0;
        let cols = grid.columns().0;
        
        let mut new_buffer = vec![vec![TermCell::default(); cols]; rows];
        
        // 从 Term 的 grid 复制可见内容
        for row in 0..rows {
            let line = &grid[Line(row)];
            for col in 0..cols {
                let cell = &line[Column(col)];
                let (fg, bg) = convert_color(cell.fg(), cell.bg());
                new_buffer[row][col] = TermCell {
                    ch: cell.c(),
                    fg,
                    bg,
                    flags: cell.flags(),
                };
            }
        }
        
        // 获取光标位置
        let cursor = term.cursor();
        self.cursor = (cursor.point.column.0, cursor.point.line.0);
        self.screen_buffer = new_buffer;
    }
    
    /// 发送输入到 PTY（处理特殊键）
    fn send_input(&mut self, key: egui::Key, text: &str, modifiers: &egui::Modifiers) {
        let mut writer = self.pty_writer.lock().unwrap();
        
        // 处理特殊键映射
        let input_bytes: Vec<u8> = match key {
            egui::Key::Enter => vec![b'\r'],
            egui::Key::Backspace => vec![0x7f], // DEL
            egui::Key::Tab => vec![b'\t'],
            egui::Key::Escape => vec![0x1b],
            egui::Key::ArrowUp => vec![0x1b, 0x5b, 0x41],    // ESC[A
            egui::Key::ArrowDown => vec![0x1b, 0x5b, 0x42],  // ESC[B
            egui::Key::ArrowRight => vec![0x1b, 0x5b, 0x43], // ESC[C
            egui::Key::ArrowLeft => vec![0x1b, 0x5b, 0x44],  // ESC[D
            _ => {
                // 普通字符
                if !text.is_empty() {
                    text.as_bytes().to_vec()
                } else {
                    return;
                }
            }
        };
        
        use std::io::Write;
        let _ = writer.write_all(&input_bytes);
        let _ = writer.flush();
    }
}

impl eframe::App for TermGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 持续刷新（终端需要高帧率响应）
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        
        // 同步终端内容（每帧从 alacritty Term 复制）
        if self.last_update.elapsed() > std::time::Duration::from_millis(16) {
            self.sync_screen();
            self.last_update = std::time::Instant::now();
        }
        
        // 1. 顶部 Matrix 控制栏（真 GUI）
        egui::TopBottomPanel::top("control_bar")
            .exact_height(32.0)
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(40, 40, 40)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    
                    // Matrix 连接按钮（真·GUI 按钮，控制 Matrix 网络）
                    let (label, color) = if self.matrix_connected {
                        (" ● MATRIX ", egui::Color32::from_rgb(46, 204, 113))
                    } else {
                        (" ○ MATRIX ", egui::Color32::from_rgb(231, 76, 60))
                    };
                    
                    if ui.add(
                        egui::Button::new(egui::RichText::new(label).monospace())
                            .fill(color)
                            .rounding(4.0)
                    ).clicked() {
                        self.matrix_connected = !self.matrix_connected;
                        // 向终端内发送命令（或直接调用 6767）
                        let cmd = if self.matrix_connected { "matrix connect\n" } else { "matrix disconnect\n" };
                        let mut writer = self.pty_writer.lock().unwrap();
                        use std::io::Write;
                        let _ = writer.write_all(cmd.as_bytes());
                    }
                    
                    ui.separator();
                    
                    // 节点下拉菜单按钮
                    let node_btn = egui::Button::new(
                        egui::RichText::new(" ≡ 节点 ▼ ").monospace().color(egui::Color32::LIGHT_GRAY)
                    ).fill(egui::Color32::from_gray(60));
                    
                    if ui.add(node_btn).clicked() {
                        self.show_node_menu = !self.show_node_menu;
                    }
                    
                    if self.show_node_menu {
                        egui::Window::new("nodes")
                            .fixed_pos(ui.next_widget_position())
                            .fixed_size([200.0, 150.0])
                            .collapsible(false)
                            .title_bar(false)
                            .show(ctx, |ui| {
                                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                                if ui.button("Munin-macmini (本机)").clicked() {
                                    let mut writer = self.pty_writer.lock().unwrap();
                                    use std::io::Write;
                                    let _ = writer.write_all(b"node select munin-macmini\n");
                                    self.show_node_menu = false;
                                }
                                if ui.button("Hugin-pc").clicked() {
                                    // ...
                                }
                            });
                    }
                    
                    // 右侧控制
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(" × ").clicked() {
                            // 向 PTY 发送 Ctrl+C
                            let mut writer = self.pty_writer.lock().unwrap();
                            use std::io::Write;
                            let _ = writer.write_all(&[0x03]); // ETX (Ctrl+C)
                        }
                    });
                });
            });
        
        // 2. 主终端显示区（alacritty 渲染）
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::BLACK))
            .show(ctx, |ui| {
                let available = ui.available_size();
                
                // 调整 PTY 大小以适应窗口（响应式）
                let cols = (available.x / 9.0) as u16;  // 估算字符宽度
                let rows = (available.y / 16.0) as u16; // 估算行高
                if cols > 0 && rows > 0 {
                    let mut pair = self.pty_pair.lock().unwrap();
                    let _ = pair.master.resize(PtySize {
                        rows,
                        cols,
                        pixel_width: available.x as u16,
                        pixel_height: available.y as u16,
                    });
                }
                
                // 使用等宽字体显示终端内容
                ui.style_mut().override_font_id = Some(
                    egui::FontId::monospace(14.0)
                );
                
                // 绘制屏幕缓冲区
                ui.vertical(|ui| {
                    for (row_idx, line) in self.screen_buffer.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0; // 字符紧密排列
                            
                            for (col_idx, cell) in line.iter().enumerate() {
                                let is_cursor = (col_idx, row_idx) == self.cursor;
                                
                                // 光标样式（反转颜色或方块）
                                let (fg, bg) = if is_cursor {
                                    (cell.bg, cell.fg) // 反转
                                } else {
                                    (cell.fg, cell.bg)
                                };
                                
                                let text = if cell.ch == '\0' || cell.ch == ' ' {
                                    " ".to_string()
                                } else {
                                    cell.ch.to_string()
                                };
                                
                                ui.colored_label(
                                    egui::RichText::new(text)
                                        .color(fg)
                                        .background_color(bg)
                                );
                            }
                        });
                    }
                });
            });
        
        // 3. 捕获键盘输入（焦点在窗口任意位置）
        if ctx.input(|i| i.focused) {
            ctx.input(|i| {
                // 处理按键
                for event in &i.raw.events {
                    if let egui::Event::Key { key, pressed: true, modifiers, .. } = event {
                        self.send_input(*key, "", modifiers);
                    }
                    if let egui::Event::Text(text) = event {
                        // 普通文本输入
                        for ch in text.chars() {
                            let mut writer = self.pty_writer.lock().unwrap();
                            use std::io::Write;
                            let _ = writer.write_all(&[ch as u8]);
                        }
                    }
                }
            });
        }
    }
}

// 颜色转换
fn convert_color(fg: alacritty_terminal::vte::ansi::Color, bg: alacritty_terminal::vte::ansi::Color) -> (egui::Color32, egui::Color32) {
    let convert = |c: alacritty_terminal::vte::ansi::Color| match c {
        alacritty_terminal::vte::ansi::Color::Named(n) => match n {
            alacritty_terminal::vte::ansi::NamedColor::Black => egui::Color32::BLACK,
            alacritty_terminal::vte::ansi::NamedColor::Red => egui::Color32::from_rgb(255, 100, 100),
            alacritty_terminal::vte::ansi::NamedColor::Green => egui::Color32::from_rgb(100, 255, 100),
            alacritty_terminal::vte::ansi::NamedColor::Yellow => egui::Color32::from_rgb(255, 255, 100),
            alacritty_terminal::vte::ansi::NamedColor::Blue => egui::Color32::from_rgb(100, 100, 255),
            alacritty_terminal::vte::ansi::NamedColor::Magenta => egui::Color32::from_rgb(255, 100, 255),
            alacritty_terminal::vte::ansi::NamedColor::Cyan => egui::Color32::from_rgb(100, 255, 255),
            alacritty_terminal::vte::ansi::NamedColor::White => egui::Color32::WHITE,
            alacritty_terminal::vte::ansi::NamedColor::Foreground => egui::Color32::LIGHT_GRAY,
            alacritty_terminal::vte::ansi::NamedColor::Background => egui::Color32::BLACK,
            _ => egui::Color32::GRAY,
        },
        alacritty_terminal::vte::ansi::Color::Spec(rgb) => {
            egui::Color32::from_rgb(rgb.r, rgb.g, rgb.b)
        }
        _ => egui::Color32::WHITE,
    };
    (convert(fg), convert(bg))
}

// 事件代理（alacritty 需要）
#[derive(Clone)]
struct EventProxy;

impl EventListener for EventProxy {
    fn send_event(&self, _event: Event) {
        // 可以在这里处理标题变更、响铃等事件
    }
}

fn main() -> eframe::Result {
    // macOS 无 Dock 图标（可选）
    #[cfg(target_os = "macos")]
    unsafe {
        let app: *mut objc::runtime::Object = objc::msg_send![objc::class!(NSApplication), sharedApplication];
        let _: () = objc::msg_send![app, setActivationPolicy: 1i64];
    }
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_title("CIS Terminal"),
        ..Default::default()
    };
    
    eframe::run_native(
        "CIS",
        options,
        Box::new(|_cc| Ok(Box::new(TermGui::new()))),
    )
}
```

## 3. 使用方式

```bash
# 编译
cargo build --release

# 运行（确保 claude/cis 在 PATH 中）
./target/release/cis-term-gui
```

**功能**：
- 顶部 **绿色 "● MATRIX"** 按钮：点击向终端发送 `matrix connect`
- 顶部 **红色 "○ MATRIX"** 按钮：点击向终端发送 `matrix disconnect`
- **节点下拉**：选择节点向终端发送切换命令
- **真终端**：完整支持 `htop`, `vim`, `claude` 的 ANSI 动画
- **Ctrl+C 按钮**：右上角 × 按钮发送中断信号

**性能**：`sync_screen()` 每帧复制整个屏幕缓冲区，对于大终端（>10000 单元格）会卡。优化方案：只在 PTY 有输出时同步（用 `mpsc` 通道通知），或使用 `alacritty_renderer` 直接渲染到纹理。

现在你的 CIS 有了一个**带 Matrix 网络开关的真终端**。