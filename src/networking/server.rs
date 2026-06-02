use std::{
    net::{SocketAddr, UdpSocket},
    str::{self, FromStr},
    time::Duration,
    sync::*,
};

use rusty_enet as enet;

pub fn run_server(server: &str, respond_message: &str, response: Arc<Mutex<String>>) {
    let ip = server.split_once(':')
        .map(|(_, right)| right.trim());

    let server_ip = format!("0.0.0.0:{}", ip.unwrap_or("6060"));
    let socket = UdpSocket::bind(SocketAddr::from_str(&server_ip).unwrap()).unwrap();
    let mut host = enet::Host::new(
        socket,
        enet::HostSettings {
            peer_limit: 32,
            channel_limit: 2,
            compressor: Some(Box::new(enet::RangeCoder::new())),
            checksum: Some(Box::new(enet::crc32)),
            ..Default::default()
        },
    )
    .unwrap();

    loop {
        while let Some(event) = host.service().unwrap() {
            match event {
                enet::Event::Connect { peer, .. } => {
                    println!("Peer {} connected", peer.id().0);
                    let msg = enet::Packet::reliable(respond_message.as_bytes());
                    peer.send(0, &msg).unwrap();
                }
                enet::Event::Disconnect { peer, .. } => {
                    println!("Peer {} disconnected", peer.id().0);
                }
                enet::Event::Receive {
                    peer,
                    channel_id,
                    packet,
                } => {
                    if let Ok(message) = str::from_utf8(packet.data()) {
                        println!("Server received packet: {:?}", message);
                        *response.lock().unwrap() = message.to_string();
                    }
                    let reply = enet::Packet::reliable("pong".as_bytes());
                    peer.send(channel_id, &reply).unwrap();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
