//! Authoritative permission codes. Seeded by migration `0002_rbac.sql` in P1.
//!
//! There is NO `notification.read` permission: notifications are purely
//! self-scoped (see `docs/design.md §Permissions`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    UserManage,
    RoleAssign,
    RetentionManage,
    MonitoringRead,
    AllowlistManage,
    MtlsManage,

    ProductRead,
    ProductWrite,
    ProductImport,
    ProductHistoryRead,
    RefWrite,

    MetricRead,
    MetricConfigure,

    AlertManage,
    AlertAck,

    ReportSchedule,
    ReportRun,

    KpiRead,

    TalentRead,
    TalentManage,
    TalentFeedback,
}

impl Permission {
    /// Canonical `permissions.code` database value.
    pub fn code(&self) -> &'static str {
        match self {
            Permission::UserManage => "user.manage",
            Permission::RoleAssign => "role.assign",
            Permission::RetentionManage => "retention.manage",
            Permission::MonitoringRead => "monitoring.read",
            Permission::AllowlistManage => "allowlist.manage",
            Permission::MtlsManage => "mtls.manage",
            Permission::ProductRead => "product.read",
            Permission::ProductWrite => "product.write",
            Permission::ProductImport => "product.import",
            Permission::ProductHistoryRead => "product.history.read",
            Permission::RefWrite => "ref.write",
            Permission::MetricRead => "metric.read",
            Permission::MetricConfigure => "metric.configure",
            Permission::AlertManage => "alert.manage",
            Permission::AlertAck => "alert.ack",
            Permission::ReportSchedule => "report.schedule",
            Permission::ReportRun => "report.run",
            Permission::KpiRead => "kpi.read",
            Permission::TalentRead => "talent.read",
            Permission::TalentManage => "talent.manage",
            Permission::TalentFeedback => "talent.feedback",
        }
    }

    pub const ALL: &'static [Permission] = &[
        Permission::UserManage,
        Permission::RoleAssign,
        Permission::RetentionManage,
        Permission::MonitoringRead,
        Permission::AllowlistManage,
        Permission::MtlsManage,
        Permission::ProductRead,
        Permission::ProductWrite,
        Permission::ProductImport,
        Permission::ProductHistoryRead,
        Permission::RefWrite,
        Permission::MetricRead,
        Permission::MetricConfigure,
        Permission::AlertManage,
        Permission::AlertAck,
        Permission::ReportSchedule,
        Permission::ReportRun,
        Permission::KpiRead,
        Permission::TalentRead,
        Permission::TalentManage,
        Permission::TalentFeedback,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_permission_has_stable_code_and_is_in_all() {
        // Assert code() is stable + distinct for every Permission. This is
        // the canonical permissions.code DB column value, shared with the
        // 0002_rbac.sql seed.
        let expected: &[(Permission, &str)] = &[
            (Permission::UserManage, "user.manage"),
            (Permission::RoleAssign, "role.assign"),
            (Permission::RetentionManage, "retention.manage"),
            (Permission::MonitoringRead, "monitoring.read"),
            (Permission::AllowlistManage, "allowlist.manage"),
            (Permission::MtlsManage, "mtls.manage"),
            (Permission::ProductRead, "product.read"),
            (Permission::ProductWrite, "product.write"),
            (Permission::ProductImport, "product.import"),
            (Permission::ProductHistoryRead, "product.history.read"),
            (Permission::RefWrite, "ref.write"),
            (Permission::MetricRead, "metric.read"),
            (Permission::MetricConfigure, "metric.configure"),
            (Permission::AlertManage, "alert.manage"),
            (Permission::AlertAck, "alert.ack"),
            (Permission::ReportSchedule, "report.schedule"),
            (Permission::ReportRun, "report.run"),
            (Permission::KpiRead, "kpi.read"),
            (Permission::TalentRead, "talent.read"),
            (Permission::TalentManage, "talent.manage"),
            (Permission::TalentFeedback, "talent.feedback"),
        ];
        for (p, code) in expected {
            assert_eq!(p.code(), *code, "unexpected code for {p:?}");
        }
        assert_eq!(Permission::ALL.len(), expected.len());
        for (p, _) in expected {
            assert!(Permission::ALL.contains(p), "{p:?} missing from ALL");
        }
        // All codes must be distinct.
        let mut codes: Vec<&str> = Permission::ALL.iter().map(|p| p.code()).collect();
        codes.sort();
        let before = codes.len();
        codes.dedup();
        assert_eq!(codes.len(), before, "duplicate permission code");
    }

    #[test]
    fn permission_serde_uses_snake_case_variant_names() {
        // Enum is #[serde(rename_all = "snake_case")]. Roundtrip proves the
        // wire format stays stable across releases.
        let s = serde_json::to_string(&Permission::ProductHistoryRead).unwrap();
        assert_eq!(s, "\"product_history_read\"");
        let back: Permission = serde_json::from_str(&s).unwrap();
        assert_eq!(back, Permission::ProductHistoryRead);
    }
}
