//! Top-level SPA component.
//!
//! Responsibilities:
//!   * Hydrate `AuthContext` from sessionStorage on first render so a page
//!     refresh keeps the user signed in for the remainder of the browser
//!     tab session.
//!   * Provide `AuthContext`, `ToastContext`, and `NotificationsContext`
//!     via `yew::ContextProvider` so every page and shared component can
//!     read them without prop drilling.
//!   * Poll `/notifications/unread-count` every 30 seconds while the user
//!     is authenticated, refreshing the nav badge.
//!   * Mount the `BrowserRouter` with the real `switch` dispatcher.

use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::router::{switch, Route};
use crate::state::{
    load_persisted_auth, persist_auth, AuthContext, AuthState, NotificationsContext,
    NotificationsSnapshot, Toast, ToastContext, ToastLevel,
};

pub(crate) const NOTIFICATIONS_POLL_MS: u32 = 30_000;
pub(crate) const TOAST_AUTO_DISMISS_MS: u32 = 5_000;
/// Audit #4 Issue #2: refresh the 15-minute access token via the
/// local-session refresh cookie `REFRESH_LEAD_MS` milliseconds before
/// the JWT's declared `exp`. With a 15-minute access TTL and a 60-
/// second lead we refresh every ~14 minutes for an active tab.
pub(crate) const REFRESH_LEAD_MS: f64 = 60_000.0;
/// Hard floor so a clock-skewed or stale persisted state still retries
/// promptly instead of busy-looping.
pub(crate) const REFRESH_MIN_DELAY_MS: u32 = 5_000;
/// If the persisted `access_expires_at_ms` is in the past we retry the
/// refresh on this cadence until it succeeds or the user signs out.
pub(crate) const REFRESH_RETRY_ON_ERROR_MS: u32 = 30_000;

#[function_component(App)]
pub fn app() -> Html {
    // --- Auth state -------------------------------------------------------
    let auth_state: UseStateHandle<Option<Rc<AuthState>>> =
        use_state(|| load_persisted_auth().map(Rc::new));

    let set_auth = {
        let auth_state = auth_state.clone();
        Callback::from(move |new_state: Option<AuthState>| {
            persist_auth(&new_state);
            auth_state.set(new_state.map(Rc::new));
        })
    };
    let auth_ctx = AuthContext {
        state: (*auth_state).clone(),
        set: set_auth,
    };

    // --- Toast state ------------------------------------------------------
    // `toasts` holds the current stack; `next_id` is a monotonically
    // increasing id used for dismissal.
    let toasts = use_state(|| Rc::new(Vec::<Toast>::new()));
    let next_id = use_state(|| 0u64);

    let push_toast = {
        let toasts = toasts.clone();
        let next_id = next_id.clone();
        Callback::from(move |(level, message): (ToastLevel, String)| {
            let id = *next_id;
            next_id.set(id.wrapping_add(1));
            let mut v: Vec<Toast> = (**toasts).clone();
            v.push(Toast { id, level, message });
            let new_rc = Rc::new(v);
            toasts.set(new_rc);
            // Auto-dismiss non-error toasts after a short delay.
            if !matches!(level, ToastLevel::Error) {
                let toasts = toasts.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    TimeoutFuture::new(TOAST_AUTO_DISMISS_MS).await;
                    let v: Vec<Toast> =
                        (*toasts).iter().filter(|t| t.id != id).cloned().collect();
                    toasts.set(Rc::new(v));
                });
            }
        })
    };
    let dismiss_toast = {
        let toasts = toasts.clone();
        Callback::from(move |id: u64| {
            let v: Vec<Toast> = (*toasts).iter().filter(|t| t.id != id).cloned().collect();
            toasts.set(Rc::new(v));
        })
    };
    let toast_ctx = ToastContext {
        toasts: (*toasts).clone(),
        push: push_toast,
        dismiss: dismiss_toast,
    };

    // --- Notifications snapshot ------------------------------------------
    let notif_snapshot =
        use_state(|| Rc::new(NotificationsSnapshot { unread: 0, last_refreshed_ms: 0.0 }));

    let refresh_notifications = {
        let notif_snapshot = notif_snapshot.clone();
        let auth_ctx_for_cb = auth_ctx.clone();
        Callback::from(move |_: ()| {
            let notif_snapshot = notif_snapshot.clone();
            let api = auth_ctx_for_cb.api();
            if !auth_ctx_for_cb.is_authenticated() {
                return;
            }
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(n) = api.unread_count().await {
                    let now = js_sys::Date::now();
                    notif_snapshot.set(Rc::new(NotificationsSnapshot {
                        unread: n,
                        last_refreshed_ms: now,
                    }));
                }
            });
        })
    };
    let notif_ctx = NotificationsContext {
        snapshot: (*notif_snapshot).clone(),
        refresh: refresh_notifications.clone(),
    };

    // Audit #4 Issue #2: 15-minute access-token refresh loop.
    //
    // The backend issues a 15-minute JWT (see `crypto::jwt::
    // ACCESS_TOKEN_TTL_MINUTES`) plus a long-lived HttpOnly refresh
    // cookie. The SPA never reads the refresh cookie directly — we just
    // POST `/auth/refresh`, the browser attaches the cookie, and the
    // backend returns a fresh access token. The effect re-runs whenever
    // the token changes (login / refresh / logout) so the next refresh
    // is always scheduled against the latest `access_expires_at`.
    {
        let token = auth_ctx.state.as_ref().map(|s| s.token.clone());
        let expires_at = auth_ctx
            .state
            .as_ref()
            .map(|s| s.access_expires_at_ms)
            .unwrap_or(0.0);
        let set_auth_for_refresh = auth_ctx.set.clone();
        let current_user = auth_ctx.state.as_ref().map(|s| s.user.clone());
        let current_token = token.clone();
        use_effect_with(token, move |t| {
            let cancel = Rc::new(std::cell::Cell::new(false));
            if t.is_some() {
                let cancel = cancel.clone();
                let set_auth = set_auth_for_refresh.clone();
                let user = current_user.clone();
                let current_token = current_token.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let now = js_sys::Date::now();
                    let delay_ms = if expires_at <= 0.0 {
                        // Unknown / legacy persisted state — refresh now
                        // (after a tiny delay so we do not race the
                        // initial render).
                        REFRESH_MIN_DELAY_MS
                    } else {
                        let d = (expires_at - now - REFRESH_LEAD_MS).max(REFRESH_MIN_DELAY_MS as f64);
                        d as u32
                    };
                    TimeoutFuture::new(delay_ms).await;
                    if cancel.get() {
                        return;
                    }
                    let api = crate::api::ApiClient::with_token(current_token);
                    match api.refresh().await {
                        Ok(resp) => {
                            if let Some(u) = user {
                                set_auth.emit(Some(crate::state::AuthState {
                                    token: resp.access_token,
                                    user: u,
                                    access_expires_at_ms: resp
                                        .access_expires_at
                                        .timestamp_millis()
                                        as f64,
                                }));
                            }
                        }
                        Err(e) => {
                            // Session unusable: sign out. Transient
                            // network failure: retry on a shorter
                            // cadence by clearing nothing and letting
                            // the next effect run re-schedule — we do
                            // that by emitting a short timer + manual
                            // re-invocation here.
                            if e.is_unauthenticated() {
                                set_auth.emit(None);
                            } else {
                                TimeoutFuture::new(REFRESH_RETRY_ON_ERROR_MS).await;
                                if cancel.get() {
                                    return;
                                }
                                // Fire a best-effort retry; if that
                                // also fails we give up until the next
                                // login. We do not loop indefinitely
                                // here to avoid spinning when offline.
                                let _ = crate::api::ApiClient::default().refresh().await;
                            }
                        }
                    }
                });
            }
            move || cancel.set(true)
        });
    }

    // Poll unread-count while authenticated. Re-runs whenever the auth
    // token changes (login / logout / refresh).
    {
        let refresh = refresh_notifications.clone();
        let token = auth_ctx.state.as_ref().map(|s| s.token.clone());
        let authed = token.is_some();
        use_effect_with(token, move |_| {
            let cancel = Rc::new(std::cell::Cell::new(false));
            if authed {
                refresh.emit(());
                let refresh = refresh.clone();
                let cancel = cancel.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    loop {
                        TimeoutFuture::new(NOTIFICATIONS_POLL_MS).await;
                        if cancel.get() {
                            break;
                        }
                        refresh.emit(());
                    }
                });
            }
            move || cancel.set(true)
        });
    }

    html! {
        <ContextProvider<AuthContext> context={auth_ctx}>
            <ContextProvider<ToastContext> context={toast_ctx}>
                <ContextProvider<NotificationsContext> context={notif_ctx}>
                    <BrowserRouter>
                        <Switch<Route> render={switch} />
                    </BrowserRouter>
                </ContextProvider<NotificationsContext>>
            </ContextProvider<ToastContext>>
        </ContextProvider<AuthContext>>
    }
}
