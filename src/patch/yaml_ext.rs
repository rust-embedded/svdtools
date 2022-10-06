use super::iterators::OptIter;
use anyhow::{anyhow, Context, Result};
use yaml_rust::{yaml::Hash, Yaml};

/// Errors that can occur during building.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum YamlError {
    #[error("Value is not a hash map (dictionary): {0:?}")]
    NotHash(Yaml),
    #[error("Value is not a vector (array): {0:?}")]
    NotVec(Yaml),
    #[error("Value is not a string: {0:?}")]
    NotStr(Yaml),
    #[error("Value is not integer: {0:?}")]
    NotInt(Yaml),
    #[error("Value is not boolean: {0:?}")]
    NotBool(Yaml),
}

pub trait AsType {
    fn hash_mut(&mut self) -> Result<&mut Hash, YamlError>;
    fn hash(&self) -> Result<&Hash, YamlError>;
    fn vec(&self) -> Result<&Vec<Yaml>, YamlError>;
    fn str(&self) -> Result<&str, YamlError>;
    fn i64(&self) -> Result<i64, YamlError>;
    fn bool(&self) -> Result<bool, YamlError>;
}

impl AsType for Yaml {
    fn hash_mut(&mut self) -> Result<&mut Hash, YamlError> {
        match self {
            Yaml::Hash(h) => Ok(h),
            _ => Err(YamlError::NotHash(self.clone())),
        }
    }
    fn hash(&self) -> Result<&Hash, YamlError> {
        self.as_hash()
            .ok_or_else(|| YamlError::NotHash(self.clone()))
    }
    fn vec(&self) -> Result<&Vec<Yaml>, YamlError> {
        self.as_vec().ok_or_else(|| YamlError::NotVec(self.clone()))
    }
    fn str(&self) -> Result<&str, YamlError> {
        self.as_str().ok_or_else(|| YamlError::NotStr(self.clone()))
    }
    fn i64(&self) -> Result<i64, YamlError> {
        parse_i64(self).ok_or_else(|| YamlError::NotInt(self.clone()))
    }
    fn bool(&self) -> Result<bool, YamlError> {
        parse_bool(self).ok_or_else(|| YamlError::NotBool(self.clone()))
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
            let text = text.replace('_', "");
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
            } else if let Some(stripped) = text.strip_prefix("0b") {
                // Handle strings in the binary form of:
                // 0b01101x1
                // along with don't care character x (replaced with 0)
                i64::from_str_radix(&str::replace(stripped, "x", "0"), 2)
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

type HashIter<'a> = OptIter<linked_hash_map::Iter<'a, Yaml, Yaml>>;

pub trait GetVal {
    fn get_bool(&self, k: &str) -> Result<Option<bool>>;
    fn get_i64(&self, k: &str) -> Result<Option<i64>>;
    fn get_u64(&self, k: &str) -> Result<Option<u64>> {
        self.get_i64(k).map(|v| v.map(|v| v as u64))
    }
    fn get_u32(&self, k: &str) -> Result<Option<u32>> {
        self.get_i64(k).map(|v| v.map(|v| v as u32))
    }
    fn get_str(&self, k: &str) -> Result<Option<&str>>;
    fn get_string(&self, k: &str) -> Result<Option<String>> {
        self.get_str(k).map(|v| v.map(From::from))
    }
    fn get_hash(&self, k: &str) -> Result<Option<&Hash>>;
    fn hash_iter<'a>(&'a self, k: &str) -> HashIter<'a>;
    fn get_vec(&self, k: &str) -> Result<Option<&Vec<Yaml>>>;
    fn str_vec_iter<'a>(&'a self, k: &str) -> Result<OptIter<OverStringIter<'a>>>;
}

impl GetVal for Hash {
    fn get_bool(&self, k: &str) -> Result<Option<bool>> {
        match self.get(&k.to_yaml()) {
            None => Ok(None),
            Some(v) => v
                .bool()
                .with_context(|| format!("Under key `{k}`"))
                .map(Some),
        }
    }
    fn get_i64(&self, k: &str) -> Result<Option<i64>> {
        match self.get(&k.to_yaml()) {
            None => Ok(None),
            Some(v) => v
                .i64()
                .with_context(|| format!("Under key `{k}`"))
                .map(Some),
        }
    }
    fn get_str(&self, k: &str) -> Result<Option<&str>> {
        match self.get(&k.to_yaml()) {
            None => Ok(None),
            Some(v) => v
                .str()
                .with_context(|| format!("Under key `{k}`"))
                .map(Some),
        }
    }
    fn get_hash(&self, k: &str) -> Result<Option<&Hash>> {
        match self.get(&k.to_yaml()) {
            None => Ok(None),
            Some(v) => v
                .hash()
                .with_context(|| format!("Under key `{k}`"))
                .map(Some),
        }
    }
    fn hash_iter<'a>(&'a self, k: &str) -> HashIter<'a> {
        HashIter::new(
            self.get(&k.to_yaml())
                .and_then(Yaml::as_hash)
                .map(|h| h.iter()),
        )
    }
    fn get_vec(&self, k: &str) -> Result<Option<&Vec<Yaml>>> {
        match self.get(&k.to_yaml()) {
            None => Ok(None),
            Some(v) => v
                .vec()
                .with_context(|| format!("Under key `{k}`"))
                .map(Some),
        }
    }
    fn str_vec_iter<'a>(&'a self, k: &str) -> Result<OptIter<OverStringIter<'a>>> {
        Ok(OptIter::new(match self.get(&k.to_yaml()) {
            None => None,
            Some(y) if matches!(y, Yaml::String(_) | Yaml::Array(_)) => {
                Some(OverStringIter(y, None))
            }
            _ => return Err(anyhow!("`{k}` requires string value or array of strings")),
        }))
    }
}
