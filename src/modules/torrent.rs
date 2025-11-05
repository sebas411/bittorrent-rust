use hex::decode;
use sha1::{Digest, Sha1};

use crate::modules::{bencode::encode_value, value::Value};

fn get_pieces_hashes(input: &[u8]) -> Vec<[u8; 20]> {
    let mut i = 0;
    let mut output = vec![];
    while input.len() >= i+20 {
        let hash: [u8; 20] = input[i..i+20].try_into().unwrap();
        i += 20;
        output.push(hash);
    }
    output
}

pub struct Info {
    length: i64,
    _name: String,
    piece_length: i64,
    pieces: Vec<[u8; 20]>,
    hash: String,
}

#[allow(dead_code)]
impl Info {
    fn new(val: Value) -> Option<Self> {
        if let Value::Map(info_map) = val {
            let length = info_map.get("length")?.get_int()?;
            let name = String::from_utf8(info_map.get("name")?.get_string()?).unwrap();
            let piece_length = info_map.get("piece length")?.get_int()?;
            let pieces_raw = info_map.get("pieces")?.get_string()?;
            let pieces = get_pieces_hashes(&pieces_raw);
            
            let bencoded_info_map = encode_value(Value::Map(info_map));
            let mut hasher = Sha1::new();
            hasher.update(bencoded_info_map);
            let sha1_hash = hasher.finalize();
            let sha1_hash_hex = format!("{:x}", sha1_hash);
            return Some(Self {length, _name: name, piece_length, pieces, hash: sha1_hash_hex})
        }
        None
    }
    fn print_piece_hashes(&self) {
        for piece in &self.pieces {
            let mut piece_hash = String::new();
            for byte in piece {
                piece_hash.push_str(&format!("{:02x}", byte));
            }
            println!("{}", piece_hash);
        }
    }
    pub fn get_info_hash(&self) -> String {
        self.hash.clone()
    }
    pub fn get_info_hash_bytes(&self) -> Vec<u8> {
        let hex_str = &self.hash;
        decode(hex_str).unwrap()
    }
    pub fn get_length(&self) -> i64 {
        self.length
    }
}

pub struct Torrent {
    announce: String,
    pub info: Info,
}

impl Torrent {
    pub fn new(val: Value) -> Option<Self> {
        let torrent_map = val.get_map()?;
        let announce = String::from_utf8(torrent_map.get("announce")?.get_string()?).unwrap();
        let info = Info::new(torrent_map.get("info")?)?;
        Some(Self { announce, info })
    }
    pub fn print_info(&self) {
        println!("Tracker URL: {}", self.announce);
        println!("Length: {}", self.info.length);
        println!("Info Hash: {}", self.info.hash);
        println!("Piece Length: {}", self.info.piece_length);
        println!("Piece Hashes: ");
        self.info.print_piece_hashes();
    }
    pub fn get_url(&self) -> String {
        self.announce.clone()
    }
}