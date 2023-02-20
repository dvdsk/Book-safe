use color_eyre::eyre::Context;
use color_eyre::Result;
use indextree::NodeId;
use printpdf::{
    BuiltinFont, IndirectFontRef, Line, Mm, PdfDocument, PdfDocumentReference, PdfLayerReference,
    PdfPageIndex, Point, Pt,
};
use std::fs::{self, File};
use std::io::{BufWriter, ErrorKind};
use std::path::Path;
use time::Time;

use crate::directory::{self, Tree};
use crate::util::AcceptErr;

pub struct Doc {
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
            (Point::new(self.w - self.w_margin, self.y), false),
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
        self.add_sized_header(text, 20.);
    }

    fn add_subheader(&mut self, text: &str) {
        self.add_sized_header(text, 15.);
    }

    fn vspace(&mut self, size: f64) {
        self.y -= Mm(size);
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

pub fn build(tree: &Tree, roots: Vec<NodeId>, missing: Vec<String>, unlock: Time) -> Doc {
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
    if !missing.is_empty() {
        doc.add_header("Missing paths:");
        doc.add_text("Could not find these paths, if they where not deleted since book-safe was installed\n there is a bug in book safe. Please report it at github.com/dvdsk/book-safe");
        for path in missing {
            doc.add_subheader(&format!("- {}", &path));
        }
    }
    doc.vspace(10.);
    doc.add_header(&format!(
        "Will unlock at: {}:{:02}",
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

    doc
}

fn metadata() -> String {
    let unix_ts = time::OffsetDateTime::now_utc().unix_timestamp();
    format!(
        "{{
    \"deleted\": false,
    \"lastModified\": \"{unix_ts}000\",
    \"metadatamodified\": false,
    \"modified\": false,
    \"parent\": \"\",
    \"pinned\": false,
    \"synced\": true,
    \"type\": \"DocumentType\",
    \"version\": 1,
    \"visibleName\": \"Locked Books\"
}}"
    )
}

fn content(pages: usize) -> String {
    format!(
        "{{
    \"extraMetadata\": {{    
    }},
    \"fileType\": \"pdf\",
    \"fontName\": \"\",
    \"lastOpenedPage\": 0,
    \"lineHeight\": -1,
    \"margins\": 100,
    \"orientation\": \"portrait\",
    \"pageCount\": {pages},
    \"pages\": [
    ],
    \"textScale\": 1,
    \"transform\": {{
        \"m11\": 1,
        \"m12\": 0,
        \"m13\": 0,
        \"m21\": 0,
        \"m22\": 1,
        \"m23\": 0,
        \"m31\": 0,
        \"m32\": 0,
        \"m33\": 1
    }}
}}"
    )
}

const REPORT_UUID: &str = "64a3befb-b815-47e8-bf74-996bb6a76a5d";
pub fn save(doc: Doc) -> Result<()> {
    log::info!("report uuid: {REPORT_UUID} (constant)");
    let path = Path::new(directory::DIR).join(REPORT_UUID);

    fs::write(path.with_extension("content"), content(doc.n_pages))?;
    fs::write(path.with_extension("metadata"), metadata())?;
    fs::write(path.with_extension("pagedata"), "")?;
    for dir_ext in &["", "cache", "highlights", "thumbnails", "textconversion"] {
        fs::create_dir(path.with_extension(dir_ext))
            .accept_fn(|e| {
                e.kind() == ErrorKind::AlreadyExists && path.with_extension(dir_ext).is_dir()
            })
            .wrap_err_with(|| format!("Failed to create {dir_ext} dir"))?;
    }

    let mut writer = BufWriter::new(File::create(path.with_extension("pdf"))?);
    doc.pdf.save(&mut writer)?;
    log::info!("added report on locked files (pdf)");
    Ok(())
}

pub fn remove() -> Result<()> {
    let path = Path::new(directory::DIR).join(REPORT_UUID);
    assert!(!REPORT_UUID.is_empty(), "report uuid is empty str");
    let files = ["content", "metadata", "pagedata", "pdf"];
    let dirs = ["", "cache", "highlights", "thumbnails", "textconversion"];

    // check for the last dir we remove, if its not here neither will
    // the other files be so nothing can be removed
    if !path.with_extension(dirs.last().unwrap()).is_dir() {
        log::warn!("no lock report to remove: was not locked or report got corrupted");
        return Ok(());
    }

    for file_ext in &files {
        fs::remove_file(path.with_extension(file_ext))
            .wrap_err_with(|| format!("Failed to remove file: {file_ext}"))?;
    }
    for dir_ext in &dirs {
        fs::remove_dir_all(path.with_extension(dir_ext))
            .wrap_err_with(|| format!("Failed to remove dir: {dir_ext}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::directory::test::test_tree;

    #[test]
    pub fn pdf() -> Result<()> {
        simplelog::SimpleLogger::init(log::LevelFilter::Warn, simplelog::Config::default())
            .unwrap();
        let tree = test_tree();
        let roots = vec![*tree.root()];
        let missing = vec![
            "missing_path".to_owned(),
            "another missing path.pdf".to_owned(),
        ];
        let doc = build(
            &tree,
            roots,
            missing,
            time::Time::from_hms(12, 42, 59).unwrap(),
        );

        if built::util::detect_ci().is_some() {
            log::warn!("skipping doc save as it fails on CI");
            return Ok(());
        }

        save(doc)?; // this fails on many CI platforms
        Ok(())
    }
}
