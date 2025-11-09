use std::{io::{Read, Write}, net::TcpStream};

use hex::decode;
use sha1::{Digest, Sha1};

use crate::{generate_random_string, modules::{bencode::{decode_bencoded_value, encode_value}, helpers::{get_handshake, get_peers}, value::{Map, Value}}};

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
    pub fn get_piece(&self, piece_index: usize) -> [u8; 20] {
        let piece = self.pieces[piece_index];
        piece
    }
    pub fn get_piece_size(&self) -> usize {
        self.piece_length as usize
    }
    pub fn get_file_size(&self) -> usize {
        self.length as usize
    }
    pub fn total_pieces(&self) -> usize {
        self.pieces.len()
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
    pub fn from_magnet(magnet: Magnet) -> Option<Self> {
        let my_id = generate_random_string(20);
        let info_hash = magnet.get_info_hash_bytes();
        let handshake = get_handshake(&info_hash, &my_id, true);
        let peers = get_peers(&magnet.get_url().unwrap(), &info_hash, &my_id, 999);
        let peer = format!("{}:{}", peers[0].0, peers[0].1);

        let mut stream = TcpStream::connect(peer).expect("Failed to connect");
        stream.write_all(&handshake).expect("Failed to write to stream");

        let mut buffer = [0; 1024];
        stream.read_exact(&mut buffer[0..1]).expect("Failed to read from stream");
        let protocol_length = buffer[0] as usize;
        stream.read_exact(&mut buffer[1..1+protocol_length+8+20+20]).unwrap();
        let has_extension_support = buffer[1+protocol_length+5] & 16u8 > 0;

        // wait for bitfield
        stream.read_exact(&mut buffer[..4]).expect("Failed to read from stream");
        let length = u32::from_be_bytes(buffer[0..4].try_into().unwrap());
        stream.read_exact(&mut buffer[4..4+length as usize]).unwrap();
        let message_type = buffer[4];

        if message_type != 5 || !has_extension_support {
            return None
        }

        //extension handshake
        let mut inner_dict = Map::new();
        let my_metadata_ext_id = 2;
        inner_dict.insert("ut_metadata".as_bytes().to_vec(), Value::Int(my_metadata_ext_id));
        let mut outer_dict = Map::new();
        outer_dict.insert("m".as_bytes().to_vec(), Value::Map(inner_dict));
        let bencoded_value = encode_value(Value::Map(outer_dict));
        let mut extension_handshake = vec![];
        extension_handshake.extend((bencoded_value.len() as u32 + 2).to_be_bytes());
        extension_handshake.push(20);
        extension_handshake.push(0);
        extension_handshake.extend(bencoded_value);
        stream.write_all(&extension_handshake).expect("Couldn't write to stream");

        stream.read_exact(&mut buffer[0..4]).expect("Couldn't read from stream");
        let length = u32::from_be_bytes(buffer[0..4].try_into().unwrap());
        stream.read_exact(&mut buffer[4..4+length as usize]).expect("Failed to read from stream");
        let message_type = buffer[4];
        if message_type != 20 {
            println!("Message didnt match");
            return None
        }
        let bencoded_dict = &buffer[6..4+length as usize];
        let outer_dict = decode_bencoded_value(bencoded_dict).0.get_map().unwrap();
        let inner_dict = outer_dict.get("m").unwrap().get_map().unwrap();
        let metadata_ext_id = inner_dict.get("ut_metadata")?;
        let metadata_ext_id  = metadata_ext_id.get_int().unwrap() as u8;

        let mut request_dict = Map::new();
        request_dict.insert("msg_type".as_bytes().to_vec(), Value::Int(0));
        request_dict.insert("piece".as_bytes().to_vec(), Value::Int(0));
        let request_dict_encoded = encode_value(Value::Map(request_dict));

        let mut info_request = vec![];
        info_request.extend((2 + request_dict_encoded.len() as u32).to_be_bytes());
        info_request.push(20);
        info_request.push(metadata_ext_id);
        info_request.extend(request_dict_encoded);

        stream.write_all(&info_request).unwrap();

        stream.read_exact(&mut buffer[..4]).unwrap();

        let length = u32::from_be_bytes(buffer[0..4].try_into().unwrap());
        stream.read_exact(&mut buffer[4..4+length as usize]).unwrap();
        let message_id = buffer[4];
        let extension_message_id = buffer[5];
        if message_id != 20 || extension_message_id != my_metadata_ext_id as u8 {
            println!("Message types didnt match");
            println!("Message_id: {}", message_id);
            println!("Extension_message_id: {}", extension_message_id);
            println!("Metadata_ext_id: {}", metadata_ext_id);
            return None;
        }
        let (_, rest) = decode_bencoded_value(&buffer[6..4+length as usize]);
        let (metadata, _) = decode_bencoded_value(&rest);
        let info = Info::new(metadata)?;
        let info_hash = info.get_info_hash_bytes();
        if info_hash != magnet.get_info_hash_bytes() {
            println!("Info hash doesn't match");
            return None
        }
        Some(Self { announce: magnet.get_url()?, info })
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

pub struct Magnet {
    tracker: Option<String>,
    info_hash: String,
    _filename: Option<String>,
}

impl Magnet {
    pub fn new(magnet_link: &str) -> Option<Self> {
        let params_str = magnet_link.strip_prefix("magnet:?").unwrap();
        let mut filename = None;
        let mut tracker = None;
        let mut info_hash = None;
        for param_str in params_str.split('&') {
            let eq_index = param_str.find('=').unwrap();
            let (name, value) = param_str.split_at(eq_index);
            let value = value.strip_prefix('=').unwrap();
            if name == "xt" {
                info_hash = Some(value.strip_prefix("urn:btih:").unwrap().into());
            } else if name == "dn" {
                filename = Some(value.into());
            } else if name == "tr" {
                let tracker_decoded: String = urlencoding::decode(value).unwrap().into();
                tracker = Some(tracker_decoded.trim().into());
            }
        }
        Some(Self { tracker, info_hash: info_hash?, _filename: filename})
    }
    pub fn print_info(&self) {
        println!("Tracker URL: {}", self.tracker.clone().unwrap());
        println!("Info Hash: {}", self.info_hash)
    }
    pub fn get_info_hash_bytes(&self) -> Vec<u8> {
        let hex_str = &self.info_hash;
        decode(hex_str).unwrap()
    }
    pub fn get_url(&self) -> Option<String> {
        self.tracker.clone()
    }
}