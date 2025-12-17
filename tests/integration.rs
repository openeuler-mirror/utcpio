#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    // 测试完整工作流：创建->列表->提取
    #[test]
    fn test_full_workflow() {
        // 创建测试文件
        fs::write("test3.txt", "integration test").unwrap();

        // 创建归档
        let create = Command::new("./target/debug/utcpio")
            .arg("-o")
            .arg("-F")
            .arg("full.cpio")
            .output()
            .expect("Failed to create archive");
        assert!(create.status.success());

        // 列出归档内容
        let list = Command::new("./target/debug/utcpio")
            .arg("-t")
            .arg("-F")
            .arg("full.cpio")
            .output()
            .expect("Failed to list archive");
        assert!(list.status.success());

        // 提取归档
        let extract = Command::new("./target/debug/utcpio")
            .arg("-i")
            .arg("-F")
            .arg("full.cpio")
            .output()
            .expect("Failed to extract archive");
        assert!(extract.status.success());
    }
}
