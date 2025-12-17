#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    // 测试复制传递模式
    #[test]
    fn test_pass_through_mode() {
        fs::create_dir_all("source").unwrap();
        fs::write("source/test2.txt", "test content").unwrap();

        let output = Command::new("./target/debug/utcpio")
            .arg("-p") // 传递模式
            .arg("dest")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
        assert!(fs::metadata("dest/source/test2.txt").is_ok());
    }

    // 测试带目录创建的提取模式
    #[test]
    fn test_extract_with_directories() {
        let output = Command::new("./target/debug/utcpio")
            .arg("-id") // 提取并创建目录
            .arg("-F")
            .arg("test.cpio")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
    }
}
