use anyhow::{anyhow, Context};
use svd_parser::svd::{
    self, Cluster, ClusterInfo, DimElement, Interrupt, Peripheral, Register, RegisterCluster,
    RegisterInfo,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIter, Matched};
use super::register::{RegisterExt, RegisterInfoExt};
use super::yaml_ext::{AsType, GetVal, ToYaml};
use super::{
    check_offsets, make_dim_element, matchname, matchsubspec, modify_dim_element, spec_ind,
    PatchResult, VAL_LVL,
};
use super::{make_cluster, make_interrupt, make_register};

use svd::registercluster::{ClusterIterMut, RegisterIterMut};
pub type ClusterMatchIterMut<'a, 'b> = MatchIter<'b, ClusterIterMut<'a>>;
pub type RegMatchIterMut<'a, 'b> = MatchIter<'b, RegisterIterMut<'a>>;

/// Collecting methods for processing peripheral contents
pub trait PeripheralExt: InterruptExt + RegisterBlockExt {
    /// Work through a peripheral, handling all registers
    fn process(&mut self, peripheral: &Hash, update_fields: bool) -> PatchResult;
}

/// Collecting methods for processing cluster contents
pub trait ClusterExt: RegisterBlockExt {
    /// Work through a cluster, handling all registers
    fn process(&mut self, peripheral: &Hash, pname: &str, update_fields: bool) -> PatchResult;
}

/// Collecting methods for processing peripheral interrupt contents
pub trait InterruptExt {
    /// Iterates over all interrupts matching ispec
    fn iter_interrupts<'a, 'b>(
        &'a mut self,
        spec: &'b str,
    ) -> MatchIter<'b, std::slice::IterMut<'a, Interrupt>>;

    /// Delete interrupts matched by ispec
    fn delete_interrupt(&mut self, ispec: &str) -> PatchResult;

    /// Add iname given by iadd to ptag
    fn add_interrupt(&mut self, iname: &str, iadd: &Hash) -> PatchResult;

    /// Modify ispec according to imod
    fn modify_interrupt(&mut self, ispec: &str, imod: &Hash) -> PatchResult;
}

/// Collecting methods for processing peripheral/cluster contents
pub trait RegisterBlockExt {
    /// Iterates over all registers that match rspec and live inside ptag
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b>;

    /// Iterate over all clusters that match cpsec and live inside ptag
    fn iter_clusters<'a, 'b>(&'a mut self, spec: &'b str) -> ClusterMatchIterMut<'a, 'b>;

    /// Delete registers matched by rspec inside ptag
    fn delete_register(&mut self, rspec: &str) -> PatchResult;

    /// Delete clusters matched by cspec inside ptag
    fn delete_cluster(&mut self, cspec: &str) -> PatchResult;

    /// Add rname given by radd to ptag
    fn add_register(&mut self, rname: &str, radd: &Hash) -> PatchResult;

    /// Add cname given by cadd to ptag
    fn add_cluster(&mut self, cname: &str, cadd: &Hash) -> PatchResult;

    /// Remove fields from rname and mark it as derivedFrom rderive.
    /// Update all derivedFrom referencing rname
    fn derive_register(&mut self, rname: &str, rderive: &Yaml) -> PatchResult;

    /// Remove fields from rname and mark it as derivedFrom rderive.
    /// Update all derivedFrom referencing rname
    fn derive_cluster(&mut self, cname: &str, cderive: &Yaml) -> PatchResult;

    /// Add rname given by deriving from rcopy to ptag
    fn copy_register(&mut self, rname: &str, rcopy: &Hash) -> PatchResult;

    /// Add cname given by deriving from ccopy to ptag
    fn copy_cluster(&mut self, rname: &str, ccopy: &Hash) -> PatchResult;

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

    /// Work through a cluster, handling all contents
    fn process_cluster(&mut self, cspec: &str, cluster: &Hash, update_fields: bool) -> PatchResult;

    /// Delete substring from the beginning of register names inside ptag
    fn strip_start(&mut self, prefix: &str) -> PatchResult;

    /// Delete substring from the ending of register names inside ptag
    fn strip_end(&mut self, suffix: &str) -> PatchResult;

    /// Collect same registers in peripheral into register array
    fn collect_in_array(&mut self, rspec: &str, rmod: &Hash) -> PatchResult;

    /// Collect registers in peripheral into clusters
    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) -> PatchResult;

    /// Clear contents of all fields inside registers matched by rspec
    fn clear_fields(&mut self, rspec: &str) -> PatchResult;
}

impl PeripheralExt for Peripheral {
    fn process(&mut self, pmod: &Hash, update_fields: bool) -> PatchResult {
        // For derived peripherals, only process interrupts
        if self.derived_from.is_some() {
            if let Some(deletions) = pmod.get_hash("_delete").ok().flatten() {
                for ispec in deletions.str_vec_iter("_interrupts")? {
                    self.delete_interrupt(ispec)
                        .with_context(|| format!("Deleting interrupts matched to `{ispec}`"))?;
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
                            format!("Modifying interrupts matched to `{ispec}`")
                        })?;
                    }
                }
            }
            for (rname, radd) in pmod.get_hash("_add").ok().flatten().unwrap_or(&Hash::new()) {
                if rname.as_str() == Some("_interrupts") {
                    for (iname, val) in radd.hash()? {
                        let iname = iname.str()?;
                        self.add_interrupt(iname, val.hash()?)
                            .with_context(|| format!("Adding interrupt `{iname}`"))?;
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
                        .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                }
                Yaml::Array(deletions) => {
                    for rspec in deletions {
                        let rspec = rspec.str()?;
                        self.delete_register(rspec)
                            .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers")? {
                        self.delete_register(rspec)
                            .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                    }
                    for cspec in deletions.str_vec_iter("_clusters")? {
                        self.delete_cluster(cspec)
                            .with_context(|| format!("Deleting clusters matched to `{cspec}`"))?;
                    }
                    for ispec in deletions.str_vec_iter("_interrupts")? {
                        self.delete_interrupt(ispec)
                            .with_context(|| format!("Deleting interrupts matched to `{ispec}`"))?;
                    }
                    for d in deletions.keys() {
                        if !matches!(d, Yaml::String(s) if s == "_registers" ||  s == "_clusters" || s == "_interrupts")
                        {
                            return Err(anyhow!(
                                "`_delete` requires string value or array of strings"
                            ));
                        }
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "`_delete` requires string value or array of strings"
                    ))
                }
            }
        }

        // Handle any copied peripherals
        for (rname, rcopy) in pmod.hash_iter("_copy") {
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rcopy.hash()? {
                        let rname = rname.str()?;
                        let rcopy = val.hash()?;
                        self.copy_register(rname, rcopy).with_context(|| {
                            format!("Copying register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rcopy.hash()? {
                        let cname = cname.str()?;
                        let ccopy = val.hash()?;
                        self.copy_cluster(rname, ccopy)
                            .with_context(|| format!("Copying cluster `{cname}` from `{val:?}`"))?;
                    }
                }
                _ => {
                    let rcopy = rcopy.hash()?;
                    self.copy_register(rname, rcopy)
                        .with_context(|| format!("Copying register `{rname}` from `{rcopy:?}`"))?;
                }
            }
        }

        // Handle strips
        for prefix in pmod.str_vec_iter("_strip")? {
            self.strip_start(prefix)
                .with_context(|| format!("Stripping prefix `{prefix}` from register names"))?;
        }
        for suffix in pmod.str_vec_iter("_strip_end")? {
            self.strip_end(suffix)
                .with_context(|| format!("Stripping suffix `{suffix}` from register names"))?;
        }

        // Handle modifications
        for (rspec, rmod) in pmod.hash_iter("_modify") {
            let rmod = rmod.hash()?;
            match rspec.str()? {
                "_registers" => {
                    for (rspec, val) in rmod {
                        let rspec = rspec.str()?;
                        self.modify_register(rspec, val.hash()?)
                            .with_context(|| format!("Modifying registers matched to `{rspec}`"))?;
                    }
                }
                "_interrupts" => {
                    for (ispec, val) in rmod {
                        let ispec = ispec.str()?;
                        self.modify_interrupt(ispec, val.hash()?).with_context(|| {
                            format!("Modifying interrupts matched to `{ispec}`")
                        })?;
                    }
                }
                "_cluster" => {
                    for (cspec, val) in rmod {
                        let cspec = cspec.str()?;
                        self.modify_cluster(cspec, val.hash()?)
                            .with_context(|| format!("Modifying clusters matched to `{cspec}`"))?;
                    }
                }
                rspec => self
                    .modify_register(rspec, rmod)
                    .with_context(|| format!("Modifying registers matched to `{rspec}`"))?,
            }
        }

        // Handle field clearing
        for rspec in pmod.str_vec_iter("_clear_fields")? {
            self.clear_fields(rspec).with_context(|| {
                format!("Clearing contents of fields in registers matched to `{rspec}` ")
            })?;
        }

        // Handle additions
        for (rname, radd) in pmod.hash_iter("_add") {
            let radd = radd.hash()?;
            match rname.str()? {
                "_registers" => {
                    for (rname, val) in radd {
                        let rname = rname.str()?;
                        self.add_register(rname, val.hash()?)
                            .with_context(|| format!("Adding register `{rname}`"))?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in radd {
                        let cname = cname.str()?;
                        self.add_cluster(cname, val.hash()?)
                            .with_context(|| format!("Adding cluster `{cname}`"))?;
                    }
                }
                "_interrupts" => {
                    for (iname, val) in radd {
                        let iname = iname.str()?;
                        self.add_interrupt(iname, val.hash()?)
                            .with_context(|| format!("Adding interrupt `{iname}`"))?;
                    }
                }
                rname => self
                    .add_register(rname, radd)
                    .with_context(|| format!("Adding register `{rname}`"))?,
            }
        }

        for (rname, rderive) in pmod.hash_iter("_derive") {
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rderive.hash()? {
                        let rname = rname.str()?;
                        self.derive_register(rname, val).with_context(|| {
                            format!("Deriving register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rderive.hash()? {
                        let cname = cname.str()?;
                        self.derive_cluster(rname, val).with_context(|| {
                            format!("Deriving cluster `{cname}` from `{val:?}`")
                        })?;
                    }
                }
                _ => {
                    self.derive_register(rname, rderive).with_context(|| {
                        format!("Deriving register `{rname}` from `{rderive:?}`")
                    })?;
                }
            }
        }

        // Handle clusters
        for (cspec, cluster) in pmod.hash_iter("_clusters") {
            let cspec = cspec.str()?;
            if !cspec.starts_with('_') {
                self.process_cluster(cspec, cluster.hash()?, update_fields)
                    .with_context(|| format!("According to `{cspec}`"))?;
            }
        }

        // Handle registers
        for (rspec, register) in pmod {
            let rspec = rspec.str()?;
            if !rspec.starts_with('_') {
                self.process_register(rspec, register.hash()?, update_fields)
                    .with_context(|| format!("According to `{rspec}`"))?;
            }
        }

        // Collect registers in arrays
        for (rspec, rmod) in pmod.hash_iter("_array") {
            let rspec = rspec.str()?;
            self.collect_in_array(rspec, rmod.hash()?)
                .with_context(|| format!("Collecting registers matched to `{rspec}` in array"))?;
        }

        // Collect registers in clusters
        for (cname, cmod) in pmod.hash_iter("_cluster") {
            let cname = cname.str()?;
            self.collect_in_cluster(cname, cmod.hash()?)
                .with_context(|| format!("Collecting registers in cluster `{cname}`"))?;
        }

        Ok(())
    }
}

impl InterruptExt for Peripheral {
    fn iter_interrupts<'a, 'b>(
        &'a mut self,
        spec: &'b str,
    ) -> MatchIter<'b, std::slice::IterMut<'a, Interrupt>> {
        self.interrupt.iter_mut().matched(spec)
    }

    fn add_interrupt(&mut self, iname: &str, iadd: &Hash) -> PatchResult {
        if self.get_interrupt(iname).is_some() {
            return Err(anyhow!(
                "peripheral {} already has an interrupt {iname}",
                self.name
            ));
        }
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
}

impl RegisterBlockExt for Peripheral {
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b> {
        self.registers_mut().matched(spec)
    }

    fn iter_clusters<'a, 'b>(&'a mut self, spec: &'b str) -> ClusterMatchIterMut<'a, 'b> {
        self.clusters_mut().matched(spec)
    }

    fn modify_register(&mut self, rspec: &str, rmod: &Hash) -> PatchResult {
        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        if !rtags.is_empty() {
            let register_builder = make_register(rmod)?;
            let dim = make_dim_element(rmod)?;
            for rtag in rtags {
                modify_dim_element(rtag, &dim)?;
                rtag.modify_from(register_builder.clone(), VAL_LVL)?;
                if let Some("") = rmod.get_str("access")? {
                    rtag.properties.access = None;
                }
            }
        }
        Ok(())
    }

    fn add_register(&mut self, rname: &str, radd: &Hash) -> PatchResult {
        if self.registers().any(|r| r.name == rname) {
            return Err(anyhow!(
                "peripheral {} already has a register {rname}",
                self.name
            ));
        }
        self.registers
            .get_or_insert_with(Default::default)
            .push(RegisterCluster::Register({
                let reg = make_register(radd)?.name(rname.into()).build(VAL_LVL)?;
                if let Some(dim) = make_dim_element(radd)? {
                    reg.array(dim.build(VAL_LVL)?)
                } else {
                    reg.single()
                }
            }));
        Ok(())
    }

    fn add_cluster(&mut self, cname: &str, cadd: &Hash) -> PatchResult {
        if self.clusters().any(|c| c.name == cname) {
            return Err(anyhow!(
                "peripheral {} already has a cluster {cname}",
                self.name
            ));
        }
        self.registers
            .get_or_insert_with(Default::default)
            .push(RegisterCluster::Cluster({
                let cl = make_cluster(cadd)?.name(cname.into()).build(VAL_LVL)?;
                if let Some(dim) = make_dim_element(cadd)? {
                    cl.array(dim.build(VAL_LVL)?)
                } else {
                    cl.single()
                }
            }));
        Ok(())
    }

    fn derive_register(&mut self, rname: &str, rderive: &Yaml) -> PatchResult {
        let (rderive, info) = if let Some(rderive) = rderive.as_str() {
            (
                rderive,
                RegisterInfo::builder().derived_from(Some(rderive.into())),
            )
        } else if let Some(hash) = rderive.as_hash() {
            let rderive = hash.get_str("_from")?.ok_or_else(|| {
                anyhow!("derive: source register not given, please add a _from field to {rname}")
            })?;
            (
                rderive,
                make_register(hash)?.derived_from(Some(rderive.into())),
            )
        } else {
            return Err(anyhow!("derive: incorrect syntax for {rname}"));
        };

        self.get_register(rderive)
            .ok_or_else(|| anyhow!("register {rderive} not found"))?;

        match self.get_mut_register(rname) {
            Some(register) => register.modify_from(info, VAL_LVL)?,
            None => {
                let register = info.name(rname.into()).build(VAL_LVL)?.single();
                self.registers
                    .get_or_insert_with(Default::default)
                    .push(RegisterCluster::Register(register));
            }
        }
        for r in self
            .registers_mut()
            .filter(|r| r.derived_from.as_deref() == Some(rname))
        {
            r.derived_from = Some(rderive.into());
        }
        Ok(())
    }

    fn derive_cluster(&mut self, _cname: &str, _cderive: &Yaml) -> PatchResult {
        todo!()
    }

    fn copy_register(&mut self, rname: &str, rcopy: &Hash) -> PatchResult {
        let srcname = rcopy.get_str("_from")?.ok_or_else(|| {
            anyhow!("derive: source register not given, please add a _from field to {rname}")
        })?;

        let mut source = self
            .registers()
            .find(|r| r.name == srcname)
            .ok_or_else(|| anyhow!("peripheral {} does not have register {srcname}", self.name))?
            .clone();
        let fixes = make_register(rcopy)?
            .name(rname.into())
            .display_name(Some("".into()));
        // Modifying fields in derived register not implemented
        source.modify_from(fixes, VAL_LVL)?;
        if let Some(ptag) = self.registers_mut().find(|r| r.name == rname) {
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

    fn copy_cluster(&mut self, _rname: &str, _ccopy: &Hash) -> PatchResult {
        todo!()
    }

    fn delete_register(&mut self, rspec: &str) -> PatchResult {
        if let Some(registers) = &mut self.registers {
            registers.retain(
                |rc| !matches!(rc, RegisterCluster::Register(r) if matchname(&r.name, rspec)),
            );
        }
        Ok(())
    }

    fn delete_cluster(&mut self, cspec: &str) -> PatchResult {
        if let Some(registers) = &mut self.registers {
            registers.retain(
                |rc| !matches!(rc, RegisterCluster::Cluster(c) if matchname(&c.name, cspec)),
            );
        }
        Ok(())
    }

    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash) -> PatchResult {
        let ctags = self.iter_clusters(cspec).collect::<Vec<_>>();
        if !ctags.is_empty() {
            let cluster_builder = make_cluster(cmod)?;
            let dim = make_dim_element(cmod)?;
            for ctag in ctags {
                modify_dim_element(ctag, &dim)?;
                ctag.modify_from(cluster_builder.clone(), VAL_LVL)?;
            }
        }
        Ok(())
    }

    fn strip_start(&mut self, prefix: &str) -> PatchResult {
        let len = prefix.len();
        let glob = globset::Glob::new(&(prefix.to_string() + "*"))?.compile_matcher();
        for rtag in self.registers_mut() {
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
        for rtag in self.registers_mut() {
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
            collect_in_array(regs, &pname, rspec, rmod)?;
        }
        Ok(())
    }

    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) -> PatchResult {
        let pname = self.name.clone();
        if let Some(regs) = self.registers.as_mut() {
            collect_in_cluster(regs, &pname, cname, cmod)?;
        }
        Ok(())
    }

    fn clear_fields(&mut self, rspec: &str) -> PatchResult {
        for rtag in self.iter_registers(rspec) {
            if rtag.derived_from.is_some() {
                continue;
            }
            rtag.clear_field("*")?;
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
            Err(anyhow!("Could not find `{pname}:{rspec}`"))
        } else {
            Ok(())
        }
    }

    fn process_cluster(&mut self, cspec: &str, cmod: &Hash, update_fields: bool) -> PatchResult {
        // Find all clusters that match the spec
        let mut ccount = 0;
        let pname = self.name.clone();
        for ctag in self.iter_clusters(cspec) {
            ccount += 1;
            ctag.process(cmod, &pname, update_fields)
                .with_context(|| format!("Processing cluster `{}`", ctag.name))?;
        }
        if ccount == 0 {
            Err(anyhow!("Could not find `{pname}:{cspec}`"))
        } else {
            Ok(())
        }
    }
}

impl ClusterExt for Cluster {
    fn process(&mut self, pmod: &Hash, _pname: &str, update_fields: bool) -> PatchResult {
        // Handle deletions
        if let Some(deletions) = pmod.get(&"_delete".to_yaml()) {
            match deletions {
                Yaml::String(rspec) => {
                    self.delete_register(rspec)
                        .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                }
                Yaml::Array(deletions) => {
                    for rspec in deletions {
                        let rspec = rspec.str()?;
                        self.delete_register(rspec)
                            .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers")? {
                        self.delete_register(rspec)
                            .with_context(|| format!("Deleting registers matched to `{rspec}`"))?;
                    }
                    for cspec in deletions.str_vec_iter("_clusters")? {
                        self.delete_cluster(cspec)
                            .with_context(|| format!("Deleting clusters matched to `{cspec}`"))?;
                    }
                    for d in deletions.keys() {
                        if !matches!(d, Yaml::String(s) if s == "_registers" ||  s == "_clusters" || s == "_interrupts")
                        {
                            return Err(anyhow!(
                                "`_delete` requires string value or array of strings"
                            ));
                        }
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "`_delete` requires string value or array of strings"
                    ))
                }
            }
        }

        // Handle any copied peripherals
        for (rname, rcopy) in pmod.hash_iter("_copy") {
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rcopy.hash()? {
                        let rname = rname.str()?;
                        let rcopy = val.hash()?;
                        self.copy_register(rname, rcopy).with_context(|| {
                            format!("Copying register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rcopy.hash()? {
                        let cname = cname.str()?;
                        let ccopy = val.hash()?;
                        self.copy_cluster(rname, ccopy)
                            .with_context(|| format!("Copying cluster `{cname}` from `{val:?}`"))?;
                    }
                }
                _ => {
                    let rcopy = rcopy.hash()?;
                    self.copy_register(rname, rcopy)
                        .with_context(|| format!("Copying register `{rname}` from `{rcopy:?}`"))?;
                }
            }
        }

        // Handle strips
        for prefix in pmod.str_vec_iter("_strip")? {
            self.strip_start(prefix)
                .with_context(|| format!("Stripping prefix `{prefix}` from register names"))?;
        }
        for suffix in pmod.str_vec_iter("_strip_end")? {
            self.strip_end(suffix)
                .with_context(|| format!("Stripping suffix `{suffix}` from register names"))?;
        }

        // Handle modifications
        for (rspec, rmod) in pmod.hash_iter("_modify") {
            let rmod = rmod.hash()?;
            match rspec.str()? {
                "_registers" => {
                    for (rspec, val) in rmod {
                        let rspec = rspec.str()?;
                        self.modify_register(rspec, val.hash()?)
                            .with_context(|| format!("Modifying registers matched to `{rspec}`"))?;
                    }
                }
                "_cluster" => {
                    for (cspec, val) in rmod {
                        let cspec = cspec.str()?;
                        self.modify_cluster(cspec, val.hash()?)
                            .with_context(|| format!("Modifying clusters matched to `{cspec}`"))?;
                    }
                }
                rspec => self
                    .modify_register(rspec, rmod)
                    .with_context(|| format!("Modifying registers matched to `{rspec}`"))?,
            }
        }

        // Handle field clearing
        for rspec in pmod.str_vec_iter("_clear_fields")? {
            self.clear_fields(rspec).with_context(|| {
                format!("Clearing contents of fields in registers matched to `{rspec}` ")
            })?;
        }

        // Handle additions
        for (rname, radd) in pmod.hash_iter("_add") {
            let radd = radd.hash()?;
            match rname.str()? {
                "_registers" => {
                    for (rname, val) in radd {
                        let rname = rname.str()?;
                        self.add_register(rname, val.hash()?)
                            .with_context(|| format!("Adding register `{rname}`"))?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in radd {
                        let cname = cname.str()?;
                        self.add_cluster(cname, val.hash()?)
                            .with_context(|| format!("Adding cluster `{cname}`"))?;
                    }
                }
                rname => self
                    .add_register(rname, radd)
                    .with_context(|| format!("Adding register `{rname}`"))?,
            }
        }

        for (rname, rderive) in pmod.hash_iter("_derive") {
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rderive.hash()? {
                        let rname = rname.str()?;
                        self.derive_register(rname, val).with_context(|| {
                            format!("Deriving register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rderive.hash()? {
                        let cname = cname.str()?;
                        self.derive_cluster(rname, val).with_context(|| {
                            format!("Deriving cluster `{cname}` from `{val:?}`")
                        })?;
                    }
                }
                _ => {
                    self.derive_register(rname, rderive).with_context(|| {
                        format!("Deriving register `{rname}` from `{rderive:?}`")
                    })?;
                }
            }
        }

        // Handle clusters
        for (cspec, cluster) in pmod.hash_iter("_clusters") {
            let cspec = cspec.str()?;
            if !cspec.starts_with('_') {
                self.process_cluster(cspec, cluster.hash()?, update_fields)
                    .with_context(|| format!("According to `{cspec}`"))?;
            }
        }

        // Handle registers
        for (rspec, register) in pmod {
            let rspec = rspec.str()?;
            if !rspec.starts_with('_') {
                self.process_register(rspec, register.hash()?, update_fields)
                    .with_context(|| format!("According to `{rspec}`"))?;
            }
        }

        // Collect registers in arrays
        for (rspec, rmod) in pmod.hash_iter("_array") {
            let rspec = rspec.str()?;
            self.collect_in_array(rspec, rmod.hash()?)
                .with_context(|| format!("Collecting registers matched to `{rspec}` in array"))?;
        }

        // Collect registers in clusters
        for (cname, cmod) in pmod.hash_iter("_cluster") {
            let cname = cname.str()?;
            self.collect_in_cluster(cname, cmod.hash()?)
                .with_context(|| format!("Collecting registers in cluster `{cname}`"))?;
        }

        Ok(())
    }
}

impl RegisterBlockExt for Cluster {
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b> {
        self.registers_mut().matched(spec)
    }

    fn iter_clusters<'a, 'b>(&'a mut self, spec: &'b str) -> ClusterMatchIterMut<'a, 'b> {
        self.clusters_mut().matched(spec)
    }

    fn modify_register(&mut self, rspec: &str, rmod: &Hash) -> PatchResult {
        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        if !rtags.is_empty() {
            let register_builder = make_register(rmod)?;
            let dim = make_dim_element(rmod)?;
            for rtag in rtags {
                modify_dim_element(rtag, &dim)?;
                rtag.modify_from(register_builder.clone(), VAL_LVL)?;
                if let Some("") = rmod.get_str("access")? {
                    rtag.properties.access = None;
                }
            }
        }
        Ok(())
    }

    fn add_register(&mut self, rname: &str, radd: &Hash) -> PatchResult {
        if self.registers().any(|r| r.name == rname) {
            return Err(anyhow!(
                "peripheral {} already has a register {rname}",
                self.name
            ));
        }
        self.children.push(RegisterCluster::Register({
            let reg = make_register(radd)?.name(rname.into()).build(VAL_LVL)?;
            if let Some(dim) = make_dim_element(radd)? {
                reg.array(dim.build(VAL_LVL)?)
            } else {
                reg.single()
            }
        }));
        Ok(())
    }

    fn add_cluster(&mut self, cname: &str, cadd: &Hash) -> PatchResult {
        if self.clusters().any(|c| c.name == cname) {
            return Err(anyhow!(
                "peripheral {} already has a register {cname}",
                self.name
            ));
        }
        self.children.push(RegisterCluster::Cluster({
            let cl = make_cluster(cadd)?.name(cname.into()).build(VAL_LVL)?;
            if let Some(dim) = make_dim_element(cadd)? {
                cl.array(dim.build(VAL_LVL)?)
            } else {
                cl.single()
            }
        }));
        Ok(())
    }

    fn derive_register(&mut self, rname: &str, rderive: &Yaml) -> PatchResult {
        let (rderive, info) = if let Some(rderive) = rderive.as_str() {
            (
                rderive,
                RegisterInfo::builder().derived_from(Some(rderive.into())),
            )
        } else if let Some(hash) = rderive.as_hash() {
            let rderive = hash.get_str("_from")?.ok_or_else(|| {
                anyhow!("derive: source register not given, please add a _from field to {rname}")
            })?;
            (
                rderive,
                make_register(hash)?.derived_from(Some(rderive.into())),
            )
        } else {
            return Err(anyhow!("derive: incorrect syntax for {rname}"));
        };

        self.get_register(rderive)
            .ok_or_else(|| anyhow!("register {rderive} not found"))?;

        match self.get_mut_register(rname) {
            Some(register) => register.modify_from(info, VAL_LVL)?,
            None => {
                let register = info.name(rname.into()).build(VAL_LVL)?.single();
                self.children.push(RegisterCluster::Register(register));
            }
        }
        for r in self
            .registers_mut()
            .filter(|r| r.derived_from.as_deref() == Some(rname))
        {
            r.derived_from = Some(rderive.into());
        }
        Ok(())
    }

    fn derive_cluster(&mut self, _cname: &str, _cderive: &Yaml) -> PatchResult {
        todo!()
    }

    fn copy_register(&mut self, rname: &str, rcopy: &Hash) -> PatchResult {
        let srcname = rcopy.get_str("_from")?.ok_or_else(|| {
            anyhow!("derive: source register not given, please add a _from field to {rname}")
        })?;

        let mut source = self
            .registers()
            .find(|r| r.name == srcname)
            .ok_or_else(|| anyhow!("peripheral {} does not have register {srcname}", self.name,))?
            .clone();
        let fixes = make_register(rcopy)?
            .name(rname.into())
            .display_name(Some("".into()));
        // Modifying fields in derived register not implemented
        source.modify_from(fixes, VAL_LVL)?;
        if let Some(ptag) = self.registers_mut().find(|r| r.name == rname) {
            source.address_offset = ptag.address_offset;
            *ptag = source;
        } else {
            self.children.push(RegisterCluster::Register(source))
        }
        Ok(())
    }

    fn copy_cluster(&mut self, _rname: &str, _ccopy: &Hash) -> PatchResult {
        todo!()
    }

    fn delete_register(&mut self, rspec: &str) -> PatchResult {
        self.children
            .retain(|rc| !matches!(rc, RegisterCluster::Register(r) if matchname(&r.name, rspec)));
        Ok(())
    }

    fn delete_cluster(&mut self, cspec: &str) -> PatchResult {
        self.children
            .retain(|rc| !matches!(rc, RegisterCluster::Cluster(c) if matchname(&c.name, cspec)));
        Ok(())
    }

    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash) -> PatchResult {
        let ctags = self.iter_clusters(cspec).collect::<Vec<_>>();
        if !ctags.is_empty() {
            let cluster_builder = make_cluster(cmod)?;
            let dim = make_dim_element(cmod)?;
            for ctag in ctags {
                modify_dim_element(ctag, &dim)?;
                ctag.modify_from(cluster_builder.clone(), VAL_LVL)?;
            }
        }
        Ok(())
    }

    fn strip_start(&mut self, prefix: &str) -> PatchResult {
        let len = prefix.len();
        let glob = globset::Glob::new(&(prefix.to_string() + "*"))?.compile_matcher();
        for rtag in self.registers_mut() {
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
        for rtag in self.registers_mut() {
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
        let regs = &mut self.children;
        collect_in_array(regs, &pname, rspec, rmod)
    }

    fn collect_in_cluster(&mut self, cname: &str, cmod: &Hash) -> PatchResult {
        let pname = self.name.clone();
        let regs = &mut self.children;
        collect_in_cluster(regs, &pname, cname, cmod)
    }

    fn clear_fields(&mut self, rspec: &str) -> PatchResult {
        for rtag in self.iter_registers(rspec) {
            if rtag.derived_from.is_some() {
                continue;
            }
            rtag.clear_field("*")?;
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
            Err(anyhow!("Could not find `{pname}:{rspec}`"))
        } else {
            Ok(())
        }
    }

    fn process_cluster(&mut self, cspec: &str, cmod: &Hash, update_fields: bool) -> PatchResult {
        // Find all clusters that match the spec
        let mut ccount = 0;
        let pname = self.name.clone();
        for ctag in self.iter_clusters(cspec) {
            ccount += 1;
            ctag.process(cmod, &pname, update_fields)
                .with_context(|| format!("Processing cluster `{}`", ctag.name))?;
        }
        if ccount == 0 {
            Err(anyhow!("Could not find `{pname}:{cspec}`"))
        } else {
            Ok(())
        }
    }
}

fn collect_in_array(
    regs: &mut Vec<RegisterCluster>,
    path: &str,
    rspec: &str,
    rmod: &Hash,
) -> PatchResult {
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
        return Err(anyhow!("{path}: registers {rspec} not found"));
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
    if !(check_offsets(&offsets, dim_increment) && bitmasks.iter().all(|&m| m == bitmasks[0])) {
        return Err(anyhow!(
            "{}: registers cannot be collected into {rspec} array",
            path
        ));
    }
    let mut rinfo = registers.swap_remove(0);
    rinfo.name = if let Some(name) = rmod.get_str("name")? {
        name.into()
    } else {
        format!("{}%s{}", &rspec[..li], &rspec[rspec.len() - ri..])
    };
    if let Some(desc) = rmod.get_str("description")? {
        if desc != "_original" {
            rinfo.description = Some(desc.into());
        }
    } else if dim_index[0] == "0" {
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
    reg.process(rmod, path, true)
        .with_context(|| format!("Processing register `{}`", reg.name))?;
    regs.insert(place, RegisterCluster::Register(reg));
    Ok(())
}

fn collect_in_cluster(
    regs: &mut Vec<RegisterCluster>,
    path: &str,
    cname: &str,
    cmod: &Hash,
) -> PatchResult {
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
            return Err(anyhow!("{path}: registers {rspec} not found"));
        }
        if single {
            if registers.len() > 1 {
                return Err(anyhow!("{path}: more than one registers {rspec} found"));
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
            } else if (dim != registers.len())
                || (dim_index != new_dim_index)
                || (!check_offsets(&offsets, dim_increment))
                || (!bitmasks.iter().all(|&m| m == bitmasks[0]))
            {
                check = false;
                break;
            }
        }
        rdict.insert(rspec.to_string(), registers);
        first = false;
    }
    if !check {
        return Err(anyhow!(
            "{path}: registers cannot be collected into {cname} cluster"
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
            format!("Cluster {cname}, containing {}", rspecs.join(", "))
        }))
        .address_offset(address_offset);
    let cluster = if single {
        for (rspec, mut registers) in rdict.into_iter() {
            let mut reg = registers.swap_remove(0).single();
            let rmod = cmod.get_hash(rspec.as_str())?.unwrap();
            reg.process(rmod, path, true)
                .with_context(|| format!("Processing register `{}`", reg.name))?;
            if let Some(name) = rmod.get_str("name")? {
                reg.name = name.into();
            }
            reg.address_offset -= address_offset;
            children.push(RegisterCluster::Register(reg));
        }

        cinfo.children(children).build(VAL_LVL)?.single()
    } else {
        for (rspec, mut registers) in rdict.into_iter() {
            let mut reg = registers.swap_remove(0).single();
            let rmod = cmod.get_hash(rspec.as_str())?.unwrap();
            reg.process(rmod, path, true)
                .with_context(|| format!("Processing register `{}`", reg.name))?;
            reg.name = if let Some(name) = rmod.get_str("name")? {
                name.into()
            } else {
                let (li, ri) = spec_ind(&rspec);
                format!("{}{}", &rspec[..li], &rspec[rspec.len() - ri..])
            };
            if let Some(desc) = rmod.get_str("description")? {
                reg.description = Some(desc.into());
            }
            reg.address_offset -= address_offset;
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
    Ok(())
}
