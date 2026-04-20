//! Shared DTOs, errors, roles, permissions, pagination, and time helpers.
//!
//! Consumed by both `terraops-backend` and `terraops-frontend`. Scaffold level:
//! stable type surfaces are present; implementation details (e.g. formula
//! executors, ranking math) arrive in P1+.

pub mod error;
pub mod pagination;
pub mod permissions;
pub mod roles;
pub mod time;
pub mod tristate;

pub mod dto;
