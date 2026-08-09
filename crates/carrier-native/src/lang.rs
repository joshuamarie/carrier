use anyhow::{bail, Result};

/// A supported native-code language for a module's `[native]` block.
/// Parsed from the `--native` CLI flag / `carrier.toml`, and used to
/// pick the right scaffold (starter source file + build glue) when a
/// module opts into compiled code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLang {
    C,
    Cpp,
    // Rust,
    Fortran,
}

impl NativeLang {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "c" => Ok(NativeLang::C),
            "cpp" | "c++" | "cxx" => Ok(NativeLang::Cpp),
            // "rust" | "rs" => Ok(NativeLang::Rust),
            "fortran" | "f90" | "f" => Ok(NativeLang::Fortran),
            other => bail!(
                "Unknown native language '{other}'. Expected one of: c, cpp, rust, fortran"
            ),
        }
    }

    /// File extension used for the generated starter source file.
    pub fn src_extension(self) -> &'static str {
        match self {
            NativeLang::C => "c",
            NativeLang::Cpp => "cpp",
            // NativeLang::Rust => "rs",
            NativeLang::Fortran => "f90",
        }
    }
}