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

const NOTIFICATIONS_POLL_MS: u32 = 30_000;
const TOAST_AUTO_DISMISS_MS: u32 = 5_000;

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
