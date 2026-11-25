use std::fmt;

/// CPU architecture of a Lambda function. These are the two names the Lambda
/// API uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Arch {
    /// Graviton / arm64. Declared first so sort order matches the name.
    Arm64,
    /// x86_64.
    X86_64,
}

impl Arch {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "arm64" => Some(Self::Arm64),
            "x86_64" => Some(Self::X86_64),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64",
            Self::X86_64 => "x86_64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Arch;

    #[test]
    fn only_the_two_api_names_parse() {
        assert_eq!(Arch::parse("arm64"), Some(Arch::Arm64));
        assert_eq!(Arch::parse("x86_64"), Some(Arch::X86_64));
        assert_eq!(Arch::parse("x86"), None);
        assert_eq!(Arch::parse("arm"), None);
        assert_eq!(Arch::parse("ARM64"), None);
        assert_eq!(Arch::parse(""), None);
    }

    #[test]
    fn arm_sorts_before_x86() {
        assert!(Arch::Arm64 < Arch::X86_64);
        assert_eq!(Arch::Arm64.as_str(), "arm64");
        assert_eq!(Arch::X86_64.as_str(), "x86_64");
    }
}
