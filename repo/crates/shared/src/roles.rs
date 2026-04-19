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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_as_db_and_display_are_stable_for_every_variant() {
        let expected: &[(Role, &str, &str)] = &[
            (Role::Administrator, "administrator", "Administrator"),
            (Role::DataSteward, "data_steward", "Data Steward"),
            (Role::Analyst, "analyst", "Analyst"),
            (Role::Recruiter, "recruiter", "Recruiter"),
            (Role::RegularUser, "regular_user", "Regular User"),
        ];
        for (r, db, disp) in expected {
            assert_eq!(r.as_db(), *db, "unexpected as_db for {r:?}");
            assert_eq!(r.display(), *disp, "unexpected display for {r:?}");
        }
        assert_eq!(Role::ALL.len(), expected.len());
        for (r, _, _) in expected {
            assert!(Role::ALL.contains(r), "{r:?} missing from ALL");
        }
    }

    #[test]
    fn role_serde_snake_case() {
        let s = serde_json::to_string(&Role::DataSteward).unwrap();
        assert_eq!(s, "\"data_steward\"");
        let back: Role = serde_json::from_str(&s).unwrap();
        assert_eq!(back, Role::DataSteward);
    }
}
