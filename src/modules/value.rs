use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone)]
pub struct Map(BTreeMap<Vec<u8>, Value>);

impl Map {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    pub fn insert(&mut self, k: Vec<u8>, v: Value) {
        self.0.insert(k, v);
    }
    pub fn to_string(&self) -> String {
        let mut map_string = String::from('{');
        for (i, (k, v)) in self.0.iter().enumerate() {
            let key_string = String::from_utf8(k.clone()).unwrap_or("String not in utf8".into());
            map_string.push_str(&format!("\"{}\":{}", key_string, v));
            if i != self.0.len() - 1 {
                map_string.push(',');
            }
        }
        map_string.push('}');
        map_string
    }
    pub fn get(&self, key: &str) -> Option<Value> {
        match self.0.get(key.as_bytes()) {
            Some(value) => Some(value.clone()),
            None => None
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    String(Vec<u8>),
    Int(i64),
    List(Vec<Value>),
    Map(Map),
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Self::String(value) => {
                if let Ok(string_value) = String::from_utf8(value.clone()) {
                    string_value
                } else {
                    "String not in utf8".into()
                }
            },
            Self::Int(value) => value.to_string(),
            Self::List(list) => {
                let mut output = String::from("[");
                for (i, val) in list.iter().enumerate() {
                    output.push_str(&format!("{}", val));
                    if i != list.len() - 1 {
                        output.push(',');
                    }
                }
                output.push(']');
                output
            },
            Self::Map(map) => {
                map.to_string()
            },
        }
    }
    pub fn get_map(self) -> Option<Map> {
        if let Self::Map(map) = self {
            Some(map)
        } else {
            None
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::String(_) = self {
            return write!(f, "\"{}\"", self.to_string())
        }
        write!(f, "{}", self.to_string())
    }
}