use anyhow::Result;
use eframe::egui::{
    self, Color32, FontFamily, FontId,
    text::{LayoutJob, TextFormat},
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{
    io::{Read, Write},
    mem,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

struct Colors {
    white: Color32,
    black: Color32,
    red: Color32,
    green: Color32,
    yellow: Color32,
    blue: Color32,
    magenta: Color32,
    cyan: Color32,
    grey: Color32,
    bright_red: Color32,
    bright_green: Color32,
    bright_yellow: Color32,
    bright_blue: Color32,
    bright_magenta: Color32,
    bright_cyan: Color32,
}

struct TerminalApp {
    output_buffer: Arc<Mutex<Vec<u8>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    _master_pty: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    layout_job: LayoutJob,
    input_text: String,
    stick_to_bottom: bool,
    current_format: TextFormat,
    partial_char_buffer: Vec<u8>,
    colors: Colors,
}

impl TerminalApp {
    fn append_new_output(&mut self, new_output: &[u8]) {
        let mut text_to_append = Vec::new();

        for &byte in new_output {
            match byte {
                b'\x1b' => {
                    if !text_to_append.is_empty() {
                        self.layout_job.append(
                            &String::from_utf8_lossy(&text_to_append),
                            0.0,
                            self.current_format.clone(),
                        );
                        text_to_append.clear();
                    }
                    self.partial_char_buffer.push(byte);
                }
                b'[' if self.partial_char_buffer.last() == Some(&b'\x1b') => {
                    self.partial_char_buffer.push(byte);
                }
                b'\n' | b'\r' => {
                    if !text_to_append.is_empty() {
                        self.layout_job.append(
                            &String::from_utf8_lossy(&text_to_append),
                            0.0,
                            self.current_format.clone(),
                        );
                        text_to_append.clear();
                    }
                    self.layout_job.append(
                        &String::from_utf8_lossy(&[byte]),
                        0.0,
                        self.current_format.clone(),
                    );
                }
                _ if !self.partial_char_buffer.is_empty() => {
                    self.partial_char_buffer.push(byte);
                    if let Some(command_char) = self.partial_char_buffer.last() {
                        if command_char.is_ascii_alphabetic() {
                            let ansi_sequence = String::from_utf8_lossy(&self.partial_char_buffer);
                            if ansi_sequence.ends_with('m') {
                                if let Some(start_index) = ansi_sequence.find('[') {
                                    let code_str =
                                        &ansi_sequence[start_index + 1..ansi_sequence.len() - 1];
                                    for part in code_str.split(';') {
                                        if let Ok(num) = part.parse::<u32>() {
                                            match num {
                                                0 => {
                                                    // Reset all attributes.
                                                    self.current_format.color = self.colors.white;
                                                    self.current_format.underline =
                                                        egui::Stroke::NONE;
                                                }
                                                30 => self.current_format.color = self.colors.black,
                                                31 => self.current_format.color = self.colors.red,
                                                32 => self.current_format.color = self.colors.green,
                                                33 => {
                                                    self.current_format.color = self.colors.yellow
                                                }
                                                34 => self.current_format.color = self.colors.blue,
                                                35 => {
                                                    self.current_format.color = self.colors.magenta
                                                }
                                                36 => self.current_format.color = self.colors.cyan,
                                                37 => self.current_format.color = self.colors.white,
                                                90 => self.current_format.color = self.colors.grey,
                                                91 => {
                                                    self.current_format.color =
                                                        self.colors.bright_red
                                                }
                                                92 => {
                                                    self.current_format.color =
                                                        self.colors.bright_green
                                                }
                                                93 => {
                                                    self.current_format.color =
                                                        self.colors.bright_yellow
                                                }
                                                94 => {
                                                    self.current_format.color =
                                                        self.colors.bright_blue
                                                }
                                                95 => {
                                                    self.current_format.color =
                                                        self.colors.bright_magenta
                                                }
                                                96 => {
                                                    self.current_format.color =
                                                        self.colors.bright_cyan
                                                }
                                                97 => self.current_format.color = self.colors.white,
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            self.partial_char_buffer.clear();
                        }
                    }
                }
                _ => {
                    if !self.partial_char_buffer.is_empty() {
                        self.layout_job.append(
                            &String::from_utf8_lossy(&self.partial_char_buffer),
                            0.0,
                            self.current_format.clone(),
                        );
                        self.partial_char_buffer.clear();
                    }
                    text_to_append.push(byte);
                }
            }
        }
        if !text_to_append.is_empty() {
            self.layout_job.append(
                &String::from_utf8_lossy(&text_to_append),
                0.0,
                self.current_format.clone(),
            );
        }
    }
}

impl eframe::App for TerminalApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let new_output = {
                let mut output_buffer = self.output_buffer.lock().unwrap();
                mem::take(&mut *output_buffer)
            };
            if !new_output.is_empty() {
                self.append_new_output(&new_output);
            }
            let scroll_area_response = egui::ScrollArea::vertical()
                .stick_to_bottom(self.stick_to_bottom)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(self.layout_job.clone()));
                });

            let max_offset_y =
                scroll_area_response.content_size.y - scroll_area_response.inner_rect.height();
            if scroll_area_response.state.offset.y < max_offset_y - 1.0 {
                self.stick_to_bottom = false;
            }

            let text_edit_response = ui.add(
                egui::TextEdit::singleline(&mut self.input_text)
                    .desired_width(ui.available_width())
                    .hint_text("Type commands here...")
                    .font(egui::TextStyle::Monospace)
                    .margin(egui::Margin::symmetric(5, 5)),
            );

            if text_edit_response.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                let mut writer = self.writer.lock().unwrap();
                let _ = writer.write_all(self.input_text.as_bytes());
                let _ = writer.write_all(b"\n");
                let _ = writer.flush();
                self.input_text.clear();
                self.stick_to_bottom = true;
                text_edit_response.request_focus();
            }

            ctx.request_repaint_after(Duration::from_millis(1));
        });
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
                _master_pty: app_master_pty,
                layout_job: LayoutJob::default(),
                input_text: String::new(),
                stick_to_bottom: true,
                current_format: TextFormat {
                    font_id: FontId::new(14.0, FontFamily::Monospace),
                    color: Color32::WHITE,
                    ..Default::default()
                },
                partial_char_buffer: Vec::new(),
                colors: Colors {
                    white: Color32::WHITE,
                    black: Color32::BLACK,
                    red: Color32::RED,
                    green: Color32::GREEN,
                    yellow: Color32::YELLOW,
                    blue: Color32::BLUE,
                    magenta: Color32::MAGENTA,
                    cyan: Color32::CYAN,
                    grey: Color32::GRAY,
                    bright_red: Color32::LIGHT_RED,
                    bright_green: Color32::LIGHT_GREEN,
                    bright_yellow: Color32::LIGHT_YELLOW,
                    bright_blue: Color32::LIGHT_BLUE,
                    bright_magenta: Color32::PURPLE,
                    bright_cyan: Color32::DARK_BLUE,
                },
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}
