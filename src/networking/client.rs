use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    str::{self, FromStr},
    time::Duration,
    sync::*,
};

use rusty_enet as enet;

pub fn run_client(address: &str, respond_message: &str, response: Arc<Mutex<String>>) {
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
    let address = match SocketAddr::from_str(address.trim()) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("Invalid address '{address}': {e}");
            return;
        }
    };
    let peer = host.connect(address, 2, 0).unwrap();
    peer.set_ping_interval(100);
    loop {
        while let Some(event) = host.service().unwrap() {
            match event {
                enet::Event::Connect { peer, .. } => {
                    println!("Connected");
                    let packet = enet::Packet::reliable(respond_message.as_bytes());
                    _ = peer.send(0, &packet);
                }
                enet::Event::Disconnect { .. } => {
                    println!("Disconnected");
                }
                enet::Event::Receive { packet, .. } => {
                    if let Ok(message) = str::from_utf8(packet.data()) {
                        println!("Client received packet: {:?}", message);
                        if message != "pong" {
                            *response.lock().unwrap() = message.to_string();
                        }
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
