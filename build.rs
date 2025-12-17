use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
fn main() {
    // 定义输入目录
    let po_dir: &Path = Path::new("po");
    let doc_dir: &Path = Path::new("doc");
    // 获取构建 profile (debug 或 release)
    let profile = env::var("PROFILE").unwrap();
    // 定义输出目录，根据 profile 放在 target 目录下
    let out_dir = Path::new("target").join(&profile);
    let locale_out_dir = out_dir.join("locale");
    let domain = "utcpio"; // 与你的项目名称一致
                           // 确保输出目录存在
    fs::create_dir_all(&locale_out_dir).expect("Failed to create output directory");
    // 遍历 po 目录中的 .po 文件
    for entry in fs::read_dir(po_dir).expect("Failed to read po directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("po") {
            let lang = path.file_stem().unwrap().to_str().unwrap();
            let mo_dir = locale_out_dir.join(lang).join("LC_MESSAGES");
            fs::create_dir_all(&mo_dir).expect("Failed to create locale directory");
            let mo_file = mo_dir.join(format!("{}.mo", domain));
            // 调用 msgfmt 生成 .mo 文件
            let status = Command::new("msgfmt")
                .arg(&path)
                .arg("-o")
                .arg(&mo_file)
                .status()
                .expect("Failed to execute msgfmt");
            if !status.success() {
                panic!("msgfmt failed for {}", path.display());
            }
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    // 复制 doc 目录
    let dest_path_doc = out_dir.join("doc");
    // Create destination directories if they don't exist
    fs::create_dir_all(&dest_path_doc).unwrap();
    if doc_dir.exists() && doc_dir.is_dir() {
        for entry in fs::read_dir(doc_dir).unwrap() {
            let entry = entry.unwrap();
            let file_path = entry.path();
            if file_path.is_file() {
                let dest_file_path = dest_path_doc.join(entry.file_name());
                fs::copy(&file_path, dest_file_path).unwrap();
                // println!("cargo:warning=Copied doc file: {:?}", file_path);
            }
        }
    }
    // 通知 Cargo 当 po 和 doc 文件变化时重新运行构建脚本
    println!("cargo:rerun-if-changed=po");
    println!("cargo:rerun-if-changed=doc");
    // 获取安装目录
    let install_root = env::var("CARGO_INSTALL_ROOT").unwrap_or("./run".to_string());
    // let install_path = Path::new(&install_root);
    // 获取 buildroot 环境变量 (如果存在)
    let buildroot = env::var("RPM_BUILD_ROOT").unwrap_or(install_root.clone());
    let buildroot_path = Path::new(&buildroot);
    // 复制 doc 目录
    let dest_path_doc = buildroot_path.join("share/doc/utcpio");
    fs::create_dir_all(&dest_path_doc).unwrap();
    if doc_dir.exists() && doc_dir.is_dir() {
        for entry in fs::read_dir(doc_dir).unwrap() {
            let entry = entry.unwrap();
            let file_path = entry.path();
            if file_path.is_file() {
                let dest_file_path = dest_path_doc.join(entry.file_name());
                fs::copy(&file_path, dest_file_path).unwrap();
                // println!("cargo:warning=Copied doc file: {:?}", file_path);
            }
        }
    }
    // 复制 po 目录
    let dest_path_locale = buildroot_path.join("share/locale");
    fs::create_dir_all(&dest_path_locale).unwrap();
    if po_dir.exists() && po_dir.is_dir() {
        for entry in fs::read_dir(po_dir).unwrap() {
            let entry = entry.unwrap();
            let file_path = entry.path();
            if file_path.is_file() && file_path.extension().and_then(|s| s.to_str()) == Some("po") {
                let lang = file_path.file_stem().unwrap().to_str().unwrap();
                let mo_dir = dest_path_locale.join(lang).join("LC_MESSAGES");
                fs::create_dir_all(&mo_dir).expect("Failed to create locale directory");
                let mo_file = mo_dir.join("utcpio.mo");
                // 调用 msgfmt 生成 .mo 文件
                let status = Command::new("msgfmt")
                    .arg(&file_path)
                    .arg("-o")
                    .arg(&mo_file)
                    .status()
                    .expect("Failed to execute msgfmt");
                if !status.success() {
                    panic!("msgfmt failed for {}", file_path.display());
                }
                // println!("cargo:warning=Copied po file: {:?}", file_path);
            }
        }
    }
}
