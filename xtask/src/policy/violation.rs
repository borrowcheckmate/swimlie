use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) location: String,
    pub(crate) message: String,
}

impl Violation {
    pub(crate) fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            message: message.into(),
        }
    }

    pub(crate) fn at_line(location: &str, line: usize, message: impl Into<String>) -> Self {
        Self::new(format!("{location}:{line}"), message)
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::Violation;

    #[test]
    fn displays_location_and_message() {
        let violation = Violation::at_line("src/lib.rs", 3, "bad");
        assert_eq!(violation.to_string(), "src/lib.rs:3: bad");
    }
}
