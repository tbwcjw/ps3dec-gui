use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use arboard::Clipboard;
use std::error::Error;

const CONFIG_FILE: &str = "ps3dec_gui.json";

#[derive(Serialize, Deserialize, Default)]
struct AppConfig {
    iso_path: String,
    decryption_key: String,
    thread_count: u32,
    auto: bool,
    ps3dec_path: String,
}

struct PS3DecGUI {
    config: AppConfig,
    status: String,
    output: String,
    rx: Option<Receiver<String>>,
}

impl Default for PS3DecGUI {
    fn default() -> Self {
        // try load from file
        let config = fs::read_to_string(CONFIG_FILE)
            .ok()
            .and_then(|data| serde_json::from_str(&data).ok())
            .unwrap_or_else(|| AppConfig {
                ps3dec_path: "".to_string(),
                thread_count: 1,
                ..Default::default()
            });
        Self {
            config,
            status: String::new(),
            output: String::new(),
            rx: None,
        }
    }
}

impl eframe::App for PS3DecGUI {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("PS3Dec GUI");
            ui.label(egui::RichText::new("Yet another GUI for PS3Dec"));
            ui.separator();
            egui::Grid::new("config_grid")
                .spacing([10.0, 8.0])
                .min_col_width(75.0)
                .max_col_width(400.0)
                .show(ui, |ui| {
                    //ps3dec executable get set
                    ui.label("ps3dec Executable:");
                    if ui.button("Select Executable").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.config.ps3dec_path = path.display().to_string();
                            self.save_config();
                        }
                    }
                    ui.add(egui::Label::new(&self.config.ps3dec_path).wrap(true));
                    ui.end_row();
                    
                    //iso file get set
                    ui.label("ISO File:");
                    if ui.button("Select ISO").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("ISO file", &["iso"])
                            .pick_file()
                        {
                            self.config.iso_path = path.display().to_string();
                            self.save_config();
                        }
                    }
                    ui.add(egui::Label::new(&self.config.iso_path).wrap(true));
                    ui.end_row();

                    //decryption get set (no auto)
                    ui.label("Decryption Key:");
                    if ui.text_edit_singleline(&mut self.config.decryption_key).changed() {
                        self.save_config();
                    }
                    ui.end_row();

                    //thread count get set
                    ui.label("Thread Count:");
                    if ui
                        .add(egui::DragValue::new(&mut self.config.thread_count).clamp_range(1..=256))
                        .changed()
                    {
                        self.save_config();
                    }
                    ui.add(egui::Label::new("Note: too many threads will hang ps3dec.").wrap(true));
                    ui.end_row();
                    
                    //auto key detection
                    ui.label("Automatic key detection:");
                    if ui.checkbox(&mut self.config.auto, "").changed() {
                        self.save_config();
                    }
                    ui.add(egui::Label::new("Note: ''keys/'' in ps3dec directory").wrap(true));
                    ui.end_row();
                });
                
            ui.label(&self.status);
            
            //ps3dec stdout stderr 
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.output)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(15)
                        .desired_width(f32::INFINITY)
                        .interactive(false)
                        
                );
            });
            
            //run/copy buttions
            ui.horizontal(|ui| {
                if ui.button("Run ps3dec").clicked() {
                    self.start_ps3dec();
                }

                if ui.button("Copy to clipboard").clicked() {
                    if let Err(e) = copy(&self.output) {
                        self.status = format!("Clipboard copy failed: {}", e);
                    }
                }
            });
            
            //useful link buttons, open in browser with open_url
            ui.horizontal(|ui| {
                ui.add(egui::Label::new("Redump Decryption Keys:").wrap(true));
                if ui.button("Aldos Tools").clicked() {
                    open_url("https://ps3.aldostools.org/dkey.html");
                }
                ui.add(egui::Label::new("PlayStation 3 Redumps:").wrap(true));
                if ui.button("Myrient").clicked() {
                    open_url("https://myrient.erista.me/");
                }
                ui.add(egui::Label::new("Recommended VPN:").wrap(true));
                if ui.button("iVPN").clicked() {
                    open_url("https://www.ivpn.net/");
                }
                ui.add(egui::Label::new("Source & Support").wrap(true));
                if ui.button("Github").clicked() {
                    open_url("https://github.com/tbwcjw");
                }
            });


        });

        //catch exit code from ps3dec
        if let Some(rx) = &self.rx {
            while let Ok(line) = rx.try_recv() {
                if let Some(code_str) = line.strip_prefix("__EXIT_CODE__") {
                    match code_str.parse::<i32>() {
                        Ok(0) => self.status = "ps3dec exited with code 0".to_string(),
                        Ok(code) => self.status = format!("ps3dec exited with code {}", code),
                        Err(_) => self.status = "ps3dec exited with unknown code".to_string(),
                    }
                } else {
                    //write out
                    self.output.push_str(&line);
                    self.output.push('\n');
                }
                //repaint
                ctx.request_repaint();
            }
        }
    }
}


impl PS3DecGUI {
    fn save_config(&self) {
        if let Ok(data) = serde_json::to_string_pretty(&self.config) {
            let _ = fs::write(CONFIG_FILE, data);
        }
    }

    fn start_ps3dec(&mut self) {
        //no iso given
        if self.config.iso_path.is_empty() {
            self.status = "Please select an ISO file.".to_string();
            return;
        }
        //no ps3dec executable given
        if self.config.ps3dec_path.is_empty() {
            self.status = "Please select the ps3dec executable.".to_string();
            return;
        }

        //--iso arg
        let mut args = vec![
            "--iso".to_string(),
            self.config.iso_path.clone(),
        ];

        //--auto arg
        if self.config.auto {
            args.push("--auto".to_string());
        } else {
            // else use decryption key
            if self.config.decryption_key.is_empty() {
                //no decryption key given
                self.status = "Please enter a decryption key.".to_string();
                return;
            }
            //--dk arg
            args.push("--dk".to_string());
            args.push(self.config.decryption_key.clone());
        }
        
        //--tc arg
        args.push("--tc".to_string());
        args.push(self.config.thread_count.to_string());
        args.push("--skip".to_string());
        
        //write full command to output first
        self.output.clear();
        let full_command = format!("Running command: {} {}", self.config.ps3dec_path, args.join(" "));
        self.output.push_str(&full_command);
        self.output.push('\n');
        
        self.status = "Running ps3dec...".to_string();

        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);

        let cmd = self.config.ps3dec_path.clone();

        let args_clone = args.clone();
        
        //spawn ps3dec thread with exit code catching
        thread::spawn(move || {
            let mut child = match Command::new(cmd)
                .args(&args_clone)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(format!("Failed to start ps3dec: {}", e));
                    let _ = tx.send("__EXIT_CODE__-1".to_string());
                    return;
                }
            };

            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            
            let tx_stdout = tx.clone();

            //read stdout
            let stdout_thread = thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let _ = tx_stdout.send(l);
                        }
                        Err(e) => {
                            let _ = tx_stdout.send(format!("Error reading stdout: {}", e));
                            break;
                        }
                    }
                }
            });
            
            //read stderr
            let tx_stderr = tx.clone();
            let stderr_thread = thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let _ = tx_stderr.send(format!("ERR: {}", l));
                        }
                        Err(e) => {
                            let _ = tx_stderr.send(format!("Error reading stderr: {}", e));
                            break;
                        }
                    }
                }
            });

            let status = child.wait();

            let _ = stdout_thread.join();
            let _ = stderr_thread.join();

            match status {
                Ok(exit_status) => {
                    let code = exit_status.code().unwrap_or(-1);
                    let _ = tx.send(format!("__EXIT_CODE__{}", code));
                }
                Err(_) => {
                    let _ = tx.send("__EXIT_CODE__-1".to_string());
                }
            }
        });
    }
}

fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(&["/C", "start", url])
        .spawn();

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open")
        .arg(url)
        .spawn();

    #[cfg(target_os = "macos")] //haven't build for macos, but this is futureproofing
    let _ = std::process::Command::new("open")
        .arg(url)
        .spawn();
}

fn copy<S: Into<String>>(text: S) -> Result<(), Box<dyn Error>> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text.into())?;
    Ok(())
}

fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 500.0])
            .with_resizable(false), // need to validate on windows
        ..Default::default()
    };
    
    eframe::run_native(
        "PS3Dec GUI",
        native_options,
        Box::new(|_cc| Box::new(PS3DecGUI::default())),
    )
    .unwrap();
}