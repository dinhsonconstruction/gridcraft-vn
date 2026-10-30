//! TrueType (`CIDFontType2` / `Identity-H`) subsetting and embedding for Unicode text (such as
//! Vietnamese `Times New Roman`) that lies outside `WinAnsiEncoding`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::OnceLock;

pub(crate) struct TtfFont {
    upem: u16,
    num_glyphs: usize,
    widths: Vec<u16>,
    loca: Vec<usize>,
    glyf: Vec<u8>,
    head: Vec<u8>,
    hhea: Vec<u8>,
    hmtx: Vec<u8>,
    maxp: Vec<u8>,
    cvt: Option<Vec<u8>>,
    fpgm: Option<Vec<u8>>,
    prep: Option<Vec<u8>>,
    end_codes: Vec<u16>,
    start_codes: Vec<u16>,
    id_deltas: Vec<i16>,
    id_range_offsets: Vec<u16>,
    glyph_id_array: Vec<u16>,
}

fn u16_be(data: &[u8], off: usize) -> Option<u16> {
    let b = data.get(off..off + 2)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

fn i16_be(data: &[u8], off: usize) -> Option<i16> {
    let b = data.get(off..off + 2)?;
    Some(i16::from_be_bytes([b[0], b[1]]))
}

fn u32_be(data: &[u8], off: usize) -> Option<u32> {
    let b = data.get(off..off + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

impl TtfFont {
    fn parse(ttf: &[u8]) -> Option<Self> {
        let num_tables = usize::from(u16_be(ttf, 4)?);
        let mut tables: BTreeMap<[u8; 4], &[u8]> = BTreeMap::new();
        for i in 0..num_tables {
            let off = 12usize.checked_add(i.checked_mul(16)?)?;
            let tag_s = ttf.get(off..off + 4)?;
            let tag = [tag_s[0], tag_s[1], tag_s[2], tag_s[3]];
            let t_off = u32_be(ttf, off + 8)? as usize;
            let t_len = u32_be(ttf, off + 12)? as usize;
            let slice = ttf.get(t_off..t_off.checked_add(t_len)?)?;
            tables.insert(tag, slice);
        }

        let head = (*tables.get(b"head")?).to_vec();
        let hhea = (*tables.get(b"hhea")?).to_vec();
        let maxp = (*tables.get(b"maxp")?).to_vec();
        let hmtx = (*tables.get(b"hmtx")?).to_vec();
        let loca_raw = *tables.get(b"loca")?;
        let glyf = (*tables.get(b"glyf")?).to_vec();
        let cmap = *tables.get(b"cmap")?;

        let upem = u16_be(&head, 18)?.max(1);
        let loca_fmt = i16_be(&head, 50)?;
        let num_hmetrics = usize::from(u16_be(&hhea, 34)?);
        let num_glyphs = usize::from(u16_be(&maxp, 4)?);
        if num_glyphs == 0 || num_glyphs > 65_535 {
            return None;
        }

        let mut widths = Vec::with_capacity(num_glyphs);
        for i in 0..num_hmetrics.min(num_glyphs) {
            let aw = u16_be(&hmtx, i * 4)?;
            let w = ((u32::from(aw) * 1000 + u32::from(upem) / 2) / u32::from(upem)) as u16;
            widths.push(w);
        }
        let last_w = widths.last().copied().unwrap_or(500);
        while widths.len() < num_glyphs {
            widths.push(last_w);
        }

        let mut loca = Vec::with_capacity(num_glyphs + 1);
        if loca_fmt == 0 {
            for i in 0..=num_glyphs {
                loca.push(usize::from(u16_be(loca_raw, i * 2)?) * 2);
            }
        } else {
            for i in 0..=num_glyphs {
                loca.push(u32_be(loca_raw, i * 4)? as usize);
            }
        }

        let num_sub = usize::from(u16_be(cmap, 2)?);
        let mut sub_off = None;
        for i in 0..num_sub {
            let entry = 4 + i * 8;
            let pid = u16_be(cmap, entry)?;
            let eid = u16_be(cmap, entry + 2)?;
            let soff = u32_be(cmap, entry + 4)? as usize;
            if (pid == 3 && eid == 1) || pid == 0 {
                if u16_be(cmap, soff) == Some(4) {
                    sub_off = Some(soff);
                    if pid == 3 && eid == 1 {
                        break;
                    }
                }
            }
        }
        let sub_off = sub_off?;
        let sub_len = usize::from(u16_be(cmap, sub_off + 2)?);
        let seg_count = usize::from(u16_be(cmap, sub_off + 6)?) / 2;
        let end_off = sub_off + 14;
        let start_off = end_off + 2 * seg_count + 2;
        let delta_off = start_off + 2 * seg_count;
        let range_off = delta_off + 2 * seg_count;
        let gid_off = range_off + 2 * seg_count;
        let sub_end = (sub_off + sub_len).min(cmap.len());

        let mut end_codes = Vec::with_capacity(seg_count);
        let mut start_codes = Vec::with_capacity(seg_count);
        let mut id_deltas = Vec::with_capacity(seg_count);
        let mut id_range_offsets = Vec::with_capacity(seg_count);
        for i in 0..seg_count {
            end_codes.push(u16_be(cmap, end_off + i * 2)?);
            start_codes.push(u16_be(cmap, start_off + i * 2)?);
            id_deltas.push(i16_be(cmap, delta_off + i * 2)?);
            id_range_offsets.push(u16_be(cmap, range_off + i * 2)?);
        }
        let mut glyph_id_array = Vec::new();
        let mut p = gid_off;
        while p + 2 <= sub_end {
            glyph_id_array.push(u16_be(cmap, p)?);
            p += 2;
        }

        let _ = upem;
        Some(TtfFont {
            upem,
            num_glyphs,
            widths,
            loca,
            glyf,
            head,
            hhea,
            hmtx,
            maxp,
            cvt: tables.get(b"cvt ").map(|s| (*s).to_vec()),
            fpgm: tables.get(b"fpgm").map(|s| (*s).to_vec()),
            prep: tables.get(b"prep").map(|s| (*s).to_vec()),
            end_codes,
            start_codes,
            id_deltas,
            id_range_offsets,
            glyph_id_array,
        })
    }

    pub(crate) fn glyph_id(&self, c: char) -> u16 {
        let Ok(code) = u16::try_from(u32::from(c)) else { return 0 };
        let seg_count = self.end_codes.len();
        let idx = self.end_codes.partition_point(|&end| end < code);
        if idx >= seg_count {
            return 0;
        }
        let start = self.start_codes.get(idx).copied().unwrap_or(u16::MAX);
        if code < start {
            return 0;
        }
        let ro = self.id_range_offsets.get(idx).copied().unwrap_or(0);
        let delta = self.id_deltas.get(idx).copied().unwrap_or(0);
        if ro == 0 {
            return code.wrapping_add(delta as u16);
        }
        let word_offset = (usize::from(ro) / 2)
            .saturating_add(usize::from(code - start))
            .checked_sub(seg_count - idx);
        let Some(gid) = word_offset.and_then(|o| self.glyph_id_array.get(o).copied()) else {
            return 0;
        };
        if gid == 0 { 0 } else { gid.wrapping_add(delta as u16) }
    }

    pub(crate) fn can_encode(&self, text: &str) -> bool {
        text.chars().all(|c| c.is_control() || self.glyph_id(c) != 0)
    }

    pub(crate) fn encode_hex(&self, text: &str, used_chars: &mut BTreeMap<u16, char>) -> String {
        let mut s = String::with_capacity(text.len() * 4 + 2);
        s.push('<');
        for c in text.chars() {
            let ch = if c == '\t' { ' ' } else if c.is_control() { continue } else { c };
            let gid = self.glyph_id(ch);
            used_chars.entry(gid).or_insert(ch);
            let _ = write!(s, "{gid:04X}");
        }
        s.push('>');
        s
    }

    fn components(&self, gid: u16) -> Vec<u16> {
        let g = usize::from(gid);
        let (Some(&s), Some(&e)) = (self.loca.get(g), self.loca.get(g + 1)) else {
            return Vec::new();
        };
        if e <= s + 10 || e > self.glyf.len() {
            return Vec::new();
        }
        if i16_be(&self.glyf, s).unwrap_or(0) >= 0 {
            return Vec::new();
        }
        let mut pos = s + 10;
        let mut out = Vec::new();
        while pos + 4 <= e {
            let Some(flags) = u16_be(&self.glyf, pos) else { break };
            let Some(c_gid) = u16_be(&self.glyf, pos + 2) else { break };
            out.push(c_gid);
            pos += 4;
            pos += if flags & 0x0001 != 0 { 4 } else { 2 };
            if flags & 0x0008 != 0 {
                pos += 2;
            } else if flags & 0x0040 != 0 {
                pos += 4;
            } else if flags & 0x0080 != 0 {
                pos += 8;
            }
            if flags & 0x0020 == 0 {
                break;
            }
        }
        out
    }

    fn subset_ttf(&self, used_gids: &BTreeSet<u16>) -> Vec<u8> {
        let mut all_used = BTreeSet::new();
        all_used.insert(0u16);
        let mut stack: Vec<u16> = used_gids.iter().copied().collect();
        while let Some(g) = stack.pop() {
            if usize::from(g) < self.num_glyphs && all_used.insert(g) {
                stack.extend(self.components(g));
            }
        }

        let mut new_glyf: Vec<u8> = Vec::new();
        let mut new_loca: Vec<u8> = Vec::with_capacity((self.num_glyphs + 1) * 4);
        new_loca.extend_from_slice(&0u32.to_be_bytes());
        for g in 0..self.num_glyphs {
            if all_used.contains(&(g as u16))
                && let (Some(&s), Some(&e)) = (self.loca.get(g), self.loca.get(g + 1))
                && e > s
                && let Some(chunk) = self.glyf.get(s..e)
            {
                new_glyf.extend_from_slice(chunk);
                while !new_glyf.len().is_multiple_of(4) {
                    new_glyf.push(0);
                }
            }
            new_loca.extend_from_slice(&(new_glyf.len() as u32).to_be_bytes());
        }

        let mut new_head = self.head.clone();
        if let Some(slot) = new_head.get_mut(50..52) {
            slot.copy_from_slice(&1i16.to_be_bytes());
        }

        let mut kept: Vec<(&[u8; 4], &[u8])> = Vec::new();
        if let Some(cvt) = &self.cvt {
            kept.push((b"cvt ", cvt));
        }
        if let Some(fpgm) = &self.fpgm {
            kept.push((b"fpgm", fpgm));
        }
        kept.push((b"glyf", &new_glyf));
        kept.push((b"head", &new_head));
        kept.push((b"hhea", &self.hhea));
        kept.push((b"hmtx", &self.hmtx));
        kept.push((b"loca", &new_loca));
        kept.push((b"maxp", &self.maxp));
        if let Some(prep) = &self.prep {
            kept.push((b"prep", prep));
        }

        let n_t = kept.len() as u16;
        let mut p2 = 1u16;
        let mut entry_sel = 0u16;
        while p2 * 2 <= n_t {
            p2 *= 2;
            entry_sel += 1;
        }
        let search_range = p2 * 16;
        let range_shift = n_t * 16 - search_range;

        let mut out = Vec::new();
        out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        out.extend_from_slice(&n_t.to_be_bytes());
        out.extend_from_slice(&search_range.to_be_bytes());
        out.extend_from_slice(&entry_sel.to_be_bytes());
        out.extend_from_slice(&range_shift.to_be_bytes());

        let mut offset = 12 + usize::from(n_t) * 16;
        let mut bodies = Vec::new();
        for (tag, data) in kept {
            out.extend_from_slice(tag);
            out.extend_from_slice(&0u32.to_be_bytes());
            out.extend_from_slice(&(offset as u32).to_be_bytes());
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bodies.extend_from_slice(data);
            while !bodies.len().is_multiple_of(4) {
                bodies.push(0);
            }
            offset = 12 + usize::from(n_t) * 16 + bodies.len();
        }
        out.extend_from_slice(&bodies);
        let _ = self.upem;
        out
    }

    pub(crate) fn build_pdf_objects(
        &self,
        style: usize,
        base_id: u32,
        used_chars: &BTreeMap<u16, char>,
    ) -> Vec<(u32, Vec<u8>)> {
        let base_name = match style {
            1 => "TimesNewRomanPS-BoldMT",
            2 => "TimesNewRomanPS-ItalicMT",
            3 => "TimesNewRomanPS-BoldItalicMT",
            _ => "TimesNewRomanPSMT",
        };
        let (cid_id, desc_id, file_id, touni_id) = (base_id + 1, base_id + 2, base_id + 3, base_id + 4);
        let used_gids: BTreeSet<u16> = used_chars.keys().copied().collect();
        let mut w_str = String::new();
        for &g in &used_gids {
            let w = self.widths.get(usize::from(g)).copied().unwrap_or(500);
            let _ = write!(w_str, "{g} [{w}] ");
        }

        let ttf_bytes = self.subset_ttf(&used_gids);
        let mut font_file_obj = format!("<< /Length {} /Length1 {} >>\nstream\n", ttf_bytes.len(), ttf_bytes.len()).into_bytes();
        font_file_obj.extend_from_slice(&ttf_bytes);
        font_file_obj.extend_from_slice(b"\nendstream");

        let mut cmap_body = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );
        let entries: Vec<(u16, char)> = used_chars.iter().map(|(&g, &c)| (g, c)).collect();
        for chunk in entries.chunks(100) {
            let _ = writeln!(cmap_body, "{} beginbfchar", chunk.len());
            for &(g, c) in chunk {
                let mut u16_hex = String::new();
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    let _ = write!(u16_hex, "{unit:04X}");
                }
                let _ = writeln!(cmap_body, "<{g:04X}> <{u16_hex}>");
            }
            cmap_body.push_str("endbfchar\n");
        }
        cmap_body.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let mut touni_obj = format!("<< /Length {} >>\nstream\n", cmap_body.len()).into_bytes();
        touni_obj.extend_from_slice(cmap_body.as_bytes());
        touni_obj.extend_from_slice(b"endstream");

        vec![
            (
                base_id,
                format!(
                    "<< /Type /Font /Subtype /Type0 /BaseFont /{base_name} /Encoding /Identity-H /DescendantFonts [{cid_id} 0 R] /ToUnicode {touni_id} 0 R >>"
                )
                .into_bytes(),
            ),
            (
                cid_id,
                format!(
                    "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base_name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {desc_id} 0 R /CIDToGIDMap /Identity /DW 500 /W [{w_str}] >>"
                )
                .into_bytes(),
            ),
            (
                desc_id,
                format!(
                    "<< /Type /FontDescriptor /FontName /{base_name} /Flags 32 /FontBBox [-568 -307 2000 1007] /ItalicAngle 0 /Ascent 891 /Descent -216 /CapHeight 662 /StemV 80 /FontFile2 {file_id} 0 R >>"
                )
                .into_bytes(),
            ),
            (file_id, font_file_obj),
            (touni_id, touni_obj),
        ]
    }
}

fn load_variant(style: usize) -> Option<TtfFont> {
    let candidates: &[&str] = match style {
        1 => &[
            "/System/Library/Fonts/Supplemental/Times New Roman Bold.ttf",
            "/Library/Fonts/Times New Roman Bold.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSerif-Bold.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
            "C:\\Windows\\Fonts\\timesbd.ttf",
        ],
        2 => &[
            "/System/Library/Fonts/Supplemental/Times New Roman Italic.ttf",
            "/Library/Fonts/Times New Roman Italic.ttf",
            "/System/Library/Fonts/Supplemental/Arial Italic.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSerif-Italic.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Italic.ttf",
            "C:\\Windows\\Fonts\\timesi.ttf",
        ],
        3 => &[
            "/System/Library/Fonts/Supplemental/Times New Roman Bold Italic.ttf",
            "/Library/Fonts/Times New Roman Bold Italic.ttf",
            "/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSerif-BoldItalic.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif-BoldItalic.ttf",
            "C:\\Windows\\Fonts\\timesbi.ttf",
        ],
        _ => &[
            "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
            "/Library/Fonts/Times New Roman.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
            "C:\\Windows\\Fonts\\times.ttf",
        ],
    };
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path)
            && let Some(font) = TtfFont::parse(&bytes)
        {
            return Some(font);
        }
    }
    if style != 0 {
        return load_variant(0);
    }
    None
}

static TTF_FONTS: [OnceLock<Option<TtfFont>>; 4] =
    [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];

pub(crate) fn get(style: usize) -> Option<&'static TtfFont> {
    TTF_FONTS.get(style.min(3))?.get_or_init(|| load_variant(style.min(3))).as_ref()
}
