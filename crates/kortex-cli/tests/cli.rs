use std::fs;
use std::path::PathBuf;

fn run(args: &[&str]) -> Result<String, String> {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut out = Vec::new();
    kortex_cli::execute(&args, &mut out).map(|()| String::from_utf8(out).expect("utf8"))
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("kortex-cli-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).expect("tmp dir");
    d
}

#[test]
fn every_registered_tool_is_linked_into_the_cli() {
    // registry/index.json is the source of truth; the CLI list is generated from the same tools.
    let registry = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../registry/index.json"
    ))
    .expect("registry");
    let ids: Vec<&str> = registry
        .lines()
        .filter_map(|l| l.trim().strip_prefix("\"id\": \""))
        .filter_map(|l| l.split('"').next())
        .filter(|id| id.contains('.')) // tool ids only (`universe.name`)
        .collect();
    assert!(!ids.is_empty());
    let listed = run(&["list", "--lang", "en"]).expect("list");
    for id in ids {
        assert!(
            listed.lines().any(|l| l.starts_with(&format!("{id}\t"))),
            "{id} missing from `kortex list`"
        );
    }
}

#[test]
fn run_text_tool_and_typed_params() {
    let out = run(&[
        "run",
        "secure.hash-calculator",
        "--input",
        "text=abc",
        "--param",
        "algorithm=md5",
    ])
    .expect("run");
    assert!(
        out.contains("hash: 900150983cd24fb0d6963f7d28e17f72"),
        "{out}"
    );
    let upper = run(&[
        "run",
        "secure.hash-calculator",
        "--input",
        "text=abc",
        "--param",
        "algorithm=md5",
        "--param",
        "uppercase=true",
    ])
    .expect("run");
    assert!(
        upper.contains("900150983CD24FB0D6963F7D28E17F72"),
        "{upper}"
    );
}

#[test]
fn file_inputs_via_at_path_and_text_from_file() {
    let d = tmp("files");
    fs::write(d.join("a.txt"), "abc").expect("write");
    let by_file = run(&[
        "run",
        "secure.hash-calculator",
        "--input",
        &format!("file=@{}", d.join("a.txt").display()),
        "--param",
        "algorithm=sha1",
    ])
    .expect("run");
    assert!(
        by_file.contains("a9993e364706816aba3e25717850c26c9cd0d89d"),
        "{by_file}"
    );
    let by_text_file = run(&[
        "run",
        "secure.hash-calculator",
        "--input",
        &format!("text=@{}", d.join("a.txt").display()),
    ])
    .expect("run");
    assert!(
        by_text_file.contains("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
        "{by_text_file}"
    );
}

#[test]
fn file_outputs_are_written_and_never_overwritten() {
    let d = tmp("out");
    let args = [
        "run",
        "qr.generator",
        "--input",
        "content=hello",
        "--param",
        "format=svg",
        "--out",
        d.to_str().expect("path"),
    ];
    let first = run(&args).expect("run");
    let second = run(&args).expect("run");
    assert!(
        first.contains("qrcode.svg") && second.contains("qrcode (2).svg"),
        "{first} / {second}"
    );
    assert!(
        fs::read_to_string(d.join("qrcode.svg"))
            .expect("svg")
            .starts_with("<svg")
    );
    assert!(d.join("qrcode (2).svg").exists());
}

#[test]
fn json_output_is_machine_readable() {
    let out = run(&[
        "run",
        "secure.hash-calculator",
        "--input",
        "text=a\"b",
        "--param",
        "algorithm=md5",
        "--json",
    ])
    .expect("run");
    let line = out.trim();
    assert!(
        line.starts_with("{\"hash\":\"") && line.ends_with('}'),
        "{line}"
    );
    assert!(line.contains("\"verdict\":\"none\""));
}

#[test]
fn multi_file_input_and_pdf_merge_from_the_command_line() {
    let d = tmp("pdf");
    // Tiny valid PDFs (hand-written, same as the browser test).
    let pdf = |label: &str| {
        let content = format!("BT /F1 24 Tf 50 700 Td ({label}) Tj ET");
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
        ];
        let mut out = String::from("%PDF-1.4\n");
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.push_str(&format!("{} 0 obj\n{o}\nendobj\n", i + 1));
        }
        let x = out.len();
        out.push_str(&format!(
            "xref\n0 {}\n0000000000 65535 f \n",
            objs.len() + 1
        ));
        for o in offs {
            out.push_str(&format!("{o:010} 00000 n \n"));
        }
        out.push_str(&format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{x}\n%%EOF\n",
            objs.len() + 1
        ));
        out
    };
    fs::write(d.join("a.pdf"), pdf("FIRST")).expect("write");
    fs::write(d.join("b.pdf"), pdf("SECOND")).expect("write");
    let out = run(&[
        "run",
        "pdf.merge",
        "--input",
        &format!("files=@{}", d.join("b.pdf").display()),
        "--input",
        &format!("files=@{}", d.join("a.pdf").display()),
        "--out",
        d.join("o").to_str().expect("path"),
    ])
    .expect("merge");
    assert!(out.contains("merged.pdf"), "{out}");
    let merged = fs::read(d.join("o/merged.pdf")).expect("merged file");
    assert!(merged.starts_with(b"%PDF-"));
    let text = String::from_utf8_lossy(&merged);
    assert!(
        text.find("SECOND").expect("second") < text.find("FIRST").expect("first"),
        "order must follow the arguments"
    );
}

#[test]
fn search_info_and_errors() {
    assert!(
        run(&["search", "mot", "de", "passe", "--lang", "fr"])
            .expect("search")
            .contains("secure.password-generator")
    );
    assert!(run(&["search", "zzzzqq"]).expect("search").is_empty());
    let info = run(&["info", "pdf.merge"]).expect("info");
    assert!(info.contains("files<application/pdf>") && info.contains("(requis)"));
    assert!(run(&["run", "nope.nope"]).is_err_and(|e| e.contains("outil inconnu")));
    assert!(
        run(&["run", "secure.hash-calculator", "--param", "bogus=1"])
            .is_err_and(|e| e.contains("paramètre inconnu"))
    );
    assert!(
        run(&[
            "run",
            "secure.hash-calculator",
            "--input",
            "text=a",
            "--param",
            "uppercase=maybe"
        ])
        .is_err_and(|e| e.contains("booléen"))
    );
    assert!(
        run(&["run", "calc.gross-net-salary", "--param", "amount=abc"])
            .is_err_and(|e| e.contains("nombre"))
    );
    assert!(
        run(&[
            "run",
            "pdf.merge",
            "--input",
            "files=@/definitely/missing.pdf"
        ])
        .is_err_and(|e| e.contains("impossible"))
    );
    assert!(run(&["frobnicate"]).is_err());
    assert!(run(&[]).expect("usage").contains("Usage"));
}
