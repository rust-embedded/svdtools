use anyhow::{anyhow, Context};
use svd_parser::svd::{
    self, Cluster, ClusterInfo, DimElement, Interrupt, Peripheral, Register, RegisterCluster,
    RegisterInfo,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIterMut, Matched};
use super::register::{RegisterExt, RegisterInfoExt};
use super::yaml_ext::{AsType, GetVal, ToYaml};
use super::{check_offsets, matchname, matchsubspec, spec_ind, PatchResult, VAL_LVL};
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
    fn process(&mut self, peripheral: &Hash, update_fields: bool) -> PatchResult;

    /// Delete interrupts matched by ispec
    fn delete_interrupt(&mut self, ispec: &str) -> PatchResult;

    /// Add iname given by iadd to ptag
    fn add_interrupt(&mut self, iname: &str, iadd: &Hash) -> PatchResult;

    /// Modify ispec according to imod
    fn modify_interrupt(&mut self, ispec: &str, imod: &Hash) -> PatchResult;

    /// Delete registers matched by rspec inside ptag
    fn delete_register(&mut self, rspec: &str) -> PatchResult;

    /// Add rname given by radd to ptag
    fn add_register(&mut self, rname: &str, radd: &Hash) -> PatchResult;

    /// Add rname given by deriving from rsource to ptag
    fn copy_register(&mut self, rname: &str, rderive: &Hash) -> PatchResult;

    /// Modify rspec inside ptag according to rmod
    fn modify_register(&mut self, rspec: &str, rmod: &Hash) -> PatchResult;

    /// Modify cspec inside ptag according to cmod
    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash) -> PatchResult;
    /// Work through a register, handling all fields
    fn process_register(
        &mut self,
        rspec: &str,
        register: &Hash,
        update_fields: bool,
    ) -> PatchResult;

    /// Delete substring from the beginning of register names inside ptag
    fn strip_start(&mut self, prefix: &str) -> PatchResult;

    /// Delete substring from the ending of register names inside ptag
    fn strip_end(&mut self, suffix: &str) -> PatchResult;

    /// Collect same registers in peripheral into register array
    fn collect_in_array(&mut self, rspec: &str, rmod: &Hash) -> PatchResult;

    /// Collect registers in peripheral into clusters
    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) -> PatchResult;
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

    fn process(&mut self, pmod: &Hash, update_fields: bool) -> PatchResult {
        // For derived peripherals, only process interrupts
        if self.derived_from.is_some() {
            if let Some(deletions) = pmod.get_hash("_delete").ok().flatten() {
                for ispec in deletions.str_vec_iter("_interrupts") {
                    self.delete_interrupt(ispec)
                        .with_context(|| format!("Deleting interrupts matched to `{}`", ispec))?;
                }
            }
            for (rspec, rmod) in pmod
                .get_hash("_modify")
                .ok()
                .flatten()
                .unwrap_or(&Hash::new())
            {
                if rspec.as_str() == Some("_interrupts") {
                    for (ispec, val) in rmod.hash()? {
                        let ispec = ispec.str()?;
                        self.modify_interrupt(ispec, val.hash()?).with_context(|| {
                            format!("Modifying interrupts matched to `{}`", ispec)
                        })?;
                    }
                }
            }
            for (rname, radd) in pmod.get_hash("_add").ok().flatten().unwrap_or(&Hash::new()) {
                if rname.as_str() == Some("_interrupts") {
                    for (iname, val) in radd.hash()? {
                        let iname = iname.str()?;
                        self.add_interrupt(iname, val.hash()?)
                            .with_context(|| format!("Adding interrupt `{}`", iname))?;
                    }
                }
            }
            // Don't do any further processing on derived peripherals
            return Ok(());
        }

        // Handle deletions
        if let Some(deletions) = pmod.get(&"_delete".to_yaml()) {
            match deletions {
                Yaml::String(rspec) => {
                    self.delete_register(rspec)
                        .with_context(|| format!("Deleting registers matched to `{}`", rspec))?;
                }
                Yaml::Array(deletions) => {
                    for rspec in deletions {
                        let rspec = rspec.str()?;
                        self.delete_register(rspec).with_context(|| {
                            format!("Deleting registers matched to `{}`", rspec)
                        })?;
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers") {
                        self.delete_register(rspec).with_context(|| {
                            format!("Deleting registers matched to `{}`", rspec)
                        })?;
                    }
                    for ispec in deletions.str_vec_iter("_interrupts") {
                        self.delete_interrupt(ispec).with_context(|| {
                            format!("Deleting interrupts matched to `{}`", ispec)
                        })?;
                    }
                }
                _ => {}
            }
        }

        // Handle modifications
        for (rspec, rmod) in pmod.hash_iter("_modify") {
            let rmod = rmod.hash()?;
            match rspec.str()? {
                "_registers" => {
                    for (rspec, val) in rmod {
                        let rspec = rspec.str()?;
                        self.modify_register(rspec, val.hash()?).with_context(|| {
                            format!("Modifying registers matched to `{}`", rspec)
                        })?;
                    }
                }
                "_interrupts" => {
                    for (ispec, val) in rmod {
                        let ispec = ispec.str()?;
                        self.modify_interrupt(ispec, val.hash()?).with_context(|| {
                            format!("Modifying interrupts matched to `{}`", ispec)
                        })?;
                    }
                }
                "_cluster" => {
                    for (cspec, val) in rmod {
                        let cspec = cspec.str()?;
                        self.modify_cluster(cspec, val.hash()?).with_context(|| {
                            format!("Modifying clusters matched to `{}`", cspec)
                        })?;
                    }
                }
                rspec => self
                    .modify_register(rspec, rmod)
                    .with_context(|| format!("Modifying registers matched to `{}`", rspec))?,
            }
        }

        // Handle strips
        for prefix in pmod.str_vec_iter("_strip") {
            self.strip_start(prefix)
                .with_context(|| format!("Stripping prefix `{}` from register names", prefix))?;
        }
        for suffix in pmod.str_vec_iter("_strip_end") {
            self.strip_end(suffix)
                .with_context(|| format!("Stripping suffix `{}` from register names", suffix))?;
        }

        // Handle additions
        for (rname, radd) in pmod.hash_iter("_add") {
            let radd = radd.hash()?;
            match rname.str()? {
                "_registers" => {
                    for (rname, val) in radd {
                        let rname = rname.str()?;
                        self.add_register(rname, val.hash()?)
                            .with_context(|| format!("Adding register `{}`", rname))?;
                    }
                }
                "_interrupts" => {
                    for (iname, val) in radd {
                        let iname = iname.str()?;
                        self.add_interrupt(iname, val.hash()?)
                            .with_context(|| format!("Adding interrupt `{}`", iname))?;
                    }
                }
                rname => self
                    .add_register(rname, radd)
                    .with_context(|| format!("Adding register `{}`", rname))?,
            }
        }

        for (rname, rderive) in pmod.hash_iter("_derive") {
            let rderive = rderive.hash()?;
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rderive {
                        let rname = rname.str()?;
                        self.copy_register(rname, val.hash()?)
                            .with_context(|| format!("Copying register `{}`", rname))?;
                    }
                }
                "_interrupts" => {
                    return Err(anyhow!(
                        "deriving interrupts not implemented yet: {}",
                        rname
                    ))
                }
                _ => self
                    .copy_register(rname, rderive)
                    .with_context(|| format!("Copying register `{}`", rname))?,
            }
        }

        // Handle registers
        for (rspec, register) in pmod {
            let rspec = rspec.str()?;
            if !rspec.starts_with("_") {
                self.process_register(rspec, register.hash()?, update_fields)
                    .with_context(|| format!("According to `{}`", rspec))?;
            }
        }

        // Handle register arrays
        for (rspec, rmod) in pmod.hash_iter("_array") {
            let rspec = rspec.str()?;
            self.collect_in_array(rspec, rmod.hash()?)
                .with_context(|| format!("Collecting registers matched to `{}` in array", rspec))?;
        }

        // Handle clusters
        for (cname, cmod) in pmod.hash_iter("_cluster") {
            let cname = cname.str()?;
            self.collect_in_cluster(cname, cmod.hash()?)
                .with_context(|| format!("Collecting registers in cluster `{}`", cname))?;
        }

        Ok(())
    }

    fn iter_clusters<'a, 'b>(&mut self, _spec: &str) -> ClusterMatchIterMut<'a, 'b> {
        todo!()
    }

    fn add_interrupt(&mut self, iname: &str, iadd: &Hash) -> PatchResult {
        assert!(
            self.interrupt.iter().find(|i| &i.name == iname).is_none(),
            "peripheral {} already has an interrupt {}",
            self.name,
            iname
        );
        self.interrupt
            .push(make_interrupt(iadd)?.name(iname.into()).build(VAL_LVL)?);
        Ok(())
    }

    fn modify_interrupt(&mut self, ispec: &str, imod: &Hash) -> PatchResult {
        for itag in self.iter_interrupts(ispec) {
            itag.modify_from(make_interrupt(imod)?, VAL_LVL)?;
        }
        Ok(())
    }

    fn delete_interrupt(&mut self, ispec: &str) -> PatchResult {
        self.interrupt.retain(|i| !(matchname(&i.name, ispec)));
        Ok(())
    }

    fn modify_register(&mut self, rspec: &str, rmod: &Hash) -> PatchResult {
        for rtag in self.iter_registers(rspec) {
            rtag.modify_from(make_register(rmod)?, VAL_LVL)?;
        }
        Ok(())
    }

    fn add_register(&mut self, rname: &str, radd: &Hash) -> PatchResult {
        if self.reg_iter().find(|r| r.name == rname).is_some() {
            return Err(anyhow!(
                "peripheral {} already has a register {}",
                self.name,
                rname
            ));
        }
        self.registers
            .get_or_insert_with(Default::default)
            .push(RegisterCluster::Register(
                make_register(radd)?
                    .name(rname.into())
                    .build(VAL_LVL)?
                    .single(),
            ));
        Ok(())
    }

    fn copy_register(&mut self, rname: &str, rderive: &Hash) -> PatchResult {
        let srcname = rderive.get_str("_from")?.ok_or_else(|| {
            anyhow!(
                "derive: source register not given, please add a _from field to {}",
                rname
            )
        })?;

        let mut source = self
            .reg_iter()
            .find(|p| &p.name == srcname)
            .ok_or_else(|| {
                anyhow!(
                    "peripheral {} does not have register {}",
                    self.name,
                    srcname
                )
            })?
            .clone();
        let fixes = make_register(rderive)?
            .name(rname.into())
            .display_name(Some("".into()));
        // Modifying fields in derived register not implemented
        source.modify_from(fixes, VAL_LVL)?;
        if let Some(ptag) = self.reg_iter_mut().find(|r| &r.name == rname) {
            source.address_offset = ptag.address_offset;
            *ptag = source;
        } else {
            self.registers
                .as_mut()
                .unwrap()
                .push(RegisterCluster::Register(source))
        }
        Ok(())
    }

    fn delete_register(&mut self, rspec: &str) -> PatchResult {
        // TODO: delete registers in clusters
        if let Some(registers) = &mut self.registers {
            registers.retain(
                |r| !matches!(r, RegisterCluster::Register(r) if matchname(&r.name, rspec)),
            );
        }
        Ok(())
    }

    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash) -> PatchResult {
        for ctag in self.iter_clusters(cspec) {
            ctag.modify_from(make_cluster(cmod)?, VAL_LVL)?;
        }
        Ok(())
    }

    fn strip_start(&mut self, prefix: &str) -> PatchResult {
        let len = prefix.len();
        let glob = globset::Glob::new(&(prefix.to_string() + "*"))?.compile_matcher();
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
        Ok(())
    }

    fn strip_end(&mut self, suffix: &str) -> PatchResult {
        let len = suffix.len();
        let glob = globset::Glob::new(&("*".to_string() + suffix))
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
        Ok(())
    }

    fn collect_in_array(&mut self, rspec: &str, rmod: &Hash) -> PatchResult {
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
                return Err(anyhow!("{}: registers {} not found", pname, rspec));
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
                return Err(anyhow!(
                    "{}: registers cannot be collected into {} array",
                    self.name,
                    rspec
                ));
            }
            let mut rinfo = registers.swap_remove(0);
            if let Some(name) = rmod.get_str("name")? {
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
                    .build(VAL_LVL)?,
            );
            reg.process(rmod, &pname, true)
                .with_context(|| format!("Processing register `{}`", reg.name))?;
            regs.insert(place, RegisterCluster::Register(reg));
        }
        Ok(())
    }

    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) -> PatchResult {
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
                let rspec = rspec.str()?;
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
                    return Err(anyhow!("{}: registers {} not found", pname, rspec));
                }
                if single {
                    if registers.len() > 1 {
                        return Err(anyhow!(
                            "{}: more than one registers {} found",
                            pname,
                            rspec
                        ));
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
                return Err(anyhow!(
                    "{}: registers cannot be collected into {} cluster",
                    self.name,
                    cname
                ));
            }
            let address_offset = rdict
                .values()
                .min_by_key(|rs| rs[0].address_offset)
                .unwrap()[0]
                .address_offset;
            let mut children = Vec::new();
            let cinfo = ClusterInfo::builder()
                .name(cname.into())
                .description(Some(if let Some(desc) = cmod.get_str("description")? {
                    desc.into()
                } else {
                    format!("Cluster {}, containing {}", cname, rspecs.join(", "))
                }))
                .address_offset(address_offset);
            let cluster = if single {
                for (rspec, mut registers) in rdict.into_iter() {
                    let mut reg = registers.swap_remove(0).single();
                    let rmod = cmod.get_hash(rspec.as_str())?.unwrap();
                    reg.process(rmod, &pname, true)
                        .with_context(|| format!("Processing register `{}`", reg.name))?;
                    if let Some(name) = rmod.get_str("name")? {
                        reg.name = name.into();
                    }
                    reg.address_offset = reg.address_offset - address_offset;
                    children.push(RegisterCluster::Register(reg));
                }

                cinfo.children(children).build(VAL_LVL)?.single()
            } else {
                for (rspec, mut registers) in rdict.into_iter() {
                    let mut reg = registers.swap_remove(0).single();
                    let rmod = cmod.get_hash(rspec.as_str())?.unwrap();
                    reg.process(rmod, &pname, true)
                        .with_context(|| format!("Processing register `{}`", reg.name))?;
                    if let Some(name) = rmod.get_str("name")? {
                        reg.name = name.into();
                    } else {
                        let (li, ri) = spec_ind(&rspec);
                        reg.name = format!("{}{}", &rspec[..li], &rspec[rspec.len() - ri..]);
                    }
                    reg.address_offset = reg.address_offset - address_offset;
                    children.push(RegisterCluster::Register(reg));
                }

                cinfo.children(children).build(VAL_LVL)?.array(
                    DimElement::builder()
                        .dim(dim as u32)
                        .dim_increment(dim_increment)
                        .dim_index(Some(dim_index))
                        .build(VAL_LVL)?,
                )
            };
            regs.insert(place, RegisterCluster::Cluster(cluster));
        }
        Ok(())
    }

    fn process_register(&mut self, rspec: &str, rmod: &Hash, update_fields: bool) -> PatchResult {
        // Find all registers that match the spec
        let mut rcount = 0;
        let pname = self.name.clone();
        for rtag in self.iter_registers(rspec) {
            rcount += 1;
            rtag.process(rmod, &pname, update_fields)
                .with_context(|| format!("Processing register `{}`", rtag.name))?;
        }
        if rcount == 0 {
            Err(anyhow!("Could not find `{}:{}`", &pname, rspec))
        } else {
            Ok(())
        }
    }
}
