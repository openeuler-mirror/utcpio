/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::argp::*;
use crate::version_etc::*;
use std::io::Write;
use std::sync::OnceLock;

static PROGRAM_CANONICAL_NAME: OnceLock<&'static str> = OnceLock::new();
static PROGRAM_AUTHORS: OnceLock<&'static [&'static str]> = OnceLock::new();

fn version_etc_hook(stream: Option<&mut dyn Write>, _state: &mut ArgpState) {
    if let Some(stream) = stream {
        version_etc(
            stream,
            PROGRAM_CANONICAL_NAME.get().copied(),
            PACKAGE_NAME,
            VERSION,
            PROGRAM_AUTHORS.get().copied().unwrap_or(&[]),
        );
    }
}

/// Set up the version information for Argp.
pub fn argp_version_setup(name: Option<&'static str>, authors: Option<&'static [&'static str]>) {
    ARGP_PROGRAM_VERSION_HOOK.set(version_etc_hook).unwrap();

    if let Some(name) = name {
        PROGRAM_CANONICAL_NAME.set(name).unwrap();
    }
    if let Some(authors) = authors {
        PROGRAM_AUTHORS.set(authors).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

    // Mock writer for testing version_etc_hook
    struct MockWriter {
        content: String,
    }

    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.content.push_str(&String::from_utf8_lossy(buf));
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_version_etc_hook_with_stream() {
        let mut mock_writer = MockWriter {
            content: String::new(),
        };

        let mut state = ArgpState::default();
        version_etc_hook(Some(&mut mock_writer), &mut state);

        // Verify version info was written
        assert!(mock_writer.content.contains(PACKAGE_NAME));
        assert!(mock_writer.content.contains(VERSION));
    }

    #[test]
    fn test_version_etc_hook_without_stream() {
        let mut state = ArgpState::default();
        version_etc_hook(None, &mut state);
        // Should not panic when stream is None
    }

    #[test]
    fn test_argp_version_setup_with_values() {
        let name = "test_program";
        let authors = &["Author1", "Author2"];

        argp_version_setup(Some(name), Some(authors));

        assert_eq!(PROGRAM_CANONICAL_NAME.get().unwrap(), &name);
        assert_eq!(PROGRAM_AUTHORS.get().unwrap(), &authors);
    }

    #[test]
    fn test_argp_version_setup_with_none() {
        argp_version_setup(None, None);

        // Default values should remain unchanged
        assert!(PROGRAM_CANONICAL_NAME.get().is_none());
        assert!(PROGRAM_AUTHORS.get().unwrap().is_empty());
    }
}
