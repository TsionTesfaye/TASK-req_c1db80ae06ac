//! DTOs for the Talent Intelligence module (T1–T13).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Candidates ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateListItem {
    pub id: Uuid,
    pub full_name: String,
    pub email_mask: String,
    pub location: Option<String>,
    pub years_experience: i32,
    pub skills: Vec<String>,
    pub completeness_score: i32,
    pub last_active_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateDetail {
    pub id: Uuid,
    pub full_name: String,
    pub email_mask: String,
    pub location: Option<String>,
    pub years_experience: i32,
    pub skills: Vec<String>,
    pub bio: Option<String>,
    pub completeness_score: i32,
    pub last_active_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertCandidateRequest {
    pub full_name: String,
    pub email_mask: String,
    pub location: Option<String>,
    pub years_experience: i32,
    pub skills: Vec<String>,
    pub bio: Option<String>,
    pub completeness_score: i32,
    pub last_active_at: Option<DateTime<Utc>>,
}

// ── Open Roles ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleOpenItem {
    pub id: Uuid,
    pub title: String,
    pub department_id: Option<Uuid>,
    pub required_skills: Vec<String>,
    pub min_years: i32,
    pub site_id: Option<Uuid>,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRoleRequest {
    pub title: String,
    pub department_id: Option<Uuid>,
    pub required_skills: Vec<String>,
    pub min_years: i32,
    pub site_id: Option<Uuid>,
    pub status: Option<String>,
}

// ── Recommendations ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecommendationResult {
    pub cold_start: bool,
    pub total_feedback: i64,
    pub role_id: Uuid,
    pub candidates: Vec<RankedCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedCandidate {
    pub candidate: CandidateListItem,
    pub score: f64,
    pub reasons: Vec<String>,
}

// ── Weights ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalentWeights {
    pub user_id: Uuid,
    pub skills_weight: i32,
    pub experience_weight: i32,
    pub recency_weight: i32,
    pub completeness_weight: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateWeightsRequest {
    pub skills_weight: i32,
    pub experience_weight: i32,
    pub recency_weight: i32,
    pub completeness_weight: i32,
}

// ── Watchlists ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistItem {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub item_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWatchlistRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WatchlistEntry {
    pub candidate: CandidateListItem,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddWatchlistItemRequest {
    pub candidate_id: Uuid,
}

// ── Feedback ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFeedbackRequest {
    pub candidate_id: Uuid,
    pub role_id: Option<Uuid>,
    pub thumb: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedbackRecord {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub role_id: Option<Uuid>,
    pub owner_id: Uuid,
    pub thumb: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}
