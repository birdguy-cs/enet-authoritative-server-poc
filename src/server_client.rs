use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    str::{self, FromStr},
    sync::{mpsc::Receiver, *},
    time::Duration,
};

use rusty_enet as enet;

use crate::Command;
use crate::extract_quoted;

pub fn run_client(authority: &str, response: Arc<Mutex<String>>, rx: Receiver<Command>) {
    let socket =
        UdpSocket::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))).unwrap();
    let mut host = enet::Host::<UdpSocket>::new(
        socket,
        enet::HostSettings {
            peer_limit: 1,
            channel_limit: 2,
            compressor: Some(Box::new(enet::RangeCoder::new())),
            checksum: Some(Box::new(enet::crc32)),
            ..Default::default()
        },
    )
    .unwrap();
    let address = match SocketAddr::from_str(authority.trim()) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Invalid address '{authority}': {e}");
            return;
        }
    };
    let mut ticket = "".to_string();

    let peer_id = host.connect(address, 2, 0).unwrap().id();
    host.peer_mut(peer_id).set_ping_interval(100);
    loop {
        while let Some(event) = host.service().unwrap() {
            match event {
                enet::Event::Connect { .. } => {
                    println!("Connected");
                }
                enet::Event::Disconnect { .. } => {
                    println!("Disconnected");
                }
                enet::Event::Receive { packet, .. } => {
                    if let Ok(message) = str::from_utf8(packet.data()) {
                        println!("Client received packet: {:?}", message);
                        if message != "pong" {
                            *response.lock().unwrap() = message.to_string();
                            if message.contains("verify ticket,") {
                                for text in extract_quoted(message) {
                                    ticket = text.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
        
        match rx.try_recv() {
            Ok(cmd) => match cmd {
                Command::CreateAccount(username, password) => {
                    let message = format!("create account, username'{}', password'{}'", username, password);
                    let packet = enet::Packet::reliable(message.as_bytes());
                    _ = host.peer_mut(peer_id).send(0, &packet);
                },
                Command::GetTicket(username, password, server) => {
                    let message = format!("get account ticket, username'{}', password'{}', serverip'{}'", username, password, server);
                    let packet = enet::Packet::reliable(message.as_bytes());
                    _ = host.peer_mut(peer_id).send(0, &packet);
                },
                Command::VerifyTicket => {
                    let message = format!("verify ticket, ticket'{}'", ticket);
                    let packet = enet::Packet::reliable(message.as_bytes());
                    _ = host.peer_mut(peer_id).send(0, &packet);
                },
            },
            Err(_) => {
                // no command, continue normal work
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
