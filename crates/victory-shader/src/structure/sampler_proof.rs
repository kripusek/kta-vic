use super::*;
use crate::gfx10::{Fields, Operand};
use crate::ir::{EXEC, Exit, Reg, VCC};
use crate::native::{HwStage, NativeProgram, RootParam, RootType};

/// The backward walk propagates
/// the lanes that still need a tuple word through actual EXEC guards and CFG
/// edges. It stops on unknown writes, uncovered lanes, and cycles.
struct Proof<'a> {
    p: &'a Program,
    st: Structured,
    entered: Vec<Bool>,
    wave_edges: Vec<(Bool, Bool)>,
    order_index: Vec<usize>,
    loop_entry_exec: HashMap<(usize, usize), Bool>,
    loops: Vec<LoopRegion>,
    defs: HashMap<Value, &'a Inst>,
    sampler_base: Value,
    sampler_sgpr: u8,
}

struct WordWalk<'a> {
    reg: Reg,
    word: u8,
    slot: &'a mut Option<u32>,
    remaining: &'a mut usize,
    visiting: HashSet<usize>,
    proved: HashMap<usize, Bool>,
}

struct LoopRegion {
    header: usize,
    entry: usize,
    exit_from: usize,
    blocks: HashSet<usize>,
    bound: Option<(Key, Bool)>,
}

fn natural_loops(p: &Program, lanes: &mut Lanes<'_>) -> Vec<LoopRegion> {
    let mut regions = Vec::new();
    for (header, block) in p.blocks.iter().enumerate() {
        for &latch in &block.preds {
            if lanes.index[latch] < lanes.index[header] {
                continue;
            }
            let mut blocks = HashSet::from([header, latch]);
            let mut queue = if latch == header {
                Vec::new()
            } else {
                vec![latch]
            };
            while let Some(b) = queue.pop() {
                for &pred in &p.blocks[b].preds {
                    if blocks.insert(pred) {
                        queue.push(pred);
                    }
                }
            }
            if blocks
                .iter()
                .any(|&b| matches!(p.blocks[b].exit, Exit::End | Exit::Indirect))
            {
                continue;
            }
            let region_blocks = &blocks;
            let entries: Vec<_> = blocks
                .iter()
                .flat_map(|&b| {
                    p.blocks[b]
                        .preds
                        .iter()
                        .enumerate()
                        .filter_map(move |(n, &pred)| {
                            (!region_blocks.contains(&pred)).then_some((b, n, pred))
                        })
                })
                .collect();
            let [(entry_to, entry_index, entry)] = entries.as_slice() else {
                continue;
            };
            if *entry_to != header
                || p.blocks[*entry].exit != Exit::Jump(header)
                || lanes.index[*entry] >= lanes.index[header]
            {
                continue;
            }
            let exits: Vec<_> = blocks
                .iter()
                .flat_map(|&b| {
                    p.blocks[b]
                        .exit
                        .successors()
                        .into_iter()
                        .filter_map(move |to| (!region_blocks.contains(&to)).then_some(b))
                })
                .collect();
            let [exit_from] = exits.as_slice() else {
                continue;
            };
            let phi_arg = |reg| {
                block
                    .phis
                    .iter()
                    .find(|p| p.reg == reg)
                    .map(|p| p.args[*entry_index])
            };
            // A loop with no EXEC phi can still preserve an unwritten VGPR.
            // A possible write requires a proved mask bound.
            let bound = phi_arg(Reg::S(EXEC))
                .zip(phi_arg(Reg::S(EXEC + 1)))
                .map(|key| (key, lanes.mask(key).0));
            regions.push(LoopRegion {
                header,
                entry: *entry,
                exit_from: *exit_from,
                blocks,
                bound,
            });
        }
    }
    regions
}

impl<'a> Proof<'a> {
    fn new(p: &'a Program, sampler_sgpr: u8) -> Option<Self> {
        let sampler_base = p
            .inputs
            .iter()
            .find(|&&(r, _)| r == Reg::S(sampler_sgpr))?
            .1;
        let defs = p
            .blocks
            .iter()
            .flat_map(|b| &b.insts)
            .flat_map(|inst| inst.writes.iter().map(move |&(_, v)| (v, inst)))
            .collect();
        let mut lanes = Lanes::new(p).ok()?;
        lanes.wave_edges = Some(vec![(Bool::TRUE, Bool::TRUE); p.blocks.len()]);
        loop {
            let execs = lanes.masks_in_order();
            let mut loop_entry_exec = HashMap::new();
            for (header, block) in p.blocks.iter().enumerate() {
                if waterfall(p, header).is_none() {
                    continue;
                }
                let first = &block.insts[0];
                let low = first.read(Reg::S(EXEC))?;
                let high = first.read(Reg::S(EXEC + 1))?;
                for (n, &pred) in block
                    .preds
                    .iter()
                    .enumerate()
                    .filter(|&(_, pred)| *pred != header)
                {
                    let incoming = |v| {
                        block
                            .phis
                            .iter()
                            .find(|phi| phi.value == v)
                            .map_or(v, |phi| phi.args[n])
                    };
                    let value = lanes.mask((incoming(low), incoming(high))).0;
                    loop_entry_exec.insert((header, pred), value);
                }
            }
            let loops = natural_loops(p, &mut lanes);
            let entered = lanes.paths().ok()?;
            let wrong = lanes.wrong_joins(&entered);
            if wrong.is_empty() {
                let wave_edges = lanes.wave_edges.take()?;
                let order_index = lanes.index.clone();
                return Some(Self {
                    p,
                    st: lanes.finish(&execs, &entered).ok()?,
                    entered,
                    wave_edges,
                    order_index,
                    loop_entry_exec,
                    loops,
                    defs,
                    sampler_base,
                    sampler_sgpr,
                });
            }
            lanes.unresolved.extend(wrong);
            lanes.reset();
        }
    }

    fn confined(
        &self,
        region: &LoopRegion,
        mask: Key,
        checked: &mut HashSet<Key>,
        steps: &mut usize,
    ) -> Option<()> {
        if mask == region.bound?.0 || !checked.insert(mask) {
            return Some(());
        }
        *steps = steps.checked_sub(1)?;
        let source =
            |inst: &Inst, code| Some((inst.read(Reg::S(code))?, inst.read(Reg::S(code + 1))?));
        let read_pair = |inst: &Inst, op: &Operand| match *op {
            Operand::Scalar { code, dwords: 2 } if code < 127 => source(inst, code),
            _ => None,
        };
        if let Some(inst) = self.defs.get(&mask.0) {
            if !std::ptr::eq(*inst, *self.defs.get(&mask.1)?)
                || !region.blocks.iter().any(|&b| {
                    self.p.blocks[b]
                        .insts
                        .iter()
                        .any(|i| std::ptr::eq(i, *inst))
                })
            {
                return None;
            }
            let exec_out = inst.write(Reg::S(EXEC)) == Some(mask.0)
                && inst.write(Reg::S(EXEC + 1)) == Some(mask.1);
            let explicit_dst = matches!(&inst.fields,
                Fields::Sop { dst: Some(Operand::Scalar { code, dwords: 2 }), .. }
                if *code < 127
                    && inst.write(Reg::S(*code)) == Some(mask.0)
                    && inst.write(Reg::S(*code + 1)) == Some(mask.1));
            let mut bounded = |next| self.confined(region, next, checked, steps);
            return match (&inst.fields, inst.name()) {
                // SDWA changes compared operands, not which lanes execute.
                (Fields::Valu(v), name)
                    if exec_out && name.starts_with("v_cmpx") && v.dpp.is_none() =>
                {
                    bounded(source(inst, EXEC)?)
                }
                (Fields::Sop { .. }, "s_and_saveexec_b64") if exec_out || explicit_dst => {
                    bounded(source(inst, EXEC)?)
                }
                (Fields::Sop { src, .. }, "s_mov_b64" | "s_andn2_b64") if explicit_dst => {
                    bounded(read_pair(inst, src.first()?)?)
                }
                (Fields::Sop { src, .. }, "s_and_b64") if explicit_dst => {
                    // Either operand can bound the result. A failed attempt
                    // may not contribute assumptions to the other.
                    if read_pair(inst, src.first()?).is_some_and(|first| {
                        self.confined(region, first, &mut checked.clone(), steps)
                            .is_some()
                    }) {
                        Some(())
                    } else {
                        self.confined(region, read_pair(inst, src.get(1)?)?, checked, steps)
                    }
                }
                _ => None,
            };
        }
        for &b in &region.blocks {
            let block = &self.p.blocks[b];
            let low = block.phis.iter().find(|phi| phi.value == mask.0);
            let high = block.phis.iter().find(|phi| phi.value == mask.1);
            if let (Some(low), Some(high)) = (low, high) {
                for (&lo, &hi) in low.args.iter().zip(&high.args) {
                    self.confined(region, (lo, hi), checked, steps)?;
                }
                return Some(());
            }
        }
        None
    }

    fn loop_preserves(&self, region: &LoopRegion, reg: Reg, steps: &mut usize) -> Option<()> {
        for &b in &region.blocks {
            for inst in &self.p.blocks[b].insts {
                if matches!(
                    inst.name(),
                    "s_call_b64" | "s_swappc_b64" | "s_setpc_b64" | "s_rfe_b64"
                ) {
                    return None;
                }
                if inst.write(reg).is_none() {
                    continue;
                }
                if inst.name() == "v_writelane_b32" {
                    return None;
                }
                let exec = (inst.read(Reg::S(EXEC))?, inst.read(Reg::S(EXEC + 1))?);
                self.confined(region, exec, &mut HashSet::new(), steps)?;
            }
        }
        Some(())
    }

    fn edge_condition(&mut self, from: usize, to: usize) -> Option<Bool> {
        let b = &mut self.st.bools;
        let edge = match self.st.exits[from] {
            LaneExit::Jump(target) if target == to => Bool::TRUE,
            LaneExit::Branch {
                taken, not_taken, ..
            } => match (taken == to, not_taken == to) {
                (true, true) => b.or(self.wave_edges[from].0, self.wave_edges[from].1),
                (true, false) => self.wave_edges[from].0,
                (false, true) => self.wave_edges[from].1,
                _ => return None,
            },
            _ => return None,
        };
        Some(b.and(self.entered[from], edge))
    }

    fn scalar_base(&self, mut v: Value) -> Option<Value> {
        for _ in 0..8 {
            let Some(def) = self.defs.get(&v) else {
                return Some(v);
            };
            let Fields::Sop { src, .. } = &def.fields else {
                return None;
            };
            let [Operand::Scalar { code, .. }] = src.as_slice() else {
                return None;
            };
            if def.name() != "s_mov_b32" {
                return None;
            }
            v = def.read(Reg::S(*code))?;
        }
        None
    }

    fn sampler_word(&self, inst: &Inst, word: u8) -> Option<u32> {
        let Fields::Valu(v) = &inst.fields else {
            return None;
        };
        let [Operand::Scalar { code, .. }] = v.src.as_slice() else {
            return None;
        };
        if inst.name() != "v_mov_b32" || !plain_valu(v) {
            return None;
        }
        let load = self.defs.get(&inst.read(Reg::S(*code))?)?;
        let Fields::Smem {
            data:
                Some(Operand::Scalar {
                    code: dst,
                    dwords: 4,
                }),
            base:
                Operand::Scalar {
                    code: base,
                    dwords: 2,
                },
            offset,
            soffset: Operand::Scalar { code: NULL, .. },
            ..
        } = load.fields
        else {
            return None;
        };
        if load.name() != "s_load_dwordx4"
            || dst.checked_add(word) != Some(*code)
            || self.scalar_base(load.read(Reg::S(base))?)? != self.sampler_base
            || offset < 0
            || offset % 16 != 0
        {
            return None;
        }
        let high = self.defs.get(&load.read(Reg::S(base + 1))?)?;
        let Fields::Sop { src, .. } = &high.fields else {
            return None;
        };
        if high.name() != "s_mov_b32" || src.as_slice() != [Operand::Const(128)] {
            return None;
        }
        Some(offset as u32 / 16)
    }

    fn prove_word(&mut self, block: usize, mut needed: Bool, walk: &mut WordWalk) -> Option<()> {
        // Earlier writes must cover every lane that a later branch might need.
        // Keeping later branch choices here multiplies equivalent predecessor
        // queries and can exhaust the decision diagram on sequential waterfalls.
        needed = self
            .st
            .bools
            .forget(needed, self.order_index[block] + 1, &mut Map::default());
        let prior = walk.proved.get(&block).copied().unwrap_or(Bool::FALSE);
        let unproved = self.st.bools.not(prior);
        needed = self.st.bools.and(needed, unproved);
        if needed == Bool::FALSE {
            return Some(());
        }
        *walk.remaining = walk.remaining.checked_sub(1)?;
        if let Some(region) = self.loops.iter().find(|region| region.exit_from == block) {
            let outside = region
                .bound
                .map_or(Bool::FALSE, |(_, mask)| self.st.bools.not(mask));
            let writes = region.blocks.iter().any(|&b| {
                self.p.blocks[b]
                    .insts
                    .iter()
                    .any(|i| i.write(walk.reg).is_some())
            });
            if (!writes || self.st.bools.implies(needed, outside))
                && self
                    .loop_preserves(region, walk.reg, walk.remaining)
                    .is_some()
            {
                let before = self.st.bools.forget(
                    needed,
                    self.order_index[region.header],
                    &mut Map::default(),
                );
                let before = if writes {
                    self.st.bools.and(before, outside)
                } else {
                    before
                };
                let before = self.st.bools.and(before, self.entered[region.entry]);
                self.prove_word(region.entry, before, walk)?;
                walk.proved.insert(block, self.st.bools.or(prior, needed));
                return Some(());
            }
        }
        let originally_needed = needed;
        if !walk.visiting.insert(block) {
            return None;
        }

        for (i, inst) in self.p.blocks[block].insts.iter().enumerate().rev() {
            if inst.write(walk.reg).is_none() {
                continue;
            }
            // A scalar-selected lane write ignores EXEC. It may clobber
            // one of the required lanes even in an entirely inactive arm.
            if inst.name() == "v_writelane_b32" {
                return None;
            }
            let guard = self.st.guards[block][i].unwrap_or(Bool::TRUE);
            let writes_needed = self.st.bools.and(needed, guard);
            if writes_needed != Bool::FALSE {
                let slot = self.sampler_word(inst, walk.word)?;
                if walk.slot.is_some_and(|recorded| recorded != slot) {
                    return None;
                }
                *walk.slot = Some(slot);
            }
            let excluded = self.st.bools.not(guard);
            needed = self.st.bools.and(needed, excluded);
            if needed == Bool::FALSE {
                walk.visiting.remove(&block);

                walk.proved
                    .insert(block, self.st.bools.or(prior, originally_needed));
                return Some(());
            }
        }

        let mut covered = Bool::FALSE;
        for &pred in &self.p.blocks[block].preds {
            // The incoming iteration can make different lane and branch
            // decisions even when its static SSA names match this iteration.
            if self.order_index[pred] >= self.order_index[block] {
                return None;
            }
            let edge = self.edge_condition(pred, block)?;
            let on_edge = self.st.bools.and(needed, edge);
            if on_edge != Bool::FALSE {
                covered = self.st.bools.or(covered, on_edge);
                self.prove_word(pred, on_edge, walk)?;
            }
        }
        walk.visiting.remove(&block);

        if !self.st.bools.overflow && self.st.bools.implies(needed, covered) {
            walk.proved
                .insert(block, self.st.bools.or(prior, originally_needed));
            Some(())
        } else {
            None
        }
    }

    fn verify_restored_exec(&self, header: usize) -> Option<()> {
        let block = &self.p.blocks[header];
        let Exit::Branch { not_taken, .. } = block.exit else {
            return None;
        };
        let restore = self.p.blocks[not_taken].insts.first()?;
        let Fields::Sop { src, .. } = &restore.fields else {
            return None;
        };
        let [Operand::Scalar { code, .. }] = src.as_slice() else {
            return None;
        };
        let low = self.defs.get(&restore.read(Reg::S(*code))?)?;
        let high = self.defs.get(&restore.read(Reg::S(*code + 1))?)?;
        let Fields::Sop { src, .. } = &low.fields else {
            return None;
        };
        if !std::ptr::eq(*low, *high)
            || low.name() != "s_mov_b64"
            || src.as_slice()
                != [Operand::Scalar {
                    code: EXEC,
                    dwords: 2,
                }]
        {
            return None;
        }
        for reg in [Reg::S(EXEC), Reg::S(EXEC + 1)] {
            let entry_value = block.insts[0].read(reg)?;
            let saved_value = low.read(reg)?;
            let phi = block.phis.iter().find(|p| p.value == entry_value);
            for (index, &pred) in block.preds.iter().enumerate() {
                if pred != header && phi.map_or(entry_value, |p| p.args[index]) != saved_value {
                    return None;
                }
            }
        }
        Some(())
    }

    /// A rewritten sampler load may use the original root SGPR pair only
    /// when that pair is unchanged before this loop. Later unrelated uses
    /// can repurpose it. Check only blocks that can reach this one.
    fn stable_base(&self, header: usize) -> Option<()> {
        let mut ancestors = HashSet::new();
        let mut pending = vec![header];
        while let Some(b) = pending.pop() {
            if ancestors.insert(b) {
                pending.extend(self.p.blocks[b].preds.iter().copied());
            }
        }
        let high = Reg::S(self.sampler_sgpr + 1);
        let low = Reg::S(self.sampler_sgpr);
        let mut high_defs = self
            .p
            .blocks
            .iter()
            .enumerate()
            .filter(|(b, _)| ancestors.contains(b))
            .flat_map(|(_, b)| &b.insts)
            .filter(|inst| inst.write(high).is_some());
        let init = high_defs.next()?;
        let Fields::Sop { src, .. } = &init.fields else {
            return None;
        };
        if high_defs.next().is_some()
            || init.name() != "s_mov_b32"
            || src.as_slice() != [Operand::Const(128)]
            || self
                .p
                .blocks
                .iter()
                .enumerate()
                .filter(|(b, _)| ancestors.contains(b))
                .flat_map(|(_, b)| &b.insts)
                .any(|inst| inst.write(low).is_some())
        {
            return None;
        }
        let init_value = init.write(high)?;
        // The sampled descriptor source already sees this initialization.
        // Ensure every path reaching the header passes through it as well.
        let defining_block = self.p.blocks.iter().position(|b| {
            b.insts
                .iter()
                .any(|inst| inst.write(high) == Some(init_value))
        })?;
        let mut reachable = HashSet::new();
        let mut pending = vec![0];
        while let Some(b) = pending.pop() {
            if b != defining_block && reachable.insert(b) {
                pending.extend(self.p.blocks[b].exit.successors());
            }
        }
        (!reachable.contains(&header) && defining_block != header).then_some(())
    }

    fn sampler_at(&mut self, header: usize, budget: &mut usize) -> Option<u32> {
        let vector = waterfall(self.p, header)?;
        self.verify_restored_exec(header)?;
        let mut slot = None;
        for &pred in &self.p.blocks[header].preds {
            if pred == header {
                continue;
            }
            if self.order_index[pred] >= self.order_index[header] {
                return None;
            }
            let entering = self.edge_condition(pred, header)?;
            let active = *self.loop_entry_exec.get(&(header, pred))?;
            let needed = self.st.bools.and(entering, active);
            if needed != Bool::FALSE {
                for word in 0..4 {
                    let mut walk = WordWalk {
                        reg: Reg::V(vector + word),
                        word,
                        slot: &mut slot,
                        remaining: budget,
                        visiting: HashSet::new(),
                        proved: HashMap::new(),
                    };
                    self.prove_word(pred, needed, &mut walk)?;
                }
            }
        }
        (!self.st.bools.overflow).then_some(slot).flatten()
    }
}

/// A bounded, whole-block match for the exact read/compare/EXEC loop idiom.
/// Operand checks use SSA identities and machine register roles, so changes
/// in register allocation need not change the proof.
fn waterfall(p: &Program, header: usize) -> Option<u8> {
    if !p.wave64 {
        return None;
    }
    let block = &p.blocks[header];
    let Exit::Branch { taken, not_taken } = block.exit else {
        return None;
    };
    if taken != header || !block.preds.contains(&header) {
        return None;
    }
    let [
        a,
        b,
        c,
        d,
        eq0,
        eq1,
        combine,
        save,
        load,
        sample,
        remove,
        branch,
    ] = block.insts.as_slice()
    else {
        return None;
    };
    let Fields::Image {
        sampler: Some(Operand::Scalar {
            code: sampler,
            dwords: 4,
        }),
        ..
    } = sample.fields
    else {
        return None;
    };
    if sampler.checked_add(3)? >= VCC {
        return None;
    }
    let mut vector = None;
    for (word, inst) in [a, b, c, d].into_iter().enumerate() {
        let Fields::Valu(v) = &inst.fields else {
            return None;
        };
        let ([Operand::Vector { reg, dwords: 1 }], Some(Operand::Scalar { code, dwords: 1 })) =
            (&v.src[..], v.dst)
        else {
            return None;
        };
        let word = word as u8;
        if inst.name() != "v_readfirstlane_b32"
            || !plain_valu(v)
            || sampler.checked_add(word) != Some(code)
            || vector.is_some_and(|base: u8| base.checked_add(word) != Some(*reg))
        {
            return None;
        }
        vector.get_or_insert(*reg);
    }
    let vector = vector?;
    let end = vector.checked_add(3)?;
    if block
        .insts
        .iter()
        .any(|inst| (vector..=end).any(|v| inst.write(Reg::V(v)).is_some()))
    {
        return None;
    }
    let pair = |inst: &Inst, start| {
        let Fields::Valu(v) = &inst.fields else {
            return None;
        };
        let Some(Operand::Scalar {
            code: dst,
            dwords: 2,
        }) = v.dst
        else {
            return None;
        };
        let [
            Operand::Scalar {
                code: scalar,
                dwords: 2,
            },
            Operand::Vector { reg, dwords: 2 },
        ] = &v.src[..]
        else {
            return None;
        };
        (inst.name() == "v_cmp_eq_u64"
            && plain_valu(v)
            && *scalar == sampler + start
            && *reg == vector + start)
            .then_some(dst)
    };
    let mask = pair(eq1, 2)?;
    if mask.checked_add(1)? >= VCC
        || pair(eq0, 0)? != VCC
        || sample.name() != "image_sample_lz"
        || load.name() != "s_load_dwordx8"
        || branch.name() != "s_cbranch_execnz"
    {
        return None;
    }
    let scalar = |reg| Operand::Scalar {
        code: reg,
        dwords: 2,
    };
    let sop = |inst: &Inst, name, dst, expect: &[Operand]| {
        matches!(&inst.fields, Fields::Sop { dst: Some(d), src, .. }
            if inst.name() == name && *d == scalar(dst) && src.as_slice() == expect)
    };
    let Fields::Sop {
        dst: Some(Operand::Scalar {
            code: saved,
            dwords: 2,
        }),
        ..
    } = save.fields
    else {
        return None;
    };
    if saved.checked_add(1)? >= VCC
        || !sop(combine, "s_and_b64", mask, &[scalar(VCC), scalar(mask)])
        || !sop(save, "s_and_saveexec_b64", saved, &[scalar(mask)])
        || !sop(remove, "s_xor_b64", EXEC, &[scalar(EXEC), scalar(saved)])
    {
        return None;
    }
    // The intervening SMEM load and sample must leave both the selected
    // EXEC and its saved copy intact until the XOR removes these lanes.
    let flows = |producer: &Inst, consumer: &Inst, code: u8| {
        [Reg::S(code), Reg::S(code + 1)].into_iter().all(|reg| {
            producer
                .write(reg)
                .is_some_and(|value| consumer.read(reg) == Some(value))
        })
    };
    if !flows(eq0, combine, VCC)
        || !flows(eq1, combine, mask)
        || !flows(combine, save, mask)
        || !flows(save, remove, saved)
        || !flows(save, sample, EXEC)
        || !flows(save, remove, EXEC)
        || !flows(remove, branch, EXEC)
    {
        return None;
    }
    for reg in [Reg::S(EXEC), Reg::S(EXEC + 1)] {
        let incoming = a.read(reg)?;
        if [b, c, d, eq0, eq1, save]
            .iter()
            .any(|inst| inst.read(reg) != Some(incoming))
        {
            return None;
        }
    }
    let restore = p.blocks[not_taken].insts.first()?;
    if !matches!(&restore.fields, Fields::Sop { dst: Some(Operand::Scalar { code: EXEC, dwords: 2 }), src, .. }
        if restore.name() == "s_mov_b64" && matches!(src.as_slice(), [Operand::Scalar { dwords: 2, .. }]))
    {
        return None;
    }
    // Only the comparisons and the sample may use the selected scalar words.
    // Check phi arguments too, including values carried to the loop exit.
    for (word, inst) in [a, b, c, d].into_iter().enumerate() {
        let reg = Reg::S(sampler + word as u8);
        let value = inst.write(reg)?;
        let compare = if word < 2 { eq0 } else { eq1 };
        if compare.read(reg) != Some(value) || sample.read(reg) != Some(value) {
            return None;
        }
        for q in &p.blocks {
            if q.phis.iter().any(|phi| phi.args.contains(&value)) {
                return None;
            }
            for user in &q.insts {
                if user.reads.iter().any(|&(_, v)| v == value)
                    && !std::ptr::eq(user, sample)
                    && !std::ptr::eq(user, if word < 2 { eq0 } else { eq1 })
                {
                    return None;
                }
            }
        }
    }
    Some(vector)
}

fn plain_valu(v: &crate::gfx10::Valu) -> bool {
    v.abs == 0
        && v.neg == 0
        && v.neg_hi == 0
        && v.opsel == 0
        && v.opsel_hi == 0
        && !v.clamp
        && v.omod == 0
        && v.sdwa.is_none()
        && v.dpp.is_none()
}

fn unused_scalar_pair(p: &Program, layout: &[RootParam]) -> Option<u8> {
    if p.blocks.iter().any(|b| b.exit == Exit::Indirect) {
        return None;
    }
    let used: HashSet<_> = p
        .inputs
        .iter()
        .map(|&(reg, _)| reg)
        .chain(
            p.blocks
                .iter()
                .flat_map(|b| &b.insts)
                .flat_map(|i| i.reads.iter().chain(&i.writes))
                .map(|&(reg, _)| reg),
        )
        .collect();
    // Mesh is a merged stage with eight system SGPRs. Also reserve the
    // entire declared root layout, including root words this shader never
    // reads. The scratch values exist only in the translator's guest IR.
    let free = |reg: u8| {
        !used.contains(&Reg::S(reg))
            && !layout.iter().any(|root| {
                let start = u32::from(root.user_data) + 8;
                (start..start.saturating_add(root.sgprs)).contains(&u32::from(reg))
            })
    };
    (32..=104)
        .rev()
        .find(|&r| r % 2 == 0 && free(r) && free(r + 1))
}

const MAX_BLOCKS: usize = 512;
const MAX_LOOPS: usize = 16;
const WALK_STEPS: usize = 1 << 19;

/// Attempt only a Wave64 mesh diagnostic at a matching sampler use. The
/// guest's sampler heap root (rather than a general resource root) is the
/// only permitted source for the shared tuple.
pub(crate) fn retry_mesh(
    native: &NativeProgram,
    p: &Program,
    offset: usize,
) -> Option<NativeProgram> {
    if native.stage != HwStage::Ms || !p.wave64 || p.blocks.len() > MAX_BLOCKS {
        return None;
    }
    let error_loop = p
        .blocks
        .iter()
        .enumerate()
        .find(|(b, block)| {
            block
                .insts
                .get(9)
                .is_some_and(|i| i.machine.offset == offset)
                && waterfall(p, *b).is_some()
        })?
        .0;
    let mut samplers = native
        .layout
        .iter()
        .filter(|root| root.root == RootType::SamplerHeap);
    let sampler = samplers.next()?;
    if samplers.next().is_some() || sampler.sgprs != 1 || sampler.user_data >= 97 {
        return None;
    }
    let sgpr = native.stage.user_sgpr(sampler.user_data);
    if sgpr % 2 != 0 {
        return None;
    }
    let mut rewritten = native.clone();
    let (count, repaired_error) = prove_and_rewrite(
        p,
        &mut rewritten.code,
        sgpr,
        &native.layout,
        Some(error_loop),
    )?;
    (count != 0 && repaired_error).then_some(rewritten)
}

fn prove_and_rewrite(
    p: &Program,
    code: &mut Vec<u8>,
    sampler_sgpr: u8,
    layout: &[RootParam],
    error_loop: Option<usize>,
) -> Option<(usize, bool)> {
    let mut proof = Proof::new(p, sampler_sgpr)?;
    let mut count = 0;
    let mut repaired_error = false;
    let mut scratch_used = None;
    let mut budget = WALK_STEPS;
    let original_bools = proof.st.bools.clone();
    for (index, block) in p.blocks.iter().enumerate() {
        if waterfall(p, index).is_none() {
            continue;
        }
        // All incoming constraints refer to nodes in the shared base.
        // Discard transient nodes generated by the previous query.
        proof.st.bools = original_bools.clone();
        let Some(slot) = proof.sampler_at(index, &mut budget) else {
            continue;
        };
        if p.blocks.iter().filter(|q| q.offset == block.offset).count() != 1 {
            continue;
        }
        let [a, b, c, d, eq0, eq1, ..] = block.insts.as_slice() else {
            continue;
        };
        // Source assembly positions must be contiguous. This rules out
        // unrelated instructions or branch entries hidden between them.
        if [a, b, c, d, eq0, eq1]
            .iter()
            .enumerate()
            .any(|(i, inst)| inst.machine.offset != a.machine.offset + i * 4)
        {
            continue;
        }
        let Fields::Valu(first) = &a.fields else {
            continue;
        };
        let Fields::Valu(hi_compare) = &eq1.fields else {
            continue;
        };
        let Some(Operand::Scalar { code: sampler, .. }) = first.dst else {
            continue;
        };
        let Some(Operand::Scalar { code: mask, .. }) = hi_compare.dst else {
            continue;
        };
        if eq1.machine.words.len() != 2 {
            continue;
        }
        let base = if proof.stable_base(index).is_some() {
            sampler_sgpr
        } else if let Some(scratch) = scratch_used.or_else(|| unused_scalar_pair(p, layout)) {
            scratch_used = Some(scratch);
            scratch
        } else {
            continue;
        };
        // The selected four words are uniformly from one descriptor.
        // Equality on active lanes gives precisely EXEC, including zeros
        // in inactive lanes. Leave the original EXEC operations, branch,
        // sampler-use site, and restoration in place.
        let sop = |dst: u8| 0xbe80_047eu32 | (u32::from(dst) << 16);
        let rewritten = [
            0xf408_0000 | (u32::from(sampler) << 6) | u32::from(base / 2),
            0xfa00_0000 | slot * 16,
            0xbf80_0000,
            0xbf80_0000,
            sop(VCC),
            sop(mask),
            0xbf80_0000,
        ];
        let dest = code.get_mut(a.machine.offset..a.machine.offset + 4 * rewritten.len())?;
        for (bytes, word) in dest.chunks_exact_mut(4).zip(rewritten) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        count += 1;
        repaired_error |= error_loop == Some(index);
        if count > MAX_LOOPS {
            return None;
        }
    }
    if let Some(scratch) = scratch_used {
        let prefix = [
            0xbe80_0300u32 | (u32::from(scratch) << 16) | u32::from(sampler_sgpr),
            0xbe80_0380u32 | (u32::from(scratch + 1) << 16),
        ];
        // Prepending both copies preserves relative code/data offsets. The
        // pair is unused in the original program and immutable afterwards.
        code.splice(..0, prefix.into_iter().flat_map(u32::to_le_bytes));
    }
    Some((count, repaired_error))
}

#[cfg(test)]
mod tests;
