use anyhow::{Context, Result};
use eframe::egui::{
    self, Color32, FontFamily, FontId,
    text::{LayoutJob, TextFormat},
};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    mem,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

#[derive(Debug, Serialize, Deserialize)]
struct ColorPalette {
    background: [u8; 4],
    white: [u8; 4],
    black: [u8; 4],
    red: [u8; 4],
    green: [u8; 4],
    yellow: [u8; 4],
    blue: [u8; 4],
    magenta: [u8; 4],
    cyan: [u8; 4],
    grey: [u8; 4],
    bright_red: [u8; 4],
    bright_green: [u8; 4],
    bright_yellow: [u8; 4],
    bright_blue: [u8; 4],
    bright_magenta: [u8; 4],
    bright_cyan: [u8; 4],
}

impl From<&Colors> for ColorPalette {
    fn from(colors: &Colors) -> Self {
        Self {
            background: colors.background.to_array(),
            white: colors.white.to_array(),
            black: colors.black.to_array(),
            red: colors.red.to_array(),
            green: colors.green.to_array(),
            yellow: colors.yellow.to_array(),
            blue: colors.blue.to_array(),
            magenta: colors.magenta.to_array(),
            cyan: colors.cyan.to_array(),
            grey: colors.grey.to_array(),
            bright_red: colors.bright_red.to_array(),
            bright_green: colors.bright_green.to_array(),
            bright_yellow: colors.bright_yellow.to_array(),
            bright_blue: colors.bright_blue.to_array(),
            bright_magenta: colors.bright_magenta.to_array(),
            bright_cyan: colors.bright_cyan.to_array(),
        }
    }
}

impl From<ColorPalette> for Colors {
    fn from(palette: ColorPalette) -> Self {
        Self {
            background: Color32::from_rgba_unmultiplied(
                palette.background[0],
                palette.background[1],
                palette.background[2],
                palette.background[3],
            ),
            white: Color32::from_rgba_unmultiplied(
                palette.white[0],
                palette.white[1],
                palette.white[2],
                palette.white[3],
            ),
            black: Color32::from_rgba_unmultiplied(
                palette.black[0],
                palette.black[1],
                palette.black[2],
                palette.black[3],
            ),
            red: Color32::from_rgba_unmultiplied(
                palette.red[0],
                palette.red[1],
                palette.red[2],
                palette.red[3],
            ),
            green: Color32::from_rgba_unmultiplied(
                palette.green[0],
                palette.green[1],
                palette.green[2],
                palette.green[3],
            ),
            yellow: Color32::from_rgba_unmultiplied(
                palette.yellow[0],
                palette.yellow[1],
                palette.yellow[2],
                palette.yellow[3],
            ),
            blue: Color32::from_rgba_unmultiplied(
                palette.blue[0],
                palette.blue[1],
                palette.blue[2],
                palette.blue[3],
            ),
            magenta: Color32::from_rgba_unmultiplied(
                palette.magenta[0],
                palette.magenta[1],
                palette.magenta[2],
                palette.magenta[3],
            ),
            cyan: Color32::from_rgba_unmultiplied(
                palette.cyan[0],
                palette.cyan[1],
                palette.cyan[2],
                palette.cyan[3],
            ),
            grey: Color32::from_rgba_unmultiplied(
                palette.grey[0],
                palette.grey[1],
                palette.grey[2],
                palette.grey[3],
            ),
            bright_red: Color32::from_rgba_unmultiplied(
                palette.bright_red[0],
                palette.bright_red[1],
                palette.bright_red[2],
                palette.bright_red[3],
            ),
            bright_green: Color32::from_rgba_unmultiplied(
                palette.bright_green[0],
                palette.bright_green[1],
                palette.bright_green[2],
                palette.bright_green[3],
            ),
            bright_yellow: Color32::from_rgba_unmultiplied(
                palette.bright_yellow[0],
                palette.bright_yellow[1],
                palette.bright_yellow[2],
                palette.bright_yellow[3],
            ),
            bright_blue: Color32::from_rgba_unmultiplied(
                palette.bright_blue[0],
                palette.bright_blue[1],
                palette.bright_blue[2],
                palette.bright_blue[3],
            ),
            bright_magenta: Color32::from_rgba_unmultiplied(
                palette.bright_magenta[0],
                palette.bright_magenta[1],
                palette.bright_magenta[2],
                palette.bright_magenta[3],
            ),
            bright_cyan: Color32::from_rgba_unmultiplied(
                palette.bright_cyan[0],
                palette.bright_cyan[1],
                palette.bright_cyan[2],
                palette.bright_cyan[3],
            ),
        }
    }
}

fn load_colors(path: &str) -> Result<ColorPalette> {
    let toml_string = fs::read_to_string(path).context("Failed to read colors.toml file")?;
    let palette = toml::from_str(&toml_string).context("Failed to parse colors.toml")?;
    Ok(palette)
}

fn save_colors(path: &str, colors: &ColorPalette) -> Result<()> {
    let toml_string =
        toml::to_string_pretty(colors).context("Failed to serialize colors to TOML")?;
    fs::write(path, toml_string).context("Failed to write to colors.toml")?;
    Ok(())
}

struct Colors {
    background: Color32,
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
        let frame = egui::Frame::central_panel(&ctx.style()).fill(self.colors.background);
        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
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
        viewport: egui::ViewportBuilder::default()
            .with_resizable(true)
            .with_inner_size(egui::vec2(1920.0, 1080.0)),
        ..Default::default()
    };
    eframe::run_native(
        "YATE",
        options,
        Box::new(|_cc| {
            let colors = match load_colors("colors.toml") {
                Ok(palette) => Colors::from(palette),
                Err(e) => {
                    eprintln!("Failed to load colors.toml: {e}. Using default colors.");
                    let default_colors = Colors {
                        background: Color32::from_rgb(20, 20, 20),
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
                    };
                    let _ = save_colors("colors.toml", &ColorPalette::from(&default_colors));
                    default_colors
                }
            };

            Ok(Box::new(TerminalApp {
                output_buffer,
                writer: app_writer,
                _master_pty: app_master_pty,
                layout_job: LayoutJob::default(),
                input_text: String::new(),
                stick_to_bottom: true,
                current_format: TextFormat {
                    font_id: FontId::new(14.0, FontFamily::Monospace),
                    color: colors.white,
                    ..Default::default()
                },
                partial_char_buffer: Vec::new(),
                colors,
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))?;

    Ok(())
}
