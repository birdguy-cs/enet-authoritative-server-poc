use std::{
    fmt::format, fs, net::{SocketAddr, UdpSocket}, str::{self, FromStr}, sync::*, time::{Duration, Instant}
};
use rustc_hash::{FxHashMap, FxHashSet};
use rusqlite::{Connection, OptionalExtension};
use argon2::{
    Argon2,
    PasswordHash,
    PasswordVerifier,
    PasswordHasher, 
    password_hash::SaltString,
    password_hash::rand_core::OsRng
};
use uuid::Uuid;
use rusty_enet as enet;
use crate::extract_quoted;

pub struct Ticket {
    pub served_user: String,
    pub creation_time: Instant,
}

pub struct Server {
    pub served_tickets: FxHashMap<String, Ticket>,
    pub user_db: Connection, 
    pub blacklisted_servers: FxHashSet<String>,
}

impl Server {
    pub fn new() -> rusqlite::Result<Self> {
        let user_db = Connection::open("database.sqlite")?;

        user_db.execute(
            "
            CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL
            )
            ",
            [],
        )?;

        let blacklisted_servers = load_blacklist("blacklist.txt");

        Ok(Self {
            served_tickets: FxHashMap::default(),
            user_db,
            blacklisted_servers
        })
    }

    pub fn get_user_hash(
        &self,
        username: &str,
    ) -> rusqlite::Result<Option<String>> {
        self.user_db
            .query_row(
                "SELECT password_hash FROM users WHERE username = ?1",
                [username],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn create_user(&mut self, username: &str, password: &str) -> rusqlite::Result<bool> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt).unwrap().to_string();

        let rows = self.user_db.execute(
            "INSERT OR IGNORE INTO users
             (username, password_hash)
             VALUES (?1, ?2)",
            (username, password_hash),
        )?;
        Ok(rows > 0)
    }
    
    pub fn generate_ticket(
        &mut self,
        username: &str,
        password: &str,
        server_ip: &str,
    ) -> Option<String> {
        if self.blacklisted_servers.contains(server_ip) {
            return None;
        }
        // Get stored hash
        let hash_str = self.get_user_hash(username).ok()??;
        let parsed_hash = PasswordHash::new(&hash_str).ok()?;
    
        // Verify password
        if Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_err()
        {
            return None;
        }
    
        // Generate random ticket and store it
        let ticket_id = Uuid::new_v4().to_string();
        
        let ticket = Ticket {
            served_user: username.to_string(),
            creation_time: Instant::now(),
        };
        
        self.served_tickets.insert(ticket_id.clone(), ticket);
    
        Some(ticket_id)
    }

    pub fn check_ticket(&mut self, ticket_id: &str) -> Option<String> {
        self.served_tickets
            .remove(ticket_id)
            .map(|t| t.served_user.clone())
    }

    pub fn tick(&mut self) {
        self.served_tickets.retain(|_, ticket| {
            ticket.creation_time.elapsed() <= Duration::from_secs(60)
        });
    }
}

fn load_blacklist(path: &str) -> FxHashSet<String> {
    let content = fs::read_to_string(path).unwrap_or_default();

    content
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}


pub fn run_server(server: &str, response: Arc<Mutex<String>>) {
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
    let mut server = Server::new().unwrap();

    loop {
        server.tick();
        while let Some(event) = host.service().unwrap() {
            match event {
                enet::Event::Connect { peer, .. } => {
                    println!("Peer {} connected", peer.id().0);
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
                        let message_recieved = extract_quoted(message);
                        if message.contains("create account") {
                            let mut message = format!("");
                            match server.create_user(message_recieved[0], message_recieved[1]) {
                                Ok(true)  => { message = format!("Account creation successful user is: '{}'", message_recieved[0]); }
                                Ok(false) => { message = format!("Account creation fail user already exists") }
                                Err(e)    => { eprintln!("DB error: {e}"); message = format!("DB error: {e}"); }
                            }
                            let reply = enet::Packet::reliable(message.as_bytes());
                            peer.send(channel_id, &reply).unwrap();
                        } else if message.contains("get account ticket") {
                            if let Some(ticket) = server.generate_ticket(message_recieved[0], message_recieved[1], message_recieved[2]) {
                                let message = format!("verify ticket, '{}'", ticket);
                                let reply = enet::Packet::reliable(message.as_bytes());
                                peer.send(channel_id, &reply).unwrap();
                            } else {
                                let message = format!("could not log you in");
                                let reply = enet::Packet::reliable(message.as_bytes());
                                peer.send(channel_id, &reply).unwrap();
                            }
                        } else if message.contains("verify ticket") {
                            if let Some(user) = server.check_ticket(message_recieved[0]) {
                                let message = format!("ticket verified for '{}'", user);
                                let reply = enet::Packet::reliable(message.as_bytes());
                                peer.send(channel_id, &reply).unwrap();
                            } else {
                                let message = format!("ticket invalid");
                                let reply = enet::Packet::reliable(message.as_bytes());
                                peer.send(channel_id, &reply).unwrap();
                            }
                        }
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}
