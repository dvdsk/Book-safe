use color_eyre::Result;
use indextree::NodeId;
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;
use time::Time;

use crate::directory::Tree;

struct Doc {
    h: Mm,
    w: Mm,
    y: Mm,
    h_margin: Mm,
    w_margin: Mm,
    font: IndirectFontRef,
    layer: PdfLayerReference,
    page: PdfPageIndex,
    pdf: PdfDocumentReference,
    n_pages: usize,
}

impl Doc {
    fn hline(&mut self) {
        let points = vec![
            (Point::new(self.w_margin, self.y), false),
            (Point::new(self.w-self.w_margin, self.y), false),
        ];

        let line = Line {
            points,
            is_closed: true,
            has_fill: true,
            has_stroke: true,
            is_clipping_path: false,
        };
        self.layer.add_shape(line);
    }

    fn add_title(&mut self, text: &str) {
        let size = 40.;
        self.layer.begin_text_section();
        self.layer.set_font(&self.font, size);
        self.layer.set_text_cursor(Mm(50.0), self.y);
        self.layer.set_line_height(size);
        self.layer.write_text(text, &self.font);
        self.layer.add_line_break();
        self.layer.end_text_section();
        self.y -= Mm::from(Pt(size));
    }
    fn add_sized_header(&mut self, text: &str, size: f64) {
        self.layer.begin_text_section();
        self.layer.set_font(&self.font, size);

        self.layer.set_text_cursor(self.w_margin, self.y);
        self.layer.set_line_height(size);
        self.layer.write_text(text, &self.font);
        self.layer.add_line_break();
        self.layer.end_text_section();
        self.y -= Mm::from(Pt(size));
    }

    fn add_header(&mut self, text: &str) {
        self.add_sized_header(text, 20.)
    }

    fn add_subheader(&mut self, text: &str) {
        self.add_sized_header(text, 15.)
    }

    fn vspace(&mut self, size: f64) {
        self.y -= Mm(size)
    }

    fn add_text(&mut self, text: &str) {
        let size_pt = 12.0;
        self.layer.begin_text_section();
        self.layer.set_font(&self.font, size_pt);
        self.layer.set_text_cursor(self.w_margin, self.y);
        self.layer.set_line_height(size_pt);

        let size_mm = Mm::from(Pt(size_pt));
        for line in text.lines() {
            if self.y < size_mm + self.h_margin {
                self.layer.end_text_section();
                self.next_page();
                self.layer.begin_text_section();
                self.layer.set_font(&self.font, size_pt);
                self.layer.set_text_cursor(self.w_margin, self.y);
                self.layer.set_line_height(size_pt);
            }

            self.layer.write_text(line, &self.font);
            self.layer.add_line_break();
            self.y -= size_mm;
        }
        self.layer.end_text_section();
    }

    fn next_page(&mut self) {
        let (page, layer) = self
            .pdf
            .add_page(self.w, self.h, format!("Page {}", self.n_pages));
        self.layer = self.pdf.get_page(page).get_layer(layer);
        self.page = page;
        self.y = self.h - self.h_margin;
        self.n_pages += 1;
    }
}

pub fn build(tree: Tree, roots: Vec<NodeId>, unlock: Time) -> PdfDocumentReference {
    let (w, h) = (Mm(210.), Mm(297.));
    let (pdf, page, layer1) = PdfDocument::new("Book-locker", w, h, "Layer 1");
    let layer = pdf.get_page(page).get_layer(layer1);
    let font = pdf.add_builtin_font(BuiltinFont::TimesRoman).unwrap();

    let mut doc = Doc {
        w,
        h,
        y: h - Mm(30.),
        font,
        layer,
        pdf,
        page,
        w_margin: Mm(30.),
        h_margin: Mm(30.),
        n_pages: 0,
    };

    doc.add_title("Folders are locked");
    doc.vspace(10.);
    doc.add_header(&format!(
        "Will unlock at: {}:{}",
        unlock.hour(),
        unlock.minute()
    ));
    doc.hline();
    doc.vspace(8.);
    doc.add_header("Locked files:");
    for root in roots {
        doc.vspace(8.);
        let subtree = tree.subtree(root);
        doc.add_subheader(&format!("path: {:?}", subtree.path));
        let subtree = format!("{subtree}");
        doc.add_text(&subtree);
    }

    doc.pdf
}

pub fn save(pdf: PdfDocumentReference) -> Result<()> {
    let mut writer = BufWriter::new(File::create("test.pdf")?);
    pdf.save(&mut writer)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::directory::test::test_tree;
    use crate::directory::Uuid;

    #[test]
    pub fn pdf() -> Result<()> {
        let tree = test_tree();
        let roots = vec![*tree.root(Uuid::from(""))];
        let pdf = build(tree, roots);
        let mut writer = BufWriter::new(File::create("test.pdf")?);
        pdf.save(&mut writer)?;
        Ok(())
    }
}
