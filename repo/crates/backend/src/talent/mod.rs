//! Talent Intelligence module (T1–T13).
//!
//! Sub-modules:
//!   candidates  — T1 (list + search), T2 (create), T3 (get)
//!   roles_open  — T4 (list), T5 (create)
//!   scoring     — pure scoring functions (cold-start vs blended)
//!   search      — TSV full-text + filter logic
//!   weights     — T7 (get), T8 (put) — SELF-scoped
//!   watchlists  — T10 (list), T11 (create), T12 (add item), T13 (remove item) — SELF-scoped
//!   feedback    — T9 (create) — PERM(talent.feedback)
//!   handlers    — route wiring

pub mod candidates;
pub mod feedback;
pub mod handlers;
pub mod roles_open;
pub mod scoring;
pub mod search;
pub mod watchlists;
pub mod weights;
