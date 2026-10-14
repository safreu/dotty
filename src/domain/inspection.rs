use crate::domain::ManagedEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedEntryIssue {
    MissingSource,
    MissingTarget,
    TargetNotSymlink,
    WrongSourceKind,
    WrongSymlinkDestination,
}

impl std::fmt::Display for ManagedEntryIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            Self::MissingSource => "Missing source",
            Self::MissingTarget => "Missing target",
            Self::TargetNotSymlink => "Target is not a symlink",
            Self::WrongSourceKind => "Wrong source kind",
            Self::WrongSymlinkDestination => "Wrong symlink destination",
        };

        f.write_str(description)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedEntryInspection {
    entry: ManagedEntry,
    issues: Vec<ManagedEntryIssue>,
}

impl ManagedEntryInspection {
    pub fn new(entry: ManagedEntry, issues: Vec<ManagedEntryIssue>) -> Self {
        Self { entry, issues }
    }

    pub fn entry(&self) -> &ManagedEntry {
        &self.entry
    }

    pub fn issues(&self) -> &[ManagedEntryIssue] {
        &self.issues
    }

    pub fn is_healthy(&self) -> bool {
        self.issues().is_empty()
    }
}
