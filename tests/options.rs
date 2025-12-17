#[cfg(test)]
mod tests {
    use std::process::Command;

    // 测试空字符分隔符选项
    #[test]
    fn test_null_delimiter() {
        let output = Command::new("./target/debug/utcpio")
            .arg("-0")
            .arg("--null")
            .arg("-o")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
    }

    // 测试交换字节选项
    #[test]
    fn test_swap_options() {
        let output = Command::new("./target/debug/utcpio")
            .arg("-s") // 交换字节
            .arg("-S") // 交换半字
            .arg("-i")
            .output()
            .expect("Failed to execute command");

        assert!(output.status.success());
    }
}
