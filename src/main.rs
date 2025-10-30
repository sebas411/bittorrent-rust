use serde_json::{self, Map, Number, Value};
use std::env;

fn decode_bencoded_value(encoded_value: &str) -> (Value, String) {
    if encoded_value.chars().next().unwrap().is_digit(10) {
        // Example: "5:hello" -> "hello"
        let colon_index = encoded_value.find(':').unwrap();
        let number_string = &encoded_value[..colon_index];
        let number = number_string.parse::<usize>().unwrap();
        let string = &encoded_value[colon_index + 1..colon_index + 1 + number];
        return (Value::String(string.to_string()), encoded_value[colon_index + 1 + number..].into());
    } else if encoded_value.chars().next().unwrap() == 'i' {
        // Example: "i-52e" -> -52
        let i_index = encoded_value.find('i').unwrap();
        let e_index = encoded_value.find('e').unwrap();
        let number_string = &encoded_value[i_index + 1..e_index];
        let number = number_string.parse::<i64>().unwrap();
        return (Value::Number(Number::from(number)), encoded_value[e_index+1..].into());
    } else if encoded_value.chars().next().unwrap() == 'l' {
        // Example: "l5:helloi52ee" -> ["hello",52]
        let mut current_string = String::from(&encoded_value[1..]);
        let mut value_list = vec![];
        while current_string.chars().next().unwrap() != 'e' {
            let (value, rest) = decode_bencoded_value(&current_string);
            value_list.push(value);
            current_string = rest;
        }
        return (Value::Array(value_list), current_string[1..].into())
    } else if encoded_value.chars().next().unwrap() == 'd' {
        // Example: d3:foo3:bar5:helloi52ee
        let mut map = Map::new();
        let mut current_string = String::from(&encoded_value[1..]);
        while current_string.chars().next().unwrap() != 'e' {
            let (key, rest) = decode_bencoded_value(&current_string);
            current_string = rest;
            if let Value::String(key) = key {
                let (val, rest) = decode_bencoded_value(&current_string);
                map.insert(key, val);
                current_string = rest;
            } else {
                panic!("Dictionary keys should be Strings.");
            }
        }
        return (Value::Object(map), current_string[1..].into());
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let (decoded_value, _) = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
