use kortex::runtime::{Cx, FileData, block_on};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream, dictionary};
use tool_pdf_merge::{Inputs, Params, engine::run};

/// Build a PDF whose page N shows `labels[N]`. `inherit` puts MediaBox on the Pages node only.
fn make_pdf(name: &str, labels: &[&str], inherit: bool) -> FileData {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(
        dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
    );
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => font } });
    let mut kids = Vec::new();
    for label in labels {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![50.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(*label)]),
                Operation::new("ET", vec![]),
            ],
        };
        let cid = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("encode"),
        ));
        let mut page = dictionary! { "Type" => "Page", "Parent" => pages_id, "Contents" => cid };
        if !inherit {
            page.set("MediaBox", vec![0.into(), 0.into(), 595.into(), 842.into()]);
            page.set("Resources", resources);
        }
        kids.push(Object::Reference(doc.add_object(page)));
    }
    let mut pages = dictionary! { "Type" => "Pages", "Count" => i64::try_from(kids.len()).expect("len"), "Kids" => kids };
    if inherit {
        pages.set("MediaBox", vec![0.into(), 0.into(), 300.into(), 400.into()]);
        pages.set("Resources", resources);
    }
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save");
    FileData {
        name: name.to_owned(),
        mime: "application/pdf".to_owned(),
        bytes,
    }
}

fn merge(files: Vec<FileData>, params: Params) -> Result<FileData, &'static str> {
    block_on(run(Inputs { files }, params, &mut Cx::new()))
        .map(|o| o.merged)
        .map_err(|e| e.code)
}

fn page_texts(pdf: &FileData) -> Vec<String> {
    let doc = Document::load_mem(&pdf.bytes).expect("merged pdf loads");
    doc.get_pages()
        .keys()
        .map(|n| doc.extract_text(&[*n]).expect("text").trim().to_owned())
        .collect()
}

#[test]
fn merges_in_given_order_and_keeps_all_pages() {
    let a = make_pdf("a.pdf", &["A1"], false);
    let b = make_pdf("b.pdf", &["B1", "B2"], false);
    let c = make_pdf("c.pdf", &["C1"], false);
    let out = merge(vec![b, a, c], Params::default()).expect("merged");
    assert_eq!(out.name, "merged.pdf");
    assert_eq!(page_texts(&out), ["B1", "B2", "A1", "C1"]);
}

#[test]
fn inherited_mediabox_survives_reparenting() {
    let a = make_pdf("a.pdf", &["A1"], true);
    let b = make_pdf("b.pdf", &["B1"], false);
    let out = merge(vec![a, b], Params::default()).expect("merged");
    let doc = Document::load_mem(&out.bytes).expect("loads");
    let first = *doc.get_pages().values().next().expect("page");
    let media = doc
        .get_dictionary(first)
        .expect("page dict")
        .get(b"MediaBox")
        .expect("mediabox");
    let nums: Vec<i64> = media
        .as_array()
        .expect("array")
        .iter()
        .filter_map(|o| o.as_i64().ok())
        .collect();
    assert_eq!(nums, [0, 0, 300, 400]);
}

#[test]
fn compress_option_still_produces_valid_pdf() {
    let out = merge(
        vec![
            make_pdf("a.pdf", &["A1"], false),
            make_pdf("b.pdf", &["B1"], false),
        ],
        Params { compress: true },
    )
    .expect("merged");
    assert_eq!(page_texts(&out), ["A1", "B1"]);
}

#[test]
fn errors_on_too_few_files_and_invalid_pdf() {
    assert_eq!(
        merge(vec![make_pdf("a.pdf", &["A1"], false)], Params::default()).map(|_| ()),
        Err("need_two_files")
    );
    let junk = FileData {
        name: "x.pdf".into(),
        mime: "application/pdf".into(),
        bytes: b"not a pdf".to_vec(),
    };
    assert_eq!(
        merge(
            vec![make_pdf("a.pdf", &["A1"], false), junk],
            Params::default()
        )
        .map(|_| ()),
        Err("invalid_pdf")
    );
}

#[test]
fn dynamic_entry_rejects_a_file_that_is_not_a_pdf_by_content() {
    use kortex::runtime::{ToolError, Value, ValueMap};
    let fake = FileData {
        name: "report.pdf".into(),
        mime: "application/pdf".into(),
        bytes: b"plain text renamed to .pdf".to_vec(),
    };
    let real = make_pdf("real.pdf", &["A"], false);
    let rejected = block_on(tool_pdf_merge::run_map(
        &ValueMap::from([("files".to_owned(), Value::Files(vec![real.clone(), fake]))]),
        &ValueMap::new(),
        &mut Cx::new(),
    ));
    assert_eq!(
        rejected,
        Err(ToolError::with_detail("file_type_mismatch", "report.pdf"))
    );
    let accepted = block_on(tool_pdf_merge::run_map(
        &ValueMap::from([("files".to_owned(), Value::Files(vec![real.clone(), real]))]),
        &ValueMap::new(),
        &mut Cx::new(),
    ));
    assert!(accepted.is_ok(), "{accepted:?}");
}
