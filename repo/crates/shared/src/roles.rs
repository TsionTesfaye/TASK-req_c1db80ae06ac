//! The five canonical workspace role names — verbatim from the product prompt.
//!
//! Do not add, rename, or collapse these without an explicit planning change.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Administrator,
    DataSteward,
    Analyst,
    Recruiter,
    RegularUser,
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::Administrator,
        Role::DataSteward,
        Role::Analyst,
        Role::Recruiter,
        Role::RegularUser,
    ];

    /// Canonical `roles.name` database value.
    pub fn as_db(&self) -> &'static str {
        match self {
            Role::Administrator => "administrator",
            Role::DataSteward => "data_steward",
            Role::Analyst => "analyst",
            Role::Recruiter => "recruiter",
            Role::RegularUser => "regular_user",
        }
    }

    /// Display name shown in the UI nav.
    pub fn display(&self) -> &'static str {
        match self {
            Role::Administrator => "Administrator",
            Role::DataSteward => "Data Steward",
            Role::Analyst => "Analyst",
            Role::Recruiter => "Recruiter",
            Role::RegularUser => "Regular User",
        }
    }
}
