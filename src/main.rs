#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use std::sync::{mpsc::Sender, mpsc::Receiver, *};
use egui::*;
mod authority_server;
mod server_client;

enum Command {
    CreateAccount(String, String),
    GetTicket(String, String, String),
    VerifyTicket,
}

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Server networking test",
        options,
        Box::new(|_| {
            Ok(Box::<MyApp>::default())
        }),
    )
}

struct MyApp {
    server: String,
    wanted_ip: String,
    username: String,
    password: String,
    tx: Sender<Command>,
    rx: Option<Receiver<Command>>, 
    client_response: Arc<Mutex<String>>,
    server_response: Arc<Mutex<String>>,
}

impl Default for MyApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel::<Command>();
        Self {
            server: "127.0.0.1:6060".to_owned(),
            wanted_ip: "192.168.1.148".to_owned(),
            username: "user".to_owned(),
            password: "1234".to_owned(),
            tx,
            rx: Some(rx),
            client_response: Arc::new(Mutex::new("n/a".to_string())),
            server_response: Arc::new(Mutex::new("n/a".to_string())),
        }
    }
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Server test");
            ui.horizontal(|ui| {
                let server_label = ui.label("Auth server: ");
                ui.text_edit_singleline(&mut self.server)
                    .labelled_by(server_label.id);
            });

            ui.horizontal(|ui| {
                let message_label = ui.label("Wanted ip: ");
                ui.text_edit_singleline(&mut self.wanted_ip)
                    .labelled_by(message_label.id);
            });
            
            ui.horizontal(|ui| {
                let message_label = ui.label("Username: ");
                ui.text_edit_singleline(&mut self.username)
                    .labelled_by(message_label.id);
            });
            
            ui.horizontal(|ui| {
                let message_label = ui.label("Password: ");
                ui.text_edit_singleline(&mut self.password)
                    .labelled_by(message_label.id);
            });
            
            if ui.button("Connect to server").clicked() {
                if let Some(rx) = self.rx.take() { 
                    let server = self.server.clone();
                    let server_response = self.server_response.clone();
            
                    std::thread::spawn(move || {
                        server_client::run_client(&server, server_response, rx);
                    });
                }
            }
            
            if ui.button("Create account").clicked() {
                let username = self.username.clone();
                let password = self.password.clone();

                self.tx.send(Command::CreateAccount(username, password)).unwrap();
            }
            
            if ui.button("Get account ticket for ip").clicked() {
                let username = self.username.clone();
                let password = self.password.clone();
                let ip = self.wanted_ip.clone();
                
                self.tx.send(Command::GetTicket(username, password, ip)).unwrap();
            }
            
            if ui.button("Verify ticket").clicked() {
                self.tx.send(Command::VerifyTicket).unwrap();
            }

            if ui.button("Create server").clicked() {
                let server = self.server.clone();
                let client_response = self.client_response.clone();

                std::thread::spawn(move || {
                    authority_server::run_server(&server, client_response);
                });
            }

            ui.label(format!("Server '{}'", self.server));
            let server_response = self.server_response.lock().unwrap().clone();
            let client_response = self.client_response.lock().unwrap().clone();
            ui.label(format!("Client recieved: '{}' and hosted server recieved: '{}'", server_response, client_response));
        });
    }
}

fn extract_quoted(input: &str) -> Vec<&str> {
    let mut results = Vec::new();
    let mut remaining = input;
    let mut offset = 0;

    while let Some(start) = remaining.find('\'') {
        let content_start = offset + start + 1;
        remaining = &input[content_start..];

        if let Some(end) = remaining.find('\'') {
            results.push(&input[content_start..content_start + end]);
            remaining = &input[content_start + end + 1..];
            offset = content_start + end + 1;
        } else {
            break;
        }
    }

    results
}
