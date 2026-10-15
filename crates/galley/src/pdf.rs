use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::font::Embed;
use crate::typeset::Typeset;

impl Typeset {
    /// The pages as a PDF at real size, its font embedded whole so it prints and reads the same anywhere, and its text copied out as the characters it was set from. `title` is the document's.
    pub fn pdf(&self, title: &str) -> Vec<u8> {
        let font = &self.font;
        let size = self.setting.size;
        let (width, height) = self.setting.paper.size();
        let mut used: BTreeMap<u16, char> = BTreeMap::new();
        for line in self.pages.iter().flat_map(|p| &p.lines) {
            for g in &line.glyphs {
                used.entry(g.id).or_insert(g.c);
            }
        }
        let name: String = font.name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let name = if name.is_empty() { "Font".to_string() } else { name };
        let mut pdf = Writer::default();
        let pages_id = 2;
        let font_id = 3;
        let first_page = 9;
        let kids: Vec<String> = (0..self.pages.len()).map(|k| format!("{} 0 R", first_page + 2 * k)).collect();
        pdf.object(1, "<< /Type /Catalog /Pages 2 0 R >>");
        pdf.object(pages_id, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), self.pages.len()));
        pdf.object(font_id, &format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [4 0 R] /ToUnicode 5 0 R >>"));
        let widths: String = used.keys().map(|&id| format!("{id} [{}]", number(font.advance(id) * 1000.0))).collect::<Vec<_>>().join(" ");
        let subtype = if font.embeddable() == Some(Embed::OpenType) { "CIDFontType0" } else { "CIDFontType2" };
        let map = if subtype == "CIDFontType2" { " /CIDToGIDMap /Identity" } else { "" };
        pdf.object(
            4,
            &format!(
                "<< /Type /Font /Subtype /{subtype} /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor 6 0 R /DW 0 /W [{widths}]{map} >>"
            ),
        );
        pdf.stream(5, "", to_unicode(&used).as_bytes());
        let b = font.bounds.map(|v| number(v * 1000.0));
        let file = match font.embeddable() {
            Some(Embed::TrueType) => " /FontFile2 7 0 R",
            Some(Embed::OpenType) => " /FontFile3 7 0 R",
            None => "",
        };
        pdf.object(
            6,
            &format!(
                "<< /Type /FontDescriptor /FontName /{name} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle {} /Ascent {} /Descent {} /CapHeight {} /StemV 80{file} >>",
                b[0],
                b[1],
                b[2],
                b[3],
                number(font.italic_angle),
                number(font.ascent * 1000.0),
                number(font.descent * 1000.0),
                number(font.cap_height * 1000.0),
            ),
        );
        match font.embeddable() {
            Some(Embed::TrueType) => pdf.stream(7, &format!(" /Length1 {}", font.data().len()), font.data()),
            Some(Embed::OpenType) => pdf.stream(7, " /Subtype /OpenType", font.data()),
            None => pdf.object(7, "null"),
        }
        pdf.object(8, &format!("<< /Title <{}> /Producer (Graticule) >>", utf16(title)));
        for (k, page) in self.pages.iter().enumerate() {
            let id = first_page + 2 * k;
            pdf.object(id, &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>", number(width), number(height), id + 1));
            let mut content = format!("BT /F1 {} Tf\n", number(size));
            for line in &page.lines {
                let Some(first) = line.glyphs.first() else { continue };
                let _ = write!(content, "1 0 0 1 {} {} Tm [<", number(first.x), number(height - line.baseline));
                let mut pen = first.x;
                for g in &line.glyphs {
                    let gap = g.x - pen;
                    if gap.abs() > 0.001 {
                        let _ = write!(content, "> {} <", number(-gap * 1000.0 / size));
                    }
                    let _ = write!(content, "{:04X}", g.id);
                    pen = g.x + font.advance(g.id) * size;
                }
                content.push_str(">] TJ\n");
            }
            content.push_str("ET\n");
            pdf.stream(id + 1, "", content.as_bytes());
        }
        pdf.finish(first_page + 2 * self.pages.len())
    }
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
    offsets: BTreeMap<usize, usize>,
}

impl Writer {
    fn start(&mut self, id: usize) {
        if self.out.is_empty() {
            self.out.extend_from_slice(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n");
        }
        self.offsets.insert(id, self.out.len());
        self.out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    }

    fn object(&mut self, id: usize, body: &str) {
        self.start(id);
        self.out.extend_from_slice(body.as_bytes());
        self.out.extend_from_slice(b"\nendobj\n");
    }

    fn stream(&mut self, id: usize, extra: &str, data: &[u8]) {
        self.start(id);
        self.out.extend_from_slice(format!("<< /Length {}{extra} >>\nstream\n", data.len()).as_bytes());
        self.out.extend_from_slice(data);
        self.out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    fn finish(mut self, size: usize) -> Vec<u8> {
        let xref = self.out.len();
        let mut table = format!("xref\n0 {size}\n0000000000 65535 f \n");
        for id in 1..size {
            match self.offsets.get(&id) {
                Some(at) => {
                    let _ = writeln!(table, "{at:010} 00000 n ");
                }
                None => table.push_str("0000000000 65535 f \n"),
            }
        }
        let _ = write!(table, "trailer\n<< /Size {size} /Root 1 0 R /Info 8 0 R >>\nstartxref\n{xref}\n%%EOF\n");
        self.out.extend_from_slice(table.as_bytes());
        self.out
    }
}

fn number(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".to_string() } else { s.to_string() }
}

fn utf16(text: &str) -> String {
    let mut out = String::from("FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(out, "{unit:04X}");
    }
    out
}

fn to_unicode(used: &BTreeMap<u16, char>) -> String {
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pairs: Vec<(&u16, &char)> = used.iter().collect();
    for chunk in pairs.chunks(100) {
        let _ = writeln!(out, "{} beginbfchar", chunk.len());
        for (id, c) in chunk {
            let mut buf = [0u16; 2];
            let units: String = c.encode_utf16(&mut buf).iter().map(|u| format!("{u:04X}")).collect();
            let _ = writeln!(out, "<{id:04X}> <{units}>");
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /defineresource pop\nend\nend\n");
    out
}
