use crate::patch::ToYaml;
use svd_rs::EnumeratedValues;
use yaml_rust::yaml::{self, Yaml};

pub trait HasEnums {
    fn has_enums(&self) -> bool;
}

impl HasEnums for svd_rs::Field {
    fn has_enums(&self) -> bool {
        !self.enumerated_values.is_empty()
    }
}

impl HasEnums for svd_rs::Register {
    fn has_enums(&self) -> bool {
        if let Some(fields) = self.fields.as_ref() {
            for f in fields {
                if f.has_enums() {
                    return true;
                }
            }
        }
        false
    }
}

impl HasEnums for svd_rs::RegisterCluster {
    fn has_enums(&self) -> bool {
        match self {
            svd_rs::RegisterCluster::Cluster(c) => c.has_enums(),
            svd_rs::RegisterCluster::Register(r) => r.has_enums(),
        }
    }
}

impl HasEnums for svd_rs::Cluster {
    fn has_enums(&self) -> bool {
        for rc in &self.children {
            if rc.has_enums() {
                return true;
            }
        }
        false
    }
}

impl HasEnums for svd_rs::Peripheral {
    fn has_enums(&self) -> bool {
        if let Some(regs) = self.registers.as_ref() {
            for rc in regs {
                if rc.has_enums() {
                    return true;
                }
            }
        }
        false
    }
}

fn evs_to_hash(evs: &EnumeratedValues) -> yaml::Hash {
    let mut hash = yaml::Hash::with_capacity(evs.values.len());
    if let Some(n) = evs.name.as_ref() {
        hash.insert("_name".to_yaml(), n.to_yaml());
    }
    if let Some(d) = evs.derived_from.as_ref() {
        hash.insert("_derivedFrom".to_yaml(), d.to_yaml());
    } else {
        for ev in &evs.values {
            let val = if let Some(val) = ev.value {
                Yaml::Integer(val as _)
            } else if ev.is_default() {
                Yaml::Integer(-1)
            } else {
                panic!("EnumeratedValue without value");
            };
            hash.insert(
                ev.name.to_yaml(),
                Yaml::Array(vec![val, ev.description.as_deref().unwrap_or("").to_yaml()]),
            );
        }
    }
    hash
}

fn rc_enum_extact(regs: &[svd_rs::RegisterCluster]) -> Yaml {
    let mut phash = yaml::Hash::new();
    let mut pchash = yaml::Hash::new();
    for rc in regs {
        if rc.has_enums() {
            match rc {
                svd_rs::RegisterCluster::Cluster(c) => {
                    pchash.insert(c.name.to_yaml(), rc_enum_extact(&c.children));
                }
                svd_rs::RegisterCluster::Register(r) => {
                    let mut rhash = yaml::Hash::new();
                    for f in r.fields() {
                        if f.has_enums() {
                            let mut fhash = yaml::Hash::with_capacity(f.enumerated_values.len());
                            for evs in &f.enumerated_values {
                                match evs.usage {
                                    Some(svd_rs::Usage::Read) => {
                                        fhash.insert(
                                            "_read".to_yaml(),
                                            Yaml::Hash(evs_to_hash(evs)),
                                        );
                                    }
                                    Some(svd_rs::Usage::Write) => {
                                        fhash.insert(
                                            "_write".to_yaml(),
                                            Yaml::Hash(evs_to_hash(evs)),
                                        );
                                    }
                                    _ => {
                                        assert_eq!(f.enumerated_values.len(), 1);
                                        fhash.extend(evs_to_hash(evs));
                                    }
                                }
                            }
                            rhash.insert(f.name.to_yaml(), Yaml::Hash(fhash));
                        }
                    }
                    phash.insert(r.name.to_yaml(), Yaml::Hash(rhash));
                }
            }
        }
    }
    if !pchash.is_empty() {
        phash.insert("_clusters".to_yaml(), Yaml::Hash(pchash));
    }
    Yaml::Hash(phash)
}

pub fn enum_extract(device: &svd_rs::Device) -> Yaml {
    let mut hash = yaml::Hash::new();
    for p in &device.peripherals {
        if let Some(regs) = p.registers.as_ref() {
            if p.has_enums() {
                hash.insert(p.name.to_yaml(), rc_enum_extact(regs));
            }
        }
    }
    Yaml::Hash(hash)
}
