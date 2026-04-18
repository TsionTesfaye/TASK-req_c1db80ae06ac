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
