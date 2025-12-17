#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    // 测试创建归档文件
    #[test]
    fn test_create_archive() {
        fs::write("test1.txt", "test content").unwrap();

        let output = Command::new("./target/debug/utcpio")
            .arg("-o")
            .arg("-F")
            .arg("test.cpio")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        assert!(fs::metadata("test.cpio").is_ok());
    }

    // 测试提取归档文件
    #[test]
    fn test_extract_archive() {
        let output = Command::new("./target/debug/utcpio")
            .arg("-i")
            .arg("--extract")
            .arg("-F")
            .arg("test.cpio")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        assert!(fs::metadata("test1.txt").is_ok());
    }
}
