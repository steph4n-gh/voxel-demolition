//! ============================================================================
//! EMPIRICAL CHALLENGER TEST SUITE: MILESTONE G5-M3
//! ADVERSARIAL STRESS-TESTING: STACKSTR<N>, HUD FORMATTING & FIT_TEXT_TO_WIDTH
//! Target: tests/g5_m3_challenger_2_stress.rs
//! ============================================================================

#![allow(clippy::new_without_default)]
#![allow(clippy::write_literal)]
#![allow(unused_imports, dead_code)]

use voxel_demolition::*;

use std::fmt::Write;
use ui::{char_glyph_width, fit_text_to_width, fit_text_to_width_stack, StackStr};

// ============================================================================
// SUITE 1: BUFFER CAPACITY TRUNCATION & BOUNDARY EDGE CASES
// ============================================================================

#[test]
fn test_adversarial_stack_str_zero_capacity() {
    let mut s = StackStr::<0>::new();
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert_eq!(s.capacity(), 0);
    assert_eq!(s.remaining(), 0);
    assert_eq!(s.as_str(), "");
    assert_eq!(&*s, "");

    // Push into zero-capacity buffer must never panic, write bytes, or overflow
    s.push_str("HELLO WORLD");
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
    assert_eq!(s.as_str(), "");

    // write! into zero-capacity buffer
    let res = write!(s, "Formatted {} {}", 42, "text");
    assert!(res.is_ok());
    assert_eq!(s.len(), 0);
    assert_eq!(s.as_str(), "");

    s.clear();
    assert_eq!(s.len(), 0);
    assert_eq!(s.as_str(), "");
}

#[test]
fn test_adversarial_stack_str_single_byte_capacity() {
    let mut s = StackStr::<1>::new();
    assert_eq!(s.capacity(), 1);
    assert_eq!(s.remaining(), 1);

    // Single ASCII char fits
    s.push_str("X");
    assert_eq!(s.len(), 1);
    assert_eq!(s.as_str(), "X");
    assert_eq!(s.remaining(), 0);

    // Pushing more does nothing
    s.push_str("Y");
    assert_eq!(s.len(), 1);
    assert_eq!(s.as_str(), "X");

    s.clear();
    assert_eq!(s.len(), 0);
    assert_eq!(s.remaining(), 1);

    // Pushing a 2-byte char into 1-byte capacity buffer must NOT write 1st byte
    s.push_str("°"); // 2 bytes: C2 B0
    assert_eq!(s.len(), 0, "1-byte capacity must reject 2-byte UTF-8 char");
    assert_eq!(s.as_str(), "");

    // Pushing a 3-byte char
    s.push_str("•"); // 3 bytes: E2 80 A2
    assert_eq!(s.len(), 0, "1-byte capacity must reject 3-byte UTF-8 char");
    assert_eq!(s.as_str(), "");

    // Pushing a 4-byte emoji
    s.push_str("🚀"); // 4 bytes: F0 9F 9A 80
    assert_eq!(s.len(), 0, "1-byte capacity must reject 4-byte UTF-8 emoji");
    assert_eq!(s.as_str(), "");
}

#[test]
fn test_adversarial_stack_str_exact_capacity_and_huge_overshoot() {
    // Exact capacity match
    let mut s = StackStr::<16>::new();
    s.push_str("0123456789ABCDEF");
    assert_eq!(s.len(), 16);
    assert_eq!(s.remaining(), 0);
    assert_eq!(s.as_str(), "0123456789ABCDEF");

    // Overflow by 1 byte
    s.push_str("!");
    assert_eq!(s.len(), 16);
    assert_eq!(s.as_str(), "0123456789ABCDEF");

    // Overflow by 100,000 bytes
    let huge = "Z".repeat(100_000);
    s.push_str(&huge);
    assert_eq!(s.len(), 16);
    assert_eq!(s.as_str(), "0123456789ABCDEF");

    // Clear and write huge string directly
    s.clear();
    assert_eq!(s.len(), 0);
    s.push_str(&huge);
    assert_eq!(s.len(), 16);
    assert_eq!(s.as_str(), "ZZZZZZZZZZZZZZZZ");
}

#[test]
fn test_adversarial_stack_str_incremental_byte_by_byte_fill() {
    let mut s = StackStr::<8>::new();
    for i in 0..8 {
        assert_eq!(s.remaining(), 8 - i);
        write!(s, "{}", i).unwrap();
        assert_eq!(s.len(), i + 1);
    }
    assert_eq!(s.as_str(), "01234567");
    assert_eq!(s.remaining(), 0);

    // Attempting further writes
    write!(s, "8").unwrap();
    write!(s, "9").unwrap();
    assert_eq!(s.len(), 8);
    assert_eq!(s.as_str(), "01234567");
}

// ============================================================================
// SUITE 2: MULTI-BYTE UTF-8 SEQUENCE SOUNDNESS & BOUNDARY TRUNCATION
// ============================================================================

#[test]
fn test_adversarial_utf8_2byte_rune_truncation_boundaries() {
    // Degree symbol '°' is 2 bytes (0xC2, 0xB0)
    // Euro sign '€' is 3 bytes (0xE2, 0x82, 0xAC)
    // Section sign '§' is 2 bytes (0xC2, 0xA7)

    // Test with buffer capacity 1 to 6
    for cap in 1..=6 {
        let mut s = StackStr::<6>::new();
        // Artificially restrict capacity by filling first (6 - cap) bytes
        let pad = "A".repeat(6 - cap);
        s.push_str(&pad);
        assert_eq!(s.remaining(), cap);

        // Now push a string of degree symbols "°°°" (each 2 bytes, total 6 bytes)
        s.push_str("°°°");

        // Oracle verification: must be 100% valid UTF-8
        let slice = s.as_str();
        assert!(std::str::from_utf8(slice.as_bytes()).is_ok());

        // The number of '°' chars must be cap / 2
        let expected_degrees = cap / 2;
        let expected_str = format!("{}{}", pad, "°".repeat(expected_degrees));
        assert_eq!(s.as_str(), expected_str);
        assert_eq!(s.len(), pad.len() + expected_degrees * 2);
    }
}

#[test]
fn test_adversarial_utf8_3byte_box_drawing_and_bullets() {
    // Box-drawing runes used in HUDs:
    // '•' (U+2022, 3 bytes: 0xE2 0x80 0xA2)
    // '│' (U+2502, 3 bytes: 0xE2 0x94 0x82)
    // '⚡' (U+26A1, 3 bytes: 0xE2 0x9A 0xA1)
    // '─' (U+2500, 3 bytes: 0xE2 0x94 0x80)

    let test_runes = ["•", "│", "⚡", "─", "★", "▲", "■"];

    for rune in test_runes {
        assert_eq!(rune.len(), 3, "Rune must be exactly 3 UTF-8 bytes");

        // Test capacity 0, 1, 2, 3, 4, 5, 6
        for cap in 0..=8 {
            let mut s = StackStr::<8>::new();
            let pad = "X".repeat(8 - cap);
            s.push_str(&pad);
            assert_eq!(s.remaining(), cap);

            // Push multiple 3-byte runes
            let rune_stream = format!("{}{}{}", rune, rune, rune);
            s.push_str(&rune_stream);

            // Oracle verification: valid UTF-8
            assert!(
                std::str::from_utf8(s.as_bytes()).is_ok(),
                "Memory corruption: buffer contains invalid UTF-8 for rune {}",
                rune
            );

            let expected_runes = cap / 3;
            let expected_str = format!("{}{}", pad, rune.repeat(expected_runes));
            assert_eq!(s.as_str(), expected_str);
            assert_eq!(s.len(), pad.len() + expected_runes * 3);
        }
    }
}

#[test]
fn test_adversarial_utf8_4byte_emojis() {
    // 4-byte emojis:
    // 🚀 (U+1F680, 4 bytes: 0xF0 0x9F 0x9A 0x80)
    // 💥 (U+1F4A5, 4 bytes: 0xF0 0x9F 0x92 0xA5)
    // 🔥 (U+1F525, 4 bytes: 0xF0 0x9F 0x94 0xA5)
    let emojis = ["🚀", "💥", "🔥", "🎯", "🛸"];

    for emoji in emojis {
        assert_eq!(emoji.len(), 4, "Emoji must be exactly 4 UTF-8 bytes");

        for cap in 0..=10 {
            let mut s = StackStr::<10>::new();
            let pad = "K".repeat(10 - cap);
            s.push_str(&pad);
            assert_eq!(s.remaining(), cap);

            let stream = format!("{}{}{}", emoji, emoji, emoji);
            s.push_str(&stream);

            // Soundness verification: valid UTF-8 check
            assert!(
                std::str::from_utf8(s.as_bytes()).is_ok(),
                "Buffer contains malformed UTF-8 for emoji {}",
                emoji
            );

            let expected_emojis = cap / 4;
            let expected_str = format!("{}{}", pad, emoji.repeat(expected_emojis));
            assert_eq!(s.as_str(), expected_str);
            assert_eq!(s.len(), pad.len() + expected_emojis * 4);
        }
    }
}

#[test]
fn test_adversarial_generative_fuzzing_mixed_utf8() {
    // Deterministic pseudo-random sequence generator
    let mut lcg: u64 = 0xDEADBEEFCAFE;
    let mut next_u32 = || -> u32 {
        lcg = lcg.wrapping_mul(6364136223846793005).wrapping_add(1);
        (lcg >> 32) as u32
    };

    let sample_corpus = [
        "A", "b", "9", " ", ":", "|", "/", "-",
        "°", "£", "§", "ä", "é", "ö", "ñ",
        "•", "│", "⚡", "─", "★", "▲", "■", "中", "文",
        "🚀", "💥", "🔥", "🎯", "🛸", "🤖",
    ];

    // Run 5,000 iterations of randomized string assemblies
    for _ in 0..5_000 {
        let chunk_count = (next_u32() % 15) as usize + 1;
        let mut full_input = String::new();
        for _ in 0..chunk_count {
            let idx = (next_u32() as usize) % sample_corpus.len();
            full_input.push_str(sample_corpus[idx]);
        }

        let mut s = StackStr::<37>::new();
        s.push_str(&full_input);

        // Soundness check 1: valid UTF-8
        let s_str = s.as_str();
        assert!(std::str::from_utf8(s_str.as_bytes()).is_ok());

        // Soundness check 2: length bounds
        assert!(s.len() <= 37);
        assert_eq!(s.remaining(), 37 - s.len());

        // Soundness check 3: must be a strict prefix of full_input
        assert!(full_input.starts_with(s_str));

        // Soundness check 4: if truncated, the next character in full_input couldn't fit
        if s_str.len() < full_input.len() {
            let next_char = full_input[s_str.len()..].chars().next().unwrap();
            let remaining = 37 - s_str.len();
            assert!(
                next_char.len_utf8() > remaining,
                "Premature truncation: next char {:?} (len {}) should have fit in {} remaining bytes",
                next_char,
                next_char.len_utf8(),
                remaining
            );
        }
    }
}

// ============================================================================
// SUITE 3: HIGH-FREQUENCY REUSE (CLEAR AND WRITE IN RAPID LOOPS)
// ============================================================================

#[test]
fn test_adversarial_high_frequency_reuse_rapid_loop() {
    let mut s = StackStr::<64>::new();

    for i in 0..100_000 {
        s.clear();
        assert_eq!(s.len(), 0);
        assert!(s.is_empty());
        assert_eq!(s.remaining(), 64);
        assert_eq!(s.as_str(), "");

        match i % 5 {
            0 => {
                write!(s, "HEADING: {:03.0}°", (i as f32 * 0.1) % 360.0).unwrap();
                assert!(s.starts_with("HEADING: "));
                assert!(s.ends_with("°"));
            }
            1 => {
                write!(s, "SEISMIC: {:.1}G", (i as f32 * 0.05) % 10.0).unwrap();
                assert!(s.starts_with("SEISMIC: "));
                assert!(s.ends_with("G"));
            }
            2 => {
                write!(s, "INTEGRITY: {:3.0}%", (i % 101) as f32).unwrap();
                assert!(s.starts_with("INTEGRITY: "));
                assert!(s.ends_with("%"));
            }
            3 => {
                write!(s, "FPS: {:2.0} | REBAR: {:3.0}%", 60.0, 95.0).unwrap();
                assert!(s.contains("FPS: 60"));
                assert!(s.contains("REBAR:  95%"));
            }
            _ => {
                s.push_str("C4 GRID: [ACTIVE] • CHARGES: 8/8");
                assert_eq!(s.as_str(), "C4 GRID: [ACTIVE] • CHARGES: 8/8");
            }
        }

        // Verify valid UTF-8 on every single iteration
        assert!(std::str::from_utf8(s.as_bytes()).is_ok());
    }
}

#[test]
fn test_adversarial_repeated_write_without_clear() {
    let mut s = StackStr::<20>::new();
    for i in 0..100 {
        let _ = write!(s, "{},", i);
    }
    assert_eq!(s.len(), 20);
    assert_eq!(s.as_str(), "0,1,2,3,4,5,6,7,8,9,");
    assert!(std::str::from_utf8(s.as_bytes()).is_ok());
}

// ============================================================================
// SUITE 4: HUD FORMAT SPECIFIERS & EXTREME NUMERICAL INPUTS
// ============================================================================

#[test]
fn test_adversarial_hud_format_specifiers() {
    let mut s = StackStr::<64>::new();

    // 1. Heading angle: {:03.0}°
    write!(s, "{:03.0}°", 5.0).unwrap();
    assert_eq!(s.as_str(), "005°");
    s.clear();

    write!(s, "{:03.0}°", 125.4).unwrap();
    assert_eq!(s.as_str(), "125°");
    s.clear();

    // 2. Percentage: {:4.1}%
    write!(s, "{:4.1}%", 8.5).unwrap();
    assert_eq!(s.as_str(), " 8.5%");
    s.clear();

    write!(s, "{:4.1}%", 100.0).unwrap();
    assert_eq!(s.as_str(), "100.0%");
    s.clear();

    // 3. Multiplier: {:.1}x
    write!(s, "{:.1}x", 2.345).unwrap();
    assert_eq!(s.as_str(), "2.3x");
    s.clear();

    // 4. Distance / range: {:.1}m
    write!(s, "{:.1}m", 48.78).unwrap();
    assert_eq!(s.as_str(), "48.8m");
    s.clear();

    // 5. Integrity: {:3.0}%
    write!(s, "{:3.0}%", 99.0).unwrap();
    assert_eq!(s.as_str(), " 99%");
    s.clear();

    // 6. Index: {:02}
    write!(s, "D[{:02}]", 7).unwrap();
    assert_eq!(s.as_str(), "D[07]");
    s.clear();
}

#[test]
fn test_adversarial_extreme_floats_formatting() {
    let mut s = StackStr::<32>::new();

    // Extreme floats: Infinity, -Infinity, NaN, Subnormals, Max
    let extreme_cases: &[(f32, &str)] = &[
        (f32::INFINITY, "inf"),
        (f32::NEG_INFINITY, "-inf"),
        (f32::NAN, "NaN"),
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (f32::MAX, "340282350000000000000000000000000000000.0"),
        (f32::MIN_POSITIVE, "0.0"),
    ];

    for &(val, _) in extreme_cases {
        s.clear();
        let res = write!(s, "{:.1}", val);
        assert!(res.is_ok());
        assert!(std::str::from_utf8(s.as_bytes()).is_ok());
        assert!(s.len() <= 32);
    }
}

// ============================================================================
// SUITE 5: FIT_TEXT_TO_WIDTH_STACK ADVERSARIAL STRESS TESTING
// ============================================================================

#[test]
fn test_adversarial_fit_text_to_width_empty_and_zero_width() {
    // Empty text
    let empty_res = fit_text_to_width_stack::<32>("", 100.0, 14.0);
    assert_eq!(empty_res.as_str(), "");
    assert_eq!(empty_res.as_str(), fit_text_to_width("", 100.0, 14.0));

    // Zero width
    let zero_w = fit_text_to_width_stack::<32>("TEST_STRING", 0.0, 14.0);
    assert_eq!(zero_w.as_str(), fit_text_to_width("TEST_STRING", 0.0, 14.0));

    // Negative width
    let neg_w = fit_text_to_width_stack::<32>("TEST_STRING", -10.0, 14.0);
    assert_eq!(neg_w.as_str(), fit_text_to_width("TEST_STRING", -10.0, 14.0));
}

#[test]
fn test_adversarial_fit_text_to_width_short_strings() {
    let text = "SHORT";
    let res = fit_text_to_width_stack::<64>(text, 200.0, 14.0);
    assert_eq!(res.as_str(), text);
    assert_eq!(res.as_str(), fit_text_to_width(text, 200.0, 14.0));
    assert!(!res.contains("..."));
}

#[test]
fn test_adversarial_fit_text_to_width_long_unicode() {
    let long_unicode = "STRUCTURAL_COLLAPSE_WARNING_•_SEISMIC_FAULT_│_OVERLOAD_99%_⚡⚡⚡";
    for width in [10.0, 30.0, 50.0, 100.0, 150.0, 300.0, 1000.0] {
        let stack_res = fit_text_to_width_stack::<128>(long_unicode, width, 14.0);
        let heap_res = fit_text_to_width(long_unicode, width, 14.0);

        // Verify valid UTF-8
        assert!(std::str::from_utf8(stack_res.as_bytes()).is_ok());

        // Verify equivalence with oracle fit_text_to_width
        assert_eq!(
            stack_res.as_str(),
            heap_res.as_str(),
            "Mismatch at width {}",
            width
        );
    }
}

#[test]
fn test_adversarial_fit_text_to_width_tiny_stack_capacities() {
    let text = "VERY_LONG_HEADER_TELEMETRY_LINE";

    // Capacity 0
    let res0 = fit_text_to_width_stack::<0>(text, 50.0, 14.0);
    assert_eq!(res0.as_str(), "");

    // Capacity 1
    let res1 = fit_text_to_width_stack::<1>(text, 50.0, 14.0);
    assert_eq!(res1.len(), 1);
    assert!(std::str::from_utf8(res1.as_bytes()).is_ok());

    // Capacity 2
    let res2 = fit_text_to_width_stack::<2>(text, 50.0, 14.0);
    assert_eq!(res2.len(), 2);
    assert!(std::str::from_utf8(res2.as_bytes()).is_ok());

    // Capacity 3
    let res3 = fit_text_to_width_stack::<3>(text, 50.0, 14.0);
    assert_eq!(res3.len(), 3);
    assert!(std::str::from_utf8(res3.as_bytes()).is_ok());

    // Capacity 4
    let res4 = fit_text_to_width_stack::<4>(text, 50.0, 14.0);
    assert_eq!(res4.len(), 4);
    assert!(std::str::from_utf8(res4.as_bytes()).is_ok());
}

// ============================================================================
// SUITE 6: STACKSTR TRAITS, DEREF, COPY/CLONE & EQUALITY
// ============================================================================

#[test]
fn test_adversarial_stack_str_traits_and_ergonomics() {
    let mut s = StackStr::<32>::new();
    write!(s, "Demolition Core").unwrap();

    // Deref to str
    assert_eq!(s.len(), 15);
    assert_eq!(&s[..10], "Demolition");
    assert!(s.contains("Core"));
    assert!(s.starts_with("Demo"));
    assert!(s.ends_with("Core"));

    // Copy semantics
    let s_copy = s;
    assert_eq!(s.as_str(), s_copy.as_str());

    // Clone semantics
    #[allow(clippy::clone_on_copy)]
    let s_clone = s.clone();
    assert_eq!(s.as_str(), s_clone.as_str());

    // PartialEq
    assert_eq!(s, "Demolition Core");
    assert_eq!(s, "Demolition Core".to_string().as_str());
    assert_ne!(s, "Something Else");

    // Cross-capacity PartialEq
    let mut s2 = StackStr::<64>::new();
    write!(s2, "Demolition Core").unwrap();
    assert_eq!(s, s2);

    // Display & Debug
    assert_eq!(format!("{}", s), "Demolition Core");
    assert_eq!(format!("{:?}", s), "\"Demolition Core\"");

    // Default
    let def: StackStr<16> = Default::default();
    assert_eq!(def.len(), 0);
    assert_eq!(def.as_str(), "");
}
