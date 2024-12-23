use anyhow::{anyhow, Context, Ok};
use itertools::Itertools;
use svd::Name;
use svd_parser::expand::BlockPath;
use svd_parser::svd::{
    self, Cluster, ClusterInfo, DimElement, Interrupt, Peripheral, Register, RegisterCluster,
    RegisterInfo,
};
use yaml_rust::{yaml::Hash, Yaml};

use super::iterators::{MatchIter, Matched};
use super::register::RegisterExt;
use super::yaml_ext::{AsType, GetVal, ToYaml};
use super::{
    adding_pos, check_offsets, common_description, make_dim_element, matchname, matchsubspec,
    modify_dim_element, spec_ind, Config, PatchResult, Spec, VAL_LVL,
};
use super::{make_cluster, make_interrupt, make_register};

use svd::registercluster::{
    AllRegistersIterMut, ClusterIter, ClusterIterMut, RegisterIter, RegisterIterMut,
};
pub type ClusterMatchIterMut<'a, 'b> = MatchIter<'b, ClusterIterMut<'a>>;
pub type RegMatchIterMut<'a, 'b> = MatchIter<'b, RegisterIterMut<'a>>;

/// Collecting methods for processing peripheral contents
pub(crate) trait PeripheralExt: InterruptExt + RegisterBlockExt {
    const KEYWORDS: &'static [&'static str] = &[
        "_include",
        "_path",
        "_delete",
        "_copy",
        "_strip",
        "_strip_end",
        "_prefix",
        "_suffix",
        "_modify",
        "_clear_fields",
        "_add",
        "_derive",
        "_expand_array",
        "_expand_cluster",
        "_array",
        "_cluster",
        "_clusters",
        "_interrupts",
    ];

    /// Work through a peripheral, handling all registers
    fn process(&mut self, peripheral: &Hash, config: &Config) -> PatchResult;
}

/// Collecting methods for processing cluster contents
pub(crate) trait ClusterExt: RegisterBlockExt {
    const KEYWORDS: &'static [&'static str] = &[
        "_include",
        "_path",
        "_delete",
        "_copy",
        "_strip",
        "_strip_end",
        "_prefix",
        "_suffix",
        "_modify",
        "_clear_fields",
        "_add",
        "_derive",
        "_expand_array",
        "_expand_cluster",
        "_array",
        "_cluster",
        "_clusters",
    ];

    /// Work through a cluster, handling all registers
    fn process(&mut self, cmod: &Hash, parent: &BlockPath, config: &Config) -> PatchResult;

    /// Work through a cluster, handling all registers
    fn pre_process(
        &mut self,
        peripheral: &Hash,
        parent: &BlockPath,
        config: &Config,
    ) -> PatchResult;

    /// Work through a cluster, handling all registers
    fn post_process(
        &mut self,
        peripheral: &Hash,
        parent: &BlockPath,
        config: &Config,
    ) -> PatchResult;
}

/// Collecting methods for processing peripheral interrupt contents
pub(crate) trait InterruptExt {
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
pub(crate) trait RegisterBlockExt: Name {
    const RB_TYPE: &'static str;

    /// Returns mutable iterator over child registers
    fn regs(&self) -> RegisterIter<'_>;

    /// Returns mutable iterator over child registers
    fn regs_mut(&mut self) -> RegisterIterMut<'_>;

    /// Returns mutable iterator over child clusters
    fn clstrs(&self) -> ClusterIter<'_>;

    /// Returns mutable iterator over child clusters
    fn clstrs_mut(&mut self) -> ClusterIterMut<'_>;

    /// Returns mutable iterator over all descendant registers
    fn all_regs_mut(&mut self) -> AllRegistersIterMut<'_>;

    /// Get register by name
    fn get_reg(&self, name: &str) -> Option<&Register>;

    /// Register/cluster block
    #[allow(unused)]
    fn children(&self) -> Option<&Vec<RegisterCluster>>;

    /// Register/cluster block
    fn children_mut(&mut self) -> Option<&mut Vec<RegisterCluster>>;

    /// Iterates over all registers that match rspec and live inside ptag
    fn iter_registers<'a, 'b>(&'a mut self, spec: &'b str) -> RegMatchIterMut<'a, 'b> {
        self.regs_mut().matched(spec)
    }

    /// Iterate over all clusters that match cpsec and live inside ptag
    fn iter_clusters<'a, 'b>(&'a mut self, spec: &'b str) -> ClusterMatchIterMut<'a, 'b> {
        self.clstrs_mut().matched(spec)
    }

    /// Returns string of register names
    fn present_registers(&self) -> String {
        self.regs().map(|r| r.name.as_str()).join(", ")
    }

    /// Returns string of cluster names
    fn present_clusters(&self) -> String {
        self.clstrs().map(|r| r.name.as_str()).join(", ")
    }

    fn add_child(&mut self, child: RegisterCluster);

    fn insert_child(&mut self, pos: usize, child: RegisterCluster) {
        if let Some(children) = self.children_mut() {
            children.insert(pos, child);
        } else {
            self.add_child(child);
        }
    }

    /// Delete registers and clusters matched by rspec inside ptag
    fn delete_child(&mut self, rcspec: &str, bpath: &BlockPath) -> PatchResult {
        if let Some(children) = self.children_mut() {
            let mut done = false;
            children.retain(|rc| {
                let del = matchname(rc.name(), rcspec);
                done |= del;
                !del
            });
            if !done {
                log::info!(
                    "Trying to delete absent `{}` register/cluster from {}",
                    rcspec,
                    bpath
                );
            }
            Ok(())
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    /// Delete registers matched by rspec inside ptag
    fn delete_register(&mut self, rspec: &str, bpath: &BlockPath) -> PatchResult {
        if let Some(children) = self.children_mut() {
            let mut done = false;
            children.retain(|rc| {
                let del = matches!(rc, RegisterCluster::Register(r) if matchname(&r.name, rspec));
                done |= del;
                !del
            });
            if !done {
                log::info!(
                    "Trying to delete absent `{}` register from {}",
                    rspec,
                    bpath
                );
            }
            Ok(())
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    fn delete_cluster(&mut self, cspec: &str) -> PatchResult {
        let (cspec, ignore) = cspec.spec();

        if let Some(children) = self.children_mut() {
            let mut done = false;
            children.retain(|rc| {
                let del = matches!(rc, RegisterCluster::Cluster(c) if matchname(&c.name, cspec));
                done |= del;
                !del
            });
            if !done && !ignore {
                Err(anyhow!("No matching clusters found"))
            } else {
                Ok(())
            }
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    /// Add rname given by radd to ptag
    fn add_register(&mut self, rname: &str, radd: &Hash, bpath: &BlockPath) -> PatchResult {
        if self.regs().any(|r| r.name == rname) {
            return Err(anyhow!(
                "{} {bpath} already has a register {rname}",
                Self::RB_TYPE
            ));
        }

        let rnew = RegisterCluster::Register({
            let reg = make_register(radd, Some(bpath))?
                .name(rname.into())
                .build(VAL_LVL)?;
            if let Some(dim) = make_dim_element(radd)? {
                reg.array(dim.build(VAL_LVL)?)
            } else {
                reg.single()
            }
        });

        if let Some(children) = self.children() {
            let pos = adding_pos(&rnew, children, |rc| match rc {
                RegisterCluster::Register(r) => r.address_offset,
                RegisterCluster::Cluster(c) => c.address_offset,
            });
            self.insert_child(pos, rnew);
        } else {
            self.add_child(rnew);
        }

        Ok(())
    }

    /// Add cname given by cadd to ptag
    fn add_cluster(&mut self, cname: &str, cadd: &Hash, bpath: &BlockPath) -> PatchResult {
        if self.clstrs().any(|c| c.name == cname) {
            return Err(anyhow!(
                "{} {bpath} already has a cluster {cname}",
                Self::RB_TYPE
            ));
        }

        let cnew = RegisterCluster::Cluster({
            let cl = make_cluster(cadd, Some(bpath))?
                .name(cname.into())
                .build(VAL_LVL)?;
            if let Some(dim) = make_dim_element(cadd)? {
                cl.array(dim.build(VAL_LVL)?)
            } else {
                cl.single()
            }
        });

        if let Some(children) = self.children() {
            let pos = adding_pos(&cnew, children, |rc| match rc {
                RegisterCluster::Register(r) => r.address_offset,
                RegisterCluster::Cluster(c) => c.address_offset,
            });
            self.insert_child(pos, cnew);
        } else {
            self.add_child(cnew);
        }

        Ok(())
    }

    /// Remove fields from rname and mark it as derivedFrom rderive.
    /// Update all derivedFrom referencing rname
    fn derive_register(&mut self, rspec: &str, rderive: &Yaml, bpath: &BlockPath) -> PatchResult {
        fn make_path(dpath: &str, bpath: &BlockPath) -> String {
            let mut parts = dpath.split(".");
            match (parts.next(), parts.next(), parts.next()) {
                (Some(cname), Some(rname), None) if !bpath.path.is_empty() => bpath
                    .parent()
                    .unwrap()
                    .new_cluster(cname)
                    .new_register(rname)
                    .to_string(),
                _ => dpath.into(),
            }
        }
        let (rspec, ignore) = rspec.spec();
        let (rderive, dim, info) = if let Some(rderive) = rderive.as_str() {
            (
                rderive,
                None,
                RegisterInfo::builder().derived_from(Some(make_path(rderive, bpath))),
            )
        } else if let Some(hash) = rderive.as_hash() {
            let rderive = hash.get_str("_from")?.ok_or_else(|| {
                anyhow!("derive: source register not given, please add a _from field to {rspec}")
            })?;
            (
                rderive,
                make_dim_element(hash)?,
                make_register(hash, Some(bpath))?.derived_from(Some(make_path(rderive, bpath))),
            )
        } else {
            return Err(anyhow!("derive: incorrect syntax for {rspec}"));
        };

        // Attempt to verify that the destination register name is correct.
        if rderive.contains('.') {
            // This is an absolute identifier name
            // TODO: at the moment we cannot verify absolute names.  We don't have a reference
            // to the Device in order to try and look up the name.  Since we are mutating a member
            // of the device, we cannot get a reference to it.
        } else {
            self.get_reg(rderive).ok_or_else(|| {
                let present = self.present_registers();
                anyhow!("Could not find `{bpath}:{rderive}. Present registers: {present}.")
            })?;
        }

        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        let mut found = Vec::new();
        if !rtags.is_empty() {
            for rtag in rtags {
                found.push(rtag.name.to_string());
                modify_dim_element(rtag, &dim)?;
                rtag.modify_from(info.clone(), VAL_LVL)?;
            }
        } else if !ignore {
            super::check_dimable_name(rspec)?;
            let reg = info.name(rspec.into()).build(VAL_LVL)?;
            self.add_child(RegisterCluster::Register({
                if let Some(dim) = dim {
                    reg.array(dim.build(VAL_LVL)?)
                } else {
                    reg.single()
                }
            }));
        }
        for rname in found {
            for r in self
                .regs_mut()
                .filter(|r| r.derived_from.as_deref() == Some(&rname))
            {
                r.derived_from = Some(rderive.into());
            }
        }
        Ok(())
    }

    /// Remove fields from rname and mark it as derivedFrom rderive.
    /// Update all derivedFrom referencing rname
    fn derive_cluster(&mut self, _cspec: &str, _cderive: &Yaml, _bpath: &BlockPath) -> PatchResult {
        todo!()
    }

    /// Add rname given by deriving from rcopy to ptag
    fn copy_register(&mut self, rname: &str, rcopy: &Hash, bpath: &BlockPath) -> PatchResult {
        let srcname = rcopy.get_str("_from")?.ok_or_else(|| {
            anyhow!("derive: source register not given, please add a _from field to {rname}")
        })?;

        let mut source = self
            .regs()
            .find(|r| r.name == srcname)
            .ok_or_else(|| {
                let present = self.present_registers();
                anyhow!(
                    "{} {bpath} does not have register {srcname}. Present registers: {present}.`",
                    Self::RB_TYPE,
                )
            })?
            .clone();
        let fixes = make_register(rcopy, Some(bpath))?
            .name(rname.into())
            .display_name(Some("".into()));
        // Modifying fields in derived register not implemented
        source.modify_from(fixes, VAL_LVL)?;
        if let Some(ptag) = self.regs_mut().find(|r| r.name == rname) {
            source.address_offset = ptag.address_offset;
            *ptag = source;
        } else {
            self.add_child(RegisterCluster::Register(source))
        }
        Ok(())
    }

    /// Add cname given by deriving from ccopy to ptag
    fn copy_cluster(&mut self, _rname: &str, _ccopy: &Hash, _bpath: &BlockPath) -> PatchResult {
        todo!()
    }

    fn modify_child(&mut self, rcspec: &str, rcmod: &Hash, bpath: &BlockPath) -> PatchResult {
        let (rcspec, ignore) = rcspec.spec();
        let rtags = self.iter_registers(rcspec).collect::<Vec<_>>();
        if rtags.is_empty() && !ignore {
            let ctags = self.iter_clusters(rcspec).collect::<Vec<_>>();
            if ctags.is_empty() {
                let present = self.present_registers();
                Err(anyhow!(
                    "Could not find `{bpath}:{rcspec}. Present registers: {present}.`"
                ))
            } else {
                modify_cluster(ctags, rcmod, bpath)
            }
        } else {
            modify_register(rtags, rcmod, bpath)
        }
    }

    /// Modify rspec inside ptag according to rmod
    fn modify_register(&mut self, rspec: &str, rmod: &Hash, bpath: &BlockPath) -> PatchResult {
        let (rspec, ignore) = rspec.spec();
        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        if rtags.is_empty() && !ignore {
            let present = self.present_registers();
            return Err(anyhow!(
                "Could not find `{bpath}:{rspec}. Present registers: {present}.`"
            ));
        }
        modify_register(rtags, rmod, bpath)
    }

    /// Modify cspec inside ptag according to cmod
    fn modify_cluster(&mut self, cspec: &str, cmod: &Hash, bpath: &BlockPath) -> PatchResult {
        let (cspec, ignore) = cspec.spec();
        let ctags = self.iter_clusters(cspec).collect::<Vec<_>>();
        if ctags.is_empty() && !ignore {
            let present = self.present_clusters();
            return Err(anyhow!(
                "Could not find cluster `{bpath}:{cspec}. Present clusters: {present}.`"
            ));
        }
        modify_cluster(ctags, cmod, bpath)
    }
    /// Work through a register or cluster
    fn process_child(
        &mut self,
        rcspec: &str,
        rcmod: &Hash,
        bpath: &BlockPath,
        config: &Config,
    ) -> PatchResult {
        let (rspec, ignore) = rcspec.spec();
        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        if rtags.is_empty() && !ignore {
            let ctags = self.iter_clusters(rspec).collect::<Vec<_>>();
            if ctags.is_empty() {
                let present = self.present_registers();
                Err(anyhow!(
                    "Could not find `{bpath}:{rspec}. Present registers: {present}.`"
                ))
            } else {
                for ctag in ctags {
                    ctag.process(rcmod, bpath, config)
                        .with_context(|| format!("Processing cluster `{}`", ctag.name))?;
                }
                Ok(())
            }
        } else {
            for rtag in rtags {
                rtag.process(rcmod, bpath, config)
                    .with_context(|| format!("Processing register `{}`", rtag.name))?;
            }
            Ok(())
        }
    }
    /// Work through a register, handling all fields
    #[allow(unused)]
    fn process_register(
        &mut self,
        rspec: &str,
        rmod: &Hash,
        bpath: &BlockPath,
        config: &Config,
    ) -> PatchResult {
        let (rspec, ignore) = rspec.spec();
        let rtags = self.iter_registers(rspec).collect::<Vec<_>>();
        if rtags.is_empty() && !ignore {
            let present = self.present_registers();
            return Err(anyhow!(
                "Could not find `{bpath}:{rspec}. Present registers: {present}.`"
            ));
        }
        for rtag in rtags {
            rtag.process(rmod, bpath, config)
                .with_context(|| format!("Processing register `{}`", rtag.name))?;
        }
        Ok(())
    }

    /// Work through a cluster, handling all contents
    fn process_cluster(
        &mut self,
        cspec: &str,
        cmod: &Hash,
        bpath: &BlockPath,
        config: &Config,
    ) -> PatchResult {
        let (cspec, ignore) = cspec.spec();
        let ctags = self.iter_clusters(cspec).collect::<Vec<_>>();
        if ctags.is_empty() && !ignore {
            let present = self.present_clusters();
            return Err(anyhow!(
                "Could not find cluster `{bpath}:{cspec}. Present clusters: {present}.`"
            ));
        }
        for ctag in self.iter_clusters(cspec) {
            ctag.process(cmod, bpath, config)
                .with_context(|| format!("Processing cluster `{}`", ctag.name))?;
        }
        Ok(())
    }

    /// Delete substring from the beginning of register names inside ptag
    fn strip_start(&mut self, prefix: &str) -> PatchResult {
        let len = prefix.len();
        let glob = globset::Glob::new(&(prefix.to_string() + "*"))?.compile_matcher();
        for rtag in self.regs_mut() {
            if glob.is_match(&rtag.name) {
                rtag.name.drain(..len);
            }
            if let Some(dname) = rtag.display_name.as_mut() {
                if glob.is_match(dname.as_str()) {
                    dname.drain(..len);
                }
            }
            if let Some(name) = rtag.alternate_register.as_mut() {
                if glob.is_match(name.as_str()) {
                    name.drain(..len);
                }
            }
        }
        for ctag in self.clstrs_mut() {
            if glob.is_match(&ctag.name) {
                ctag.name.drain(..len);
            }
            if let Some(dname) = ctag.header_struct_name.as_mut() {
                if glob.is_match(dname.as_str()) {
                    dname.drain(..len);
                }
            }
            if let Some(name) = ctag.alternate_cluster.as_mut() {
                if glob.is_match(name.as_str()) {
                    name.drain(..len);
                }
            }
        }
        Ok(())
    }

    /// Delete substring from the ending of register names inside ptag
    fn strip_end(&mut self, suffix: &str) -> PatchResult {
        let len = suffix.len();
        let glob = globset::Glob::new(&("*".to_string() + suffix))
            .unwrap()
            .compile_matcher();
        for rtag in self.regs_mut() {
            if glob.is_match(&rtag.name) {
                let nlen = rtag.name.len();
                rtag.name.truncate(nlen - len);
            }
            if let Some(dname) = rtag.display_name.as_mut() {
                if glob.is_match(dname.as_str()) {
                    let nlen = dname.len();
                    dname.truncate(nlen - len);
                }
            }
            if let Some(name) = rtag.alternate_register.as_mut() {
                if glob.is_match(name.as_str()) {
                    let nlen = name.len();
                    name.truncate(nlen - len);
                }
            }
        }
        for ctag in self.clstrs_mut() {
            if glob.is_match(&ctag.name) {
                let nlen = ctag.name.len();
                ctag.name.truncate(nlen - len);
            }
            if let Some(dname) = ctag.header_struct_name.as_mut() {
                if glob.is_match(dname.as_str()) {
                    let nlen = dname.len();
                    dname.truncate(nlen - len);
                }
            }
            if let Some(name) = ctag.alternate_cluster.as_mut() {
                if glob.is_match(name.as_str()) {
                    let nlen = name.len();
                    name.truncate(nlen - len);
                }
            }
        }
        Ok(())
    }

    /// Add prefix at the beginning of register names inside ptag
    fn add_prefix(&mut self, prefix: &str) -> PatchResult {
        for rtag in self.regs_mut() {
            rtag.name.insert_str(0, prefix);
            if let Some(dname) = rtag.display_name.as_mut() {
                dname.insert_str(0, prefix);
            }
            if let Some(name) = rtag.alternate_register.as_mut() {
                name.insert_str(0, prefix);
            }
        }
        Ok(())
    }

    /// Add suffix at the ending of register names inside ptag
    fn add_suffix(&mut self, suffix: &str) -> PatchResult {
        for rtag in self.regs_mut() {
            rtag.name.push_str(suffix);
            if let Some(dname) = rtag.display_name.as_mut() {
                dname.push_str(suffix);
            }
            if let Some(name) = rtag.alternate_register.as_mut() {
                name.push_str(suffix);
            }
        }
        Ok(())
    }

    /// Collect same registers in peripheral into register array
    fn collect_in_array(
        &mut self,
        rspec: &str,
        rmod: &Hash,
        bpath: &BlockPath,
        config: &Config,
    ) -> PatchResult {
        if let Some(regs) = self.children_mut() {
            collect_in_array(regs, bpath, rspec, rmod, config)
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    fn get_cluster_registers(
        &self,
        cspec: &str,
    ) -> Vec<(&ClusterInfo, DimElement, Vec<RegisterCluster>)> {
        let (cspec, _) = cspec.spec();
        self.clstrs()
            .filter(|ctag| matchname(&ctag.name, cspec))
            .filter_map(|ctag| match ctag.clone() {
                svd_rs::MaybeArray::Array(cluster, dim) => {
                    let mut clusters_and_registers = cluster
                        .registers()
                        .map(|reg| reg.clone().into())
                        .collect::<Vec<_>>();
                    clusters_and_registers.extend(
                        cluster
                            .clusters()
                            .map(|reg| reg.clone().into())
                            .collect::<Vec<_>>(),
                    );
                    Some((std::ops::Deref::deref(ctag), dim, clusters_and_registers))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    }

    /// Expand register cluster
    fn expand_cluster(
        &mut self,
        cspec: &str,
        bpath: &BlockPath,
        pre_index_delim: Option<&str>,
        post_index_delim: Option<&str>,
        zeroindex: Option<bool>,
        noprefix: Option<bool>,
    ) -> PatchResult {
        let mut clusters_to_expand_with_info = Vec::new();
        let mut clusters_to_delete = Vec::new();

        let (_, ignore) = cspec.spec();

        // some fancy footwork to satisfy the borrow checker gods
        let cluster_data = self.get_cluster_registers(cspec);
        if cluster_data.is_empty() && !ignore {
            let present = self.present_clusters().clone();
            return Err(anyhow!(
                "Could not find cluster `{bpath}:{cspec}. Present clusters: {present}.`"
            ));
        }

        for (ci, dim, rc) in cluster_data {
            let mut regs = Vec::new();
            for reg in rc {
                regs.push(<svd_rs::RegisterCluster as Clone>::clone(&reg));
            }
            clusters_to_expand_with_info.push((ci.clone(), dim, regs));
        }

        if let Some(regs) = self.children_mut() {
            for (ctag, dim, cluster_registers) in clusters_to_expand_with_info.clone() {
                let mut found = false;
                let cluster_offset = ctag.address_offset;
                log::info!(
                    "Expanding {} element cluster {} for peripheral {}",
                    dim.dim,
                    ctag.name,
                    bpath
                );
                // iterate through each dim to expand each dim of a cluster
                for n_dim in 0..dim.dim {
                    let prefix = Self::expand_cluster_register_name_prefix(
                        n_dim,
                        ctag.clone(),
                        bpath,
                        dim.clone(),
                        pre_index_delim,
                        post_index_delim,
                        zeroindex,
                        noprefix,
                    )?;
                    for reg in cluster_registers.clone() {
                        let reg = match reg {
                            RegisterCluster::Register(mut register) => {
                                register.address_offset += cluster_offset;
                                register.address_offset += n_dim * dim.dim_increment;
                                register.name = format!("{}{}", prefix, register.name);
                                RegisterCluster::Register(register)
                            }
                            RegisterCluster::Cluster(mut cluster) => {
                                cluster.address_offset += cluster_offset;
                                cluster.address_offset += n_dim * dim.dim_increment;
                                cluster.name = format!("{}{}", prefix, cluster.name);
                                RegisterCluster::Cluster(cluster)
                            }
                        };
                        found = true;
                        log::info!(
                            "Adding register at offset 0x{:08x}: {}",
                            reg.address_offset(),
                            reg.name(),
                        );
                        regs.push(reg.clone())
                    }
                }
                if !found {
                    return Err(anyhow!("No registers found in cluster {:?}", cspec));
                } else {
                    clusters_to_delete.push(ctag.name);
                }
            }
        } else {
            return Err(anyhow!("No registers or clusters"));
        };

        self.delete_cluster(cspec)
            .with_context(|| format!("Deleting clusters matched to `{cspec}`"))?;

        Ok(())
    }

    /// get the prefix to apply to a register name in a cluster that is being expanded
    #[allow(clippy::too_many_arguments)]
    fn expand_cluster_register_name_prefix(
        n_dim: u32,
        ctag: ClusterInfo,
        bpath: &BlockPath,
        dim: DimElement,
        pre_index_delim: Option<&str>,
        post_index_delim: Option<&str>,
        zeroindex: Option<bool>,
        noprefix: Option<bool>,
    ) -> anyhow::Result<String> {
        let pre_index_delim = pre_index_delim.unwrap_or("_").to_string();
        let post_index_delim = post_index_delim.unwrap_or("_").to_string();

        let has_bracket_delim = ctag.name.find(r#"[%s]"#);
        let has_nobracket_delim = ctag.name.find(r#"[%s]"#);
        let prefix = if dim.dim > 1 || matches!(zeroindex, Some(true)) {
            if let Some(true) = noprefix {
                return Err(anyhow!(
                    "Cannot expand cluster {}:{} with multiple elements with noprefix",
                    bpath,
                    ctag.name
                ));
            }
            match (
                dim.dim_index.clone(),
                has_bracket_delim,
                has_nobracket_delim,
            ) {
                (Some(_), Some(_), _) => {
                    return Err(anyhow!("Cannot expand cluster {}:{} with multiple elements that uses dim_index and [%s] substitution https://open-cmsis-pack.github.io/svd-spec/main/elem_registers.html", bpath, ctag.name));
                }
                (Some(dim_index), None, Some(_)) => {
                    if dim_index.len() != dim.dim as usize {
                        return Err(anyhow!("Cannot expand cluster {}:{} with multiple elements that has a dim_index with a number of elements unequal to dim length. _modify cluster dim or index before expanding cluster", bpath, ctag.name));
                    } else {
                        format!(
                            "{}{}",
                            &ctag.name.replace(
                                "%s",
                                &format!(
                                    "{}{}",
                                    pre_index_delim,
                                    &dim_index[n_dim as usize].to_string()
                                )
                            ),
                            post_index_delim
                        )
                    }
                }
                (Some(dim_index), None, None) => {
                    if dim_index.len() != dim.dim as usize {
                        return Err(anyhow!("Cannot expand cluster {}:{} with multiple elements that has a dim_index with a number of elements unequal to dim length. _modify cluster dim or index before expanding cluster ", bpath, ctag.name));
                    } else {
                        format!(
                            "{}{}",
                            &ctag.name.replace(
                                r#"%s"#,
                                &format!(
                                    "{}{}",
                                    pre_index_delim,
                                    &dim_index[n_dim as usize].to_string()
                                )
                            ),
                            post_index_delim
                        )
                    }
                }
                (None, Some(_), _) => {
                    format!(
                        "{}{}",
                        &ctag.name.replace(
                            r#"[%s]"#,
                            &format!("{}{}", pre_index_delim, &n_dim.to_string())
                        ),
                        post_index_delim
                    )
                }
                (None, None, _) => {
                    format!(
                        "{}{}{}{}",
                        ctag.name, pre_index_delim, n_dim, post_index_delim
                    )
                }
            }
        } else {
            if let Some(true) = noprefix {
                return Ok("".to_string());
            }
            // the cluster is a single element and zeroindex is false, so we will skip adding an index
            match (has_bracket_delim, has_nobracket_delim) {
                (Some(_), _) => {
                    format!("{}{}", &ctag.name.replace(r#"[%s]"#, ""), post_index_delim)
                }
                (None, Some(_)) => {
                    format!("{}{}", &ctag.name.replace(r#"%s"#, ""), post_index_delim)
                }
                (None, None) => {
                    format!("{}{}", &ctag.name, post_index_delim)
                }
            }
        };
        Ok(prefix)
    }

    /// Expand register array
    fn expand_array(&mut self, rspec: &str, _rmod: &Hash, _config: &Config) -> PatchResult {
        if let Some(regs) = self.children_mut() {
            let mut found = false;
            for rc in std::mem::take(regs) {
                match rc {
                    RegisterCluster::Register(Register::Array(r, d))
                        if matchname(&r.name, rspec) =>
                    {
                        found = true;
                        for ri in svd::register::expand(&r, &d) {
                            regs.push(RegisterCluster::Register(ri.single()))
                        }
                    }
                    rc => regs.push(rc),
                }
            }
            if !found {
                Err(anyhow!("Register {rspec} not found"))
            } else {
                Ok(())
            }
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    /// Collect registers in peripheral into clusters
    fn collect_in_cluster(
        &mut self,
        cname: &str,
        cmod: &Hash,
        bpath: &BlockPath,
        config: &Config,
    ) -> PatchResult {
        if let Some(regs) = self.children_mut() {
            collect_in_cluster(regs, bpath, cname, cmod, config)
        } else {
            Err(anyhow!("No registers or clusters"))
        }
    }

    /// Clear contents of all fields inside registers matched by rspec
    fn clear_fields(&mut self, rspec: &str) -> PatchResult {
        for rtag in self.all_regs_mut().matched(rspec) {
            if rtag.derived_from.is_some() {
                continue;
            }
            rtag.clear_field("*")?;
        }
        Ok(())
    }
}

fn modify_register(rtags: Vec<&mut Register>, rmod: &Hash, bpath: &BlockPath) -> PatchResult {
    let register_builder = make_register(rmod, Some(bpath))?;
    let dim = make_dim_element(rmod)?;
    for rtag in rtags {
        modify_dim_element(rtag, &dim)?;
        rtag.modify_from(register_builder.clone(), VAL_LVL)?;
        if let Some("") = rmod.get_str("access")? {
            rtag.properties.access = None;
        }
    }
    Ok(())
}

fn modify_cluster(ctags: Vec<&mut Cluster>, cmod: &Hash, bpath: &BlockPath) -> PatchResult {
    let cluster_builder = make_cluster(cmod, Some(bpath))?;
    let dim = make_dim_element(cmod)?;
    for ctag in ctags {
        modify_dim_element(ctag, &dim)?;
        ctag.modify_from(cluster_builder.clone(), VAL_LVL)?;
    }
    Ok(())
}

impl RegisterBlockExt for Peripheral {
    const RB_TYPE: &'static str = "peripheral";

    fn regs(&self) -> RegisterIter<'_> {
        self.registers()
    }
    fn regs_mut(&mut self) -> RegisterIterMut<'_> {
        self.registers_mut()
    }
    fn clstrs(&self) -> ClusterIter<'_> {
        self.clusters()
    }
    fn clstrs_mut(&mut self) -> ClusterIterMut<'_> {
        self.clusters_mut()
    }
    fn all_regs_mut(&mut self) -> AllRegistersIterMut<'_> {
        self.all_registers_mut()
    }
    fn get_reg(&self, name: &str) -> Option<&Register> {
        self.get_register(name)
    }
    fn children(&self) -> Option<&Vec<RegisterCluster>> {
        self.registers.as_ref()
    }
    fn children_mut(&mut self) -> Option<&mut Vec<RegisterCluster>> {
        self.registers.as_mut()
    }

    fn add_child(&mut self, child: RegisterCluster) {
        self.registers
            .get_or_insert_with(Default::default)
            .push(child)
    }
}

impl RegisterBlockExt for Cluster {
    const RB_TYPE: &'static str = "cluster";

    fn regs(&self) -> RegisterIter<'_> {
        self.registers()
    }
    fn regs_mut(&mut self) -> RegisterIterMut<'_> {
        self.registers_mut()
    }
    fn all_regs_mut(&mut self) -> AllRegistersIterMut<'_> {
        self.all_registers_mut()
    }
    fn clstrs(&self) -> ClusterIter<'_> {
        self.clusters()
    }
    fn clstrs_mut(&mut self) -> ClusterIterMut<'_> {
        self.clusters_mut()
    }
    fn get_reg(&self, name: &str) -> Option<&Register> {
        self.get_register(name)
    }
    fn children(&self) -> Option<&Vec<RegisterCluster>> {
        Some(&self.children)
    }
    fn children_mut(&mut self) -> Option<&mut Vec<RegisterCluster>> {
        Some(&mut self.children)
    }
    fn add_child(&mut self, child: RegisterCluster) {
        self.children.push(child)
    }
}

impl PeripheralExt for Peripheral {
    fn process(&mut self, pmod: &Hash, config: &Config) -> PatchResult {
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

        let ppath = BlockPath::new(&self.name);

        // Handle deletions
        if let Some(deletions) = pmod.get_yaml("_delete") {
            match deletions {
                Yaml::String(rcspec) => {
                    self.delete_child(rcspec, &ppath).with_context(|| {
                        format!("Deleting registers and clusters matched to `{rcspec}`")
                    })?;
                }
                Yaml::Array(deletions) => {
                    for rcspec in deletions {
                        let rcspec = rcspec.str()?;
                        self.delete_child(rcspec, &ppath).with_context(|| {
                            format!("Deleting registers and clusters matched to `{rcspec}`")
                        })?;
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers")? {
                        self.delete_register(rspec, &ppath)
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
                        self.copy_register(rname, rcopy, &ppath).with_context(|| {
                            format!("Copying register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rcopy.hash()? {
                        let cname = cname.str()?;
                        let ccopy = val.hash()?;
                        self.copy_cluster(rname, ccopy, &ppath)
                            .with_context(|| format!("Copying cluster `{cname}` from `{val:?}`"))?;
                    }
                }
                _ => {
                    let rcopy = rcopy.hash()?;
                    self.copy_register(rname, rcopy, &ppath)
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

        if let Some(prefix) = pmod.get_str("_prefix")? {
            self.add_prefix(prefix)
                .with_context(|| format!("Adding prefix `{prefix}` to register names"))?;
        }
        if let Some(suffix) = pmod.get_str("_suffix")? {
            self.add_suffix(suffix)
                .with_context(|| format!("Adding suffix `{suffix}` to register names"))?;
        }

        // Handle modifications
        for (rspec, rmod) in pmod.hash_iter("_modify") {
            let rmod = rmod.hash()?;
            match rspec.str()? {
                "_registers" => {
                    for (rspec, val) in rmod {
                        let rspec = rspec.str()?;
                        self.modify_register(rspec, val.hash()?, &ppath)
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
                "_clusters" => {
                    for (cspec, val) in rmod {
                        let cspec = cspec.str()?;
                        self.modify_cluster(cspec, val.hash()?, &ppath)
                            .with_context(|| format!("Modifying clusters matched to `{cspec}`"))?;
                    }
                }
                rcspec => self.modify_child(rcspec, rmod, &ppath).with_context(|| {
                    format!("Modifying registers or clusters matched to `{rcspec}`")
                })?,
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
                        self.add_register(rname, val.hash()?, &ppath)
                            .with_context(|| format!("Adding register `{rname}`"))?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in radd {
                        let cname = cname.str()?;
                        self.add_cluster(cname, val.hash()?, &ppath)
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
                    .add_register(rname, radd, &ppath)
                    .with_context(|| format!("Adding register `{rname}`"))?,
            }
        }

        for (rspec, rderive) in pmod.hash_iter("_derive") {
            let rspec = rspec.str()?;
            match rspec {
                "_registers" => {
                    for (rspec, val) in rderive.hash()? {
                        let rspec = rspec.str()?;
                        self.derive_register(rspec, val, &ppath).with_context(|| {
                            format!("Deriving register `{rspec}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cspec, val) in rderive.hash()? {
                        let cspec = cspec.str()?;
                        self.derive_cluster(cspec, val, &ppath).with_context(|| {
                            format!("Deriving cluster `{cspec}` from `{val:?}`")
                        })?;
                    }
                }
                _ => {
                    self.derive_register(rspec, rderive, &ppath)
                        .with_context(|| {
                            format!("Deriving register `{rspec}` from `{rderive:?}`")
                        })?;
                }
            }
        }

        // Handle registers or clusters
        for (rcspec, rcmod) in pmod {
            let rcspec = rcspec.str()?;
            if Self::KEYWORDS.contains(&rcspec) {
                continue;
            }
            self.process_child(rcspec, rcmod.hash()?, &ppath, config)
                .with_context(|| format!("According to `{rcspec}`"))?;
        }

        // Expand register arrays
        for (rspec, rmod) in pmod.hash_iter("_expand_array") {
            let rspec = rspec.str()?;
            self.expand_array(rspec, rmod.hash()?, config)
                .with_context(|| format!("During expand of `{rspec}` array"))?;
        }
        // Collect registers in arrays
        for (rspec, rmod) in pmod.hash_iter("_array") {
            let rspec = rspec.str()?;
            self.collect_in_array(rspec, rmod.hash()?, &ppath, config)
                .with_context(|| format!("Collecting registers matched to `{rspec}` in array"))?;
        }

        // Collect registers in clusters
        for (cname, cmod) in pmod.hash_iter("_cluster") {
            let cname = cname.str()?;
            self.collect_in_cluster(cname, cmod.hash()?, &ppath, config)
                .with_context(|| format!("Collecting registers in cluster `{cname}`"))?;
        }

        // Handle clusters
        for (cspec, cluster) in pmod.hash_iter("_clusters") {
            let cspec = cspec.str()?;
            self.process_cluster(cspec, cluster.hash()?, &ppath, config)
                .with_context(|| format!("According to `{cspec}`"))?;
        }

        // Handle cluster expansions
        if let Some(expand_cluster) = pmod.get_yaml("_expand_cluster") {
            match expand_cluster {
                Yaml::String(cspec) => {
                    self.expand_cluster(cspec, &ppath, None, None, None, None)
                        .with_context(|| format!("During expand of `{cspec}` cluster"))?;
                }
                Yaml::Array(cspec) => {
                    for cname in cspec {
                        let cname = cname.str()?;
                        self.expand_cluster(cname, &ppath, None, None, None, None)
                            .with_context(|| format!("During expand of `{cname}` cluster"))?;
                    }
                }
                Yaml::Hash(cspec) => {
                    for (cname, cspec) in cspec {
                        let cspec = cspec.hash().ok();
                        let cname = cname.str()?;
                        let mut preindex = None;
                        let mut postindex = None;
                        let mut zeroindex = None;
                        let mut noprefix = None;
                        if let Some(cspec) = cspec {
                            for (key, val) in cspec {
                                match key.str()? {
                                    "_preindex" => preindex = Some(val.str()?),
                                    "_postindex" => postindex = Some(val.str()?),
                                    "_zeroindex" => zeroindex = Some(val.bool()?),
                                    "_noprefix" => noprefix = Some(val.bool()?),
                                    _ => {
                                        return Err(anyhow!(
                                            "`_expand_cluster` requires string value or array of strings"
                                        ))
                                    }
                                }
                            }
                        };
                        self.expand_cluster(
                            cname, &ppath, preindex, postindex, zeroindex, noprefix,
                        )
                        .with_context(|| format!("During expand of `{cname}` cluster"))?;
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "`_expand_cluster` requires string value or array of strings"
                    ))
                }
            }
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
        let mut done = false;
        self.interrupt.retain(|i| {
            let del = matchname(&i.name, ispec);
            done |= del;
            !del
        });
        if !done {
            log::info!(
                "Trying to delete absent `{}` interrupt from {}",
                ispec,
                self.name
            );
        }
        Ok(())
    }
}

impl ClusterExt for Cluster {
    fn pre_process(&mut self, cmod: &Hash, parent: &BlockPath, _config: &Config) -> PatchResult {
        let cpath = parent.new_cluster(&self.name);

        // Handle deletions
        if let Some(deletions) = cmod.get_yaml("_delete") {
            match deletions {
                Yaml::String(rcspec) => {
                    self.delete_child(rcspec, &cpath).with_context(|| {
                        format!("Deleting registers and clusters matched to `{rcspec}`")
                    })?;
                }
                Yaml::Array(deletions) => {
                    for rcspec in deletions {
                        let rcspec = rcspec.str()?;
                        self.delete_child(rcspec, &cpath).with_context(|| {
                            format!("Deleting registers and clusters matched to `{rcspec}`")
                        })?;
                    }
                }
                Yaml::Hash(deletions) => {
                    for rspec in deletions.str_vec_iter("_registers")? {
                        self.delete_register(rspec, &cpath)
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
        for (rname, rcopy) in cmod.hash_iter("_copy") {
            let rname = rname.str()?;
            match rname {
                "_registers" => {
                    for (rname, val) in rcopy.hash()? {
                        let rname = rname.str()?;
                        let rcopy = val.hash()?;
                        self.copy_register(rname, rcopy, &cpath).with_context(|| {
                            format!("Copying register `{rname}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in rcopy.hash()? {
                        let cname = cname.str()?;
                        let ccopy = val.hash()?;
                        self.copy_cluster(rname, ccopy, &cpath)
                            .with_context(|| format!("Copying cluster `{cname}` from `{val:?}`"))?;
                    }
                }
                _ => {
                    let rcopy = rcopy.hash()?;
                    self.copy_register(rname, rcopy, &cpath)
                        .with_context(|| format!("Copying register `{rname}` from `{rcopy:?}`"))?;
                }
            }
        }

        // Handle strips
        for prefix in cmod.str_vec_iter("_strip")? {
            self.strip_start(prefix)
                .with_context(|| format!("Stripping prefix `{prefix}` from register names"))?;
        }
        for suffix in cmod.str_vec_iter("_strip_end")? {
            self.strip_end(suffix)
                .with_context(|| format!("Stripping suffix `{suffix}` from register names"))?;
        }

        if let Some(prefix) = cmod.get_str("_prefix")? {
            self.add_prefix(prefix)
                .with_context(|| format!("Adding prefix `{prefix}` to register names"))?;
        }
        if let Some(suffix) = cmod.get_str("_suffix")? {
            self.add_suffix(suffix)
                .with_context(|| format!("Adding suffix `{suffix}` to register names"))?;
        }

        // Handle modifications
        for (rspec, rmod) in cmod.hash_iter("_modify") {
            let rmod = rmod.hash()?;
            match rspec.str()? {
                "_registers" => {
                    for (rspec, val) in rmod {
                        let rspec = rspec.str()?;
                        self.modify_register(rspec, val.hash()?, &cpath)
                            .with_context(|| format!("Modifying registers matched to `{rspec}`"))?;
                    }
                }
                "_clusters" => {
                    for (cspec, val) in rmod {
                        let cspec = cspec.str()?;
                        self.modify_cluster(cspec, val.hash()?, &cpath)
                            .with_context(|| format!("Modifying clusters matched to `{cspec}`"))?;
                    }
                }
                rcspec => self.modify_child(rcspec, rmod, &cpath).with_context(|| {
                    format!("Modifying registers or clusters matched to `{rcspec}`")
                })?,
            }
        }

        // Handle field clearing
        for rspec in cmod.str_vec_iter("_clear_fields")? {
            self.clear_fields(rspec).with_context(|| {
                format!("Clearing contents of fields in registers matched to `{rspec}` ")
            })?;
        }

        // Handle additions
        for (rname, radd) in cmod.hash_iter("_add") {
            let radd = radd.hash()?;
            match rname.str()? {
                "_registers" => {
                    for (rname, val) in radd {
                        let rname = rname.str()?;
                        self.add_register(rname, val.hash()?, &cpath)
                            .with_context(|| format!("Adding register `{rname}`"))?;
                    }
                }
                "_clusters" => {
                    for (cname, val) in radd {
                        let cname = cname.str()?;
                        self.add_cluster(cname, val.hash()?, &cpath)
                            .with_context(|| format!("Adding cluster `{cname}`"))?;
                    }
                }
                rname => self
                    .add_register(rname, radd, &cpath)
                    .with_context(|| format!("Adding register `{rname}`"))?,
            }
        }

        for (rspec, rderive) in cmod.hash_iter("_derive") {
            let rspec = rspec.str()?;
            match rspec {
                "_registers" => {
                    for (rspec, val) in rderive.hash()? {
                        let rspec = rspec.str()?;
                        self.derive_register(rspec, val, &cpath).with_context(|| {
                            format!("Deriving register `{rspec}` from `{val:?}`")
                        })?;
                    }
                }
                "_clusters" => {
                    for (cspec, val) in rderive.hash()? {
                        let cspec = cspec.str()?;
                        self.derive_cluster(cspec, val, &cpath).with_context(|| {
                            format!("Deriving cluster `{cspec}` from `{val:?}`")
                        })?;
                    }
                }
                _ => {
                    self.derive_register(rspec, rderive, &cpath)
                        .with_context(|| {
                            format!("Deriving register `{rspec}` from `{rderive:?}`")
                        })?;
                }
            }
        }

        Ok(())
    }

    fn process(&mut self, cmod: &Hash, parent: &BlockPath, config: &Config) -> PatchResult {
        self.pre_process(cmod, parent, config)?;

        let cpath = parent.new_cluster(&self.name);

        // Handle clusters
        for (cspec, cluster) in cmod.hash_iter("_clusters") {
            let cspec = cspec.str()?;
            self.process_cluster(cspec, cluster.hash()?, &cpath, config)
                .with_context(|| format!("According to `{cspec}`"))?;
        }

        // Handle registers or clusters
        for (rcspec, rcmod) in cmod {
            let rcspec = rcspec.str()?;
            if Self::KEYWORDS.contains(&rcspec) {
                continue;
            }
            self.process_child(rcspec, rcmod.hash()?, &cpath, config)
                .with_context(|| format!("According to `{rcspec}`"))?;
        }

        self.post_process(cmod, parent, config)
    }

    fn post_process(&mut self, cmod: &Hash, parent: &BlockPath, config: &Config) -> PatchResult {
        let cpath = parent.new_cluster(&self.name);

        // Expand register arrays
        for (rspec, rmod) in cmod.hash_iter("_expand_array") {
            let rspec = rspec.str()?;
            self.expand_array(rspec, rmod.hash()?, config)
                .with_context(|| format!("During expand of `{rspec}` array"))?;
        }

        // Collect registers in arrays
        for (rspec, rmod) in cmod.hash_iter("_array") {
            let rspec = rspec.str()?;
            self.collect_in_array(rspec, rmod.hash()?, &cpath, config)
                .with_context(|| format!("Collecting registers matched to `{rspec}` in array"))?;
        }

        // Collect registers in clusters
        for (cname, incmod) in cmod.hash_iter("_cluster") {
            let cname = cname.str()?;
            self.collect_in_cluster(cname, incmod.hash()?, &cpath, config)
                .with_context(|| format!("Collecting registers in cluster `{cname}`"))?;
        }

        Ok(())
    }
}

fn collect_in_array(
    regs: &mut Vec<RegisterCluster>,
    path: &BlockPath,
    rspec: &str,
    rmod: &Hash,
    config: &Config,
) -> PatchResult {
    let mut registers = Vec::new();
    let mut place = usize::MAX;
    let mut i = 0;
    let (rspec, ignore) = rspec.spec();
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
        if ignore {
            return Ok(());
        }
        return Err(anyhow!(
            "{path}: registers {rspec} not found. Present registers: {}.`",
            regs.iter()
                .filter_map(|rc| match rc {
                    RegisterCluster::Register(r) => Some(r.name.as_str()),
                    _ => None,
                })
                .join(", ")
        ));
    }
    registers.sort_by_key(|r| r.address_offset);
    let Some((li, ri)) = spec_ind(rspec) else {
        return Err(anyhow!(
            "`{rspec}` contains no tokens or contains more than one token"
        ));
    };
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
    let dim_increment = if dim > 1 {
        offsets[1] - offsets[0]
    } else {
        rmod.get_u32("dimIncrement")?
            .or_else(|| registers[0].properties.size.map(|s| s / 8))
            .unwrap_or_default()
    };
    if dim_increment == 0 {
        return Err(anyhow!("Need to specify dimIncrement"));
    }
    if !check_offsets(&offsets, dim_increment) {
        return Err(anyhow!("{path}: registers cannot be collected into {rspec} array. Different addressOffset increments"));
    }
    let bitmasks = registers.iter().map(|r| r.bitmask()).collect::<Vec<_>>();
    if !bitmasks.iter().all(|&m| m == bitmasks[0]) {
        return Err(anyhow!(
            "{path}: registers cannot be collected into {rspec} array. Different bit masks"
        ));
    }

    registers[0].name = if let Some(name) = rmod.get_str("name")? {
        name.into()
    } else {
        format!("{}%s{}", &rspec[..li], &rspec[rspec.len() - ri..])
    };

    if let Some(desc) = rmod.get_str("description")? {
        if desc != "_original" {
            registers[0].description = Some(desc.into());
        }
    } else {
        let descs: Vec<_> = registers.iter().map(|r| r.description.as_deref()).collect();
        registers[0].description = common_description(&descs, &dim_index).ok_or_else(||
            anyhow!("{path}: registers cannot be collected into {rspec} array. Please, specify description")
        )?;
    }
    if let Some(dname) = rmod.get_str("displayName")? {
        if dname != "_original" {
            registers[0].display_name = Some(dname.into());
        }
    } else {
        let names: Vec<_> = registers
            .iter()
            .map(|r| r.display_name.as_deref())
            .collect();
        registers[0].display_name = common_description(&names, &dim_index).ok_or_else(||
            anyhow!("{path}: registers cannot be collected into {rspec} array. Please, specify displayName")
        )?;
    }
    let rinfo = registers.swap_remove(0);
    let mut reg = rinfo.array(
        DimElement::builder()
            .dim(dim as u32)
            .dim_increment(dim_increment)
            .dim_index(Some(dim_index))
            .build(VAL_LVL)?,
    );
    let mut config = config.clone();
    config.update_fields = true;
    reg.process(rmod, path, &config)
        .with_context(|| format!("Processing register `{}`", reg.name))?;
    regs.insert(place, RegisterCluster::Register(reg));
    Ok(())
}

fn collect_in_cluster(
    regs: &mut Vec<RegisterCluster>,
    path: &BlockPath,
    cname: &str,
    cmod: &Hash,
    config: &Config,
) -> PatchResult {
    let mut rdict = super::linked_hash_map::LinkedHashMap::new();
    let mut first = None;
    let mut dim = 0;
    let mut dim_index = Vec::new();
    let mut dim_increment = cmod.get_u32("dimIncrement")?.unwrap_or(0);
    let mut offsets = Vec::new();
    let mut place = usize::MAX;
    let mut rspecs = Vec::new();
    let single = !cname.contains("%s");

    for (rspec, rmod) in cmod {
        let rspec = rspec.str()?;
        if ["description", "dimIncrement"].contains(&rspec) || Cluster::KEYWORDS.contains(&rspec) {
            continue;
        }
        let mut registers = Vec::new();
        let mut i = 0;
        let (rspec, ignore) = rspec.spec();
        while i < regs.len() {
            match &regs[i] {
                RegisterCluster::Register(r) if matchname(&r.name, rspec) => {
                    if let RegisterCluster::Register(r) = regs.remove(i) {
                        registers.push(r);
                        place = place.min(i);
                    }
                }
                _ => i += 1,
            }
        }
        if registers.is_empty() {
            if ignore {
                continue;
            }
            return Err(anyhow!(
                "{path}: registers {rspec} not found. Present registers: {}.`",
                regs.iter()
                    .filter_map(|rc| match rc {
                        RegisterCluster::Register(r) => Some(r.name.as_str()),
                        _ => None,
                    })
                    .join(", ")
            ));
        }
        rspecs.push(rspec.to_string());

        if single {
            if registers.len() > 1 {
                return Err(anyhow!("{path}: more than one registers {rspec} found"));
            }
        } else {
            registers.sort_by_key(|r| r.address_offset);
            if let Register::Array(_, rdim) = &registers[0] {
                if !registers
                    .iter()
                    .skip(1)
                    .all(|r| matches!(r, Register::Array(_, d) if d == rdim))
                {
                    return Err(anyhow!("`{rspec}` have different dim blocks"));
                }
            } else if !registers.iter().skip(1).all(|r| r.is_single()) {
                return Err(anyhow!(
                    "Some of `{rspec}` registers are arrays and some are not"
                ));
            }
            let bitmasks = registers.iter().map(|r| r.bitmask()).collect::<Vec<_>>();
            let new_dim_index = registers
                .iter()
                .map(|r| {
                    let match_rspec = matchsubspec(&r.name, rspec).unwrap();
                    let Some((li, ri)) = spec_ind(match_rspec) else {
                        return Err(anyhow!(
                            "`{match_rspec}` contains no tokens or contains more than one token"
                        ));
                    };
                    Ok(r.name[li..r.name.len() - ri].to_string())
                })
                .collect::<Result<Vec<_>, _>>();
            let new_dim_index = new_dim_index?;
            if let Some(rspec1) = first.as_ref() {
                let len = registers.len();
                if dim != len {
                    return Err(anyhow!(
                        "{path}: registers cannot be collected into {cname} cluster. Different number of registers {rspec} ({len}) and {rspec1} ({dim})"
                    ));
                }
                if dim_index != new_dim_index {
                    return Err(anyhow!(
                        "{path}: registers cannot be collected into {cname} cluster. {rspec} and {rspec1} have different indeces"
                    ));
                }
            } else {
                dim = registers.len();
                dim_index = new_dim_index;
                offsets = registers
                    .iter()
                    .map(|r| r.address_offset)
                    .collect::<Vec<_>>();
                if dim > 1 {
                    dim_increment = offsets[1] - offsets[0];
                }
                first = Some(rspec);
            }
            if !check_offsets(&offsets, dim_increment) {
                return Err(anyhow!(
                    "{path}: registers cannot be collected into {cname} cluster. Different addressOffset increments in {rspec} registers"
                ));
            }
            if !bitmasks.iter().all(|&m| m == bitmasks[0]) {
                return Err(anyhow!(
                    "{path}: registers cannot be collected into {cname} cluster. Different bit masks in {rspec} registers"
                ));
            }
        }
        rdict.insert(rspec.to_string(), (rmod, registers));
    }
    if rdict.is_empty() {
        return Err(anyhow!(
            "{path}: registers cannot be collected into {cname} cluster. No matches found"
        ));
    }
    let address_offset = rdict
        .values()
        .map(|v| &v.1)
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
    let mut config = config.clone();
    config.update_fields = true;
    let cpath = path.new_cluster(cname);
    let mut cluster = if single {
        for (_, (rmod, mut registers)) in rdict.into_iter() {
            let mut reg = registers.swap_remove(0);
            let rmod = rmod.hash()?;
            reg.process(rmod, &cpath, &config)
                .with_context(|| format!("Processing register `{}`", reg.name))?;
            if let Some(name) = rmod.get_str("name")? {
                reg.name = name.into();
            }
            reg.address_offset -= address_offset;
            children.push(RegisterCluster::Register(reg));
        }

        cinfo.children(children).build(VAL_LVL)?.single()
    } else {
        for (rspec, (rmod, mut registers)) in rdict.into_iter() {
            let mut reg = registers.swap_remove(0);
            let rmod = rmod.hash()?;
            reg.process(rmod, &cpath, &config)
                .with_context(|| format!("Processing register `{}`", reg.name))?;
            reg.name = if let Some(name) = rmod.get_str("name")? {
                name.into()
            } else {
                let Some((li, ri)) = spec_ind(&rspec) else {
                    return Err(anyhow!(
                        "`{rspec}` contains no tokens or contains more than one token"
                    ));
                };
                format!("{}{}", &rspec[..li], &rspec[rspec.len() - ri..])
            };
            if let Some(desc) = rmod.get_str("description")? {
                reg.description = Some(desc.into());
            }
            reg.address_offset -= address_offset;
            if reg.address_offset >= dim_increment {
                return Err(anyhow!("Register {} addressOffset={} is out of cluster {cpath} dimIncrement = {dim_increment}", &reg.name, reg.address_offset));
            }
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
    cluster.pre_process(cmod, path, &config)?;
    cluster.post_process(cmod, path, &config)?;
    regs.insert(place, RegisterCluster::Cluster(cluster));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils;
    use anyhow::Result;
    use std::path::Path;

    #[test]
    fn cluster() -> Result<()> {
        test_utils::test_expected(Path::new("cluster"))
    }

    #[test]
    fn cross_cluster_derive() -> Result<()> {
        test_utils::test_expected(Path::new("cross_cluster_derive"))
    }
}
