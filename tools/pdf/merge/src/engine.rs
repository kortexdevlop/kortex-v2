//! Merge PDFs in the given order. Pages keep their inherited attributes
//! (`MediaBox`, `Resources`, `Rotate`, `CropBox`) when re-parented under the new page tree.
use std::collections::BTreeMap;

use kortex::runtime::{Cx, FileData, ToolError};
use lopdf::{Document, Object, ObjectId};

use crate::{Inputs, Outputs, Params};

const INHERITABLE: [&[u8]; 4] = [b"Resources", b"MediaBox", b"CropBox", b"Rotate"];

/// Copy attributes a page inherits from its `Pages` ancestors onto the page itself.
fn flatten_inheritance(doc: &mut Document, page: ObjectId) {
    let mut found: Vec<(&[u8], Object)> = Vec::new();
    let mut parent = doc
        .get_dictionary(page)
        .ok()
        .and_then(|d| d.get(b"Parent").ok())
        .and_then(|o| o.as_reference().ok());
    let mut guard = 0;
    while let Some(pid) = parent {
        guard += 1;
        let Ok(dict) = doc.get_dictionary(pid) else {
            break;
        };
        for key in INHERITABLE {
            if let Ok(v) = dict.get(key)
                && !found.iter().any(|(k, _)| *k == key)
            {
                found.push((key, v.clone()));
            }
        }
        parent = dict.get(b"Parent").ok().and_then(|o| o.as_reference().ok());
        if guard > 64 {
            break;
        }
    }
    if let Ok(Object::Dictionary(page_dict)) = doc.get_object_mut(page) {
        for (key, value) in found {
            if !page_dict.has(key) {
                page_dict.set(key, value);
            }
        }
    }
}

fn load(file: &FileData) -> Result<Document, ToolError> {
    let doc = Document::load_mem(&file.bytes)
        .map_err(|_| ToolError::with_detail("invalid_pdf", file.name.clone()))?;
    if doc.is_encrypted() {
        return Err(ToolError::with_detail("encrypted_pdf", file.name.clone()));
    }
    Ok(doc)
}

#[kortex::engine]
pub async fn run(input: Inputs, params: Params, cx: &mut Cx) -> Result<Outputs, ToolError> {
    params.validate()?;
    if input.files.len() < 2 {
        return Err(ToolError::new("need_two_files"));
    }
    let mut max_id = 1;
    let mut pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let total = input.files.len();

    for (i, file) in input.files.iter().enumerate() {
        cx.check_cancelled()?;
        let mut doc = load(file)?;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;
        let page_ids: Vec<ObjectId> = doc.get_pages().into_values().collect();
        if page_ids.is_empty() {
            return Err(ToolError::with_detail("invalid_pdf", file.name.clone()));
        }
        for id in &page_ids {
            flatten_inheritance(&mut doc, *id);
        }
        for id in page_ids {
            if let Ok(obj) = doc.get_object(id) {
                pages.insert(id, obj.clone());
            }
        }
        objects.extend(doc.objects);
        #[allow(clippy::cast_precision_loss)]
        cx.progress((i + 1) as f32 / (total + 1) as f32);
    }

    let mut out = Document::with_version("1.5");
    let pages_id = (max_id + 1, 0);
    let catalog_id = (max_id + 2, 0);
    for (id, obj) in objects {
        match obj.type_name().unwrap_or(b"") {
            b"Catalog" | b"Pages" | b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                out.objects.insert(id, obj);
            }
        }
    }
    let kids: Vec<Object> = pages.keys().copied().map(Object::Reference).collect();
    for (id, obj) in pages {
        if let Ok(dict) = obj.as_dict() {
            let mut dict = dict.clone();
            dict.set("Parent", Object::Reference(pages_id));
            out.objects.insert(id, Object::Dictionary(dict));
        }
    }
    let count = i64::try_from(kids.len()).unwrap_or(0);
    out.objects.insert(
        pages_id,
        Object::Dictionary(
            lopdf::dictionary! { "Type" => "Pages", "Count" => count, "Kids" => kids },
        ),
    );
    out.objects.insert(
        catalog_id,
        Object::Dictionary(lopdf::dictionary! { "Type" => "Catalog", "Pages" => pages_id }),
    );
    out.trailer.set("Root", Object::Reference(catalog_id));
    out.max_id = catalog_id.0;
    out.renumber_objects();
    if params.compress {
        out.compress();
    }
    let mut bytes = Vec::new();
    out.save_to(&mut bytes)
        .map_err(|_| ToolError::new("write_failed"))?;
    cx.progress(1.0);
    Ok(Outputs {
        merged: FileData {
            name: "merged.pdf".to_owned(),
            mime: "application/pdf".to_owned(),
            bytes,
        },
    })
}
