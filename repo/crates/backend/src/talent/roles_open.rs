//! Open role repository functions (T4 list, T5 create).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use terraops_shared::dto::talent::{CreateRoleRequest, RoleOpenItem};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct RoleOpenRow {
    pub id: Uuid,
    pub title: String,
    pub department_id: Option<Uuid>,
    pub required_skills: Vec<String>,
    pub min_years: i32,
    pub site_id: Option<Uuid>,
    // Migration 0031 — extended role requirements.
    pub required_major: Option<String>,
    pub min_education: Option<String>,
    pub required_availability: Option<String>,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<RoleOpenRow> for RoleOpenItem {
    fn from(r: RoleOpenRow) -> Self {
        RoleOpenItem {
            id: r.id,
            title: r.title,
            department_id: r.department_id,
            required_skills: r.required_skills,
            min_years: r.min_years,
            site_id: r.site_id,
            required_major: r.required_major,
            min_education: r.min_education,
            required_availability: r.required_availability,
            status: r.status,
            opened_at: r.opened_at,
            created_at: r.created_at,
        }
    }
}

const SELECT_COLS: &str =
    "id, title, department_id, required_skills, min_years, site_id, \
     required_major, min_education, required_availability, \
     status, opened_at, created_at";

/// List open roles (all statuses for internal use, or optionally filter by status).
pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<(Vec<RoleOpenRow>, i64), AppError> {
    list_filtered(pool, &RoleFilter::default(), limit, offset).await
}

/// Filter parameters for T4 `GET /talent/roles` search/filter (audit #4
/// issue #5). All fields are optional; they compose with AND.
#[derive(Debug, Default, Clone)]
pub struct RoleFilter {
    /// Case-insensitive substring match over `title`.
    pub q: Option<String>,
    /// Exact match against `status` (`open` | `closed` | `filled`).
    pub status: Option<String>,
    pub department_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    /// Minimum `min_years` required by the role.
    pub min_years: Option<i32>,
    /// Any of the listed skills must appear in `required_skills`.
    pub skills_any: Vec<String>,
    /// Audit #8 Issue #4: recruiter role search now honors the extended
    /// role attributes. Case-insensitive substring match on `required_major`.
    pub required_major: Option<String>,
    /// Minimum education level (ordinal: highschool < associate < bachelor
    /// < master < phd). Rows with NULL `min_education` are treated as 0
    /// and therefore only pass when the filter itself is `None`.
    pub min_education: Option<String>,
    /// Case-insensitive substring match on `required_availability`.
    pub required_availability: Option<String>,
    /// Sort column. Whitelisted set: `created_at` (default), `opened_at`,
    /// `title`, `min_years`, `status`. Anything else falls back to
    /// `created_at`.
    pub sort_by: Option<String>,
    /// Sort direction — `asc` or `desc` (default: `desc`).
    pub sort_dir: Option<String>,
}

/// Same as `list` but honors search/filter parameters.
pub async fn list_filtered(
    pool: &PgPool,
    f: &RoleFilter,
    limit: i64,
    offset: i64,
) -> Result<(Vec<RoleOpenRow>, i64), AppError> {
    // Build the WHERE fragment + bind pile together so the total query
    // and the page query share the exact same predicate set.
    let mut where_parts: Vec<String> = Vec::new();
    // We bind through positional $N placeholders; keep a counter.
    let mut n = 0usize;
    let mut bind_q: Option<String> = None;
    let mut bind_status: Option<String> = None;
    let mut bind_dept: Option<Uuid> = None;
    let mut bind_site: Option<Uuid> = None;
    let mut bind_min_years: Option<i32> = None;
    let mut bind_skills: Option<Vec<String>> = None;
    let mut bind_major: Option<String> = None;
    let mut bind_min_edu: Option<i32> = None;
    let mut bind_avail: Option<String> = None;

    if let Some(q) = f.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        n += 1;
        where_parts.push(format!("title ILIKE ${n}"));
        bind_q = Some(format!("%{}%", q.replace('%', "\\%")));
    }
    if let Some(s) = f
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        n += 1;
        where_parts.push(format!("status = ${n}"));
        bind_status = Some(s.to_string());
    }
    if let Some(d) = f.department_id {
        n += 1;
        where_parts.push(format!("department_id = ${n}"));
        bind_dept = Some(d);
    }
    if let Some(s) = f.site_id {
        n += 1;
        where_parts.push(format!("site_id = ${n}"));
        bind_site = Some(s);
    }
    if let Some(y) = f.min_years {
        n += 1;
        where_parts.push(format!("min_years >= ${n}"));
        bind_min_years = Some(y);
    }
    if !f.skills_any.is_empty() {
        n += 1;
        where_parts.push(format!("required_skills && ${n}"));
        bind_skills = Some(f.skills_any.clone());
    }
    if let Some(m) = f
        .required_major
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        n += 1;
        where_parts.push(format!("required_major ILIKE ${n}"));
        bind_major = Some(format!("%{}%", m.replace('%', "\\%")));
    }
    if let Some(me) = f
        .min_education
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // Map to ordinal; unknown words become 0 (no-op).
        let rank = match me.to_ascii_lowercase().as_str() {
            "highschool" => 1,
            "associate" => 2,
            "bachelor" => 3,
            "master" => 4,
            "phd" => 5,
            _ => 0,
        };
        if rank > 0 {
            n += 1;
            where_parts.push(format!(
                "COALESCE(CASE lower(min_education) \
                     WHEN 'highschool' THEN 1 \
                     WHEN 'associate'  THEN 2 \
                     WHEN 'bachelor'   THEN 3 \
                     WHEN 'master'     THEN 4 \
                     WHEN 'phd'        THEN 5 \
                     ELSE 0 END, 0) >= ${n}"
            ));
            bind_min_edu = Some(rank);
        }
    }
    if let Some(av) = f
        .required_availability
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        n += 1;
        where_parts.push(format!("required_availability ILIKE ${n}"));
        bind_avail = Some(format!("%{}%", av.replace('%', "\\%")));
    }

    let where_sql = if where_parts.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_parts.join(" AND "))
    };

    // Whitelisted sort columns — never interpolate user text directly.
    let sort_col = match f
        .sort_by
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("opened_at") => "opened_at",
        Some("title") => "title",
        Some("min_years") => "min_years",
        Some("status") => "status",
        _ => "created_at",
    };
    let sort_dir = match f
        .sort_dir
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("asc") => "ASC",
        _ => "DESC",
    };

    let page_sql = format!(
        "SELECT {SELECT_COLS} FROM roles_open{where_sql} \
         ORDER BY {sort_col} {sort_dir} LIMIT ${} OFFSET ${}",
        n + 1,
        n + 2
    );
    let total_sql = format!("SELECT COUNT(*)::BIGINT FROM roles_open{where_sql}");

    // Build both queries with the same predicate binds; the page query
    // appends limit + offset.
    let mut page_q = sqlx::query_as::<_, RoleOpenRow>(&page_sql);
    let mut total_q = sqlx::query_as::<_, (i64,)>(&total_sql);
    if let Some(v) = bind_q.as_ref() {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_status.as_ref() {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_dept {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_site {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_min_years {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_skills.as_ref() {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_major.as_ref() {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_min_edu {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }
    if let Some(v) = bind_avail.as_ref() {
        page_q = page_q.bind(v);
        total_q = total_q.bind(v);
    }

    let rows = page_q.bind(limit).bind(offset).fetch_all(pool).await?;
    let (total,) = total_q.fetch_one(pool).await?;
    Ok((rows, total))
}

/// Get a single open role by id.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<RoleOpenRow, AppError> {
    sqlx::query_as::<_, RoleOpenRow>(&format!(
        "SELECT {SELECT_COLS} FROM roles_open WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// Create a new open role.
pub async fn create(
    pool: &PgPool,
    req: &CreateRoleRequest,
    created_by: Uuid,
) -> Result<RoleOpenRow, AppError> {
    let status = req
        .status
        .as_deref()
        .unwrap_or("open");

    if !matches!(status, "open" | "closed" | "filled") {
        return Err(AppError::Validation(
            "status must be 'open', 'closed', or 'filled'".into(),
        ));
    }

    let trim_opt = |o: &Option<String>| -> Option<String> {
        o.as_ref().and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        })
    };
    let req_major = trim_opt(&req.required_major);
    let min_edu = trim_opt(&req.min_education);
    let req_avail = trim_opt(&req.required_availability);

    let row = sqlx::query_as::<_, RoleOpenRow>(&format!(
        "INSERT INTO roles_open \
         (title, department_id, required_skills, min_years, site_id, \
          required_major, min_education, required_availability, \
          status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         RETURNING {SELECT_COLS}"
    ))
    .bind(&req.title)
    .bind(req.department_id)
    .bind(&req.required_skills)
    .bind(req.min_years)
    .bind(req.site_id)
    .bind(req_major.as_deref())
    .bind(min_edu.as_deref())
    .bind(req_avail.as_deref())
    .bind(status)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
