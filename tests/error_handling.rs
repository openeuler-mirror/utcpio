#[cfg(test)]
mod tests {
    use std::process::Command;

    // 测试无效参数
    #[test]
    fn test_invalid_argument() {
        let output = Command::new("./target/debug/utcpio")
            .arg("--invalid-arg")
            .output()
            .expect("Failed to execute command");

        assert!(!output.status.success());
    }

    // 测试缺少必需参数
    #[test]
    fn test_missing_required_argument() {
        let output = Command::new("./target/debug/utcpio")
            .arg("-F") // 缺少文件名
            .output()
            .expect("Failed to execute command");

        assert!(!output.status.success());
    }
}
