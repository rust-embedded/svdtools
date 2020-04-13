use super::yaml_parser::{
    Cpu, Field, FieldCommand, ModifyPeripheral, Peripheral, PeripheralCommand, PeripheralNode,
    Register, RegisterCommand, RegisterNode, YamlBody,
};
use std::{collections::HashMap, hash::Hash};

pub trait Merge {
    fn merge(&mut self, other: &Self);
}

impl Merge for YamlBody {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
        merge_hashmap(&mut self.peripherals, &other.peripherals)
    }
}

impl Merge for PeripheralCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        merge_option(&mut self.modify, &other.modify);
        // TODO merge add
        // TODO merge copy
    }
}

impl Merge for ModifyPeripheral {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.cpu, &other.cpu);
        merge_hashmap(&mut self.peripherals, &other.peripherals);
    }
}

impl Merge for PeripheralNode {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
        merge_hashmap(&mut self.registers, &other.registers);
    }
}

impl Merge for RegisterNode {
    fn merge(&mut self, other: &Self) {
        self.commands.merge(&other.commands);
    }
}

impl Merge for RegisterCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        merge_opt_struct(&mut self.modify, &other.modify);
        // TODO merge add
    }
}

impl Merge for FieldCommand {
    fn merge(&mut self, other: &Self) {
        self.delete.extend(other.delete.clone());
        self.merge.extend(other.merge.clone());

        merge_hashmap(&mut self.modify, &other.modify);
    }
}

impl Merge for Peripheral {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.group_name, &other.group_name);
        merge_option(&mut self.base_address, &other.base_address);
        merge_option(&mut self.address_block, &other.address_block);
        merge_opt_vec(&mut self.registers, &other.registers)
    }
}

impl Merge for Register {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.display_name, &other.display_name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.address_offset, &other.address_offset);
        merge_option(&mut self.size, &other.size);
        merge_option(&mut self.access, &other.access);
        merge_option(&mut self.reset_value, &other.reset_value);
        merge_opt_vec(&mut self.fields, &other.fields)
    }
}

impl Merge for Field {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.description, &other.description);
        merge_option(&mut self.bit_offset, &other.bit_offset);
        merge_option(&mut self.bit_width, &other.bit_width);
    }
}

impl Merge for Cpu {
    fn merge(&mut self, other: &Self) {
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.name, &other.name);
        merge_option(&mut self.revision, &other.revision);
        merge_option(&mut self.endian, &other.endian);
        merge_option(&mut self.mpu_present, &other.mpu_present);
        merge_option(&mut self.fpu_present, &other.fpu_present);
        merge_option(&mut self.nvic_prio_bits, &other.nvic_prio_bits);
        merge_option(
            &mut self.vendor_systick_config,
            &other.vendor_systick_config,
        );
    }
}

fn merge_hashmap<K, V>(dest: &mut HashMap<K, V>, src: &HashMap<K, V>)
where
    K: Eq + Hash + Clone,
    V: Clone + Merge,
{
    for (key, value) in src {
        let corresponding = dest.get_mut(key);
        if let Some(entry) = corresponding {
            entry.merge(value);
        } else {
            dest.insert(key.clone(), value.clone());
        }
    }
}

fn merge_opt_vec<T: Clone + Merge>(dest: &mut Option<Vec<T>>, src: &Option<Vec<T>>) {
    if let Some(src) = src {
        let mut src = src.clone();
        match dest {
            Some(dest) => dest.append(&mut src),
            None => *dest = Some(src),
        }
    }
}

fn merge_opt_struct<T: Clone + Merge>(dest: &mut Option<T>, src: &Option<T>) {
    if let Some(src) = src {
        match dest {
            Some(dest) => dest.merge(src),
            None => *dest = Some(src.clone()),
        }
    }
}

fn merge_option<T: Clone>(dest: &mut Option<T>, src: &Option<T>) {
    if dest.is_none() && src.is_some() {
        *dest = src.clone();
    }
}
