use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let font_path = out_dir.join("InterVariable.ttf");

    if font_path.exists() {
        return;
    }

    // Download the zip
    let output = std::process::Command::new("curl")
        .args(["-sL", "https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip"])
        .output()
        .expect("curl must be available");

    assert!(output.status.success(), "failed to download Inter zip");

    // Extract InterVariable.ttf from the zip
    let cursor = std::io::Cursor::new(output.stdout);
    let mut archive = zip::ZipArchive::new(cursor).expect("valid zip");
    let mut file = archive.by_name("InterVariable.ttf").expect("InterVariable.ttf in zip");
    let mut buf = Vec::with_capacity(file.size() as usize);
    std::io::Read::read_to_end(&mut file, &mut buf).expect("read entry");
    std::fs::write(&font_path, buf).expect("write font file");
}
