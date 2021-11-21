use svd_parser::svd::{
    self, Cluster, ClusterInfo, DimElement, Interrupt, Peripheral, Register, RegisterCluster,
    RegisterInfo,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIterMut, Matched};
use super::register::{RegisterExt, RegisterInfoExt};
use super::yaml_ext::{GetVal, ToYaml};
use super::{check_offsets, matchname, matchsubspec, spec_ind, VAL_LVL};
use super::{make_cluster, make_interrupt, make_register};

pub type ClusterMatchIterMut<'a, 'b> =
    MatchIterMut<'a, 'b, Cluster, std::slice::IterMut<'a, Cluster>>;
pub type RegMatchIterMut<'a, 'b> = MatchIterMut<'a, 'b, Register, svd::register::RegIterMut<'a>>;

/// Collecting methods for processing peripheral contents
pub trait PeripheralExt {
    /// Iterates over all registers that match rspec and live inside ptag
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b>;

    /// Iterate over all clusters that match cpsec and live inside ptag
    fn iter_clusters<'a, 'b>(&mut self, spec: &str) -> ClusterMatchIterMut<'a, 'b>;

    /// Iterates over all interrupts matching ispec
    fn iter_interrupts<'a, 'b>(
        &'a mut self,
        spec: &'b str,
    ) -> MatchIterMut<'a, 'b, Interrupt, std::slice::IterMut<'a, Interrupt>>;

    /// Work through a peripheral, handling all registers
    fn process(&mut self, peripheral: &Hash, update_fields: bool);

    /// Delete interrupts matched by ispec
    fn delete_interrupt(&mut self, ispec: &str);

    /// Add iname given by iadd to ptag
    fn add_interrupt(&mut self, iname: &str, iadd: &Hash);

    /// Modify ispec according to imod
    fn modify_interrupt(&mut self, ispec: &str, imod: &Hash);

    /// Delete registers matched by rspec inside ptag
    fn delete_register(&mut self, rspec: &str);

    /// Add rname given by radd to ptag
    fn add_register(&mut self, rname: &str, radd: &Hash);

    /// Add rname given by deriving from rsource to ptag
    fn copy_register(&mut self, rname: &str, rderive: &Hash);

    /// Modify rspec inside ptag according to rmod
    fn modify_register(&mut self, rspec: &str, rmod: &Hash);

    /// Modify cspec inside ptag according to cmod
    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash);
    /// Work through a register, handling all fields
    fn process_register(&mut self, rspec: &str, register: &Hash, update_fields: bool);

    /// Delete substring from the beginning of register names inside ptag
    fn strip_start(&mut self, substr: &str);

    /// Delete substring from the ending of register names inside ptag
    fn strip_end(&mut self, substr: &str);

    /// Collect same registers in peripheral into register array
    fn collect_in_array(&mut self, rspec: &str, rmod: &Hash);

    /// Collect registers in peripheral into clusters
    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash);
}

impl PeripheralExt for Peripheral {
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b> {
        self.reg_iter_mut().matched(spec)
    }

    fn iter_interrupts<'a, 'b>(
        &'a mut self,
        spec: &'b str,
    ) -> MatchIterMut<'a, 'b, Interrupt, std::slice::IterMut<'a, Interrupt>> {
        self.interrupt.iter_mut().matched(spec)
    }

    fn process(&mut self, pmod: &Hash, update_fields: bool) {
        // For derived peripherals, only process interrupts
        if self.derived_from.is_some() {
            if let Some(deletions) = pmod.get_hash("_delete") {
                for ispec in deletions.str_vec_iter("_interrupts") {
                    self.delete_interrupt(ispec);
                }
            }
            for (rspec, rmod) in pmod.get_hash("_modify").unwrap_or(&Hash::new()) {
                if rspec.as_str() == Some("_interrupts") {
                    for (ispec, val) in rmod.as_hash().unwrap() {
                        self.modify_interrupt(ispec.as_str().unwrap(), val.as_hash().unwrap())
                    }
                }
            }
            for (rname, radd) in pmod.get_hash("_add").unwrap_or(&Hash::new()) {
                if rname.as_str() == Some("_interrupts") {
                    for (iname, val) in radd.as_hash().unwrap() {
                        self.add_interrupt(iname.as_str().unwrap(), val.as_hash().unwrap())
                    }
                }
            }
            // Don't do any further processing on derived peripherals
            return;
        }

        // Handle deletions
        if let Some(deletions) = pmod.get(&"_delete".to_yaml()) {
            match deletions {
                Yaml::String(rspec) => {
                    self.delete_register(rspec);
                }
                Yaml::Array(deletions) => {
                    for rspec in deletions {
                        self.delete_register(rspec.as_str().unwrap());
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers") {
                        self.delete_register(rspec);
                    }
                    for ispec in deletions.str_vec_iter("_interrupts") {
                        self.delete_interrupt(ispec);
                    }
                }
                _ => {}
            }
        }

        // Handle modifications
        for (rspec, rmod) in pmod.hash_iter("_modify") {
            let rmod = rmod.as_hash().unwrap();
            match rspec.as_str().unwrap() {
                "_registers" => {
                    for (rspec, val) in rmod {
                        self.modify_register(rspec.as_str().unwrap(), val.as_hash().unwrap());
                    }
                }
                "_interrupts" => {
                    for (ispec, val) in rmod {
                        self.modify_interrupt(ispec.as_str().unwrap(), val.as_hash().unwrap());
                    }
                }
                "_cluster" => {
                    for (cspec, val) in rmod {
                        self.modify_cluster(cspec.as_str().unwrap(), val.as_hash().unwrap());
                    }
                }
                rspec => self.modify_register(rspec, rmod),
            }
        }

        // Handle strips
        for prefix in pmod.str_vec_iter("_strip") {
            self.strip_start(prefix);
        }
        for suffix in pmod.str_vec_iter("_strip_end") {
            self.strip_end(suffix);
        }

        // Handle additions
        for (rname, radd) in pmod.hash_iter("_add") {
            let radd = radd.as_hash().unwrap();
            match rname.as_str().unwrap() {
                "_registers" => {
                    for (rname, val) in radd {
                        self.add_register(rname.as_str().unwrap(), val.as_hash().unwrap())
                    }
                }
                "_interrupts" => {
                    for (iname, val) in radd {
                        self.add_interrupt(iname.as_str().unwrap(), val.as_hash().unwrap())
                    }
                }
                rname => self.add_register(rname, radd),
            }
        }

        for (rname, rderive) in pmod.hash_iter("_derive") {
            let rderive = rderive.as_hash().unwrap();
            let rname = rname.as_str().unwrap();
            match rname {
                "_registers" => {
                    for (rname, val) in rderive {
                        self.copy_register(rname.as_str().unwrap(), val.as_hash().unwrap());
                    }
                }
                "_interrupts" => panic!("deriving interrupts not implemented yet: {}", rname),
                _ => self.copy_register(rname, rderive),
            }
        }

        // Handle registers
        for (rspec, register) in pmod {
            let rspec = rspec.as_str().unwrap();
            if !rspec.starts_with("_") {
                self.process_register(rspec, register.as_hash().unwrap(), update_fields)
            }
        }

        // Handle register arrays
        for (rspec, rmod) in pmod.hash_iter("_array") {
            self.collect_in_array(rspec.as_str().unwrap(), rmod.as_hash().unwrap());
        }

        // Handle clusters
        for (cname, cmod) in pmod.hash_iter("_cluster") {
            self.collect_in_cluster(cname.as_str().unwrap(), cmod.as_hash().unwrap());
        }
    }

    fn iter_clusters<'a, 'b>(&mut self, _spec: &str) -> ClusterMatchIterMut<'a, 'b> {
        todo!()
    }

    fn add_interrupt(&mut self, iname: &str, iadd: &Hash) {
        assert!(
            self.interrupt.iter().find(|i| &i.name == iname).is_none(),
            "peripheral {} already has an interrupt {}",
            self.name,
            iname
        );
        self.interrupt.push(
            make_interrupt(iadd)
                .name(iname.into())
                .build(VAL_LVL)
                .unwrap(),
        );
    }

    fn modify_interrupt(&mut self, ispec: &str, imod: &Hash) {
        for itag in self.iter_interrupts(ispec) {
            itag.modify_from(make_interrupt(imod), VAL_LVL).unwrap();
        }
    }

    fn delete_interrupt(&mut self, ispec: &str) {
        self.interrupt.retain(|i| !(matchname(&i.name, ispec)));
    }

    fn modify_register(&mut self, rspec: &str, rmod: &Hash) {
        for rtag in self.iter_registers(rspec) {
            rtag.modify_from(make_register(rmod), VAL_LVL).unwrap();
        }
    }

    fn add_register(&mut self, rname: &str, radd: &Hash) {
        if self.reg_iter().find(|r| r.name == rname).is_some() {
            panic!("peripheral {} already has a register {}", self.name, rname);
        }
        self.registers
            .get_or_insert_with(Default::default)
            .push(RegisterCluster::Register(
                make_register(radd)
                    .name(rname.into())
                    .build(VAL_LVL)
                    .unwrap()
                    .single(),
            ));
    }

    fn copy_register(&mut self, rname: &str, rderive: &Hash) {
        let srcname = rderive.get_str("_from").unwrap_or_else(|| {
            panic!(
                "derive: source register not given, please add a _from field to {}",
                rname
            )
        });

        let mut source = self
            .reg_iter()
            .find(|p| &p.name == srcname)
            .unwrap_or_else(|| {
                panic!(
                    "peripheral {} does not have register {}",
                    self.name, srcname
                )
            })
            .clone();
        let fixes = make_register(rderive)
            .name(rname.into())
            .display_name(Some("".into()));
        // Modifying fields in derived register not implemented
        source.modify_from(fixes, VAL_LVL).unwrap();
        if let Some(ptag) = self.reg_iter_mut().find(|r| &r.name == rname) {
            source.address_offset = ptag.address_offset;
            *ptag = source;
        } else {
            self.registers
                .as_mut()
                .unwrap()
                .push(RegisterCluster::Register(source))
        }
    }

    fn delete_register(&mut self, rspec: &str) {
        // TODO: delete registers in clusters
        if let Some(registers) = &mut self.registers {
            registers.retain(
                |r| !matches!(r, RegisterCluster::Register(r) if matchname(&r.name, rspec)),
            );
        }
    }

    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash) {
        for ctag in self.iter_clusters(cspec) {
            ctag.modify_from(make_cluster(cmod), VAL_LVL).unwrap();
        }
    }

    fn strip_start(&mut self, substr: &str) {
        let len = substr.len();
        let glob = globset::Glob::new(&(substr.to_string() + "*"))
            .unwrap()
            .compile_matcher();
        for rtag in self.reg_iter_mut() {
            if glob.is_match(&rtag.name) {
                rtag.name.drain(..len);
            }
            if let Some(dname) = rtag.display_name.as_mut() {
                if glob.is_match(&dname) {
                    dname.drain(..len);
                }
            }
        }
    }

    fn strip_end(&mut self, substr: &str) {
        let len = substr.len();
        let glob = globset::Glob::new(&("*".to_string() + substr))
            .unwrap()
            .compile_matcher();
        for rtag in self.reg_iter_mut() {
            if glob.is_match(&rtag.name) {
                let nlen = rtag.name.len();
                rtag.name.truncate(nlen - len);
            }
            if let Some(dname) = rtag.display_name.as_mut() {
                if glob.is_match(&dname) {
                    let nlen = dname.len();
                    dname.truncate(nlen - len);
                }
            }
        }
    }

    fn collect_in_array(&mut self, rspec: &str, rmod: &Hash) {
        let pname = self.name.clone();
        if let Some(regs) = self.registers.as_mut() {
            let mut registers = Vec::new();
            let mut place = usize::MAX;
            let mut i = 0;
            let (li, ri) = spec_ind(rspec);
            while i < regs.len() {
                match &regs[i] {
                    RegisterCluster::Register(Register::Single(r)) if matchname(&r.name, rspec) => {
                        if let RegisterCluster::Register(Register::Single(r)) = regs.remove(i) {
                            registers.push(r);
                            place = place.min(i);
                        }
                    }
                    _ => i += 1,
                }
            }
            if registers.is_empty() {
                panic!("{}: registers {} not found", pname, rspec);
            }
            registers.sort_by_key(|r| r.address_offset);
            let dim = registers.len();
            let dim_index = if rmod.contains_key(&"_start_from_zero".to_yaml()) {
                (0..dim).map(|v| v.to_string()).collect::<Vec<_>>()
            } else {
                registers
                    .iter()
                    .map(|r| r.name[li..r.name.len() - ri].to_string())
                    .collect::<Vec<_>>()
            };
            let offsets = registers
                .iter()
                .map(|r| r.address_offset)
                .collect::<Vec<_>>();
            let bitmasks = registers
                .iter()
                .map(RegisterInfo::get_bitmask)
                .collect::<Vec<_>>();
            let dim_increment = if dim > 1 { offsets[1] - offsets[0] } else { 0 };
            if !(check_offsets(&offsets, dim_increment)
                && bitmasks.iter().all(|&m| m == bitmasks[0]))
            {
                panic!(
                    "{}: registers cannot be collected into {} array",
                    self.name, rspec
                );
            }
            let mut rinfo = registers.swap_remove(0);
            if let Some(name) = rmod.get_str("name") {
                rinfo.name = name.into();
            } else {
                rinfo.name = format!("{}%s{}", &rspec[..li], &rspec[rspec.len() - ri..]);
            }
            if dim_index[0] == "0" {
                if let Some(desc) = rinfo.description.as_mut() {
                    *desc = desc.replace('0', "%s");
                }
            }
            let mut reg = rinfo.array(
                DimElement::builder()
                    .dim(dim as u32)
                    .dim_increment(dim_increment)
                    .dim_index(Some(dim_index))
                    .build(VAL_LVL)
                    .unwrap(),
            );
            reg.process(rmod, &pname, true);
            regs.insert(place, RegisterCluster::Register(reg));
        }
    }

    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) {
        let pname = self.name.clone();
        if let Some(regs) = self.registers.as_mut() {
            let mut rdict = linked_hash_map::LinkedHashMap::new();
            let mut first = true;
            let mut check = true;
            let mut dim = 0;
            let mut dim_index = Vec::new();
            let mut dim_increment = 0;
            let mut offsets = Vec::new();
            let mut place = usize::MAX;
            let mut rspecs = Vec::new();
            let single = !cname.contains("%s");

            for rspec in cmod.keys() {
                let rspec = rspec.as_str().unwrap();
                if rspec == "description" {
                    continue;
                }
                rspecs.push(rspec.to_string());
                let mut registers = Vec::new();
                let mut i = 0;
                while i < regs.len() {
                    match &regs[i] {
                        RegisterCluster::Register(Register::Single(r))
                            if matchname(&r.name, rspec) =>
                        {
                            if let RegisterCluster::Register(Register::Single(r)) = regs.remove(i) {
                                registers.push(r);
                                place = place.min(i);
                            }
                        }
                        _ => i += 1,
                    }
                }
                if registers.is_empty() {
                    panic!("{}: registers {} not found", pname, rspec);
                }
                if single {
                    if registers.len() > 1 {
                        panic!("{}: more than one registers {} found", pname, rspec);
                    }
                } else {
                    registers.sort_by_key(|r| r.address_offset);
                    let bitmasks = registers
                        .iter()
                        .map(RegisterInfo::get_bitmask)
                        .collect::<Vec<_>>();
                    let new_dim_index = registers
                        .iter()
                        .map(|r| {
                            let match_rspec = matchsubspec(&r.name, rspec).unwrap();
                            let (li, ri) = spec_ind(match_rspec);
                            r.name[li..r.name.len() - ri].to_string()
                        })
                        .collect::<Vec<_>>();
                    if first {
                        dim = registers.len();
                        dim_index = new_dim_index;
                        dim_increment = 0;
                        offsets = registers
                            .iter()
                            .map(|r| r.address_offset)
                            .collect::<Vec<_>>();
                        if dim > 1 {
                            dim_increment = offsets[1] - offsets[0];
                        }
                        if !(check_offsets(&offsets, dim_increment)
                            && bitmasks.iter().all(|&m| m == bitmasks[0]))
                        {
                            check = false;
                            break;
                        }
                    } else {
                        if (dim != registers.len())
                            || (dim_index != new_dim_index)
                            || (!check_offsets(&offsets, dim_increment))
                            || (!bitmasks.iter().all(|&m| m == bitmasks[0]))
                        {
                            check = false;
                            break;
                        }
                    }
                }
                rdict.insert(rspec.to_string(), registers);
                first = false;
            }
            if !check {
                panic!(
                    "{}: registers cannot be collected into {} cluster",
                    self.name, cname
                );
            }
            let address_offset = rdict
                .values()
                .min_by_key(|rs| rs[0].address_offset)
                .unwrap()[0]
                .address_offset;
            let mut children = Vec::new();
            let cinfo = ClusterInfo::builder()
                .name(cname.into())
                .description(Some(if let Some(desc) = cmod.get_str("description") {
                    desc.into()
                } else {
                    format!("Cluster {}, containing {}", cname, rspecs.join(", "))
                }))
                .address_offset(address_offset);
            let cluster = if single {
                for (rspec, mut registers) in rdict.into_iter() {
                    let mut reg = registers.swap_remove(0).single();
                    let rmod = cmod.get_hash(rspec.as_str()).unwrap();
                    reg.process(rmod, &pname, true);
                    if let Some(name) = rmod.get_str("name") {
                        reg.name = name.into();
                    }
                    reg.address_offset = reg.address_offset - address_offset;
                    children.push(RegisterCluster::Register(reg));
                }

                cinfo.children(children).build(VAL_LVL).unwrap().single()
            } else {
                for (rspec, mut registers) in rdict.into_iter() {
                    let mut reg = registers.swap_remove(0).single();
                    let rmod = cmod.get_hash(rspec.as_str()).unwrap();
                    reg.process(rmod, &pname, true);
                    if let Some(name) = rmod.get_str("name") {
                        reg.name = name.into();
                    } else {
                        let (li, ri) = spec_ind(&rspec);
                        reg.name = format!("{}{}", &rspec[..li], &rspec[rspec.len() - ri..]);
                    }
                    reg.address_offset = reg.address_offset - address_offset;
                    children.push(RegisterCluster::Register(reg));
                }

                cinfo.children(children).build(VAL_LVL).unwrap().array(
                    DimElement::builder()
                        .dim(dim as u32)
                        .dim_increment(dim_increment)
                        .dim_index(Some(dim_index))
                        .build(VAL_LVL)
                        .unwrap(),
                )
            };
            regs.insert(place, RegisterCluster::Cluster(cluster));
        }
    }

    fn process_register(&mut self, rspec: &str, rmod: &Hash, update_fields: bool) {
        // Find all registers that match the spec
        let mut rcount = 0;
        let pname = self.name.clone();
        for rtag in self.iter_registers(rspec) {
            rcount += 1;
            rtag.process(rmod, &pname, update_fields);
        }
        if rcount == 0 {
            panic!("Could not find {}:{}", &pname, rspec);
        }
    }
}
