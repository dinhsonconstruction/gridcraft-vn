//! Vietnamese worksheet functions and legacy encoding converters (TCVN3 / VNI-Windows).

use gridcraft_core::{CellError, Value};

use crate::util::*;
use crate::{Arg, Ctx};

const DIGITS: [&str; 10] = ["không", "một", "hai", "ba", "bốn", "năm", "sáu", "bảy", "tám", "chín"];

/// Reads a string of ASCII digits (`0`–`9`) into lowercase Vietnamese words.
pub fn vn_digits(d: &str) -> String {
    let trimmed = d.trim_start_matches('0');
    if trimmed.is_empty() {
        return "không".to_string();
    }
    let bytes = trimmed.as_bytes();
    let n_groups = bytes.len().div_ceil(3);
    let first_len = bytes.len() - (n_groups.saturating_sub(1)) * 3;
    let mut words: Vec<&'static str> = Vec::new();
    let mut tier_nonzero = false;

    for idx in 0..n_groups {
        let scale = n_groups - 1 - idx;
        let slice = if idx == 0 {
            bytes.get(..first_len).unwrap_or(&[])
        } else {
            let start = first_len + (idx - 1) * 3;
            bytes.get(start..start + 3).unwrap_or(&[])
        };
        let (h, t, o) = match slice {
            [o] => (0, 0, o.saturating_sub(b'0')),
            [t, o] => (0, t.saturating_sub(b'0'), o.saturating_sub(b'0')),
            [h, t, o, ..] => (h.saturating_sub(b'0'), t.saturating_sub(b'0'), o.saturating_sub(b'0')),
            [] => (0, 0, 0),
        };
        let h = usize::from(h.min(9));
        let t = usize::from(t.min(9));
        let o = usize::from(o.min(9));

        if h != 0 || t != 0 || o != 0 {
            tier_nonzero = true;
            if idx > 0 || h > 0 {
                if let Some(&w) = DIGITS.get(h) {
                    words.push(w);
                }
                words.push("trăm");
            }
            if t == 0 {
                if o > 0 && (idx > 0 || h > 0) {
                    words.push("linh");
                }
            } else if t == 1 {
                words.push("mười");
            } else if let Some(&w) = DIGITS.get(t) {
                words.push(w);
                words.push("mươi");
            }

            if o == 1 {
                words.push(if t >= 2 { "mốt" } else { "một" });
            } else if o == 4 {
                words.push(if t >= 2 { "tư" } else { "bốn" });
            } else if o == 5 {
                words.push(if t >= 1 { "lăm" } else { "năm" });
            } else if o > 0
                && let Some(&w) = DIGITS.get(o)
            {
                words.push(w);
            }

            if scale > 0 {
                let unit = match scale % 3 {
                    1 => "nghìn",
                    2 => "triệu",
                    _ => "tỷ",
                };
                words.push(unit);
            }
        } else if scale > 0 && scale.is_multiple_of(3) && tier_nonzero {
            words.push("tỷ");
        }

        if scale.is_multiple_of(3) {
            tier_nonzero = false;
        }
    }

    words.join(" ")
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut out: String = first.to_uppercase().collect();
    out.push_str(chars.as_str());
    out
}

/// `VND(number, [suffix])`: Spells out a number in Vietnamese currency words.
/// Defaults to `"đồng chẵn"` when `suffix` is omitted.
pub(crate) fn vnd(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = round_half_away(num(a, 0)?, 0);
    if !x.is_finite() {
        return Err(CellError::Num);
    }
    let suffix = if has(a, 1) {
        text(a, 1)?
    } else {
        "đồng chẵn".to_string()
    };
    let fixed = format!("{:.0}", x.abs());
    let whole = fixed.split('.').next().unwrap_or(fixed.as_str());
    let is_zero = whole.bytes().all(|b| b == b'0');
    let mut phrase = String::new();
    if x < 0.0 && !is_zero {
        phrase.push_str("âm ");
    }
    phrase.push_str(&vn_digits(whole));
    let suffix_trimmed = suffix.trim();
    if !suffix_trimmed.is_empty() {
        phrase.push(' ');
        phrase.push_str(suffix_trimmed);
    }
    text_val(capitalize_first(&phrase))
}

/// `DOCSO(number, [unit])`: Spells out an integer or decimal number in Vietnamese words.
pub(crate) fn docso(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let raw = num(a, 0)?;
    if !raw.is_finite() {
        return Err(CellError::Num);
    }
    let unit = if has(a, 1) {
        text(a, 1)?
    } else {
        String::new()
    };
    let x = round_half_away(raw, 6);
    let fixed = format!("{:.6}", x.abs());
    let (whole, frac_raw) = fixed.split_once('.').unwrap_or((fixed.as_str(), ""));
    let frac = frac_raw.trim_end_matches('0');
    let is_zero = whole.bytes().all(|b| b == b'0') && frac.is_empty();
    let mut phrase = String::new();
    if x < 0.0 && !is_zero {
        phrase.push_str("âm ");
    }
    phrase.push_str(&vn_digits(whole));
    if !frac.is_empty() {
        phrase.push_str(" phẩy ");
        let leading_zeros = frac.bytes().take_while(|&b| b == b'0').count();
        for _ in 0..leading_zeros {
            phrase.push_str("không ");
        }
        phrase.push_str(&vn_digits(frac));
    }
    let unit_trimmed = unit.trim();
    if !unit_trimmed.is_empty() {
        phrase.push(' ');
        phrase.push_str(unit_trimmed);
    }
    text_val(capitalize_first(&phrase))
}

/// `USD(number)`: Spells out a USD amount in Vietnamese words (`"... đô la Mỹ và ... xu"` / `"... đô la Mỹ chẵn"`).
pub(crate) fn usd(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    let x = round_half_away(num(a, 0)?, 2);
    if !x.is_finite() {
        return Err(CellError::Num);
    }
    let fixed = format!("{:.2}", x.abs());
    let (whole, frac) = fixed.split_once('.').unwrap_or((fixed.as_str(), "00"));
    let has_whole = whole.bytes().any(|b| b != b'0');
    let has_cents = frac.bytes().any(|b| b != b'0');
    let mut phrase = String::new();
    if x < 0.0 && (has_whole || has_cents) {
        phrase.push_str("âm ");
    }
    phrase.push_str(&vn_digits(whole));
    phrase.push_str(" đô la Mỹ");
    if has_cents {
        phrase.push_str(" và ");
        phrase.push_str(&vn_digits(frac));
        phrase.push_str(" xu");
    } else {
        phrase.push_str(" chẵn");
    }
    text_val(capitalize_first(&phrase))
}

/// Converts a TCVN3 (ABC `.VnTime`) encoded string to Unicode.
pub fn tcvn3_to_unicode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let mapped = match ch {
            // Capital letters without tone in TCVN3
            '\u{00A1}' => 'Ă',
            '\u{00A2}' => 'Â',
            '\u{00A3}' => 'Ê',
            '\u{00A4}' => 'Ô',
            '\u{00A5}' => 'Ơ',
            '\u{00A6}' => 'Ư',
            '\u{00A7}' => 'Đ',
            // Lowercase letters without tone in TCVN3
            '\u{00A8}' => 'ă',
            '\u{00A9}' => 'â',
            '\u{00AA}' => 'ê',
            '\u{00AB}' => 'ô',
            '\u{00AC}' => 'ơ',
            '\u{00AD}' => 'ư',
            '\u{00AE}' => 'đ',
            // a + tones
            '\u{00B5}' => 'à',
            '\u{00B6}' => 'ả',
            '\u{00B7}' => 'ã',
            '\u{00B8}' => 'á',
            '\u{00B9}' => 'ạ',
            // ă + tones
            '\u{00BB}' => 'ằ',
            '\u{00BC}' => 'ẳ',
            '\u{00BD}' => 'ẵ',
            '\u{00BE}' => 'ắ',
            '\u{00C6}' => 'ặ',
            // â + tones
            '\u{00C7}' => 'ầ',
            '\u{00C8}' => 'ẩ',
            '\u{00C9}' => 'ẫ',
            '\u{00CA}' => 'ấ',
            '\u{00CB}' => 'ậ',
            // e + tones
            '\u{00CC}' => 'è',
            '\u{00CE}' => 'ẻ',
            '\u{00CF}' => 'ẽ',
            '\u{00D0}' => 'é',
            '\u{00D1}' => 'ẹ',
            // ê + tones
            '\u{00D2}' => 'ề',
            '\u{00D3}' => 'ể',
            '\u{00D4}' => 'ễ',
            '\u{00D5}' => 'ế',
            '\u{00D6}' => 'ệ',
            // i + tones
            '\u{00D7}' => 'ì',
            '\u{00D8}' => 'ỉ',
            '\u{00DC}' => 'ĩ',
            '\u{00DD}' => 'í',
            '\u{00DE}' => 'ị',
            // o + tones
            '\u{00DF}' => 'ò',
            '\u{00E1}' => 'ỏ',
            '\u{00E2}' => 'õ',
            '\u{00E3}' => 'ó',
            '\u{00E4}' => 'ọ',
            // ô + tones
            '\u{00E5}' => 'ồ',
            '\u{00E6}' => 'ổ',
            '\u{00E7}' => 'ỗ',
            '\u{00E8}' => 'ố',
            '\u{00E9}' => 'ộ',
            // ơ + tones
            '\u{00EA}' => 'ờ',
            '\u{00EB}' => 'ở',
            '\u{00EC}' => 'ỡ',
            '\u{00ED}' => 'ớ',
            '\u{00EE}' => 'ợ',
            // u + tones
            '\u{00EF}' => 'ù',
            '\u{00F1}' => 'ủ',
            '\u{00F2}' => 'ũ',
            '\u{00F3}' => 'ú',
            '\u{00F4}' => 'ụ',
            // ư + tones
            '\u{00F5}' => 'ừ',
            '\u{00F6}' => 'ử',
            '\u{00F7}' => 'ữ',
            '\u{00F8}' => 'ứ',
            '\u{00F9}' => 'ự',
            // y + tones
            '\u{00FA}' => 'ỳ',
            '\u{00FB}' => 'ỷ',
            '\u{00FC}' => 'ỹ',
            '\u{00FD}' => 'ý',
            '\u{00FE}' => 'ỵ',
            other => other,
        };
        out.push(mapped);
    }
    out
}

/// Converts a VNI-Windows (`VNI-Times`) encoded string to Unicode.
pub fn vni_to_unicode(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };
        let next = chars.get(i + 1).copied();
        if let Some(m) = next
            && let Some(combined) = vni_combine(c, m)
        {
            out.push(combined);
            i += 2;
            continue;
        }
        let single = match c {
            '\u{00F1}' => 'đ',
            '\u{00D1}' => 'Đ',
            '\u{00F4}' => 'ơ',
            '\u{00D4}' => 'Ơ',
            '\u{00F6}' => 'ư',
            '\u{00D6}' => 'Ư',
            '\u{00ED}' => 'í',
            '\u{00CD}' => 'Í',
            '\u{00EC}' => 'ì',
            '\u{00CC}' => 'Ì',
            '\u{00E6}' => 'ỉ',
            '\u{00C6}' => 'Ỉ',
            '\u{00F3}' => 'ĩ',
            '\u{00D3}' => 'Ĩ',
            '\u{00F2}' => 'ị',
            '\u{00D2}' => 'Ị',
            '\u{00EE}' => 'ỵ',
            '\u{00CE}' => 'Ỵ',
            other => other,
        };
        out.push(single);
        i += 1;
    }
    out
}

fn vni_combine(base: char, m: char) -> Option<char> {
    match (base, m) {
        // a + circumflex (â) and tones
        ('a', '\u{00E2}') => Some('â'),
        ('a', '\u{00E1}') => Some('ấ'),
        ('a', '\u{00E0}') => Some('ầ'),
        ('a', '\u{00E5}') => Some('ẩ'),
        ('a', '\u{00E3}') => Some('ẫ'),
        ('a', '\u{00E4}') => Some('ậ'),
        ('A', '\u{00C2}' | '\u{00E2}') => Some('Â'),
        ('A', '\u{00C1}' | '\u{00E1}') => Some('Ấ'),
        ('A', '\u{00C0}' | '\u{00E0}') => Some('Ầ'),
        ('A', '\u{00C5}' | '\u{00E5}') => Some('Ẩ'),
        ('A', '\u{00C3}' | '\u{00E3}') => Some('Ẫ'),
        ('A', '\u{00C4}' | '\u{00E4}') => Some('Ậ'),
        // a + breve (ă) and tones
        ('a', '\u{00EA}') => Some('ă'),
        ('a', '\u{00E9}') => Some('ắ'),
        ('a', '\u{00E8}') => Some('ằ'),
        ('a', '\u{00EB}') => Some('ẳ'),
        ('a', '\u{00EC}') => Some('ẵ'),
        ('a', '\u{00ED}') => Some('ặ'),
        ('A', '\u{00CA}' | '\u{00EA}') => Some('Ă'),
        ('A', '\u{00C9}' | '\u{00E9}') => Some('Ắ'),
        ('A', '\u{00C8}' | '\u{00E8}') => Some('Ằ'),
        ('A', '\u{00CB}' | '\u{00EB}') => Some('Ẳ'),
        ('A', '\u{00CC}' | '\u{00EC}') => Some('Ẵ'),
        ('A', '\u{00CD}' | '\u{00ED}') => Some('Ặ'),
        // e + circumflex (ê) and tones
        ('e', '\u{00E2}') => Some('ê'),
        ('e', '\u{00E1}') => Some('ế'),
        ('e', '\u{00E0}') => Some('ề'),
        ('e', '\u{00E5}') => Some('ể'),
        ('e', '\u{00E3}') => Some('ễ'),
        ('e', '\u{00E4}') => Some('ệ'),
        ('E', '\u{00C2}' | '\u{00E2}') => Some('Ê'),
        ('E', '\u{00C1}' | '\u{00E1}') => Some('Ế'),
        ('E', '\u{00C0}' | '\u{00E0}') => Some('Ề'),
        ('E', '\u{00C5}' | '\u{00E5}') => Some('Ể'),
        ('E', '\u{00C3}' | '\u{00E3}') => Some('Ễ'),
        ('E', '\u{00C4}' | '\u{00E4}') => Some('Ệ'),
        // o + circumflex (ô) and tones
        ('o', '\u{00E2}') => Some('ô'),
        ('o', '\u{00E1}') => Some('ố'),
        ('o', '\u{00E0}') => Some('ồ'),
        ('o', '\u{00E5}') => Some('ổ'),
        ('o', '\u{00E3}') => Some('ỗ'),
        ('o', '\u{00E4}') => Some('ộ'),
        ('O', '\u{00C2}' | '\u{00E2}') => Some('Ô'),
        ('O', '\u{00C1}' | '\u{00E1}') => Some('Ố'),
        ('O', '\u{00C0}' | '\u{00E0}') => Some('Ồ'),
        ('O', '\u{00C5}' | '\u{00E5}') => Some('Ổ'),
        ('O', '\u{00C3}' | '\u{00E3}') => Some('Ỗ'),
        ('O', '\u{00C4}' | '\u{00E4}') => Some('Ộ'),
        // Plain tones (sắc ù/Ù, huyền ø/Ø, hỏi û/Û, ngã õ/Õ, nặng ï/Ï)
        ('a', '\u{00F9}') => Some('á'),
        ('a', '\u{00F8}') => Some('à'),
        ('a', '\u{00FB}') => Some('ả'),
        ('a', '\u{00F5}') => Some('ã'),
        ('a', '\u{00EF}') => Some('ạ'),
        ('A', '\u{00D9}' | '\u{00F9}') => Some('Á'),
        ('A', '\u{00D8}' | '\u{00F8}') => Some('À'),
        ('A', '\u{00DB}' | '\u{00FB}') => Some('Ả'),
        ('A', '\u{00D5}' | '\u{00F5}') => Some('Ã'),
        ('A', '\u{00CF}' | '\u{00EF}') => Some('Ạ'),
        ('e', '\u{00F9}') => Some('é'),
        ('e', '\u{00F8}') => Some('è'),
        ('e', '\u{00FB}') => Some('ẻ'),
        ('e', '\u{00F5}') => Some('ẽ'),
        ('e', '\u{00EF}') => Some('ẹ'),
        ('E', '\u{00D9}' | '\u{00F9}') => Some('É'),
        ('E', '\u{00D8}' | '\u{00F8}') => Some('È'),
        ('E', '\u{00DB}' | '\u{00FB}') => Some('Ẻ'),
        ('E', '\u{00D5}' | '\u{00F5}') => Some('Ẽ'),
        ('E', '\u{00CF}' | '\u{00EF}') => Some('Ẹ'),
        ('o', '\u{00F9}') => Some('ó'),
        ('o', '\u{00F8}') => Some('ò'),
        ('o', '\u{00FB}') => Some('ỏ'),
        ('o', '\u{00F5}') => Some('õ'),
        ('o', '\u{00EF}') => Some('ọ'),
        ('O', '\u{00D9}' | '\u{00F9}') => Some('Ó'),
        ('O', '\u{00D8}' | '\u{00F8}') => Some('Ò'),
        ('O', '\u{00DB}' | '\u{00FB}') => Some('Ỏ'),
        ('O', '\u{00D5}' | '\u{00F5}') => Some('Õ'),
        ('O', '\u{00CF}' | '\u{00EF}') => Some('Ọ'),
        ('u', '\u{00F9}') => Some('ú'),
        ('u', '\u{00F8}') => Some('ù'),
        ('u', '\u{00FB}') => Some('ủ'),
        ('u', '\u{00F5}') => Some('ũ'),
        ('u', '\u{00EF}') => Some('ụ'),
        ('U', '\u{00D9}' | '\u{00F9}') => Some('Ú'),
        ('U', '\u{00D8}' | '\u{00F8}') => Some('Ù'),
        ('U', '\u{00DB}' | '\u{00FB}') => Some('Ủ'),
        ('U', '\u{00D5}' | '\u{00F5}') => Some('Ũ'),
        ('U', '\u{00CF}' | '\u{00EF}') => Some('Ụ'),
        ('y', '\u{00F9}') => Some('ý'),
        ('y', '\u{00F8}') => Some('ỳ'),
        ('y', '\u{00FB}') => Some('ỷ'),
        ('y', '\u{00F5}') => Some('ỹ'),
        ('y', '\u{00EF}' | '\u{00EE}') => Some('ỵ'),
        ('Y', '\u{00D9}' | '\u{00F9}') => Some('Ý'),
        ('Y', '\u{00D8}' | '\u{00F8}') => Some('Ỳ'),
        ('Y', '\u{00DB}' | '\u{00FB}') => Some('Ỷ'),
        ('Y', '\u{00D5}' | '\u{00F5}') => Some('Ỹ'),
        ('Y', '\u{00CF}' | '\u{00EF}' | '\u{00CE}' | '\u{00EE}') => Some('Ỵ'),
        // ơ (ô in VNI) + tones
        ('\u{00F4}', '\u{00F9}') => Some('ớ'),
        ('\u{00F4}', '\u{00F8}') => Some('ờ'),
        ('\u{00F4}', '\u{00FB}') => Some('ở'),
        ('\u{00F4}', '\u{00F5}') => Some('ỡ'),
        ('\u{00F4}', '\u{00EF}' | '\u{00EE}') => Some('ợ'),
        ('\u{00D4}', '\u{00D9}' | '\u{00F9}') => Some('Ớ'),
        ('\u{00D4}', '\u{00D8}' | '\u{00F8}') => Some('Ờ'),
        ('\u{00D4}', '\u{00DB}' | '\u{00FB}') => Some('Ở'),
        ('\u{00D4}', '\u{00D5}' | '\u{00F5}') => Some('Ỡ'),
        ('\u{00D4}', '\u{00CF}' | '\u{00EF}' | '\u{00CE}' | '\u{00EE}') => Some('Ợ'),
        // ư (ö in VNI) + tones
        ('\u{00F6}', '\u{00F9}') => Some('ứ'),
        ('\u{00F6}', '\u{00F8}') => Some('ừ'),
        ('\u{00F6}', '\u{00FB}') => Some('ử'),
        ('\u{00F6}', '\u{00F5}') => Some('ữ'),
        ('\u{00F6}', '\u{00EF}') => Some('ự'),
        ('\u{00D6}', '\u{00D9}' | '\u{00F9}') => Some('Ứ'),
        ('\u{00D6}', '\u{00D8}' | '\u{00F8}') => Some('Ừ'),
        ('\u{00D6}', '\u{00DB}' | '\u{00FB}') => Some('Ử'),
        ('\u{00D6}', '\u{00D5}' | '\u{00F5}') => Some('Ữ'),
        ('\u{00D6}', '\u{00CF}' | '\u{00EF}') => Some('Ự'),
        _ => None,
    }
}

/// Checks whether a font name is a legacy Vietnamese TCVN3 (`.Vn*`) or VNI (`VNI-*`) font.
pub fn is_legacy_vn_font(font_name: &str) -> Option<&'static str> {
    let trimmed = font_name.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with(".vn") {
        Some("tcvn3")
    } else if lower.starts_with("vni-") || lower.starts_with("vni ") {
        Some("vni")
    } else {
        None
    }
}

/// Auto-detects whether `s` is encoded in VNI-Windows or TCVN3 ( optionally guided by `font_name`
/// or explicit `source` `"tcvn3"` / `"vni"` / `"auto"` ) and returns the converted Unicode text.
pub fn convert_vietnamese_text(s: &str, source: &str, font_name: &str) -> String {
    let lower_font = font_name.trim().to_ascii_lowercase();
    let is_tcvn_upper = lower_font.starts_with(".vn") && lower_font.ends_with('h');
    let finish = |out: String| if is_tcvn_upper { out.to_uppercase() } else { out };
    match source {
        "tcvn3" => return finish(tcvn3_to_unicode(s)),
        "vni" => return vni_to_unicode(s),
        _ => {}
    }
    if let Some(kind) = is_legacy_vn_font(font_name) {
        return if kind == "vni" { vni_to_unicode(s) } else { finish(tcvn3_to_unicode(s)) };
    }
    // Heuristic detection when font is already Unicode:
    // If string already contains standard Unicode Vietnamese precomposed chars (U+0102..U+1EF9), leave as-is.
    if s.chars().any(|c| matches!(c, '\u{0102}'..='\u{01B0}' | '\u{1EA0}'..='\u{1EF9}')) {
        return s.to_string();
    }
    // Check for VNI 2-character vowel+modifier pairs or VNI-specific letters (ñ/Ñ/ô/ö followed by tone).
    let chars: Vec<char> = s.chars().collect();
    let vni_pairs = chars.windows(2).filter(|w| matches!(w, [a, b] if vni_combine(*a, *b).is_some())).count();
    let vni_singles = chars
        .iter()
        .filter(|&&c| matches!(c, '\u{00F1}' | '\u{00D1}' | '\u{00F4}' | '\u{00D4}' | '\u{00F6}' | '\u{00D6}'))
        .count();
    let tcvn3_chars = chars
        .iter()
        .filter(|&&c| matches!(c, '\u{00A1}'..='\u{00AE}' | '\u{00B5}'..='\u{00FE}'))
        .count();
    if vni_pairs > 0 || (vni_singles > 0 && tcvn3_chars == vni_singles) {
        vni_to_unicode(s)
    } else if tcvn3_chars > 0 {
        tcvn3_to_unicode(s)
    } else {
        s.to_string()
    }
}

pub(crate) fn tcvn3_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_val(tcvn3_to_unicode(&text(a, 0)?))
}

pub(crate) fn vni_fn(a: &[Arg], _c: &mut dyn Ctx) -> R<Value> {
    text_val(vni_to_unicode(&text(a, 0)?))
}
