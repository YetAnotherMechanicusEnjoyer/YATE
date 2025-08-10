use anyhow::Result;
use eframe::egui;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

struct TerminalApp {
    output_buffer: Arc<Mutex<Vec<u8>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master_pty: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let output_buffer = self.output_buffer.lock().unwrap();
            let text = String::from_utf8_lossy(&output_buffer).to_string();

            egui::TextEdit::multiline(&mut text.clone())
                .desired_width(ui.available_width())
                .desired_rows(25)
                .font(egui::TextStyle::Monospace)
                .show(ui);

            for event in ctx.input(|i| i.events.clone()).iter() {
                match event {
                    egui::Event::Text(input_text) => {
                        if !input_text.is_empty() {
                            let mut writer = self.writer.lock().unwrap();
                            let _ = writer.write_all(input_text.as_bytes());
                            let _ = writer.flush();
                        }
                    }
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => {
                        let mut writer = self.writer.lock().unwrap();
                        let bytes_to_send: Option<&'static [u8]> = match key {
                            egui::Key::Enter => Some(b"\n"),
                            egui::Key::Backspace => Some(b"\x7f"),
                            egui::Key::ArrowLeft => Some(b"\x1b[D"),
                            egui::Key::ArrowRight => Some(b"\x1b[C"),
                            egui::Key::ArrowUp => Some(b"\x1b[A"),
                            egui::Key::ArrowDown => Some(b"\x1b[B"),
                            egui::Key::Tab => Some(b"\t"),
                            egui::Key::Escape => Some(b"\x1b"),
                            _ => None,
                        };

                        if let Some(bytes) = bytes_to_send {
                            let _ = writer.write_all(bytes);
                            let _ = writer.flush();
                        }
                    }
                    _ => {}
                }
            }

            ctx.request_repaint();
        });

        let new_size = ctx.input(|i| i.screen_rect).size();
        let current_pty_size = self
            .master_pty
            .lock()
            .unwrap()
            .get_size()
            .unwrap_or(PtySize {
                rows: 0,
                cols: 0,
                pixel_width: 0,
                pixel_height: 0,
            });

        if (new_size.x as u16) != current_pty_size.pixel_width
            || (new_size.y as u16) != current_pty_size.pixel_height
        {
            let _ = self.master_pty.lock().unwrap().resize(PtySize {
                rows: (new_size.y / 15.0).max(1.0) as u16,
                cols: (new_size.x / 8.0).max(1.0) as u16,
                pixel_width: new_size.x as u16,
                pixel_height: new_size.y as u16,
            });
        }
    }
}

fn main() -> Result<()> {
    let pty_system = native_pty_system();

    let shell_path = if cfg!(target_os = "windows") {
        "cmd.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string())
    };

    let initial_pty_size = PtySize {
        rows: 25,
        cols: 80,
        pixel_width: 640,
        pixel_height: 375,
    };

    let pty_pair = pty_system.openpty(initial_pty_size)?;

    let cmd = CommandBuilder::new(shell_path);

    let _child = pty_pair.slave.spawn_command(cmd)?;

    let master = pty_pair.master;
    let writer = master.take_writer()?;
    let mut reader = master.try_clone_reader()?;

    let output_buffer = Arc::new(Mutex::new(Vec::new()));
    let app_writer = Arc::new(Mutex::new(writer));
    let app_master_pty = Arc::new(Mutex::new(master));
    let cloned_output_buffer = output_buffer.clone();

    thread::spawn(move || {
        let mut buffer = [0; 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(count) if count > 0 => {
                    let mut output = cloned_output_buffer.lock().unwrap();
                    output.extend_from_slice(&buffer[..count]);
                }
                Ok(_) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error reading from PTY: {e}");
                    break;
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(egui::vec2(640.0, 375.0)),
        ..Default::default()
    };
    eframe::run_native(
        "YATE",
        options,
        Box::new(|_cc| {
            Ok(Box::new(TerminalApp {
                output_buffer,
                writer: app_writer,
                master_pty: app_master_pty,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}
