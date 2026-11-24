use super::*;
// A Wave64 compare partitions the starting EXEC into two complementary
// masks. Each branch writes the same four sampler words. The sample loop
// compares the tuple, samples matching lanes, removes those lanes, and
// restores the saved EXEC on exit.
const MIXED_MASK: [u32; 37] = [
    0xbe99_0380, // s25 = 0, high part of the heap pointer
    0x7d88_0085, // v_cmp_gt_u32 vcc, 5, v0
    0xbe94_246a, // s_and_saveexec_b64 s[20:21], vcc
    0xbf88_0006, // skip first branch if EXEC empty
    0xf408_010c,
    0xfa00_0010, // s_load_dwordx4 s[4:7], s[24:25], 0x10
    0x7e08_0204,
    0x7e0a_0205, // v4/5 = s4/5, under first EXEC
    0x7e0c_0206,
    0x7e0e_0207, // v6/7 = s6/7
    0x8afe_7e14, // EXEC = saved & ~EXEC
    0xbf88_0006, // skip complementary arm if EXEC empty
    0xf408_010c,
    0xfa00_0010, // identical sampler in second arm
    0x7e08_0204,
    0x7e0a_0205,
    0x7e0c_0206,
    0x7e0e_0207,
    0xbefe_0414, // EXEC = s[20:21]
    0xbea0_047e, // s[32:33] = EXEC before loop
    0x7e38_0504,
    0x7e3a_0505, // first lane of v4/v5 into s28/s29
    0x7e3c_0506,
    0x7e3e_0507, // first lane of v6/v7 into s30/s31
    0x7dc4_081c, // equal low pair -> VCC
    0xd4e2_0006,
    0x0002_0c1e, // equal high pair -> s[6:7]
    0x8786_066a, // combine both comparisons
    0xbe86_2406, // save and restrict EXEC to matching lanes
    0xf40c_0200,
    0xfa00_0000, // load image descriptor from table
    0xf09c_0100,
    0x00e2_0a02, // image_sample_lz v10, v2, s[8:15], s[28:31]
    0x89fe_067e, // remove completed lanes
    0xbf89_fff1, // retry for the remaining lanes
    0xbefe_0420, // restore saved EXEC
    0xbf81_0000,
];

fn synthetic(words: &[u32]) -> (Program, Vec<u8>) {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    (crate::ir::build(&bytes, true, 16).unwrap(), bytes)
}

#[test]
fn loop_summary_requires_confinement_in_every_iteration() {
    fn fixture(restore_outer: bool, unmasked_write: bool) -> Program {
        let mut words = vec![
            0xbe99_0318, // use s24, the test's sampler root
            0x7d84_0080, // vcc = v0 == 0
            0xbe94_246a, // EXEC &= VCC, save outside EXEC in s20:s21
            0xbf82_0001, // jump to the loop header
            if restore_outer {
                0xbefe_0414
            } else {
                0xbefe_0408
            },
        ];
        if unmasked_write {
            words.extend([0xd761_0004, 0x0001_0081]); // v4[0] = 1, ignoring EXEC
        } else {
            words.extend([0x7e08_0281, 0xbf80_0000]); // under EXEC only
        }
        words.extend([
            0xbe88_047e, // save this iteration's EXEC in s8:s9
            0xbf06_8000, // s0 == 0
            0xbf85_fffa, // back to the latch at word 4
            0xbefe_0414, // restore outer EXEC after the loop
            0xbf81_0000,
        ]);
        synthetic(&words).0
    }
    for (widening, unmasked, preserved) in [
        (false, false, true),
        (true, false, false),
        (false, true, false),
    ] {
        let p = fixture(widening, unmasked);
        let proof = Proof::new(&p, 24).unwrap();
        let [region] = proof.loops.as_slice() else {
            panic!("synthetic loop should have a single entry and exit");
        };
        assert_eq!(
            proof
                .loop_preserves(region, Reg::V(4), &mut WALK_STEPS.clone())
                .is_some(),
            preserved,
            "widening={widening} unmasked={unmasked}"
        );
    }
}

#[test]
fn proves_one_sampler_under_complementary_exec() {
    let (p, mut code) = synthetic(&MIXED_MASK);
    let header = p.blocks.iter().position(|b| b.offset == 20 * 4).unwrap();
    let mut proof = Proof::new(&p, 24).unwrap();
    let mut budget = WALK_STEPS;
    assert_eq!(proof.sampler_at(header, &mut budget), Some(1));
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((1, false))
    );
    let old: Vec<_> = MIXED_MASK
        .iter()
        .copied()
        .flat_map(u32::to_le_bytes)
        .collect();
    // Outer mask partition remains byte-for-byte unchanged. The sampler
    // use, per-iteration EXEC changes, backedge, and restore remain too.
    assert_eq!(code[..20 * 4], old[..20 * 4]);
    assert_eq!(code[27 * 4..], old[27 * 4..]);
    let p = crate::ir::build(&code, true, 16).unwrap();
    let loop_block = p.blocks.iter().find(|b| b.offset == 20 * 4).unwrap();
    assert_eq!(loop_block.insts[0].name(), "s_load_dwordx4");
}

#[test]
fn loop_inside_an_arm_preserves_only_tuple_words_outside_its_writes() {
    let cases: &[(&[u32], bool)] = &[
        (&[0x7da4_0680, 0x7e08_0281], true), // narrow EXEC, then write v4
        (&[0x7e08_0281, 0x7da4_0680], true), // write v4, then narrow EXEC
        (&[0x7da4_0680, 0xd761_0009, 0x0001_0609], true), // writelane to unrelated v9
        (&[0x88fe_147e, 0x7e08_0281], false), // OR-widen EXEC before writing v4
        (&[0xbefe_0414, 0x7e08_0281], false), // restore the outer EXEC
        (&[0x7da4_0680, 0xbefe_03c1, 0x7e08_0281], false), // overwrite exec_lo
        (&[0x7da4_0680, 0x89fe_147e, 0x7e08_0281], false), // complement EXEC
        (&[0x7da4_0680, 0xd761_0004, 0x0001_0609], false), // writelane to v4
    ];
    for &(body, valid) in cases {
        let mut words = MIXED_MASK[..12].to_vec();
        words[11] = 0xbf88_0000 | (body.len() as u32 + 10);
        words.extend([0xf408_010c, 0xfa00_0010, 0xbea4_047e]); // load, save inner EXEC
        words.extend_from_slice(body);
        let back = 0x1_0000 - (body.len() as u32 + 2);
        words.extend([0xbf06_8000, 0xbf85_0000 | back, 0xbefe_0424]); // retry, then restore
        words.extend_from_slice(&MIXED_MASK[14..]);
        let (p, mut code) = synthetic(&words);
        let before = code.clone();
        assert_eq!(
            prove_and_rewrite(&p, &mut code, 24, &[], None),
            Some((usize::from(valid), false))
        );
        assert_eq!(code != before, valid);
    }
}

#[test]
fn nested_waterfall_requires_initialization_of_every_entering_lane() {
    let inner = &MIXED_MASK[1..MIXED_MASK.len() - 1];
    let mut nested = vec![
        MIXED_MASK[0],
        0x7d88_0287, // second, independent comparison
        0xbea2_246a, // save outer EXEC at s34:s35
        0xbf88_0000 | inner.len() as u32,
    ];
    nested.extend_from_slice(inner);
    nested.extend([0xbefe_0422, 0xbf81_0000]);
    let (p, mut code) = synthetic(&nested);
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((1, false))
    );

    // Re-enable lanes that never executed either sampler writer.
    nested[21] = 0xbefe_0422;
    let (p, mut code) = synthetic(&nested);
    let before = code.clone();
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((0, false))
    );
    assert_eq!(code, before);
}

#[test]
fn lane_writes_can_clobber_tuple_words_outside_exec() {
    // Lane 3 can be modified even when only the complementary lanes
    // are executing, or when EXEC is entirely empty.
    let write_lane_three = [0xd761_0004, 0x0001_0609];
    let mut other_arm = MIXED_MASK.to_vec();
    other_arm[11] = 0xbf88_0008;
    other_arm.splice(14..14, write_lane_three);
    let mut empty = MIXED_MASK.to_vec();
    empty.splice(
        18..18,
        [0xbefe_0480, write_lane_three[0], write_lane_three[1]],
    );
    for words in [other_arm, empty] {
        let (p, mut code) = synthetic(&words);
        let original = code.clone();
        assert_eq!(
            prove_and_rewrite(&p, &mut code, 24, &[], None),
            Some((0, false))
        );
        assert_eq!(code, original);
    }
}

#[test]
fn refuses_waterfall_when_the_texture_load_overwrites_saved_exec() {
    let mut words = MIXED_MASK;
    words[13] = 0xfa00_0020; // the other lanes carry a different tuple
    words[18] = 0xbefe_046a; // enter the loop with only VCC's lanes active
    words[28] = 0xbe88_2406; // save EXEC in s[8:9]
    words[33] = 0x89fe_087e; // remove against s[8:9], now texture words
    let (p, mut code) = synthetic(&words);
    let before = code.clone();
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((0, false))
    );
    assert_eq!(code, before);
}

#[test]
fn refuses_incomplete_sampler_or_control_proofs() {
    let cases = [
        (13, 0xfa00_0020), // different slot in the second arm
        (17, 0xbf80_0000), // one word left uninitialized in the second arm
        (33, 0x88fe_067e), // OR instead of removing selected lanes
        (34, 0xbf80_0000), // no backedge
        (35, 0xbefe_0406), // restores the per-iteration snapshot instead of outer EXEC
    ];
    for (index, word) in cases {
        let mut words = MIXED_MASK;
        words[index] = word;
        let (p, mut code) = synthetic(&words);
        let before = code.clone();
        assert_eq!(
            prove_and_rewrite(&p, &mut code, 24, &[], None),
            Some((0, false)),
            "word {index}"
        );
        assert_eq!(code, before);
    }
    let mut escaping = MIXED_MASK.to_vec();
    escaping.insert(36, 0xbe80_031c); // s0 reads the selected sampler word
    let (p, mut code) = synthetic(&escaping);
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((0, false))
    );
}

#[test]
fn wave_exec_branch_may_write_lanes_the_lane_cfg_excluded() {
    let mut words = MIXED_MASK[..4].to_vec();
    words[3] = 0xbf88_000e; // If no lane belongs to X, jump to the alternate arm.
    words.extend_from_slice(&MIXED_MASK[4..10]); // sampler A for X.
    words.push(0x89fe_147e); // EXEC = EXEC XOR saved EXEC: reactivate !X.
    words.extend_from_slice(&MIXED_MASK[4..10]);
    words[12] = 0xfa00_0020; // sampler B for !X, in the same scalar branch.
    words.push(0xbf82_0007); // Unconditional branch over the alternate arm.
    words.push(0xbefe_0414); // Restore EXEC in the alternate arm.
    words.extend_from_slice(&MIXED_MASK[4..10]); // sampler A when all lanes are !X.
    words.extend_from_slice(&MIXED_MASK[18..]); // Restore EXEC and sample by tuple.
    let (p, mut code) = synthetic(&words);
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((0, false))
    );
}

fn translate_local(
    c: &crate::Container,
    native: &crate::NativeProgram,
    code: &[u8],
) -> Result<crate::emit::HostShader, ShaderError> {
    let mut native = native.clone();
    native.code = code.to_vec();
    let native = &native;
    let p = crate::ir::build(code, native.wave64, native.vgprs)?;
    let st = structure(&p)?;
    let io = crate::io::raise(&p, native)?;
    let res = crate::resource::track(&p, &native.layout, native.stage)?;
    crate::emit::emit(c, native, &p, &st, &io, &res)
}

#[test]
fn scratch_pair_excludes_full_smem_ranges_and_unread_abi_words() {
    // Even if this shader only consumes the first dword, the decoded
    // scalar load writes all eight SGPRs of s[96:103].
    let (p, _) = synthetic(&[0xf40c_1800, 0xfa00_0000, 0xbf81_0000]);
    assert_eq!(unused_scalar_pair(&p, &[]), Some(104));
    let root = RootParam {
        root: RootType::Constants,
        table_offset_dwords: 0,
        sgprs: 2,
        base_register: 0,
        user_data: 96,
        slot: 0,
    };
    assert_eq!(unused_scalar_pair(&p, &[root]), Some(94));
}

#[cfg(all(windows, feature = "agility"))]
#[test]
fn warp_sampler_join_outputs_each_lane() {
    use crate::emit::host::{Dispatch, Host, Root, View};
    use crate::native::{ComputeIo, RootParam, StageIo};
    use windows::Win32::Graphics::Direct3D12::*;
    let host = Host::new(true).expect("explicit Microsoft software WARP");
    let cbytes = crate::dxbc::build(&[]);
    let container = crate::Container::parse(&cbytes).unwrap();

    let make = |second_slot: u32, coordinate: f32, active: u32| {
        let mut words = vec![
            0xbe81_0380, // table high
            0x7e04_02ff,
            coordinate.to_bits(), // v2 = sample u
            0x7e06_0280,          // v3 = sample v = 0
        ];
        words.extend([0x7d88_0080 | active, 0xbeaa_246a]); // EXEC = lanes < active
        let mut waterfall = MIXED_MASK;
        waterfall[13] = 0xfa00_0000 | (second_slot * 16);
        words.extend_from_slice(&waterfall[..waterfall.len() - 1]);
        words.extend([
            0x3404_0082, // lshlrev v2, 2, v0 (byte offset per lane)
            0xf408_0300,
            0xfa00_0020, // UAV descriptor at table + 32
            0xe070_1000,
            0x8003_0a02, // store v10 to UAV at v2
            0xbf81_0000,
        ]);
        synthetic(&words)
    };
    let native = |code: Vec<u8>| crate::NativeProgram {
        stage: HwStage::Cs,
        wave64: true,
        vgprs: 16,
        layout: vec![
            RootParam {
                root: RootType::Table,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 0,
                slot: 0,
            },
            RootParam {
                root: RootType::SamplerHeap,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 24,
                slot: 0,
            },
        ],
        used: Vec::new(),
        io: StageIo::Cs(ComputeIo {
            rsrc2: 50,
            threads: [64, 1, 1],
        }),
        code,
    };
    let sample = |shader: &crate::emit::HostShader| {
        let mut shader = shader.clone();
        let mut validator = crate::emit::validator::Validator::load(
            crate::emit::validator::Validator::default_path().as_deref(),
        )
        .expect("Windows SDK DXIL validator");
        validator.validate(&mut shader.dxil).unwrap();
        let (root, pso) = host
            .compute_pipeline(&shader)
            .expect("fixture WARP pipeline");
        let sampler = |mode| D3D12_SAMPLER_DESC {
            Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
            AddressU: mode,
            AddressV: mode,
            AddressW: mode,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let wrap = sampler(D3D12_TEXTURE_ADDRESS_MODE_WRAP);
        let clamp = sampler(D3D12_TEXTURE_ADDRESS_MODE_CLAMP);
        let out = host
            .dispatch(
                &root,
                &pso,
                &Dispatch {
                    roots: vec![(0, Root::Constants(vec![0]))],
                    views: &[
                        View::Texture(2, 1, &[[0.25, 0., 0., 1.], [0.75, 0., 0., 1.]]),
                        View::RawUav(64),
                    ],
                    samplers: &[wrap, wrap, clamp],
                    groups: [1, 1, 1],
                },
            )
            .expect("fixture WARP dispatch");
        out[0]
            .iter()
            .copied()
            .map(f32::from_bits)
            .collect::<Vec<_>>()
    };
    let translate = |n: &crate::NativeProgram, mode| {
        let p = crate::ir::build(&n.code, n.wave64, n.vgprs).unwrap();
        let st = structure(&p).unwrap();
        let io = crate::io::raise(&p, n).unwrap();
        let res = crate::resource::track(&p, &n.layout, n.stage).unwrap();
        crate::emit::emit_with(
            &container,
            n,
            &p,
            &st,
            &io,
            &res,
            crate::emit::TranslateOptions { wave64: mode },
        )
        .unwrap()
    };
    let direct = |second_slot: u32, active: u32| {
        let mut words = vec![
            0xbe81_0380, // table high
            0x7e04_02ff,
            1.25f32.to_bits(), // v2 = sample u
            0x7e06_0280,       // v3 = sample v
            0xf40c_0200,
            0xfa00_0000, // image descriptor
            0x7d88_0080 | active,
            0xbeaa_246a, // start EXEC with only selected lanes
            0xbe99_0380, // sampler heap high
            0x7d88_0085, // VCC = lane < 5
            0xbe94_246a, // EXEC &= VCC, save full mask
            0xbf88_0004, // skip empty first path
            0xf408_070c,
            0xfa00_0010, // s[28:31] = sampler slot 1
            0xf09c_0100,
            0x00e2_0a02, // sample
            0x8afe_7e14, // EXEC = saved & ~EXEC
            0xbf88_0004, // skip empty complementary path
            0xf408_070c,
            0xfa00_0000 | second_slot * 16,
            0xf09c_0100,
            0x00e2_0a02,
            0xbefe_0414, // restore all lanes
        ];
        words.extend([
            0x3404_0082,
            0xf408_0300,
            0xfa00_0020,
            0xe070_1000,
            0x8003_0a02,
            0xbf81_0000,
        ]);
        native(synthetic(&words).1)
    };
    use crate::emit::Wave64Mode;
    let mut modes = vec![Wave64Mode::Emulate];
    if host.supports_wave64() {
        modes.push(Wave64Mode::Native);
    }
    if host.supports_wave32() {
        modes.push(Wave64Mode::Wave32);
    }
    for active in [3, 47, 64] {
        let (p, mut code) = make(1, 1.25, active);
        assert_eq!(
            prove_and_rewrite(&p, &mut code, 24, &[], None),
            Some((1, false))
        );
        let n = native(code);
        for &mode in &modes {
            let out = sample(&translate(&n, mode));
            let positive_ref = sample(&translate(&direct(1, active), mode));
            assert_eq!(&out[..active as usize], &[0.25; 64][..active as usize]);
            assert_eq!(&out[active as usize..], &[0.; 64][active as usize..]);
            assert_eq!(out, positive_ref);
            let adversarial_ref = sample(&translate(&direct(2, active), mode));
            let split = active.min(5) as usize;
            assert_eq!(&adversarial_ref[..split], &[0.25; 5][..split]);
            assert!(
                adversarial_ref[split..active as usize]
                    .iter()
                    .all(|&v| v == 0.75)
            );
            assert_eq!(
                &adversarial_ref[active as usize..],
                &[0.; 64][active as usize..]
            );
        }
    }

    let (p, mut adversary) = make(2, 1.25, 64);
    let unchanged = adversary.clone();
    assert_eq!(
        prove_and_rewrite(&p, &mut adversary, 24, &[], None),
        Some((0, false))
    );
    assert_eq!(adversary, unchanged);
}

#[cfg(all(windows, feature = "agility"))]
#[test]
fn warp_sampler_join_mesh_rasterization() {
    use crate::emit::host::{Draw, Host, Root};
    use crate::native::{MeshIo, StageIo};
    use windows::Win32::Graphics::Direct3D12::*;
    let host = Host::new(true).expect("explicit Microsoft software WARP");
    let signature = |system: u32, name: &[u8]| {
        let mut bytes: Vec<u8> = [1, 8, 0, 40, 0, system, 3, 0, 15, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        bytes.extend_from_slice(name);
        bytes
    };
    let position = signature(1, b"SV_Position\0");
    let color = signature(64, b"SV_Target\0");
    let d = crate::dxbc::build(&[(crate::dxbc::FourCC::OSG1, &position)]);
    let pd = crate::dxbc::build(&[
        (crate::dxbc::FourCC::ISG1, &position),
        (crate::dxbc::FourCC::OSG1, &color),
    ]);
    let c = crate::Container::parse(&d).unwrap();
    let pc = crate::Container::parse(&pd).unwrap();
    let compile = |native: &crate::NativeProgram| {
        let c = if native.stage == HwStage::Ms { &c } else { &pc };
        let p = crate::ir::build(&native.code, native.wave64, native.vgprs).unwrap();
        let st = structure(&p).unwrap();
        let io = crate::io::raise(&p, native).unwrap();
        let res = crate::resource::track(&p, &native.layout, native.stage).unwrap();
        let mut shader =
            crate::emit::emit_for(c, native, &p, &st, &io, &res, host.wave_range()).unwrap();
        let mut validator = crate::emit::validator::Validator::load(
            crate::emit::validator::Validator::default_path().as_deref(),
        )
        .unwrap();
        validator.validate(&mut shader.dxil).unwrap();
        shader
    };
    let mesh = |code| crate::NativeProgram {
        stage: HwStage::Ms,
        wave64: true,
        vgprs: 16,
        layout: vec![
            RootParam {
                root: RootType::Table,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 8,
                slot: 0,
            },
            RootParam {
                root: RootType::SamplerHeap,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 16,
                slot: 0,
            },
        ],
        used: Vec::new(),
        io: StageIo::Ms(MeshIo {
            outputs: vec![],
            clip_cntl: 0,
            threads: 64,
            max_vertices: 3,
            max_primitives: 1,
            topology: 2,
            lds_bytes: 0,
        }),
        code,
    };
    let prefix = [
        0xbe91_0380,
        0x7e04_02ff,
        1.25f32.to_bits(),
        0x7e06_0280,
        0xd765_0000,
        0x0001_00c1, // lane id low
        0xd766_0000,
        0x0002_00c1, // lane id high
    ];
    let suffix = vec![
        0x7e16_02ff,
        (-1.0f32).to_bits(), // x
        0x7e18_02ff,
        (-1.0f32).to_bits(), // y
        0x7e1a_02ff,
        1.0f32.to_bits(), // w
        0x7d84_0082,
        0xbe94_246a, // select lane 2
        0x7e16_02ff,
        3.0f32.to_bits(), // x = 3 in lane 2
        0xbefe_0414,      // restore
        0x7d84_0081,
        0xbe94_246a, // select lane 1
        0x7e18_02ff,
        3.0f32.to_bits(), // y = 3 in lane 1
        0xbefe_0414,      // restore
        0x7d88_0083,
        0xbe94_246a, // only vertex lanes 0..2
        0xf800_08cf,
        0x0d0a_0c0b, // exp pos0 x, y, sampleZ, w
        0xbefe_0414,
        0x7e1c_02ff,
        0x0020_0400, // packed triangle indices 0,1,2
        0x7d84_0080,
        0xbe94_246a, // lane 0 primitive only
        0xf800_0941,
        0x0000_000e, // exp prim v14
        0xbefe_0414,
        0xbf81_0000,
    ];
    let mut waterfall = MIXED_MASK;
    waterfall[29] = 0xf40c_0208;
    waterfall[1] = 0x7d88_0081; // lane 0 in first branch; vertices 1 and 2 in complementary arm.
    let mut code = Vec::from(prefix);
    code.extend_from_slice(&waterfall[..waterfall.len() - 1]);
    code.extend_from_slice(&suffix);
    let (p, mut rewritten) = synthetic(&code);
    assert_eq!(
        prove_and_rewrite(&p, &mut rewritten, 24, &[], None),
        Some((1, false))
    );
    let mut adversarial = code.clone();
    adversarial[prefix.len() + 13] = 0xfa00_0020;
    let (p, mut unrewritten) = synthetic(&adversarial);
    let original = unrewritten.clone();
    assert_eq!(
        prove_and_rewrite(&p, &mut unrewritten, 24, &[], None),
        Some((0, false))
    );
    assert_eq!(unrewritten, original);

    let reference = |second_slot: u32| {
        let mut code = Vec::from(prefix);
        code.extend([
            0xf40c_0208,
            0xfa00_0000,
            0xbe99_0380,
            0x7d88_0081,
            0xbe94_246a,
            0xbf88_0004,
            0xf408_070c,
            0xfa00_0010,
            0xf09c_0100,
            0x00e2_0a02,
            0x8afe_7e14,
            0xbf88_0004,
            0xf408_070c,
            0xfa00_0000 | second_slot * 16,
            0xf09c_0100,
            0x00e2_0a02,
            0xbefe_0414,
        ]);
        code.extend_from_slice(&suffix);
        synthetic(&code).1
    };
    let ps = compile(&crate::NativeProgram {
        stage: HwStage::Ps,
        wave64: true,
        vgprs: 16,
        layout: vec![],
        used: vec![],
        io: StageIo::Ps {
            inputs: vec![],
            input_addr: 1 << 10,
            db_shader_control: 0,
            patches: vec![],
        },
        code: [
            0x7e02_02ffu32,
            1.0f32.to_bits(),
            0xf800_080f,
            0x0100_0000,
            0xbf81_0000,
        ]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect(),
    });
    let pixels = |code| {
        let ms = compile(&mesh(code));
        let (root, pso) = host.mesh_pipeline(&ms, &ps).expect("WARP mesh pipeline");
        let sampler = |mode| D3D12_SAMPLER_DESC {
            Filter: D3D12_FILTER_MIN_MAG_MIP_POINT,
            AddressU: mode,
            AddressV: mode,
            AddressW: mode,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let wrap = sampler(D3D12_TEXTURE_ADDRESS_MODE_WRAP);
        let clamp = sampler(D3D12_TEXTURE_ADDRESS_MODE_CLAMP);
        host.draw(
            &root,
            &pso,
            &Draw {
                roots: vec![(0, Root::Constants(vec![0]))],
                textures: &[(2, 1, &[[0.25, 0., 0., 1.], [0.75, 0., 0., 1.]])],
                samplers: &[wrap, wrap, clamp],
                vertices: &[],
                stride: 0,
                size: (4, 4),
                mesh_groups: 1,
            },
        )
        .expect("DispatchMesh and readback")
    };
    let actual = pixels(rewritten);
    let expected = pixels(reference(1));
    let different = pixels(reference(2u32));
    assert_eq!(actual, expected);
    assert_eq!(actual, [[0.25, 0.25, 0.25, 1.0]; 16]);
    let depths = [
        0.5, 0.5625, 0.625, 0.6875, 0.4375, 0.5, 0.5625, 0.625, 0.375, 0.4375, 0.5, 0.5625, 0.3125,
        0.375, 0.4375, 0.5,
    ];
    assert_eq!(different, depths.map(|z| [z, z, z, 1.0]));
}

#[test]
fn complementary_sampler_program_reaches_dxil_validation() {
    use crate::native::{ComputeIo, RootParam, StageIo};

    let mut words = Vec::from([0xbe81_0380, 0x7e04_0280]); // table address high / sample coordinate
    words.extend(MIXED_MASK);
    words.pop(); // replace endpgm with an observable store before ending
    words.extend([
        0xf408_0300,
        0xfa00_0020, // UAV descriptor at table + 32
        0xe070_1000,
        0x8003_0a02, // store sample v10 to UAV offset v2 = 0
        0xbf81_0000,
    ]);
    let (p, mut code) = synthetic(&words);
    let container = crate::dxbc::build(&[]);
    let c = crate::Container::parse(&container).unwrap();
    let native = crate::NativeProgram {
        stage: HwStage::Cs,
        wave64: true,
        vgprs: 16,
        layout: vec![
            RootParam {
                root: RootType::Table,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 0,
                slot: 0,
            },
            RootParam {
                root: RootType::SamplerHeap,
                table_offset_dwords: 0,
                sgprs: 1,
                base_register: 0,
                user_data: 24,
                slot: 0,
            },
        ],
        used: Vec::new(),
        io: StageIo::Cs(ComputeIo {
            rsrc2: 50,
            threads: [64, 1, 1],
        }),
        code: code.clone(),
    };
    assert!(matches!(
        translate_local(&c, &native, &code),
        Err(ShaderError::Unsupported {
            what: "a descriptor neither loaded nor built",
            ..
        })
    ));
    assert_eq!(
        prove_and_rewrite(&p, &mut code, 24, &[], None),
        Some((1, false))
    );
    let shader = translate_local(&c, &native, &code).unwrap();
    #[cfg(windows)]
    if let Ok(mut validator) = crate::emit::validator::Validator::load(
        crate::emit::validator::Validator::default_path().as_deref(),
    ) {
        validator.validate(&mut shader.dxil.clone()).unwrap();
    }
    assert!(!shader.dxil.is_empty());
}
