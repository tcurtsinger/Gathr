//! A PDF's bytes and a workbook's sheets, for a file's tab in the window, from a chat's folders only
//! (`files::inside`). The window draws a PDF itself; a workbook is read here by calamine.

use std::path::{Path, PathBuf};

use calamine::{Data, ExcelDateTime, Reader, SheetType, SheetVisible};
use serde::Serialize;

use crate::files::{Unshown, inside};

/// The largest PDF the window shows.
pub const PDF: u64 = 50 * 1024 * 1024;
/// The largest workbook the window shows: calamine holds a whole sheet in memory as it reads it.
pub const WORKBOOK: u64 = 10 * 1024 * 1024;
/// The most rows sent of a sheet: its heading row and the 2,000 the window's table draws.
pub const ROWS: usize = 2001;

/// The file at `path` inside one of `roots` with one of `endings`, at most `most` bytes.
fn found(roots: &[PathBuf], path: &str, endings: &[&str], most: u64) -> Result<PathBuf, Unshown> {
    let found = inside(roots, path)
        .filter(|f| f.is_file())
        .ok_or(Unshown::Gone)?;
    let ending = found.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !endings.iter().any(|e| e.eq_ignore_ascii_case(ending)) {
        return Err(Unshown::NotText);
    }
    let size = std::fs::metadata(&found).map_err(|_| Unshown::Gone)?.len();
    if size > most {
        return Err(Unshown::Large);
    }
    Ok(found)
}

/// The bytes of the PDF at `path` inside one of `roots`. Blocks on the disk.
pub fn pdf(roots: &[PathBuf], path: &str) -> Result<Vec<u8>, Unshown> {
    let found = found(roots, path, &["pdf"], PDF)?;
    std::fs::read(found).map_err(|_| Unshown::Gone)
}

/// One sheet of a workbook: its first `ROWS` rows, each cell as the person reads it, and how many
/// rows it has in all. The rows start at the sheet's first cell that holds something.
#[derive(Debug, Serialize)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<String>>,
    pub total: usize,
}

/// The visible worksheets of the workbook at `path` inside one of `roots`, in the workbook's order.
/// One calamine can't read (damaged, or locked with a password) is `NotText`. Blocks on the disk.
pub fn sheets(roots: &[PathBuf], path: &str) -> Result<Vec<Sheet>, Unshown> {
    let found = found(roots, path, &["xlsx", "xlsm", "xls", "ods"], WORKBOOK)?;
    read(&found).ok_or(Unshown::NotText)
}

fn read(path: &Path) -> Option<Vec<Sheet>> {
    let mut book = calamine::open_workbook_auto(path).ok()?;
    let names: Vec<String> = book
        .sheets_metadata()
        .iter()
        .filter(|s| s.typ == SheetType::WorkSheet && s.visible == SheetVisible::Visible)
        .map(|s| s.name.clone())
        .collect();
    let mut sheets = Vec::new();
    for name in names {
        let range = book.worksheet_range(&name).ok()?;
        let rows = range
            .rows()
            .take(ROWS)
            .map(|row| row.iter().map(cell).collect())
            .collect();
        let total = range.height();
        sheets.push(Sheet { name, rows, total });
    }
    Some(sheets)
}

/// A cell as the person reads it: a whole number without ".0", a date as `2026-05-02`, a time as
/// `14:30`, both as `2026-05-02 14:30`, a duration as `36:15:00`. The cell's own number format
/// (`2/5/2026`, `€84.00`) isn't read.
fn cell(data: &Data) -> String {
    match data {
        Data::Float(f) => number(*f),
        Data::Bool(true) => "TRUE".into(),
        Data::Bool(false) => "FALSE".into(),
        Data::DateTime(d) if d.is_duration() => duration(d.as_f64()),
        Data::DateTime(d) => date(d),
        Data::DateTimeIso(d) => d.replacen('T', " ", 1),
        other => other.to_string(),
    }
}

/// Excel shows 15 significant digits: a sum it stores as 0.30000000000000004 reads 0.3.
fn number(f: f64) -> String {
    let shown: f64 = format!("{f:.14e}").parse().unwrap_or(f);
    shown.to_string()
}

fn date(d: &ExcelDateTime) -> String {
    let (year, month, day, hour, minute, second, _) = d.to_ymd_hms_milli();
    let date = format!("{year:04}-{month:02}-{day:02}");
    let time = match second {
        0 => format!("{hour:02}:{minute:02}"),
        s => format!("{hour:02}:{minute:02}:{s:02}"),
    };
    let serial = d.as_f64();
    match (serial < 1.0, serial.fract() == 0.0) {
        (true, _) => time,
        (false, true) => date,
        (false, false) => format!("{date} {time}"),
    }
}

fn duration(days: f64) -> String {
    let seconds = (days * 86_400.0).round() as i64;
    let (hours, rest) = (seconds / 3600, seconds.abs() % 3600);
    format!("{hours}:{:02}:{:02}", rest / 60, rest % 60)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::folders;

    /// A workbook written as Excel writes one, a zip of XML parts (those calamine reads). Sheet one
    /// has text, a whole number, a decimal, a sum's float, a date, a date and time, and an empty
    /// cell; sheet two text and a yes; a third is hidden.
    fn workbook(path: &Path) {
        const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
        const PACKAGE: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        const OFFICE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
        let rels = |links: &[(&str, &str)]| {
            let links: String = links
                .iter()
                .enumerate()
                .map(|(i, (kind, to))| {
                    format!(
                        r#"<Relationship Id="rId{}" Type="{OFFICE}/{kind}" Target="{to}"/>"#,
                        i + 1
                    )
                })
                .collect();
            format!(r#"<Relationships xmlns="{PACKAGE}">{links}</Relationships>"#)
        };
        let text = |at: &str, words: &str| {
            format!(r#"<c r="{at}" t="inlineStr"><is><t>{words}</t></is></c>"#)
        };
        let value =
            |at: &str, style: u8, v: &str| format!(r#"<c r="{at}" s="{style}"><v>{v}</v></c>"#);
        let row = |cells: &[String]| format!("<row>{}</row>", cells.concat());
        let sheet = |rows: &[String]| {
            format!(
                r#"<worksheet xmlns="{MAIN}"><sheetData>{}</sheetData></worksheet>"#,
                rows.concat()
            )
        };
        let sheets = [("Orders", ""), ("Notes", ""), ("Old", r#" state="hidden""#)]
            .iter()
            .enumerate()
            .map(|(i, (name, state))| {
                format!(
                    r#"<sheet name="{name}" sheetId="{0}"{state} r:id="rId{0}"/>"#,
                    i + 1
                )
            })
            .collect::<String>();
        let parts = [
            (
                "_rels/.rels",
                rels(&[("officeDocument", "xl/workbook.xml")]),
            ),
            (
                "xl/workbook.xml",
                format!(
                    r#"<workbook xmlns="{MAIN}" xmlns:r="{OFFICE}"><sheets>{sheets}</sheets></workbook>"#
                ),
            ),
            (
                "xl/_rels/workbook.xml.rels",
                rels(&[
                    ("worksheet", "worksheets/sheet1.xml"),
                    ("worksheet", "worksheets/sheet2.xml"),
                    ("worksheet", "worksheets/sheet3.xml"),
                    ("styles", "styles.xml"),
                ]),
            ),
            (
                // Style 1 is Excel's own date format (14), style 2 its date and time (22).
                "xl/styles.xml",
                format!(
                    r#"<styleSheet xmlns="{MAIN}"><cellXfs count="3"><xf numFmtId="0"/><xf numFmtId="14"/><xf numFmtId="22"/></cellXfs></styleSheet>"#
                ),
            ),
            (
                "xl/worksheets/sheet1.xml",
                sheet(&[
                    row(&[
                        text("A1", "Order"),
                        text("B1", "Customer"),
                        text("C1", "Total"),
                        text("D1", "Date"),
                    ]),
                    row(&[
                        value("A2", 0, "1042"),
                        text("B2", "Lena Ward"),
                        value("C2", 0, "84.5"),
                        value("D2", 1, "46144"),
                    ]),
                    row(&[
                        value("A3", 0, "1043"),
                        value("C3", 0, "0.30000000000000004"),
                        value("D3", 2, "46144.5"),
                    ]),
                ]),
            ),
            (
                "xl/worksheets/sheet2.xml",
                sheet(&[
                    row(&[text("A1", "Note")]),
                    row(&[text("A2", "Ship on Monday")]),
                    row(&[r#"<c r="A3" t="b"><v>1</v></c>"#.to_string()]),
                ]),
            ),
            ("xl/worksheets/sheet3.xml", sheet(&[])),
        ];
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, xml) in parts {
            zip.start_file(name, stored).unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn a_workbook_reads_as_the_person_reads_it_and_only_in_the_chats_folders() {
        let base = std::env::temp_dir().join(format!("gathr-sheets-{}", crate::ids::span_id()));
        let (shared, elsewhere) = (base.join("Shared"), base.join("Elsewhere"));
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        workbook(&shared.join("orders.xlsx"));
        workbook(&elsewhere.join("secret.xlsx"));
        std::fs::write(shared.join("broken.xlsx"), b"not a workbook").unwrap();
        let roots = [folders::real(&shared)];
        let at = |p: PathBuf| p.to_string_lossy().into_owned();

        let read = sheets(&roots, &at(shared.join("orders.xlsx"))).unwrap();
        let names: Vec<&str> = read.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Orders", "Notes"], "the hidden sheet is left out");
        assert_eq!(
            read[0].rows,
            [
                ["Order", "Customer", "Total", "Date"],
                ["1042", "Lena Ward", "84.5", "2026-05-02"],
                ["1043", "", "0.3", "2026-05-02 12:00"],
            ]
        );
        assert_eq!(read[0].total, 3);
        assert_eq!(read[1].rows, [["Note"], ["Ship on Monday"], ["TRUE"]]);

        assert_eq!(
            sheets(&roots, &at(shared.join("broken.xlsx"))).unwrap_err(),
            Unshown::NotText,
            "a damaged workbook"
        );
        let out = shared.join("..").join("Elsewhere").join("secret.xlsx");
        assert_eq!(sheets(&roots, &at(out)).unwrap_err(), Unshown::Gone, "'..'");
        assert_eq!(
            sheets(&roots, &at(elsewhere.join("secret.xlsx"))).unwrap_err(),
            Unshown::Gone,
            "a workbook elsewhere"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
