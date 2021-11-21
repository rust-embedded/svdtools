use super::iterators::OptIter;
use yaml_rust::{yaml::Hash, Yaml};

pub trait AsTypeMut {
    fn as_hash_mut(&mut self) -> Option<&mut Hash>;
}

impl AsTypeMut for Yaml {
    fn as_hash_mut(&mut self) -> Option<&mut Hash> {
        match self {
            Yaml::Hash(h) => Some(h),
            _ => None,
        }
    }
}

pub trait ToYaml {
    fn to_yaml(self) -> Yaml;
}

impl ToYaml for &str {
    fn to_yaml(self) -> Yaml {
        Yaml::String(self.into())
    }
}

impl ToYaml for Yaml {
    fn to_yaml(self) -> Yaml {
        self
    }
}

pub fn parse_i64(val: &Yaml) -> Option<i64> {
    match val {
        Yaml::Integer(i) => Some(*i),
        Yaml::String(text) => {
            let text = text.replace("_", "");
            (if text.starts_with("0x") || text.starts_with("0X") {
                i64::from_str_radix(&text["0x".len()..], 16)
            } else if text.starts_with('#') {
                // Handle strings in the binary form of:
                // #01101x1
                // along with don't care character x (replaced with 0)
                i64::from_str_radix(
                    &str::replace(&text.to_lowercase()["#".len()..], "x", "0"),
                    2,
                )
            } else if text.starts_with("0b") {
                // Handle strings in the binary form of:
                // 0b01101x1
                // along with don't care character x (replaced with 0)
                i64::from_str_radix(&str::replace(&text["0b".len()..], "x", "0"), 2)
            } else {
                text.parse::<i64>()
            })
            .ok()
        }
        _ => None,
    }
}

pub fn parse_bool(val: &Yaml) -> Option<bool> {
    match val {
        Yaml::Boolean(b) => Some(*b),
        Yaml::Integer(i) => match *i {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        Yaml::String(text) => match text.as_str() {
            "true" | "True" => Some(true),
            "false" | "False" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

pub struct OverStringIter<'a>(&'a Yaml, Option<usize>);
impl<'a> Iterator for OverStringIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        loop {
            match &mut self.1 {
                None => {
                    if let Some(s) = self.0.as_str() {
                        self.1 = Some(0);
                        return Some(s);
                    }
                    self.1 = Some(0);
                }
                Some(n) => {
                    if let Some(v) = self.0.as_vec() {
                        if *n == v.len() {
                            return None;
                        }
                        if let Some(res) = &v[*n].as_str() {
                            *n += 1;
                            return Some(res);
                        }
                        *n += 1;
                    } else {
                        return None;
                    }
                }
            }
        }
    }
}

type HashIter<'a> = OptIter<(&'a Yaml, &'a Yaml), linked_hash_map::Iter<'a, Yaml, Yaml>>;

pub trait GetVal {
    fn get_bool<K: ToYaml>(&self, k: K) -> Option<bool>;
    fn get_i64<K: ToYaml>(&self, k: K) -> Option<i64>;
    fn get_u64<K: ToYaml>(&self, k: K) -> Option<u64> {
        self.get_i64(k).map(|v| v as u64)
    }
    fn get_u32<K: ToYaml>(&self, k: K) -> Option<u32> {
        self.get_i64(k).map(|v| v as u32)
    }
    fn get_str<K: ToYaml>(&self, k: K) -> Option<&str>;
    fn get_string<K: ToYaml>(&self, k: K) -> Option<String> {
        self.get_str(k).map(String::from)
    }
    fn get_hash<K: ToYaml>(&self, k: K) -> Option<&Hash>;
    fn hash_iter<'a, K: ToYaml>(&'a self, k: K) -> HashIter<'a>;
    fn get_vec<K: ToYaml>(&self, k: K) -> Option<&Vec<Yaml>>;
    fn str_vec_iter<'a, K: ToYaml>(&'a self, k: K) -> OptIter<&'a str, OverStringIter<'a>>;
}

impl GetVal for Hash {
    fn get_bool<K: ToYaml>(&self, k: K) -> Option<bool> {
        self.get(&k.to_yaml()).and_then(parse_bool)
    }
    fn get_i64<K: ToYaml>(&self, k: K) -> Option<i64> {
        self.get(&k.to_yaml()).and_then(parse_i64)
    }
    fn get_str<K: ToYaml>(&self, k: K) -> Option<&str> {
        self.get(&k.to_yaml()).and_then(Yaml::as_str)
    }
    fn get_hash<K: ToYaml>(&self, k: K) -> Option<&Hash> {
        self.get(&k.to_yaml()).and_then(Yaml::as_hash)
    }
    fn hash_iter<'a, K: ToYaml>(&'a self, k: K) -> HashIter<'a> {
        HashIter::new(self.get_hash(k).map(|h| h.iter()))
    }
    fn get_vec<K: ToYaml>(&self, k: K) -> Option<&Vec<Yaml>> {
        self.get(&k.to_yaml()).and_then(Yaml::as_vec)
    }
    fn str_vec_iter<'a, K: ToYaml>(&'a self, k: K) -> OptIter<&'a str, OverStringIter<'a>> {
        OptIter::new(self.get(&k.to_yaml()).map(|y| OverStringIter(y, None)))
    }
}
