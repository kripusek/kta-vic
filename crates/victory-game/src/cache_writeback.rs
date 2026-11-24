//! Native cache-line writeback for Series 1.0.2.1 on x86 hosts without CLWB.

use crate::SERIES_1021;

// Each RVA is an instruction boundary and the target of its loop's back edge.
const SITES: &[(u32, &[u8])] = &[
    (0x04e4_b680, &[0x66, 0x0f, 0xae, 0x31]),
    (0x04ec_c84a, &[0x66, 0x41, 0x0f, 0xae, 0x31]),
    (0x050f_75f5, &[0x66, 0x41, 0x0f, 0xae, 0x30]),
    (0x0532_f1b0, &[0x66, 0x0f, 0xae, 0x30]),
    (0x0532_f1f0, &[0x66, 0x0f, 0xae, 0x30]),
    (0x0532_f230, &[0x66, 0x0f, 0xae, 0x30]),
    (0x0532_f280, &[0x66, 0x0f, 0xae, 0x30]),
    (0x0532_f2ba, &[0x66, 0x0f, 0xae, 0x30]),
];

/// Selects the native alternate instruction only when a host needs and supports it.
pub fn translate(hash: &str, image: &mut [u8], file_offset: impl Fn(u32) -> Option<u32>) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::{__cpuid, __cpuid_count};
        if __cpuid(0).eax >= 7 {
            return translate_for_cpu(hash, __cpuid_count(7, 0).ebx, image, file_offset);
        }
    }
    0
}

fn translate_for_cpu(
    hash: &str,
    features: u32,
    image: &mut [u8],
    file_offset: impl Fn(u32) -> Option<u32>,
) -> usize {
    const CLFLUSHOPT: u32 = 1 << 23;
    const CLWB: u32 = 1 << 24;
    if hash != SERIES_1021 || features & (CLFLUSHOPT | CLWB) != CLFLUSHOPT {
        return 0;
    }
    let mut translated = 0;
    for &(rva, expected) in SITES {
        let Some(offset) = file_offset(rva).map(|offset| offset as usize) else {
            continue;
        };
        let Some(instruction) = image.get_mut(offset..offset + expected.len()) else {
            continue;
        };
        if instruction == expected {
            // x86 CLWB and CLFLUSHOPT have the same writeback and fence
            // requirements. CLFLUSHOPT additionally invalidates the cache line.
            // Changing /6 to /7 preserves the address, length and every register.
            instruction[expected.len() - 1] |= 0x08;
            translated += 1;
        }
    }
    translated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_only_confirmed_sites_on_eligible_hosts() {
        let original = [0x66, 0x0f, 0xae, 0x31, 0x66, 0x41, 0x0f, 0xae, 0x31];
        let offset = |rva| match rva {
            0x04e4_b680 => Some(0),
            0x04ec_c84a => Some(4),
            _ => None,
        };
        let mut image = original;
        for (hash, features) in [
            ("different build", 1 << 23),
            (SERIES_1021, 0),
            (SERIES_1021, 1 << 24),
            (SERIES_1021, (1 << 23) | (1 << 24)),
        ] {
            assert_eq!(translate_for_cpu(hash, features, &mut image, offset), 0);
            assert_eq!(image, original);
        }
        assert_eq!(
            translate_for_cpu(SERIES_1021, 1 << 23, &mut image, offset),
            2
        );
        assert_eq!(
            image,
            [0x66, 0x0f, 0xae, 0x39, 0x66, 0x41, 0x0f, 0xae, 0x39]
        );
        image[0] = 0xaa;
        assert_eq!(
            translate_for_cpu(SERIES_1021, 1 << 23, &mut image, offset),
            0
        );
    }
}
