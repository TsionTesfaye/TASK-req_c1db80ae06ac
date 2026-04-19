//! App-wide state providers: AuthContext, ToastContext, NotificationsContext.
//!
//! These are plain `yew::ContextProvider`s consumed by pages and components.
//! Auth state is persisted to sessionStorage (not localStorage) so closing
//! the tab ends the SPA session even if the refresh cookie is still valid.

use std::rc::Rc;

use gloo_storage::{SessionStorage, Storage};
use serde::{Deserialize, Serialize};
use terraops_shared::dto::auth::AuthUserDto;
use terraops_shared::roles::Role;
use uuid::Uuid;
use yew::prelude::*;

use crate::api::ApiClient;

// ---------------------------------------------------------------------------
// AuthContext
// ---------------------------------------------------------------------------

const STORAGE_KEY: &str = "terraops.auth";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthState {
    pub token: String,
    pub user: AuthUserDto,
}

impl AuthState {
    pub fn has_permission(&self, code: &str) -> bool {
        self.user.permissions.iter().any(|p| p == code)
    }
    pub fn has_role(&self, role: Role) -> bool {
        self.user.roles.contains(&role)
    }
    pub fn is_admin(&self) -> bool {
        self.has_role(Role::Administrator)
    }
    pub fn display_name(&self) -> &str {
        &self.user.display_name
    }
    pub fn user_id(&self) -> Uuid {
        self.user.id
    }
}

#[derive(Clone, PartialEq)]
pub struct AuthContext {
    pub state: Option<Rc<AuthState>>,
    /// Called on login / refresh / logout. Takes `None` to clear.
    pub set: Callback<Option<AuthState>>,
}

impl AuthContext {
    pub fn api(&self) -> ApiClient {
        ApiClient::with_token(self.state.as_ref().map(|s| s.token.clone()))
    }
    pub fn is_authenticated(&self) -> bool {
        self.state.is_some()
    }
    pub fn state(&self) -> Option<Rc<AuthState>> {
        self.state.clone()
    }
}

pub fn load_persisted_auth() -> Option<AuthState> {
    SessionStorage::get::<AuthState>(STORAGE_KEY).ok()
}

pub fn persist_auth(s: &Option<AuthState>) {
    match s {
        Some(v) => {
            let _ = SessionStorage::set(STORAGE_KEY, v);
        }
        None => SessionStorage::delete(STORAGE_KEY),
    }
}

// ---------------------------------------------------------------------------
// ToastContext
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastLevel {
    Info,
    Success,
    Warn,
    Error,
}

impl ToastLevel {
    pub fn class(&self) -> &'static str {
        match self {
            ToastLevel::Info => "tx-toast tx-toast--info",
            ToastLevel::Success => "tx-toast tx-toast--success",
            ToastLevel::Warn => "tx-toast tx-toast--warn",
            ToastLevel::Error => "tx-toast tx-toast--error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub level: ToastLevel,
    pub message: String,
}

#[derive(Clone, PartialEq)]
pub struct ToastContext {
    pub toasts: Rc<Vec<Toast>>,
    pub push: Callback<(ToastLevel, String)>,
    pub dismiss: Callback<u64>,
}

impl ToastContext {
    pub fn info(&self, msg: impl Into<String>) {
        self.push.emit((ToastLevel::Info, msg.into()));
    }
    pub fn success(&self, msg: impl Into<String>) {
        self.push.emit((ToastLevel::Success, msg.into()));
    }
    pub fn warn(&self, msg: impl Into<String>) {
        self.push.emit((ToastLevel::Warn, msg.into()));
    }
    pub fn error(&self, msg: impl Into<String>) {
        self.push.emit((ToastLevel::Error, msg.into()));
    }
}

// ---------------------------------------------------------------------------
// NotificationsContext
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq, Default)]
pub struct NotificationsSnapshot {
    pub unread: i64,
    pub last_refreshed_ms: f64,
}

#[derive(Clone, PartialEq)]
pub struct NotificationsContext {
    pub snapshot: Rc<NotificationsSnapshot>,
    pub refresh: Callback<()>,
}
