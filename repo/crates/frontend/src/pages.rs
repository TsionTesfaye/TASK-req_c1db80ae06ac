//! P1 page surface. All pages hit the real `/api/v1/*` backend through
//! `ApiClient` and render typed DTOs from `terraops-shared`.
//!
//! Layout:
//!   * `auth::Login`
//!   * `dashboard::Home` (minimal placeholder whose ownership transfers
//!     to P-B at fan-out per the Dashboard Ownership Seam rule)
//!   * `admin::Users` + `admin::Allowlist` + `admin::Retention`
//!     + `admin::Mtls` + `admin::Audit`
//!   * `notifications::Center`
//!   * `monitoring::Latency` + `Errors` + `Crashes`
//!   * `auth::ChangePassword`
//!   * `NotFound`

use std::rc::Rc;

use terraops_shared::dto::auth::{ChangePasswordRequest, LoginRequest};
use terraops_shared::dto::retention::UpdateRetentionPolicy;
use terraops_shared::dto::security::{CreateAllowlistEntry, UpdateMtlsConfig};
use terraops_shared::roles::Role;
use uuid::Uuid;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::prelude::*;

use crate::api::ApiClient;
use crate::components::{
    DataTable, Layout, LoadMore, PermAnyGate, PermGate, PlaceholderEmpty, PlaceholderError,
    PlaceholderLoading,
};
use crate::router::Route;
use crate::state::{AuthContext, AuthState, ToastContext};

/// Format a timestamp for UI display in the required MM/DD/YYYY 12-hour
/// format — e.g. `04/20/2026 02:05 PM`.
///
/// Audit #11 Issue #3: the SPA must render timestamps in the viewer's
/// local timezone, not UTC, and it must do so through the centralized
/// `terraops_shared::time::format_display` helper so the display contract
/// stays in one place across backend and frontend. We derive the offset
/// from the browser's current timezone via `Date.getTimezoneOffset()`
/// (returns minutes *west* of UTC — inverted sign — per the ECMAScript
/// spec) and hand the resulting offset-in-seconds to the shared helper.
pub(crate) fn format_ts(dt: chrono::DateTime<chrono::Utc>) -> String {
    terraops_shared::time::format_display(dt, local_offset_seconds())
}

/// Offset from UTC in **seconds** for the browser's current timezone. A
/// negative value means west of UTC (e.g. US/Eastern in April is `-14400`).
/// Falls back to UTC (`0`) outside the browser context so server-side
/// renders / tests stay deterministic.
fn local_offset_seconds() -> i32 {
    // `Date.getTimezoneOffset()` returns the difference, in minutes, from
    // local time to UTC — i.e. UTC minus local, positive for zones west of
    // UTC. We want local minus UTC in seconds, so we negate and multiply.
    let minutes_west = js_sys::Date::new_0().get_timezone_offset();
    if !minutes_west.is_finite() {
        return 0;
    }
    (-minutes_west * 60.0) as i32
}

/// Same contract as `format_ts` but for `Option<DateTime<Utc>>`; falls back
/// to an em-dash when the timestamp is absent.
pub(crate) fn format_ts_opt(dt: Option<chrono::DateTime<chrono::Utc>>) -> String {
    dt.map(format_ts).unwrap_or_else(|| "—".into())
}

/// Format a `NaiveDate` (used for daily rollup rows) in the same MM/DD/YYYY
/// tenant convention as `format_ts`. Audit #7 Issue #5: previously these
/// cells rendered the ISO `YYYY-MM-DD` via `NaiveDate::to_string()`, which
/// drifted from the required display format.
pub(crate) fn format_date(d: chrono::NaiveDate) -> String {
    d.format("%m/%d/%Y").to_string()
}

// ===========================================================================
// auth::Login
// ===========================================================================

pub mod auth {
    use super::*;

    #[function_component(Login)]
    pub fn login() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let navigator = use_navigator().unwrap();

        // Audit #4 Issue #4: sign-in contract is locally-validated
        // username + password. We keep the state variable named `email`
        // to minimize churn, but the value carried is the typed
        // username and the UI label/placeholder reflect that.
        let email = use_state(|| String::new());
        let password = use_state(|| String::new());
        let submitting = use_state(|| false);
        let error = use_state(|| None::<String>);

        // If already signed in, bounce to dashboard.
        {
            let auth = auth.clone();
            let navigator = navigator.clone();
            use_effect_with(auth.is_authenticated(), move |signed_in| {
                if *signed_in {
                    navigator.push(&Route::Dashboard);
                }
                || ()
            });
        }

        let on_email = {
            let email = email.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                email.set(t.value());
            })
        };
        let on_password = {
            let password = password.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                password.set(t.value());
            })
        };

        let onsubmit = {
            let email = email.clone();
            let password = password.clone();
            let auth = auth.clone();
            let toast = toast.clone();
            let navigator = navigator.clone();
            let submitting = submitting.clone();
            let error = error.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                if *submitting {
                    return;
                }
                submitting.set(true);
                error.set(None);
                let body = LoginRequest {
                    username: (*email).clone(),
                    password: (*password).clone(),
                };
                let client = ApiClient::new();
                let auth = auth.clone();
                let toast = toast.clone();
                let navigator = navigator.clone();
                let submitting = submitting.clone();
                let error = error.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match client.login(&body).await {
                        Ok(resp) => {
                            let new_state = AuthState {
                                token: resp.access_token,
                                user: resp.user,
                                access_expires_at_ms: resp.access_expires_at.timestamp_millis() as f64,
                            };
                            auth.set.emit(Some(new_state));
                            toast.success("Signed in.");
                            navigator.push(&Route::Dashboard);
                        }
                        Err(e) => {
                            error.set(Some(e.user_facing()));
                        }
                    }
                    submitting.set(false);
                });
            })
        };

        html! {
            <main class="tx-shell">
                <section class="tx-card" aria-labelledby="login-title">
                    <h1 id="login-title" class="tx-title">{ "TerraOps" }</h1>
                    <p class="tx-subtle">{ "Offline Environmental & Catalog Intelligence Portal" }</p>
                    <form class="tx-form" onsubmit={onsubmit}>
                        <label for="username" class="tx-subtle">{ "Username" }</label>
                        <input id="username" name="username" class="tx-input" type="text"
                               autocomplete="username" autocapitalize="none" spellcheck="false"
                               required=true value={(*email).clone()} oninput={on_email}
                               placeholder="admin" />
                        <label for="password" class="tx-subtle">{ "Password" }</label>
                        <input id="password" class="tx-input" type="password" autocomplete="current-password"
                               required=true value={(*password).clone()} oninput={on_password}
                               placeholder="••••••••" />
                        if let Some(msg) = error.as_ref() {
                            <div class="tx-error" role="alert">{ msg.clone() }</div>
                        }
                        <button class="tx-btn" type="submit" disabled={*submitting}>
                            { if *submitting { "Signing in…" } else { "Sign in" } }
                        </button>
                        <p class="tx-subtle tx-hint">
                            { "Demo accounts are documented in README.md (password: " }
                            <code>{ "TerraOps!2026" }</code>
                            { ")." }
                        </p>
                    </form>
                </section>
            </main>
        }
    }

    #[function_component(ChangePassword)]
    pub fn change_password() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let navigator = use_navigator().unwrap();

        let current = use_state(|| String::new());
        let next = use_state(|| String::new());
        let confirm = use_state(|| String::new());
        let submitting = use_state(|| false);
        let error = use_state(|| None::<String>);

        let onsubmit = {
            let current = current.clone();
            let next = next.clone();
            let confirm = confirm.clone();
            let submitting = submitting.clone();
            let error = error.clone();
            let auth = auth.clone();
            let toast = toast.clone();
            let navigator = navigator.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                if *next != *confirm {
                    error.set(Some("New passwords do not match.".into()));
                    return;
                }
                if next.len() < 12 {
                    error.set(Some("New password must be at least 12 characters.".into()));
                    return;
                }
                submitting.set(true);
                error.set(None);
                let body = ChangePasswordRequest {
                    current_password: (*current).clone(),
                    new_password: (*next).clone(),
                };
                let api = auth.api();
                let auth = auth.clone();
                let toast = toast.clone();
                let navigator = navigator.clone();
                let submitting = submitting.clone();
                let error = error.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.change_password(&body).await {
                        Ok(()) => {
                            toast.success("Password changed. Please sign in again.");
                            auth.set.emit(None);
                            navigator.push(&Route::Login);
                        }
                        Err(e) => error.set(Some(e.user_facing())),
                    }
                    submitting.set(false);
                });
            })
        };

        let on_in = |state: UseStateHandle<String>| -> Callback<InputEvent> {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                state.set(t.value());
            })
        };

        html! {
            <Layout title="Change Password" subtitle="Revokes every active session except this one.">
                <section class="tx-card">
                    <form class="tx-form" onsubmit={onsubmit}>
                        <label class="tx-subtle">{ "Current password" }</label>
                        <input class="tx-input" type="password" autocomplete="current-password"
                               required=true value={(*current).clone()} oninput={on_in(current.clone())} />
                        <label class="tx-subtle">{ "New password" }</label>
                        <input class="tx-input" type="password" autocomplete="new-password"
                               required=true value={(*next).clone()} oninput={on_in(next.clone())} />
                        <label class="tx-subtle">{ "Confirm new password" }</label>
                        <input class="tx-input" type="password" autocomplete="new-password"
                               required=true value={(*confirm).clone()} oninput={on_in(confirm.clone())} />
                        if let Some(msg) = error.as_ref() {
                            <div class="tx-error" role="alert">{ msg.clone() }</div>
                        }
                        <button class="tx-btn" type="submit" disabled={*submitting}>
                            { if *submitting { "Updating…" } else { "Change password" } }
                        </button>
                    </form>
                </section>
            </Layout>
        }
    }
}

// ===========================================================================
// dashboard::Home — role-aware KPI landing page.
//
// Replaces the earlier P1 placeholder card. The home screen now surfaces
// real operational numbers pulled from the KPI summary endpoint (K1) for
// any user with `kpi.read`, plus a role-contextual action strip below.
// Users who lack `kpi.read` still land here and get a compact self card
// plus role-specific quick links so no signed-in user is ever stuck on
// a placeholder.
// ===========================================================================

pub mod dashboard {
    use super::*;

    use crate::api::ApiError;
    use terraops_shared::dto::kpi::KpiSummary;

    #[derive(Clone, PartialEq)]
    enum KpiLoad {
        Loading,
        Loaded(KpiSummary),
        NotPermitted,
        Failed(String),
    }

    #[function_component(Home)]
    pub fn home() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let Some(state) = auth.state.as_ref().cloned() else {
            return html! { <Redirect<Route> to={Route::Login} /> };
        };

        let has_kpi = state.has_permission("kpi.read");
        let kpi_state = use_state(|| {
            if has_kpi {
                KpiLoad::Loading
            } else {
                KpiLoad::NotPermitted
            }
        });

        {
            let kpi_state = kpi_state.clone();
            let api = auth.api();
            use_effect_with(has_kpi, move |&has| {
                if has {
                    wasm_bindgen_futures::spawn_local(async move {
                        match api.kpi_summary().await {
                            Ok(s) => kpi_state.set(KpiLoad::Loaded(s)),
                            Err(ApiError::Api { code, message, .. }) => {
                                if code == terraops_shared::error::ErrorCode::AuthForbidden {
                                    kpi_state.set(KpiLoad::NotPermitted);
                                } else {
                                    kpi_state.set(KpiLoad::Failed(message));
                                }
                            }
                            Err(e) => kpi_state.set(KpiLoad::Failed(e.user_facing())),
                        }
                    });
                }
                || ()
            });
        }

        let kpi_block = match &*kpi_state {
            KpiLoad::Loading => html! {
                <article class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Operational KPIs" }</h2>
                    <PlaceholderLoading/>
                </article>
            },
            KpiLoad::Loaded(s) => html! {
                <article class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Operational KPIs" }</h2>
                    <div class="tx-kpi-grid">
                        <div class="tx-kpi">
                            <div class="tx-kpi__label">{ "Avg cycle time" }</div>
                            <div class="tx-kpi__value">{ format!("{:.1} h", s.cycle_time_avg_hours) }</div>
                        </div>
                        <div class="tx-kpi">
                            <div class="tx-kpi__label">{ "Funnel conversion" }</div>
                            <div class="tx-kpi__value">{ format!("{:.1}%", s.funnel_conversion_pct) }</div>
                        </div>
                        <div class="tx-kpi">
                            <div class="tx-kpi__label">{ "Anomalies (24h)" }</div>
                            <div class="tx-kpi__value">{ s.anomaly_count }</div>
                        </div>
                        <div class="tx-kpi">
                            <div class="tx-kpi__label">{ "Efficiency index" }</div>
                            <div class="tx-kpi__value">{ format!("{:.2}", s.efficiency_index) }</div>
                        </div>
                    </div>
                    <p class="tx-subtle tx-hint">
                        { format!("As of {}", format_ts(s.generated_at)) }
                    </p>
                    <Link<Route> to={Route::Kpi} classes={classes!("tx-btn", "tx-btn--ghost")}>
                        { "Open KPI workspace" }
                    </Link<Route>>
                </article>
            },
            KpiLoad::NotPermitted => html! {
                <article class="tx-card tx-card--hint">
                    <h2 class="tx-title tx-title--sm">{ "KPI workspace" }</h2>
                    <p class="tx-subtle">
                        { "Your role does not include KPI access. \
                           Quick links for your role appear below." }
                    </p>
                </article>
            },
            KpiLoad::Failed(msg) => html! {
                <article class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Operational KPIs" }</h2>
                    <PlaceholderError message={msg.clone()} />
                </article>
            },
        };

        html! {
            <Layout title="Dashboard" subtitle="Real-time portal home.">
                <section class="tx-grid">
                    { kpi_block }
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "You" }</h2>
                        <div class="tx-kv"><span>{ "Name" }</span><span>{ &state.user.display_name }</span></div>
                        <div class="tx-kv"><span>{ "Email" }</span><span class="tx-mono">{ &state.user.email_mask }</span></div>
                        <div class="tx-kv"><span>{ "Timezone" }</span><span>
                            { state.user.timezone.clone().unwrap_or_else(|| "—".into()) }
                        </span></div>
                        <div class="tx-kv"><span>{ "Roles" }</span><span>
                            { for state.user.roles.iter().map(|r| html!{
                                <span class="tx-chip">{ r.display() }</span>
                            }) }
                        </span></div>
                    </article>
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "Quick actions" }</h2>
                        <div class="tx-chip-cloud">
                            if state.has_role(Role::Administrator) {
                                <Link<Route> to={Route::AdminUsers} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                    { "Manage users" }
                                </Link<Route>>
                            }
                            if state.has_permission("product.read") {
                                <Link<Route> to={Route::Products} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                    { "Open catalog" }
                                </Link<Route>>
                            }
                            if state.has_permission("metric.read") {
                                <Link<Route> to={Route::MetricDefinitions} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                    { "Metric definitions" }
                                </Link<Route>>
                            }
                            if state.has_permission("talent.read") || state.has_permission("talent.manage") {
                                <Link<Route> to={Route::TalentRecommendations} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                    { "Talent recommendations" }
                                </Link<Route>>
                            }
                            <Link<Route> to={Route::Notifications} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                { "Notifications" }
                            </Link<Route>>
                        </div>
                    </article>
                </section>
            </Layout>
        }
    }
}

// ===========================================================================
// admin::* surfaces
// ===========================================================================

pub mod admin {
    use super::*;
    use terraops_shared::dto::retention::RetentionPolicy;
    use terraops_shared::dto::security::{AllowlistEntry, MtlsConfig};
    use terraops_shared::dto::user::{CreateUserRequest, RoleDto, UserListItem};
    use terraops_shared::dto::audit::AuditEntry;

    // ---------- Users ----------

    #[function_component(Users)]
    pub fn users() -> Html {
        html! {
            <Layout title="Users" subtitle="Create users, assign roles, soft-delete, unlock.">
                <PermGate permission="user.manage">
                    <UsersBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(UsersBody)]
    fn users_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let users_state = use_state(|| LoadState::<Vec<UserListItem>>::Loading);
        let roles_state = use_state(|| Vec::<RoleDto>::new());

        let reload = {
            let auth = auth.clone();
            let users_state = users_state.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let users_state = users_state.clone();
                users_state.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_users().await {
                        Ok(v) => users_state.set(LoadState::Loaded(v)),
                        Err(e) => users_state.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };

        {
            let reload = reload.clone();
            let auth = auth.clone();
            let roles_state = roles_state.clone();
            use_effect_with((), move |_| {
                reload.emit(());
                let api = auth.api();
                let roles_state = roles_state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_roles().await {
                        roles_state.set(v);
                    }
                });
                || ()
            });
        }

        let on_created = {
            let reload = reload.clone();
            let toast = toast.clone();
            Callback::from(move |()| {
                toast.success("User created.");
                reload.emit(());
            })
        };

        let body = match &*users_state {
            LoadState::Loading => html! { <PlaceholderLoading label="Loading users…"/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Name"), AttrValue::from("Email"),
                    AttrValue::from("Roles"), AttrValue::from("Status"),
                    AttrValue::from("Actions"),
                ];
                let trows: Vec<Vec<Html>> = rows
                    .iter()
                    .map(|u| user_row(u, auth.clone(), toast.clone(), reload.clone()))
                    .collect();
                html! {
                    <DataTable headers={headers} rows={trows} empty_label="No users yet."/>
                }
            }
        };

        html! {
            <>
                <CreateUserCard roles={(*roles_state).clone()} on_created={on_created}/>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "All users" }</h2>
                    { body }
                </section>
            </>
        }
    }

    fn user_row(
        u: &UserListItem,
        auth: AuthContext,
        toast: ToastContext,
        reload: Callback<()>,
    ) -> Vec<Html> {
        let id = u.id;
        let locked = u.locked;
        let active = u.is_active;
        let unlock = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.unlock_user(id).await {
                        Ok(()) => { toast.success("User unlocked."); reload.emit(()); }
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };
        let deactivate = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.delete_user(id).await {
                        Ok(()) => { toast.success("User deactivated."); reload.emit(()); }
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };
        vec![
            html!{ { u.display_name.clone() } },
            html!{ <span class="tx-mono">{ u.email_mask.clone() }</span> },
            html!{ { for u.roles.iter().map(|r| html!{ <span class="tx-chip">{ r.display() }</span> }) } },
            html!{ <>
                if active { <span class="tx-chip tx-chip--ok">{ "active" }</span> }
                else { <span class="tx-chip tx-chip--warn">{ "inactive" }</span> }
                if locked { <span class="tx-chip tx-chip--warn">{ "locked" }</span> }
            </> },
            html!{ <div class="tx-row-actions">
                if locked {
                    <button class="tx-btn tx-btn--ghost" onclick={unlock}>{ "Unlock" }</button>
                }
                if active {
                    <button class="tx-btn tx-btn--danger-ghost" onclick={deactivate}>{ "Deactivate" }</button>
                }
            </div> },
        ]
    }

    #[derive(Properties, PartialEq)]
    struct CreateUserCardProps {
        pub roles: Vec<RoleDto>,
        pub on_created: Callback<()>,
    }

    #[function_component(CreateUserCard)]
    fn create_user_card(props: &CreateUserCardProps) -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");

        let name = use_state(|| String::new());
        let email = use_state(|| String::new());
        let username = use_state(|| String::new());
        let password = use_state(|| String::new());
        let selected_role = use_state(|| Role::RegularUser);
        let submitting = use_state(|| false);

        let on_str = |s: UseStateHandle<String>| -> Callback<InputEvent> {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };
        let on_role = {
            let selected_role = selected_role.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                let role = match t.value().as_str() {
                    "administrator" => Role::Administrator,
                    "data_steward" => Role::DataSteward,
                    "analyst" => Role::Analyst,
                    "recruiter" => Role::Recruiter,
                    _ => Role::RegularUser,
                };
                selected_role.set(role);
            })
        };

        let onsubmit = {
            let name = name.clone();
            let email = email.clone();
            let username = username.clone();
            let password = password.clone();
            let selected_role = selected_role.clone();
            let submitting = submitting.clone();
            let auth = auth.clone();
            let toast = toast.clone();
            let on_created = props.on_created.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                submitting.set(true);
                let uname = (*username).trim().to_string();
                let body = CreateUserRequest {
                    display_name: (*name).clone(),
                    email: (*email).clone(),
                    username: if uname.is_empty() { None } else { Some(uname) },
                    password: (*password).clone(),
                    roles: vec![*selected_role],
                    timezone: None,
                };
                let api = auth.api();
                let toast = toast.clone();
                let on_created = on_created.clone();
                let submitting = submitting.clone();
                let name = name.clone();
                let email = email.clone();
                let username = username.clone();
                let password = password.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_user(&body).await {
                        Ok(_) => {
                            name.set(String::new());
                            email.set(String::new());
                            username.set(String::new());
                            password.set(String::new());
                            on_created.emit(());
                        }
                        Err(e) => toast.error(e.user_facing()),
                    }
                    submitting.set(false);
                });
            })
        };

        let _ = props.roles.len(); // prop read for reactivity

        html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Create user" }</h2>
                <form class="tx-form tx-form--row" onsubmit={onsubmit}>
                    <input class="tx-input" type="text" placeholder="Display name"
                           required=true value={(*name).clone()} oninput={on_str(name.clone())} />
                    <input class="tx-input" type="text" placeholder="username (optional, derived from email if blank)"
                           value={(*username).clone()} oninput={on_str(username.clone())} />
                    <input class="tx-input" type="email" placeholder="email@domain"
                           required=true value={(*email).clone()} oninput={on_str(email.clone())} />
                    <input class="tx-input" type="password" placeholder="Temp password"
                           required=true value={(*password).clone()} oninput={on_str(password.clone())} />
                    <select class="tx-input" onchange={on_role}>
                        { for Role::ALL.iter().map(|r| html!{
                            <option value={r.as_db()} selected={*r == *selected_role}>
                                { r.display() }
                            </option>
                        }) }
                    </select>
                    <button class="tx-btn" type="submit" disabled={*submitting}>
                        { if *submitting { "Creating…" } else { "Create" } }
                    </button>
                </form>
            </section>
        }
    }

    // ---------- Allowlist ----------

    #[function_component(Allowlist)]
    pub fn allowlist() -> Html {
        html! {
            <Layout title="IP Allowlist" subtitle="An empty allowlist means no restriction.">
                <PermGate permission="allowlist.manage">
                    <AllowlistBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(AllowlistBody)]
    fn allowlist_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<AllowlistEntry>>::Loading);

        let cidr = use_state(|| String::new());
        let note = use_state(|| String::new());

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_allowlist().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let onsubmit = {
            let cidr = cidr.clone();
            let note = note.clone();
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let body = CreateAllowlistEntry {
                    cidr: (*cidr).clone(),
                    note: if note.is_empty() { None } else { Some((*note).clone()) },
                    enabled: Some(true),
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let cidr = cidr.clone();
                let note = note.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_allowlist(&body).await {
                        Ok(_) => {
                            cidr.set(String::new());
                            note.set(String::new());
                            toast.success("Allowlist entry added.");
                            reload.emit(());
                        }
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };

        let on_str = |s: UseStateHandle<String>| -> Callback<InputEvent> {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };

        let body = match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading label="Loading allowlist…"/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("CIDR"), AttrValue::from("Note"),
                    AttrValue::from("Status"), AttrValue::from("Actions"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| {
                    let id = r.id;
                    let del = {
                        let auth = auth.clone();
                        let toast = toast.clone();
                        let reload = reload.clone();
                        Callback::from(move |_: MouseEvent| {
                            let api = auth.api();
                            let toast = toast.clone();
                            let reload = reload.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match api.delete_allowlist(id).await {
                                    Ok(()) => { toast.success("Entry removed."); reload.emit(()); }
                                    Err(e) => toast.error(e.user_facing()),
                                }
                            });
                        })
                    };
                    vec![
                        html!{ <span class="tx-mono">{ r.cidr.clone() }</span> },
                        html!{ { r.note.clone().unwrap_or_else(|| "—".into()) } },
                        if r.enabled {
                            html!{ <span class="tx-chip tx-chip--ok">{ "enabled" }</span> }
                        } else {
                            html!{ <span class="tx-chip tx-chip--warn">{ "disabled" }</span> }
                        },
                        html!{ <button class="tx-btn tx-btn--danger-ghost" onclick={del}>{ "Remove" }</button> },
                    ]
                }).collect();
                html!{ <DataTable headers={headers} rows={trows} empty_label="No entries — allowlist is permissive."/> }
            }
        };

        html! {
            <>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Add entry" }</h2>
                    <form class="tx-form tx-form--row" onsubmit={onsubmit}>
                        <input class="tx-input" type="text" placeholder="10.0.0.0/8"
                               required=true value={(*cidr).clone()} oninput={on_str(cidr.clone())} />
                        <input class="tx-input" type="text" placeholder="Optional note"
                               value={(*note).clone()} oninput={on_str(note.clone())} />
                        <button class="tx-btn" type="submit">{ "Add" }</button>
                    </form>
                </section>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Active entries" }</h2>
                    { body }
                </section>
            </>
        }
    }

    // ---------- Retention ----------

    #[function_component(Retention)]
    pub fn retention() -> Html {
        html! {
            <Layout title="Retention" subtitle="Default TTLs: env_raw 548d, kpi 1825d, feedback 730d, audit indefinite (0).">
                <PermGate permission="retention.manage">
                    <RetentionBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(RetentionBody)]
    fn retention_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let _toast = use_context::<ToastContext>().expect("ToastContext");
        let policies = use_state(|| LoadState::<Vec<RetentionPolicy>>::Loading);

        let reload = {
            let auth = auth.clone();
            let policies = policies.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let policies = policies.clone();
                policies.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_retention().await {
                        Ok(v) => policies.set(LoadState::Loaded(v)),
                        Err(e) => policies.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*policies {
            LoadState::Loading => html!{ <PlaceholderLoading label="Loading retention policies…"/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => html!{
                <div class="tx-stack">
                    { for rows.iter().cloned().map(|p| html!{
                        <RetentionCard policy={p} reload={reload.clone()} />
                    }) }
                </div>
            },
        }
    }

    #[derive(Properties, PartialEq)]
    pub struct RetentionCardProps {
        pub policy: RetentionPolicy,
        pub reload: Callback<()>,
    }

    #[function_component(RetentionCard)]
    pub fn retention_card(props: &RetentionCardProps) -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let p = &props.policy;
        let reload = props.reload.clone();
        let domain = p.domain.clone();
        let ttl_state = use_state(|| p.ttl_days);
        let saving = use_state(|| false);

        let on_ttl = {
            let ttl_state = ttl_state.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                ttl_state.set(t.value().parse::<i32>().unwrap_or(0));
            })
        };

        let save = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let ttl_state = ttl_state.clone();
            let saving = saving.clone();
            let domain = domain.clone();
            Callback::from(move |_: MouseEvent| {
                saving.set(true);
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let saving = saving.clone();
                let domain = domain.clone();
                let body = UpdateRetentionPolicy { ttl_days: *ttl_state };
                wasm_bindgen_futures::spawn_local(async move {
                    match api.patch_retention(&domain, &body).await {
                        Ok(()) => { toast.success(format!("{} updated.", domain)); reload.emit(()); }
                        Err(e) => toast.error(e.user_facing()),
                    }
                    saving.set(false);
                });
            })
        };

        let run = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let domain = domain.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let domain = domain.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.run_retention(&domain).await {
                        Ok(r) => {
                            toast.success(format!("{} enforced — {} deleted.", r.domain, r.deleted));
                            reload.emit(());
                        }
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };

        let last = p.last_enforced_at
            .map(|d| format_ts(d))
            .unwrap_or_else(|| "never".to_string());

        html! {
            <section class="tx-card">
                <div class="tx-row-between">
                    <div>
                        <h2 class="tx-title tx-title--sm">{ p.domain.clone() }</h2>
                        <div class="tx-subtle">{ format!("last enforced: {}", last) }</div>
                    </div>
                    <div class="tx-row-actions">
                        <input class="tx-input tx-input--sm" type="number" min="0"
                               value={ttl_state.to_string()} oninput={on_ttl} />
                        <button class="tx-btn" onclick={save} disabled={*saving}>{ "Save TTL" }</button>
                        <button class="tx-btn tx-btn--ghost" onclick={run}>{ "Run now" }</button>
                    </div>
                </div>
            </section>
        }
    }

    // ---------- mTLS ----------

    #[function_component(Mtls)]
    pub fn mtls() -> Html {
        html! {
            <Layout title="Device mTLS" subtitle="Pinned client certificates. Enforcement flips the Rustls verifier on.">
                <PermGate permission="mtls.manage">
                    <MtlsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(MtlsBody)]
    fn mtls_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let cfg = use_state(|| LoadState::<MtlsConfig>::Loading);
        let status = use_state(|| None::<serde_json::Value>);

        let reload = {
            let auth = auth.clone();
            let cfg = cfg.clone();
            let status = status.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let cfg = cfg.clone();
                let status = status.clone();
                cfg.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_mtls().await {
                        Ok(v) => cfg.set(LoadState::Loaded(v)),
                        Err(e) => cfg.set(LoadState::Failed(e.user_facing())),
                    }
                    if let Ok(s) = api.mtls_status().await {
                        status.set(Some(s));
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*cfg {
            LoadState::Loading => html!{ <PlaceholderLoading label="Loading mTLS…"/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(m) => {
                let enforced = m.enforced;
                let toggle = {
                    let auth = auth.clone();
                    let toast = toast.clone();
                    let reload = reload.clone();
                    Callback::from(move |_: MouseEvent| {
                        let api = auth.api();
                        let toast = toast.clone();
                        let reload = reload.clone();
                        let next = !enforced;
                        wasm_bindgen_futures::spawn_local(async move {
                            match api.patch_mtls(&UpdateMtlsConfig { enforced: next }).await {
                                Ok(()) => {
                                    toast.success(if next { "mTLS enforcement ENABLED." } else { "mTLS enforcement disabled." });
                                    reload.emit(());
                                }
                                Err(e) => toast.error(e.user_facing()),
                            }
                        });
                    })
                };
                let (active, revoked) = status
                    .as_ref()
                    .map(|v| (
                        v.get("active_certs").and_then(|x| x.as_i64()).unwrap_or(0),
                        v.get("revoked_certs").and_then(|x| x.as_i64()).unwrap_or(0),
                    ))
                    .unwrap_or((0, 0));
                html! {
                    <section class="tx-card">
                        <div class="tx-row-between">
                            <div>
                                <h2 class="tx-title tx-title--sm">{ "Enforcement" }</h2>
                                <div class="tx-subtle">{ format!("Last updated: {}", format_ts(m.updated_at)) }</div>
                            </div>
                            <div class="tx-row-actions">
                                if enforced {
                                    <span class="tx-chip tx-chip--ok">{ "enforced" }</span>
                                    <button class="tx-btn tx-btn--danger-ghost" onclick={toggle}>
                                        { "Disable enforcement" }
                                    </button>
                                } else {
                                    <span class="tx-chip tx-chip--warn">{ "not enforced" }</span>
                                    <button class="tx-btn" onclick={toggle}>{ "Enable enforcement" }</button>
                                }
                            </div>
                        </div>
                        <hr class="tx-sep" />
                        <div class="tx-kv"><span>{ "Active client certs" }</span><span>{ active }</span></div>
                        <div class="tx-kv"><span>{ "Revoked client certs" }</span><span>{ revoked }</span></div>
                        <p class="tx-subtle">
                            { "Issue new certs with " } <code>{ "scripts/issue_device_cert.sh" }</code>
                            { " and register their SPKI pin via the backend API. The Rustls pinned-client verifier reload ships in P4 hardening." }
                        </p>
                    </section>
                }
            }
        }
    }

    // ---------- Audit ----------

    #[function_component(Audit)]
    pub fn audit() -> Html {
        html! {
            <Layout title="Audit log" subtitle="Append-only. Immutable via DB trigger.">
                <PermGate permission="monitoring.read">
                    <AuditBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(AuditBody)]
    fn audit_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<AuditEntry>>::Loading);

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_audit().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("When"), AttrValue::from("Actor"),
                    AttrValue::from("Action"), AttrValue::from("Target"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|e| vec![
                    html!{ <span class="tx-mono">{ format_ts(e.at) }</span> },
                    html!{ { e.actor_display.clone().unwrap_or_else(|| "—".into()) } },
                    html!{ <code>{ e.action.clone() }</code> },
                    html!{ { format!("{}/{}",
                        e.target_type.clone().unwrap_or_else(|| "—".into()),
                        e.target_id.clone().unwrap_or_else(|| "—".into())
                    ) } },
                ]).collect();
                html!{ <DataTable headers={headers} rows={trows} empty_label="No audit entries."/> }
            }
        }
    }
}

// ===========================================================================
// notifications::Center
// ===========================================================================

pub mod notifications {
    use super::*;
    use terraops_shared::dto::notification::NotificationItem;

    #[function_component(Center)]
    pub fn center() -> Html {
        html! {
            <Layout title="Notifications" subtitle="Offline-only: everything lives in this Postgres + the mbox export.">
                <CenterBody/>
            </Layout>
        }
    }

    #[function_component(CenterBody)]
    fn body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<NotificationItem>>::Loading);

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_notifications().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let mark_all = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.mark_all_notifications_read().await {
                        Ok(()) => { toast.success("All marked read."); reload.emit(()); }
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };

        let body = match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading label="Loading notifications…"/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) if rows.is_empty() => html!{
                <PlaceholderEmpty label="No notifications yet. Alerts, imports, and reports will land here as they fire."/>
            },
            LoadState::Loaded(rows) => html!{
                <ul class="tx-list">
                    { for rows.iter().map(|n| render_notification(n, auth.clone(), toast.clone(), reload.clone())) }
                </ul>
            },
        };

        html! {
            <>
                <section class="tx-card">
                    <div class="tx-row-between">
                        <h2 class="tx-title tx-title--sm">{ "Inbox" }</h2>
                        <button class="tx-btn tx-btn--ghost" onclick={mark_all}>{ "Mark all read" }</button>
                    </div>
                    { body }
                </section>
            </>
        }
    }

    fn render_notification(
        n: &NotificationItem,
        auth: AuthContext,
        toast: ToastContext,
        reload: Callback<()>,
    ) -> Html {
        let id = n.id;
        let unread = n.read_at.is_none();
        let mark_read = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.mark_notification_read(id).await {
                        Ok(()) => reload.emit(()),
                        Err(e) => toast.error(e.user_facing()),
                    }
                });
            })
        };
        let cls = if unread { "tx-list-item tx-list-item--unread" } else { "tx-list-item" };
        html! {
            <li class={cls}>
                <div>
                    <div class="tx-list-item-title">
                        <code class="tx-mono">{ n.topic.clone() }</code>
                        <span>{ n.title.clone() }</span>
                    </div>
                    <p class="tx-subtle">{ n.body.clone() }</p>
                    <div class="tx-subtle tx-mono">{ format_ts(n.created_at) }</div>
                </div>
                if unread {
                    <button class="tx-btn tx-btn--ghost" onclick={mark_read}>{ "Mark read" }</button>
                }
            </li>
        }
    }
}

// ===========================================================================
// monitoring::{Latency, Errors, Crashes}
// ===========================================================================

pub mod monitoring {
    use super::*;
    use terraops_shared::dto::monitoring::{CrashReport, ErrorBucket, LatencyBucket};

    #[function_component(Latency)]
    pub fn latency() -> Html {
        html! {
            <Layout title="Latency" subtitle="Per-route percentiles from the live request metric stream.">
                <PermGate permission="monitoring.read">
                    <LatencyBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(LatencyBody)]
    fn latency_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<LatencyBucket>>::Loading);

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_latency().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Route"), AttrValue::from("Method"),
                    AttrValue::from("Count"),
                    AttrValue::from("p50"), AttrValue::from("p95"), AttrValue::from("p99"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|b| vec![
                    html!{ <code>{ b.route.clone() }</code> },
                    html!{ { b.method.clone() } },
                    html!{ { b.count } },
                    html!{ { format!("{} ms", b.p50_ms) } },
                    html!{ { format!("{} ms", b.p95_ms) } },
                    html!{ { format!("{} ms", b.p99_ms) } },
                ]).collect();
                html!{ <DataTable headers={headers} rows={trows} empty_label="No traffic recorded yet."/> }
            }
        }
    }

    #[function_component(Errors)]
    pub fn errors() -> Html {
        html! {
            <Layout title="Errors" subtitle="5xx + 4xx rollups by route and method.">
                <PermGate permission="monitoring.read">
                    <ErrorsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ErrorsBody)]
    fn errors_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<ErrorBucket>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_errors().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Route"), AttrValue::from("Method"),
                    AttrValue::from("Total"), AttrValue::from("Errors"), AttrValue::from("Rate"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|b| vec![
                    html!{ <code>{ b.route.clone() }</code> },
                    html!{ { b.method.clone() } },
                    html!{ { b.total } },
                    html!{ { b.errors } },
                    html!{ { format!("{:.2}%", b.error_rate * 100.0) } },
                ]).collect();
                html!{ <DataTable headers={headers} rows={trows} empty_label="No errors."/> }
            }
        }
    }

    #[function_component(Crashes)]
    pub fn crashes() -> Html {
        html! {
            <Layout title="Client crashes" subtitle="Yew SPA + admin tool crash reports.">
                <PermGate permission="monitoring.read">
                    <CrashesBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(CrashesBody)]
    fn crashes_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<CrashReport>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_crashes().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*list {
            LoadState::Loading => html!{ <PlaceholderLoading/> },
            LoadState::Failed(m) => html!{
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })} />
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("When"), AttrValue::from("Page"),
                    AttrValue::from("Agent"), AttrValue::from("Stack"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|c| vec![
                    html!{ <span class="tx-mono">{ format_ts(c.reported_at) }</span> },
                    html!{ { c.page.clone().unwrap_or_else(|| "—".into()) } },
                    html!{ <span class="tx-mono tx-truncate">{ c.agent.clone().unwrap_or_else(|| "—".into()) }</span> },
                    html!{ <pre class="tx-pre">{ c.stack.clone().unwrap_or_else(|| "—".into()) }</pre> },
                ]).collect();
                html!{ <DataTable headers={headers} rows={trows} empty_label="No crashes reported."/> }
            }
        }
    }
}

// ===========================================================================
// data_steward::* — P-A Catalog & Governance
// ===========================================================================

pub mod data_steward {
    use super::*;
    use terraops_shared::dto::import::{ImportBatchSummary, ImportRowDto};
    use terraops_shared::dto::product::{
        CreateProductRequest, ProductDetail, ProductListItem, SetOnShelfRequest,
    };

    #[function_component(ProductsList)]
    pub fn products_list() -> Html {
        html! {
            <Layout title="Products" subtitle="Catalog governance — SKUs, categories, tax, images.">
                <PermGate permission="product.read">
                    <ProductsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ProductsBody)]
    fn products_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<ProductListItem>>::Loading);
        // Audit #6 Issue #3: server-side pagination still drives the backend
        // call (50 rows per request). Audit #11 Issue #4: the UI now
        // accumulates rows across fetches and exposes a "Load more" button
        // instead of classic Prev/Next page navigation.
        let page = use_state(|| 1u32);
        let page_size: u32 = 50;
        let total = use_state(|| Option::<u64>::None);
        let loading_more = use_state(|| false);

        let sku = use_state(String::new);
        let name = use_state(String::new);
        // Migration 0012: product master-data extensions surfaced to the
        // data steward. SPU groups multiple SKUs operationally; barcode is
        // the scanned GTIN/UPC/EAN; shelf_life_days is the freshness window.
        let spu = use_state(String::new);
        let barcode = use_state(String::new);
        let shelf_life_days = use_state(String::new);

        // `fetch` drives every network call. `(target_page, append)`:
        //   * `(1, false)` is a fresh reload — clears rows, shows the full
        //     loading placeholder, used on mount, on successful create, and
        //     when a filter changes (not applicable here but preserves the
        //     shape used across Audit #11 Issue #4 sites).
        //   * `(n, true)` appends page `n` to the existing accumulator for
        //     the "Load more" button; rows stay visible and only the button
        //     shows a local "Loading…" state.
        let fetch = {
            let auth = auth.clone();
            let list = list.clone();
            let total = total.clone();
            let page = page.clone();
            let loading_more = loading_more.clone();
            Callback::from(move |(target_page, append): (u32, bool)| {
                let api = auth.api();
                let list = list.clone();
                let total = total.clone();
                let page = page.clone();
                let loading_more = loading_more.clone();
                let existing: Vec<ProductListItem> = if append {
                    match &*list {
                        LoadState::Loaded(v) => v.clone(),
                        _ => Vec::new(),
                    }
                } else { Vec::new() };
                page.set(target_page);
                if append { loading_more.set(true); } else { list.set(LoadState::Loading); }
                let qs = format!("page={target_page}&page_size={page_size}");
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_products_page_query(&qs).await {
                        Ok(p) => {
                            total.set(Some(p.total));
                            let mut combined = existing;
                            combined.extend(p.items);
                            list.set(LoadState::Loaded(combined));
                            loading_more.set(false);
                        }
                        Err(e) => {
                            loading_more.set(false);
                            if !append {
                                list.set(LoadState::Failed(e.user_facing()));
                            }
                        }
                    }
                });
            })
        };
        let reload = {
            let fetch = fetch.clone();
            Callback::from(move |_: ()| { fetch.emit((1, false)); })
        };
        {
            let fetch = fetch.clone();
            use_effect_with((), move |_| { fetch.emit((1, false)); || () });
        }
        let on_more = {
            let fetch = fetch.clone();
            let page = page.clone();
            Callback::from(move |_: MouseEvent| { fetch.emit((*page + 1, true)); })
        };

        let on_sku = {
            let sku = sku.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                sku.set(t.value());
            })
        };
        let on_name = {
            let name = name.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                name.set(t.value());
            })
        };
        let on_spu = {
            let spu = spu.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                spu.set(t.value());
            })
        };
        let on_barcode = {
            let barcode = barcode.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                barcode.set(t.value());
            })
        };
        let on_shelf_life = {
            let shelf_life_days = shelf_life_days.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                shelf_life_days.set(t.value());
            })
        };

        let can_create = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("product.write"))
            .unwrap_or(false);

        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let sku = sku.clone();
            let name = name.clone();
            let spu = spu.clone();
            let barcode = barcode.clone();
            let shelf_life_days = shelf_life_days.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let trim_opt = |v: &str| {
                    let t = v.trim();
                    if t.is_empty() { None } else { Some(t.to_string()) }
                };
                let shelf_parsed = (*shelf_life_days)
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|n| *n >= 0);
                let req = CreateProductRequest {
                    sku: (*sku).clone(),
                    spu: trim_opt(&*spu),
                    barcode: trim_opt(&*barcode),
                    shelf_life_days: shelf_parsed,
                    name: (*name).clone(),
                    description: None,
                    category_id: None,
                    brand_id: None,
                    unit_id: None,
                    site_id: None,
                    department_id: None,
                    on_shelf: Some(false),
                    price_cents: Some(0),
                    currency: Some("USD".into()),
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let sku = sku.clone();
                let name = name.clone();
                let spu = spu.clone();
                let barcode = barcode.clone();
                let shelf_life_days = shelf_life_days.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_product(&req).await {
                        Ok(new_id) => {
                            toast.success(&format!("Created product {new_id}"));
                            sku.set(String::new());
                            name.set(String::new());
                            spu.set(String::new());
                            barcode.set(String::new());
                            shelf_life_days.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = if can_create {
            html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "New product" }</h2>
                    <form class="tx-form tx-form--row" onsubmit={on_create}>
                        <input class="tx-input" placeholder="SKU" required=true
                               value={(*sku).clone()} oninput={on_sku} />
                        <input class="tx-input" placeholder="Name" required=true
                               value={(*name).clone()} oninput={on_name} />
                        <input class="tx-input" placeholder="SPU (optional)"
                               value={(*spu).clone()} oninput={on_spu} />
                        <input class="tx-input" placeholder="Barcode (optional)"
                               value={(*barcode).clone()} oninput={on_barcode} />
                        <input class="tx-input" type="number" min="0" placeholder="Shelf life (days)"
                               value={(*shelf_life_days).clone()} oninput={on_shelf_life} />
                        <button class="tx-btn" type="submit">{ "Create" }</button>
                    </form>
                </section>
            }
        } else {
            html!()
        };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("SKU"), AttrValue::from("SPU"),
                    AttrValue::from("Barcode"), AttrValue::from("Shelf (d)"),
                    AttrValue::from("Name"),
                    AttrValue::from("Category"), AttrValue::from("Brand"),
                    AttrValue::from("Price"), AttrValue::from("On shelf"),
                    AttrValue::from("Updated"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|p| {
                    let pid = p.id;
                    vec![
                        html! {
                            <Link<Route> to={Route::ProductDetail { id: pid }}
                                         classes={classes!("tx-link")}>
                                <code>{ p.sku.clone() }</code>
                            </Link<Route>>
                        },
                        html! { { p.spu.clone().unwrap_or_else(|| "—".into()) } },
                        html! { <span class="tx-mono">{ p.barcode.clone().unwrap_or_else(|| "—".into()) }</span> },
                        html! { { p.shelf_life_days.map(|n| n.to_string()).unwrap_or_else(|| "—".into()) } },
                        html! { { p.name.clone() } },
                        html! { { p.category_name.clone().unwrap_or_else(|| "—".into()) } },
                        html! { { p.brand_name.clone().unwrap_or_else(|| "—".into()) } },
                        html! {
                            { format!("{} {:.2}",
                                p.currency,
                                (p.price_cents as f64) / 100.0) }
                        },
                        html! { if p.on_shelf { {"✔"} } else { {"—"} } },
                        html! { <span class="tx-mono">{ format_ts(p.updated_at) }</span> },
                    ]
                }).collect();
                let loaded = rows.len() as u32;
                html! { <>
                    <DataTable headers={headers} rows={trows} empty_label="No products."/>
                    <LoadMore loaded={loaded} total={*total} loading={*loading_more}
                              on_more={on_more.clone()} />
                </> }
            }
        };

        html! {
            <>
                { create_card }
                { body }
            </>
        }
    }

    #[derive(Properties, PartialEq)]
    pub struct ProductDetailProps {
        pub id: Uuid,
    }

    #[function_component(ProductDetailPage)]
    pub fn product_detail_page(props: &ProductDetailProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Product" subtitle="Full catalog record with tax rates, images, and history.">
                <PermGate permission="product.read">
                    <ProductDetailBody {id} />
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ProductDetailBody)]
    fn product_detail_body(props: &ProductDetailProps) -> Html {
        use terraops_shared::dto::product::{CreateTaxRateRequest, ProductHistoryEntry};

        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let detail = use_state(|| LoadState::<ProductDetail>::Loading);
        let history = use_state(|| LoadState::<Vec<ProductHistoryEntry>>::Loading);

        let new_state = use_state(String::new);
        let new_rate_bp = use_state(String::new);

        let reload = {
            let auth = auth.clone();
            let detail = detail.clone();
            let history = history.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let detail = detail.clone();
                let history = history.clone();
                detail.set(LoadState::Loading);
                history.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_product(id).await {
                        Ok(v) => detail.set(LoadState::Loaded(v)),
                        Err(e) => detail.set(LoadState::Failed(e.user_facing())),
                    }
                    // History is gated by `product.history.read`; if the
                    // caller lacks it, surface the failure in its own card
                    // without blocking the main detail.
                    match api.product_history(id).await {
                        Ok(v) => history.set(LoadState::Loaded(v)),
                        Err(e) => history.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let can_manage = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("product.write"))
            .unwrap_or(false);

        let add_tax_rate = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let new_state = new_state.clone();
            let new_rate_bp = new_rate_bp.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let state_code = (*new_state).trim().to_uppercase();
                if state_code.is_empty() {
                    toast.error("State code required.");
                    return;
                }
                let rate_bp: i32 = match (*new_rate_bp).trim().parse() {
                    Ok(n) if n >= 0 => n,
                    _ => { toast.error("Rate (bp) must be a non-negative integer."); return; }
                };
                let req = CreateTaxRateRequest { state_code, rate_bp };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let new_state = new_state.clone();
                let new_rate_bp = new_rate_bp.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.add_tax_rate(id, &req).await {
                        Ok(_) => {
                            toast.success("Tax rate added.");
                            new_state.set(String::new());
                            new_rate_bp.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let delete_tax_rate = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |rid: Uuid| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.delete_tax_rate(id, rid).await {
                        Ok(_) => { toast.success("Tax rate removed."); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let delete_image = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |img_id: Uuid| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.delete_product_image(id, img_id).await {
                        Ok(_) => { toast.success("Image removed."); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let toggle_shelf = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let detail_snap = detail.clone();
            Callback::from(move |_: MouseEvent| {
                let LoadState::Loaded(p) = (*detail_snap).clone() else { return };
                let req = SetOnShelfRequest { on_shelf: !p.on_shelf };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.set_product_status(id, &req).await {
                        Ok(_) => {
                            toast.success("Shelf status updated.");
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        match &*detail {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(p) => {
                // --- Tax rates panel -------------------------------------
                let tax_headers = vec![
                    AttrValue::from("State"),
                    AttrValue::from("Rate (bp)"),
                    AttrValue::from("Updated"),
                    AttrValue::from("Actions"),
                ];
                let tax_rows: Vec<Vec<Html>> = p.tax_rates.iter().map(|t| {
                    let rid = t.id;
                    let delete_tax_rate = delete_tax_rate.clone();
                    let ondel = Callback::from(move |_: MouseEvent| delete_tax_rate.emit(rid));
                    vec![
                        html! { <code class="tx-mono">{ t.state_code.clone() }</code> },
                        html! { { format!("{} ({:.2}%)", t.rate_bp, (t.rate_bp as f64) / 100.0) } },
                        html! { <span class="tx-mono">{ format_ts(t.updated_at) }</span> },
                        html! {
                            if can_manage {
                                <button class="tx-btn tx-btn--ghost" onclick={ondel}>
                                    { "Delete" }
                                </button>
                            } else {
                                <span class="tx-subtle">{ "—" }</span>
                            }
                        },
                    ]
                }).collect();

                let bind_ns = {
                    let new_state = new_state.clone();
                    Callback::from(move |e: InputEvent| {
                        let t: HtmlInputElement = e.target_unchecked_into();
                        new_state.set(t.value());
                    })
                };
                let bind_nr = {
                    let new_rate_bp = new_rate_bp.clone();
                    Callback::from(move |e: InputEvent| {
                        let t: HtmlInputElement = e.target_unchecked_into();
                        new_rate_bp.set(t.value());
                    })
                };

                // --- Images panel ----------------------------------------
                let images_html = if p.images.is_empty() {
                    html! { <p class="tx-subtle">{ "No images." }</p> }
                } else {
                    html! {
                        <div class="tx-thumb-grid">
                            { for p.images.iter().map(|img| {
                                let iid = img.id;
                                let delete_image = delete_image.clone();
                                let ondel = Callback::from(move |_: MouseEvent|
                                    delete_image.emit(iid));
                                html! {
                                    <figure class="tx-thumb">
                                        <img src={img.signed_url.clone()}
                                             alt={AttrValue::from(format!("image {iid}"))}/>
                                        <figcaption>
                                            <span class="tx-mono">
                                                { format!("{} · {} B", img.content_type, img.size_bytes) }
                                            </span>
                                            if can_manage {
                                                <button class="tx-btn tx-btn--ghost"
                                                    onclick={ondel}>{ "Delete" }</button>
                                            }
                                        </figcaption>
                                    </figure>
                                }
                            }) }
                        </div>
                    }
                };

                // --- History panel ---------------------------------------
                let history_html = match &*history {
                    LoadState::Loading => html! { <PlaceholderLoading/> },
                    LoadState::Failed(m) => html! { <p class="tx-subtle">{ m.clone() }</p> },
                    LoadState::Loaded(rows) => {
                        let headers = vec![
                            AttrValue::from("When"),
                            AttrValue::from("Action"),
                            AttrValue::from("By"),
                        ];
                        let hrows: Vec<Vec<Html>> = rows.iter().map(|h| vec![
                            html! { <span class="tx-mono">{ format_ts(h.changed_at) }</span> },
                            html! { <span class="tx-chip">{ h.action.clone() }</span> },
                            html! { {
                                h.changed_by_name.clone()
                                    .or_else(|| h.changed_by.map(|u| u.to_string()))
                                    .unwrap_or_else(|| "—".into())
                            } },
                        ]).collect();
                        html! {
                            <DataTable headers={headers} rows={hrows}
                                empty_label="No history yet."/>
                        }
                    }
                };

                html! {
                    <>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">
                                <code>{ p.sku.clone() }</code>{ " — " }{ p.name.clone() }
                            </h2>
                            <div class="tx-kv"><span>{ "SPU" }</span>
                                <span>{ p.spu.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Barcode" }</span>
                                <span class="tx-mono">{ p.barcode.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Shelf life (days)" }</span>
                                <span>{ p.shelf_life_days.map(|n| n.to_string()).unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Description" }</span>
                                <span>{ p.description.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Category" }</span>
                                <span>{ p.category_name.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Brand" }</span>
                                <span>{ p.brand_name.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Unit" }</span>
                                <span>{ p.unit_code.clone().unwrap_or_else(|| "—".into()) }</span></div>
                            <div class="tx-kv"><span>{ "Price" }</span>
                                <span>{ format!("{} {:.2}", p.currency, (p.price_cents as f64) / 100.0) }</span></div>
                            <div class="tx-kv"><span>{ "On shelf" }</span>
                                <span>{ if p.on_shelf { "yes" } else { "no" } }</span></div>
                            <div class="tx-kv"><span>{ "Updated" }</span>
                                <span class="tx-mono">{ format_ts(p.updated_at) }</span></div>
                            if can_manage {
                                <button class="tx-btn tx-btn--ghost" onclick={toggle_shelf}>
                                    { if p.on_shelf { "Take off shelf" } else { "Put on shelf" } }
                                </button>
                            }
                        </section>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">{ "Tax rates" }</h2>
                            <DataTable headers={tax_headers} rows={tax_rows}
                                empty_label="No tax rates configured."/>
                            if can_manage {
                                <form class="tx-form tx-form--inline" onsubmit={add_tax_rate}>
                                    <label class="tx-field">
                                        <span>{ "State (e.g. CA)" }</span>
                                        <input class="tx-input" type="text" maxlength="2"
                                            value={(*new_state).clone()}
                                            oninput={bind_ns}/>
                                    </label>
                                    <label class="tx-field">
                                        <span>{ "Rate (basis points)" }</span>
                                        <input class="tx-input" type="number" min="0"
                                            value={(*new_rate_bp).clone()}
                                            oninput={bind_nr}/>
                                    </label>
                                    <div class="tx-form__actions">
                                        <button type="submit" class="tx-btn">{ "Add rate" }</button>
                                    </div>
                                </form>
                            }
                        </section>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">
                                { format!("Images ({})", p.images.len()) }
                            </h2>
                            { images_html }
                            if can_manage {
                                <p class="tx-subtle">
                                    { "Upload new images via the catalog import or API (see docs)." }
                                </p>
                            }
                        </section>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">{ "Change history" }</h2>
                            { history_html }
                        </section>
                    </>
                }
            },
        }
    }

    #[function_component(ImportsList)]
    pub fn imports_list() -> Html {
        html! {
            <Layout title="Import batches" subtitle="Upload → validate → commit CSV product batches.">
                <PermGate permission="product.import">
                    <ImportsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ImportsBody)]
    fn imports_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<ImportBatchSummary>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_imports().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Filename"), AttrValue::from("Kind"),
                    AttrValue::from("Status"), AttrValue::from("Rows"),
                    AttrValue::from("Errors"), AttrValue::from("Created"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|b| {
                    let bid = b.id;
                    vec![
                        html! {
                            <Link<Route> to={Route::ImportDetail { id: bid }}
                                         classes={classes!("tx-link")}>{ b.filename.clone() }</Link<Route>>
                        },
                        html! { { b.kind.clone() } },
                        html! { <span class="tx-chip">{ b.status.clone() }</span> },
                        html! { { b.row_count } },
                        html! { { b.error_count } },
                        html! { <span class="tx-mono">{ format_ts(b.created_at) }</span> },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No import batches yet."/> }
            }
        }
    }

    #[derive(Properties, PartialEq)]
    pub struct ImportDetailProps {
        pub id: Uuid,
    }

    #[function_component(ImportDetailPage)]
    pub fn import_detail_page(props: &ImportDetailProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Import batch" subtitle="Row-by-row validation results + commit controls.">
                <PermGate permission="product.import">
                    <ImportDetailBody {id} />
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ImportDetailBody)]
    fn import_detail_body(props: &ImportDetailProps) -> Html {
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let summary = use_state(|| LoadState::<ImportBatchSummary>::Loading);
        let rows = use_state(|| LoadState::<Vec<ImportRowDto>>::Loading);

        let reload = {
            let auth = auth.clone();
            let summary = summary.clone();
            let rows = rows.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let summary = summary.clone();
                let rows = rows.clone();
                summary.set(LoadState::Loading);
                rows.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_import(id).await {
                        Ok(v) => summary.set(LoadState::Loaded(v)),
                        Err(e) => summary.set(LoadState::Failed(e.user_facing())),
                    }
                });
                let api2 = auth.api();
                let rows2 = rows.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api2.list_import_rows(id).await {
                        Ok(v) => rows2.set(LoadState::Loaded(v)),
                        Err(e) => rows2.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let run = |op: &'static str| {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let res = match op {
                        "validate" => api.validate_import(id).await.map(|_| ()),
                        "commit" => api.commit_import(id).await.map(|_| ()),
                        "cancel" => api.cancel_import(id).await.map(|_| ()),
                        _ => Ok(()),
                    };
                    match res {
                        Ok(_) => { toast.success(&format!("Batch {op}d")); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let summary_html = match &*summary {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(b) => html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ b.filename.clone() }</h2>
                    <div class="tx-kv"><span>{ "Status" }</span>
                        <span class="tx-chip">{ b.status.clone() }</span></div>
                    <div class="tx-kv"><span>{ "Rows" }</span><span>{ b.row_count }</span></div>
                    <div class="tx-kv"><span>{ "Errors" }</span><span>{ b.error_count }</span></div>
                    <div class="tx-toolbar">
                        <button class="tx-btn tx-btn--ghost" onclick={run("validate")}>{ "Validate" }</button>
                        <button class="tx-btn" onclick={run("commit")}>{ "Commit" }</button>
                        <button class="tx-btn tx-btn--ghost" onclick={run("cancel")}>{ "Cancel" }</button>
                    </div>
                </section>
            },
        };

        let rows_html = match &*rows {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(v) => {
                let headers = vec![
                    AttrValue::from("#"), AttrValue::from("Valid"),
                    AttrValue::from("Raw"), AttrValue::from("Errors"),
                ];
                let trows: Vec<Vec<Html>> = v.iter().map(|r| vec![
                    html! { { r.row_number } },
                    html! { if r.valid { {"✔"} } else { {"✗"} } },
                    html! { <pre class="tx-pre">{ r.raw.to_string() }</pre> },
                    html! { <pre class="tx-pre">{ r.errors.to_string() }</pre> },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No rows parsed."/> }
            }
        };

        html! { <>{ summary_html }{ rows_html }</> }
    }
}

// ===========================================================================
// analyst::* — P-B Environmental Intelligence / KPI / Alerts / Reports
// ===========================================================================

pub mod analyst {
    use super::*;
    use terraops_shared::dto::alert::{AlertRuleDto, CreateAlertRuleRequest};
    use terraops_shared::dto::env_source::{CreateEnvSourceRequest, EnvSourceDto, ObservationDto};
    use terraops_shared::dto::metric::{
        CreateMetricDefinitionRequest, MetricDefinitionDto, MetricSeriesResponse,
    };
    use terraops_shared::dto::report::ReportJobDto;

    #[function_component(Sources)]
    pub fn sources() -> Html {
        html! {
            <Layout title="Environmental sources" subtitle="Sensors, meters, and manual kiosks.">
                <PermGate permission="metric.read">
                    <SourcesBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(SourcesBody)]
    fn sources_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<EnvSourceDto>>::Loading);
        let name = use_state(String::new);
        let kind = use_state(|| "temperature".to_string());

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_env_sources().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let can_manage = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("metric.configure"))
            .unwrap_or(false);

        let on_name = {
            let name = name.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                name.set(t.value());
            })
        };
        let on_kind = {
            let kind = kind.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                kind.set(t.value());
            })
        };
        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let name = name.clone();
            let kind = kind.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let req = CreateEnvSourceRequest {
                    name: (*name).clone(),
                    kind: (*kind).clone(),
                    site_id: None,
                    department_id: None,
                    unit_id: None,
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let name = name.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_env_source(&req).await {
                        Ok(_) => { toast.success("Source created."); name.set(String::new()); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = if can_manage {
            html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "New source" }</h2>
                    <form class="tx-form tx-form--row" onsubmit={on_create}>
                        <input class="tx-input" placeholder="Name" required=true
                               value={(*name).clone()} oninput={on_name}/>
                        <input class="tx-input" placeholder="kind (temperature, humidity, co2, ...)"
                               required=true value={(*kind).clone()} oninput={on_kind}/>
                        <button class="tx-btn" type="submit">{ "Create" }</button>
                    </form>
                </section>
            }
        } else { html!() };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Name"), AttrValue::from("Kind"),
                    AttrValue::from("Updated"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|s| vec![
                    html! { { s.name.clone() } },
                    html! { <span class="tx-chip">{ s.kind.clone() }</span> },
                    html! { <span class="tx-mono">{ format_ts(s.updated_at) }</span> },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No sources yet."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
    }

    #[function_component(Observations)]
    pub fn observations() -> Html {
        html! {
            <Layout title="Observations" subtitle="Latest raw readings across all env sources.">
                <PermGate permission="metric.read">
                    <ObservationsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ObservationsBody)]
    fn observations_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<ObservationDto>>::Loading);
        // Audit #6 Issue #3 kept (server pagination, 50 rows per call);
        // Audit #11 Issue #4: accumulate + "Load more" rather than Prev/Next.
        let page = use_state(|| 1u32);
        let page_size: u32 = 50;
        let total = use_state(|| Option::<u64>::None);
        let loading_more = use_state(|| false);

        let fetch = {
            let auth = auth.clone();
            let list = list.clone();
            let total = total.clone();
            let page = page.clone();
            let loading_more = loading_more.clone();
            Callback::from(move |(target_page, append): (u32, bool)| {
                let api = auth.api();
                let list = list.clone();
                let total = total.clone();
                let page = page.clone();
                let loading_more = loading_more.clone();
                let existing: Vec<ObservationDto> = if append {
                    match &*list {
                        LoadState::Loaded(v) => v.clone(),
                        _ => Vec::new(),
                    }
                } else { Vec::new() };
                page.set(target_page);
                if append { loading_more.set(true); } else { list.set(LoadState::Loading); }
                let qs = format!("page={target_page}&page_size={page_size}");
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_observations_page(&qs).await {
                        Ok(p) => {
                            total.set(Some(p.total));
                            let mut combined = existing;
                            combined.extend(p.items);
                            list.set(LoadState::Loaded(combined));
                            loading_more.set(false);
                        }
                        Err(e) => {
                            loading_more.set(false);
                            if !append {
                                list.set(LoadState::Failed(e.user_facing()));
                            }
                        }
                    }
                });
            })
        };
        let reload = {
            let fetch = fetch.clone();
            Callback::from(move |_: ()| { fetch.emit((1, false)); })
        };
        {
            let fetch = fetch.clone();
            use_effect_with((), move |_| { fetch.emit((1, false)); || () });
        }
        let on_more = {
            let fetch = fetch.clone();
            let page = page.clone();
            Callback::from(move |_: MouseEvent| { fetch.emit((*page + 1, true)); })
        };

        match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Observed"), AttrValue::from("Source"),
                    AttrValue::from("Value"), AttrValue::from("Unit"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|o| vec![
                    html! { <span class="tx-mono">{ format_ts(o.observed_at) }</span> },
                    html! { <span class="tx-mono tx-truncate">{ o.source_id.to_string() }</span> },
                    html! { { format!("{:.3}", o.value) } },
                    html! { { o.unit.clone() } },
                ]).collect();
                let loaded = rows.len() as u32;
                html! { <>
                    <DataTable headers={headers} rows={trows} empty_label="No observations yet."/>
                    <LoadMore loaded={loaded} total={*total} loading={*loading_more}
                              on_more={on_more.clone()} />
                </> }
            }
        }
    }

    #[function_component(Definitions)]
    pub fn definitions() -> Html {
        html! {
            <Layout title="Metric definitions" subtitle="Declarative formulas: moving_average, rate_of_change, comfort_index.">
                <PermGate permission="metric.read">
                    <DefinitionsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(DefinitionsBody)]
    fn definitions_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<MetricDefinitionDto>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_metric_definitions().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        // Audit #5 Issue #4: analyst-facing metric-fusion configuration
        // UI. Gated on `metric.configure` — analysts without configure
        // permission see only the read-only list below.
        let create_card = html! {
            <PermGate permission="metric.configure">
                <CreateDefinitionCard on_created={{
                    let r = reload.clone();
                    Callback::from(move |_| r.emit(()))
                }}/>
            </PermGate>
        };

        let table = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Name"), AttrValue::from("Formula"),
                    AttrValue::from("Window"), AttrValue::from("Enabled"),
                    AttrValue::from(""),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|d| {
                    let did = d.id;
                    vec![
                        html! { { d.name.clone() } },
                        html! { <span class="tx-chip">{ d.formula_kind.clone() }</span> },
                        html! { { format!("{}s", d.window_seconds) } },
                        html! { if d.enabled { {"✔"} } else { {"—"} } },
                        html! {
                            <Link<Route> to={Route::MetricDefinitionDetail { id: did }}
                                         classes={classes!("tx-link")}>{ "Series →" }</Link<Route>>
                        },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No metric definitions."/> }
            }
        };

        html! { <>{ create_card }{ table }</> }
    }

    // ------------------------------------------------------------------
    // Audit #5 Issue #4: analyst metric-fusion configuration form.
    // Captures formula kind, time window, source ids, and — for the
    // comfort_index fusion formula — the alignment rules and
    // confidence-label bands persisted via `FusionConfig`. On submit,
    // the params JSON is built and POSTed to the real
    // `POST /api/v1/metrics/definitions` endpoint.
    // ------------------------------------------------------------------

    #[derive(Properties, PartialEq)]
    struct CreateDefinitionCardProps {
        pub on_created: Callback<()>,
    }

    #[function_component(CreateDefinitionCard)]
    fn create_definition_card(props: &CreateDefinitionCardProps) -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");

        let name = use_state(String::new);
        let formula = use_state(|| "moving_average".to_string());
        let window_s = use_state(|| "300".to_string());
        let sources_csv = use_state(String::new);

        // Fusion-config state (applies when formula = comfort_index).
        let min_align = use_state(|| "0.25".to_string());
        let warn_align = use_state(|| "0.75".to_string());
        let strict = use_state(|| true);
        // Three confidence bands — label/min/max/css_class each.
        let b1_label = use_state(|| "high".to_string());
        let b1_min = use_state(|| "0.80".to_string());
        let b1_max = use_state(|| "1.01".to_string());
        let b1_css = use_state(|| "ok".to_string());
        let b2_label = use_state(|| "medium".to_string());
        let b2_min = use_state(|| "0.50".to_string());
        let b2_max = use_state(|| "0.80".to_string());
        let b2_css = use_state(|| "warn".to_string());
        let b3_label = use_state(|| "low".to_string());
        let b3_min = use_state(|| "0.0".to_string());
        let b3_max = use_state(|| "0.50".to_string());
        let b3_css = use_state(|| "bad".to_string());

        let bind_input = |s: UseStateHandle<String>| {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };
        let bind_select = |s: UseStateHandle<String>| {
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };

        let on_toggle_strict = {
            let strict = strict.clone();
            Callback::from(move |e: Event| {
                let t: HtmlInputElement = e.target_unchecked_into();
                strict.set(t.checked());
            })
        };

        let on_submit = {
            let auth = auth.clone();
            let toast = toast.clone();
            let on_created = props.on_created.clone();
            let name = name.clone();
            let formula = formula.clone();
            let window_s = window_s.clone();
            let sources_csv = sources_csv.clone();
            let min_align = min_align.clone();
            let warn_align = warn_align.clone();
            let strict = strict.clone();
            let b1_label = b1_label.clone();
            let b1_min = b1_min.clone();
            let b1_max = b1_max.clone();
            let b1_css = b1_css.clone();
            let b2_label = b2_label.clone();
            let b2_min = b2_min.clone();
            let b2_max = b2_max.clone();
            let b2_css = b2_css.clone();
            let b3_label = b3_label.clone();
            let b3_min = b3_min.clone();
            let b3_max = b3_max.clone();
            let b3_css = b3_css.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let nm = (*name).trim().to_string();
                if nm.is_empty() {
                    toast.error("name is required");
                    return;
                }
                let Ok(win_s) = (*window_s).trim().parse::<i32>() else {
                    toast.error("window must be an integer (seconds)");
                    return;
                };
                if win_s < 0 {
                    toast.error("window must be >= 0");
                    return;
                }
                // Parse source UUIDs (comma or whitespace separated).
                let mut src: Vec<Uuid> = Vec::new();
                for s in (*sources_csv)
                    .split(|c: char| c == ',' || c.is_whitespace())
                {
                    let s = s.trim();
                    if s.is_empty() { continue; }
                    match Uuid::parse_str(s) {
                        Ok(u) => src.push(u),
                        Err(_) => {
                            toast.error(&format!("source id is not a UUID: {s}"));
                            return;
                        }
                    }
                }

                // Build params JSON. For comfort_index, serialize the
                // fusion config so the backend validator records it on
                // the definition and the strict-mode alignment gate
                // applies at query time.
                let mut params = serde_json::Map::new();
                if (*formula).as_str() == "comfort_index" {
                    let parse_f = |s: &str, label: &str| -> Result<f64, String> {
                        s.trim().parse::<f64>()
                            .map_err(|_| format!("{label} must be a number"))
                    };
                    let min_a = match parse_f(&min_align, "alignment.min_alignment") {
                        Ok(v) => v, Err(m) => { toast.error(&m); return; }
                    };
                    let warn_a = match parse_f(&warn_align, "alignment.warn_alignment") {
                        Ok(v) => v, Err(m) => { toast.error(&m); return; }
                    };
                    params.insert(
                        "alignment".into(),
                        serde_json::json!({
                            "min_alignment": min_a,
                            "warn_alignment": warn_a,
                            "strict": *strict,
                        }),
                    );
                    let build_band = |label: &UseStateHandle<String>,
                                      bmin: &UseStateHandle<String>,
                                      bmax: &UseStateHandle<String>,
                                      css: &UseStateHandle<String>|
                     -> Result<serde_json::Value, String> {
                        let lbl = (**label).trim().to_string();
                        if lbl.is_empty() {
                            return Err("confidence band label is required".into());
                        }
                        let mn = parse_f(bmin, "confidence band min")?;
                        let mx = parse_f(bmax, "confidence band max")?;
                        let css_s = (**css).trim().to_string();
                        Ok(serde_json::json!({
                            "label": lbl, "min": mn, "max": mx, "css_class": css_s
                        }))
                    };
                    let bands = [
                        build_band(&b1_label, &b1_min, &b1_max, &b1_css),
                        build_band(&b2_label, &b2_min, &b2_max, &b2_css),
                        build_band(&b3_label, &b3_min, &b3_max, &b3_css),
                    ];
                    let mut arr = Vec::new();
                    for b in bands {
                        match b {
                            Ok(v) => arr.push(v),
                            Err(m) => { toast.error(&m); return; }
                        }
                    }
                    params.insert(
                        "confidence_labels".into(),
                        serde_json::Value::Array(arr),
                    );
                }
                let params_val = if params.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Object(params))
                };

                let req = CreateMetricDefinitionRequest {
                    name: nm,
                    formula_kind: (*formula).clone(),
                    params: params_val,
                    source_ids: src,
                    window_seconds: Some(win_s),
                };
                let api = auth.api();
                let toast = toast.clone();
                let on_created = on_created.clone();
                let name = name.clone();
                let sources_csv = sources_csv.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_metric_definition(&req).await {
                        Ok(_) => {
                            toast.success("Metric definition created.");
                            name.set(String::new());
                            sources_csv.set(String::new());
                            on_created.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let fusion_section = if (*formula).as_str() == "comfort_index" {
            html! {
                <section class="tx-card tx-card--hint">
                    <h3 class="tx-title tx-title--sm">{ "Fusion alignment + confidence labels" }</h3>
                    <p class="tx-subtle">
                        { "Analyst-configurable comfort_index gating. \
                           `min_alignment` discards live points below the threshold when \
                           strict mode is on; `warn_alignment` is the soft threshold for \
                           the dashboard `ok` chip. Each confidence band paints the \
                           lineage chip via `tx-chip--{css_class}`." }
                    </p>
                    <div class="tx-form tx-form--inline">
                        <label class="tx-field">
                            <span>{ "min_alignment (0..1)" }</span>
                            <input class="tx-input" type="number" step="0.01" min="0" max="1"
                                value={(*min_align).clone()}
                                oninput={bind_input(min_align.clone())}/>
                        </label>
                        <label class="tx-field">
                            <span>{ "warn_alignment (0..1)" }</span>
                            <input class="tx-input" type="number" step="0.01" min="0" max="1"
                                value={(*warn_align).clone()}
                                oninput={bind_input(warn_align.clone())}/>
                        </label>
                        <label class="tx-field">
                            <span>{ "strict" }</span>
                            <input class="tx-input" type="checkbox"
                                checked={*strict} onchange={on_toggle_strict}/>
                        </label>
                    </div>
                    <h4 class="tx-title tx-title--sm">{ "Confidence bands" }</h4>
                    <table class="tx-table">
                        <thead><tr>
                            <th>{ "Label" }</th><th>{ "Min" }</th>
                            <th>{ "Max" }</th><th>{ "CSS class" }</th>
                        </tr></thead>
                        <tbody>
                            <tr>
                                <td><input class="tx-input" value={(*b1_label).clone()}
                                    oninput={bind_input(b1_label.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b1_min).clone()} oninput={bind_input(b1_min.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b1_max).clone()} oninput={bind_input(b1_max.clone())}/></td>
                                <td><input class="tx-input" value={(*b1_css).clone()}
                                    oninput={bind_input(b1_css.clone())}/></td>
                            </tr>
                            <tr>
                                <td><input class="tx-input" value={(*b2_label).clone()}
                                    oninput={bind_input(b2_label.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b2_min).clone()} oninput={bind_input(b2_min.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b2_max).clone()} oninput={bind_input(b2_max.clone())}/></td>
                                <td><input class="tx-input" value={(*b2_css).clone()}
                                    oninput={bind_input(b2_css.clone())}/></td>
                            </tr>
                            <tr>
                                <td><input class="tx-input" value={(*b3_label).clone()}
                                    oninput={bind_input(b3_label.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b3_min).clone()} oninput={bind_input(b3_min.clone())}/></td>
                                <td><input class="tx-input" type="number" step="0.01"
                                    value={(*b3_max).clone()} oninput={bind_input(b3_max.clone())}/></td>
                                <td><input class="tx-input" value={(*b3_css).clone()}
                                    oninput={bind_input(b3_css.clone())}/></td>
                            </tr>
                        </tbody>
                    </table>
                </section>
            }
        } else {
            html! {}
        };

        html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Create metric definition" }</h2>
                <form class="tx-form" onsubmit={on_submit}>
                    <div class="tx-form tx-form--inline">
                        <label class="tx-field">
                            <span>{ "Name" }</span>
                            <input class="tx-input" required=true value={(*name).clone()}
                                oninput={bind_input(name.clone())}/>
                        </label>
                        <label class="tx-field">
                            <span>{ "Formula" }</span>
                            <select class="tx-input" onchange={bind_select(formula.clone())}>
                                <option value="moving_average"
                                    selected={&*formula == "moving_average"}>
                                    { "moving_average" }</option>
                                <option value="rate_of_change"
                                    selected={&*formula == "rate_of_change"}>
                                    { "rate_of_change" }</option>
                                <option value="comfort_index"
                                    selected={&*formula == "comfort_index"}>
                                    { "comfort_index (fusion)" }</option>
                            </select>
                        </label>
                        <label class="tx-field">
                            <span>{ "Window (seconds)" }</span>
                            <input class="tx-input" type="number" min="0"
                                value={(*window_s).clone()}
                                oninput={bind_input(window_s.clone())}/>
                        </label>
                        <label class="tx-field">
                            <span>{ "Source IDs (comma/space separated UUIDs)" }</span>
                            <input class="tx-input" value={(*sources_csv).clone()}
                                oninput={bind_input(sources_csv.clone())}/>
                        </label>
                    </div>
                    { fusion_section }
                    <div class="tx-form__actions">
                        <button type="submit" class="tx-btn">{ "Create definition" }</button>
                    </div>
                </form>
            </section>
        }
    }

    #[derive(Properties, PartialEq)]
    pub struct DefinitionSeriesProps {
        pub id: Uuid,
    }

    #[function_component(DefinitionSeries)]
    pub fn definition_series(props: &DefinitionSeriesProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Metric series" subtitle="Rolling computed series for this metric.">
                <PermGate permission="metric.read">
                    <DefinitionSeriesBody {id} />
                </PermGate>
            </Layout>
        }
    }

    #[function_component(DefinitionSeriesBody)]
    fn definition_series_body(props: &DefinitionSeriesProps) -> Html {
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let series = use_state(|| LoadState::<MetricSeriesResponse>::Loading);
        let reload = {
            let auth = auth.clone();
            let series = series.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let series = series.clone();
                series.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.metric_series(id).await {
                        Ok(v) => series.set(LoadState::Loaded(v)),
                        Err(e) => series.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*series {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(s) => {
                let headers = vec![
                    AttrValue::from("At"),
                    AttrValue::from("Value"),
                    AttrValue::from("Lineage"),
                ];
                let trows: Vec<Vec<Html>> = s.points.iter().map(|p| {
                    let why = match p.computation_id {
                        Some(cid) => html! {
                            <Link<Route>
                                to={Route::MetricComputationLineage { id: cid }}
                                classes={classes!("tx-link")}>
                                { "Why this value?" }
                            </Link<Route>>
                        },
                        None => html! { <span class="tx-subtle">{ "live" }</span> },
                    };
                    vec![
                        html! { <span class="tx-mono">{ format_ts(p.at) }</span> },
                        html! { { format!("{:.3}", p.value) } },
                        why,
                    ]
                }).collect();
                html! {
                    <section class="tx-card">
                        <h2 class="tx-title tx-title--sm">
                            { s.formula_kind.clone() }{ " · " }{ format!("{}s window", s.window_seconds) }
                        </h2>
                        <DataTable headers={headers} rows={trows} empty_label="No computations yet."/>
                    </section>
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Metric lineage ("why this value")
    // ------------------------------------------------------------------

    #[derive(Properties, PartialEq)]
    pub struct ComputationLineageProps {
        pub id: Uuid,
    }

    #[function_component(ComputationLineagePage)]
    pub fn computation_lineage_page(props: &ComputationLineageProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Computation lineage" subtitle="Formula, inputs, and confidence for one computed point.">
                <PermGate permission="metric.read">
                    <ComputationLineageBody {id} />
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ComputationLineageBody)]
    fn computation_lineage_body(props: &ComputationLineageProps) -> Html {
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let lineage = use_state(|| {
            LoadState::<terraops_shared::dto::metric::ComputationLineage>::Loading
        });

        let reload = {
            let auth = auth.clone();
            let lineage = lineage.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let lineage = lineage.clone();
                lineage.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.metric_lineage(id).await {
                        Ok(v) => lineage.set(LoadState::Loaded(v)),
                        Err(e) => lineage.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*lineage {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(l) => {
                let headers = vec![
                    AttrValue::from("Observation"),
                    AttrValue::from("Observed at"),
                    AttrValue::from("Value"),
                ];
                let trows: Vec<Vec<Html>> = l.input_observations.iter().map(|o| vec![
                    html! { <code class="tx-mono">{ o.observation_id.to_string() }</code> },
                    html! { <span class="tx-mono">{ format_ts(o.observed_at) }</span> },
                    html! { { format!("{:.4}", o.value) } },
                ]).collect();
                let align = l.alignment
                    .map(|a| format!("{:.2}", a))
                    .unwrap_or_else(|| "—".into());
                let conf = l.confidence
                    .map(|c| format!("{:.2}", c))
                    .unwrap_or_else(|| "—".into());
                let params_txt = serde_json::to_string_pretty(&l.params)
                    .unwrap_or_else(|_| "{}".into());
                html! {
                    <>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">{ "Why this value?" }</h2>
                            <div class="tx-kv"><span>{ "Computation" }</span>
                                <span class="tx-mono">{ l.computation_id.to_string() }</span></div>
                            <div class="tx-kv"><span>{ "Definition" }</span>
                                <span class="tx-mono">
                                    <Link<Route>
                                        to={Route::MetricDefinitionDetail { id: l.definition_id }}
                                        classes={classes!("tx-link")}>
                                        { l.definition_id.to_string() }
                                    </Link<Route>>
                                </span></div>
                            <div class="tx-kv"><span>{ "Formula" }</span>
                                <span><code>{ l.formula.clone() }</code></span></div>
                            <div class="tx-kv"><span>{ "Result" }</span>
                                <span>{ format!("{:.4}", l.result) }</span></div>
                            <div class="tx-kv"><span>{ "Window" }</span>
                                <span class="tx-mono">
                                    { format!("{} → {}",
                                        format_ts(l.window_start),
                                        format_ts(l.window_end)) }
                                </span></div>
                            <div class="tx-kv"><span>{ "Computed at" }</span>
                                <span class="tx-mono">{ format_ts(l.computed_at) }</span></div>
                            <div class="tx-kv"><span>{ "Alignment" }</span><span>{ align }</span></div>
                            <div class="tx-kv"><span>{ "Confidence" }</span><span>{ conf }</span></div>
                            <div class="tx-kv"><span>{ "Params" }</span>
                                <pre class="tx-pre">{ params_txt }</pre></div>
                        </section>
                        <section class="tx-card">
                            <h2 class="tx-title tx-title--sm">
                                { format!("Inputs ({} observation(s))", l.input_observations.len()) }
                            </h2>
                            <DataTable headers={headers} rows={trows}
                                empty_label="No contributing observations."/>
                        </section>
                    </>
                }
            }
        }
    }

    #[function_component(Kpi)]
    pub fn kpi() -> Html {
        html! {
            <Layout title="KPIs" subtitle="Cycle time, funnel, anomalies, efficiency.">
                <PermGate permission="kpi.read">
                    <KpiBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(KpiBody)]
    fn kpi_body() -> Html {
        use terraops_shared::dto::kpi::{AnomalyRow, CycleTimeRow, DrillRow, EfficiencyRow,
            FunnelResponse, KpiSummary};

        let auth = use_context::<AuthContext>().expect("AuthContext");
        let summary = use_state(|| LoadState::<KpiSummary>::Loading);
        let cycle = use_state(|| LoadState::<Vec<CycleTimeRow>>::Loading);
        let anomalies = use_state(|| LoadState::<Vec<AnomalyRow>>::Loading);
        let efficiency = use_state(|| LoadState::<Vec<EfficiencyRow>>::Loading);
        let drill = use_state(|| LoadState::<Vec<DrillRow>>::Loading);
        // Audit #8 Issue #2: real slice-and-drill funnel surface.
        let funnel = use_state(|| LoadState::<FunnelResponse>::Loading);

        // Filter form state (ISO-8601 timestamps + slicing axes). Empty
        // fields = unfiltered. Audit #5 Issue #3: the KPI workspace now
        // surfaces site / department / category slicing in the UI to
        // match the backend `SliceQuery` contract in
        // `crates/backend/src/kpi/handlers.rs`.
        let from_ts = use_state(String::new);
        let to_ts = use_state(String::new);
        let site_id = use_state(String::new);
        let department_id = use_state(String::new);
        let category = use_state(String::new);
        // Severity is an alert-only slice axis consumed by the funnel.
        let severity = use_state(String::new);
        // Audit #6 Issue #2: replace raw site/department UUID text entry
        // with live selectors sourced from the ref-data endpoints.
        let sites = use_state(|| Vec::<terraops_shared::dto::ref_data::SiteRef>::new());
        let depts = use_state(|| Vec::<terraops_shared::dto::ref_data::DepartmentRef>::new());
        {
            let auth = auth.clone();
            let sites = sites.clone();
            let depts = depts.clone();
            use_effect_with((), move |_| {
                let api = auth.api();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_sites().await { sites.set(v); }
                    if let Ok(v) = api.list_departments().await { depts.set(v); }
                });
                || ()
            });
        }

        let build_qs = {
            let from_ts = from_ts.clone();
            let to_ts = to_ts.clone();
            let site_id = site_id.clone();
            let department_id = department_id.clone();
            let category = category.clone();
            move || -> String {
                let mut parts = Vec::new();
                let f = (*from_ts).trim().to_string();
                let t = (*to_ts).trim().to_string();
                let s = (*site_id).trim().to_string();
                let d = (*department_id).trim().to_string();
                let c = (*category).trim().to_string();
                if !f.is_empty() { parts.push(format!("from={f}")); }
                if !t.is_empty() { parts.push(format!("to={t}")); }
                if !s.is_empty() { parts.push(format!("site_id={s}")); }
                if !d.is_empty() { parts.push(format!("department_id={d}")); }
                if !c.is_empty() { parts.push(format!("category={c}")); }
                parts.join("&")
            }
        };

        // Funnel-specific query string: drops the generic SliceQuery
        // `category` and instead passes `severity` (aliased as `category`
        // on the backend so it also surfaces through the same slice API).
        let build_funnel_qs = {
            let from_ts = from_ts.clone();
            let to_ts = to_ts.clone();
            let site_id = site_id.clone();
            let department_id = department_id.clone();
            let severity = severity.clone();
            move || -> String {
                let mut parts = Vec::new();
                let f = (*from_ts).trim().to_string();
                let t = (*to_ts).trim().to_string();
                let s = (*site_id).trim().to_string();
                let d = (*department_id).trim().to_string();
                let sv = (*severity).trim().to_string();
                if !f.is_empty() { parts.push(format!("from={f}")); }
                if !t.is_empty() { parts.push(format!("to={t}")); }
                if !s.is_empty() { parts.push(format!("site_id={s}")); }
                if !d.is_empty() { parts.push(format!("department_id={d}")); }
                if !sv.is_empty() { parts.push(format!("severity={sv}")); }
                parts.join("&")
            }
        };

        let reload = {
            let auth = auth.clone();
            let summary = summary.clone();
            let cycle = cycle.clone();
            let anomalies = anomalies.clone();
            let efficiency = efficiency.clone();
            let drill = drill.clone();
            let funnel = funnel.clone();
            let build_qs = build_qs.clone();
            let build_funnel_qs = build_funnel_qs.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let summary = summary.clone();
                let cycle = cycle.clone();
                let anomalies = anomalies.clone();
                let efficiency = efficiency.clone();
                let drill = drill.clone();
                let funnel = funnel.clone();
                let qs = build_qs();
                let fqs = build_funnel_qs();
                summary.set(LoadState::Loading);
                cycle.set(LoadState::Loading);
                anomalies.set(LoadState::Loading);
                efficiency.set(LoadState::Loading);
                drill.set(LoadState::Loading);
                funnel.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.kpi_summary().await {
                        Ok(v) => summary.set(LoadState::Loaded(v)),
                        Err(e) => summary.set(LoadState::Failed(e.user_facing())),
                    }
                    match api.kpi_cycle_time_page(&qs).await {
                        Ok(p) => cycle.set(LoadState::Loaded(p.items)),
                        Err(e) => cycle.set(LoadState::Failed(e.user_facing())),
                    }
                    match api.kpi_anomalies_page(&qs).await {
                        Ok(p) => anomalies.set(LoadState::Loaded(p.items)),
                        Err(e) => anomalies.set(LoadState::Failed(e.user_facing())),
                    }
                    match api.kpi_efficiency_page(&qs).await {
                        Ok(p) => efficiency.set(LoadState::Loaded(p.items)),
                        Err(e) => efficiency.set(LoadState::Failed(e.user_facing())),
                    }
                    match api.kpi_drill_page(&qs).await {
                        Ok(p) => drill.set(LoadState::Loaded(p.items)),
                        Err(e) => drill.set(LoadState::Failed(e.user_facing())),
                    }
                    match api.kpi_funnel_query(&fqs).await {
                        Ok(v) => funnel.set(LoadState::Loaded(v)),
                        Err(e) => funnel.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let on_submit = {
            let reload = reload.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                reload.emit(());
            })
        };
        let bind_input = |s: UseStateHandle<String>| {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };

        let filters = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Time window + slicing" }</h2>
                <form class="tx-form tx-form--inline" onsubmit={on_submit}>
                    <label class="tx-field">
                        <span>{ "From (YYYY-MM-DD)" }</span>
                        <input class="tx-input" type="date" value={(*from_ts).clone()}
                            oninput={bind_input(from_ts.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "To (YYYY-MM-DD)" }</span>
                        <input class="tx-input" type="date" value={(*to_ts).clone()}
                            oninput={bind_input(to_ts.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Site" }</span>
                        <select class="tx-input" onchange={{
                            let s = site_id.clone();
                            Callback::from(move |e: Event| {
                                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                s.set(t.value());
                            })
                        }}>
                            <option value="" selected={(*site_id).is_empty()}>{ "— all sites —" }</option>
                            { for sites.iter().map(|s| {
                                let id_s = s.id.to_string();
                                let sel = *site_id == id_s;
                                html! { <option value={id_s.clone()} selected={sel}>
                                    { format!("{} · {}", s.code, s.name) }
                                </option> }
                            }) }
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Department" }</span>
                        <select class="tx-input" onchange={{
                            let d = department_id.clone();
                            Callback::from(move |e: Event| {
                                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                d.set(t.value());
                            })
                        }}>
                            <option value="" selected={(*department_id).is_empty()}>{ "— all departments —" }</option>
                            { for depts.iter().filter(|d| {
                                (*site_id).is_empty() || d.site_id.to_string() == *site_id
                            }).map(|d| {
                                let id_s = d.id.to_string();
                                let sel = *department_id == id_s;
                                html! { <option value={id_s.clone()} selected={sel}>
                                    { format!("{} · {}", d.code, d.name) }
                                </option> }
                            }) }
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Category (optional)" }</span>
                        <input class="tx-input" type="text" value={(*category).clone()}
                            placeholder="e.g. cycle_time"
                            oninput={bind_input(category.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Severity (funnel)" }</span>
                        <select class="tx-input" onchange={{
                            let s = severity.clone();
                            Callback::from(move |e: Event| {
                                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                s.set(t.value());
                            })
                        }}>
                            <option value="" selected={(*severity).is_empty()}>{ "— any severity —" }</option>
                            <option value="info"     selected={&*severity == "info"}>{ "info" }</option>
                            <option value="warning"  selected={&*severity == "warning"}>{ "warning" }</option>
                            <option value="critical" selected={&*severity == "critical"}>{ "critical" }</option>
                        </select>
                    </label>
                    <div class="tx-form__actions">
                        <button type="submit" class="tx-btn">{ "Apply" }</button>
                        <button type="button" class="tx-btn tx-btn--ghost" onclick={{
                            let from_ts = from_ts.clone();
                            let to_ts = to_ts.clone();
                            let site_id = site_id.clone();
                            let department_id = department_id.clone();
                            let category = category.clone();
                            let severity = severity.clone();
                            let reload = reload.clone();
                            Callback::from(move |_: MouseEvent| {
                                from_ts.set(String::new());
                                to_ts.set(String::new());
                                site_id.set(String::new());
                                department_id.set(String::new());
                                category.set(String::new());
                                severity.set(String::new());
                                reload.emit(());
                            })
                        }}>{ "Clear" }</button>
                    </div>
                </form>
                <p class="tx-subtle">
                    { "Empty fields = unfiltered. Site / department / category slice every \
                       table below via the backend SliceQuery contract." }
                </p>
            </section>
        };

        let summary_cards = match &*summary {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(s) => html! {
                <section class="tx-grid">
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "Cycle time (avg h)" }</h2>
                        <div class="tx-kpi">{ format!("{:.2}", s.cycle_time_avg_hours) }</div>
                    </article>
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "Funnel conversion" }</h2>
                        <div class="tx-kpi">{ format!("{:.1}%", s.funnel_conversion_pct) }</div>
                    </article>
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "Anomalies (today)" }</h2>
                        <div class="tx-kpi">{ s.anomaly_count }</div>
                    </article>
                    <article class="tx-card">
                        <h2 class="tx-title tx-title--sm">{ "Efficiency index" }</h2>
                        <div class="tx-kpi">{ format!("{:.2}", s.efficiency_index) }</div>
                    </article>
                    <article class="tx-card tx-card--hint">
                        <p class="tx-subtle">
                            { format!("Generated {}", format_ts(s.generated_at)) }
                        </p>
                    </article>
                </section>
            },
        };

        let render_cycle = |list: &LoadState<Vec<CycleTimeRow>>| match list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <p class="tx-error">{ m.clone() }</p>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Day"),
                    AttrValue::from("Site"),
                    AttrValue::from("Dept"),
                    AttrValue::from("Avg hours"),
                    AttrValue::from("Count"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { <span class="tx-mono">{ format_date(r.day) }</span> },
                    html! { <code class="tx-mono">{
                        r.site_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { <code class="tx-mono">{
                        r.department_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { { format!("{:.2}", r.avg_hours) } },
                    html! { { r.count } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows}
                                   empty_label="No cycle-time rows in this window."/> }
            }
        };

        let render_anomalies = |list: &LoadState<Vec<AnomalyRow>>| match list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! { <p class="tx-error">{ m.clone() }</p> },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Day"),
                    AttrValue::from("Site"),
                    AttrValue::from("Dept"),
                    AttrValue::from("Count"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { <span class="tx-mono">{ format_date(r.day) }</span> },
                    html! { <code class="tx-mono">{
                        r.site_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { <code class="tx-mono">{
                        r.department_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { { r.count } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows}
                                   empty_label="No anomalies in this window."/> }
            }
        };

        let render_efficiency = |list: &LoadState<Vec<EfficiencyRow>>| match list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! { <p class="tx-error">{ m.clone() }</p> },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Day"),
                    AttrValue::from("Site"),
                    AttrValue::from("Dept"),
                    AttrValue::from("Index"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { <span class="tx-mono">{ format_date(r.day) }</span> },
                    html! { <code class="tx-mono">{
                        r.site_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { <code class="tx-mono">{
                        r.department_id.map(|u| u.to_string()).unwrap_or_else(|| "—".into())
                    }</code> },
                    html! { { format!("{:.3}", r.index) } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows}
                                   empty_label="No efficiency rows in this window."/> }
            }
        };

        let render_drill = |list: &LoadState<Vec<DrillRow>>| match list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! { <p class="tx-error">{ m.clone() }</p> },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Dimension"),
                    AttrValue::from("Label"),
                    AttrValue::from("Metric"),
                    AttrValue::from("Value"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { { r.dimension.clone() } },
                    html! { { r.label.clone() } },
                    html! { <span class="tx-chip">{ r.metric_kind.clone() }</span> },
                    html! { { format!("{:.3}", r.value) } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows}
                                   empty_label="No drill rows in this window."/> }
            }
        };

        let render_funnel = |state: &LoadState<FunnelResponse>| match state {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! { <p class="tx-error">{ m.clone() }</p> },
            LoadState::Loaded(resp) => {
                let headers = vec![
                    AttrValue::from("Stage"),
                    AttrValue::from("Count"),
                    AttrValue::from("Conversion"),
                ];
                let trows: Vec<Vec<Html>> = resp.stages.iter().map(|s| vec![
                    html! { <span class="tx-chip">{ s.stage.clone() }</span> },
                    html! { { s.count } },
                    html! { { format!("{:.1}%", s.conversion_pct) } },
                ]).collect();
                html! {
                    <>
                        <p class="tx-subtle">
                            { format!("Overall conversion: {:.1}%", resp.overall_conversion_pct) }
                        </p>
                        <DataTable headers={headers} rows={trows}
                                   empty_label="No alert events in this slice."/>
                    </>
                }
            }
        };

        html! {
            <>
                { filters }
                { summary_cards }
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Funnel (sliced)" }</h2>
                    { render_funnel(&*funnel) }
                </section>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Cycle time" }</h2>
                    { render_cycle(&*cycle) }
                </section>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Anomalies" }</h2>
                    { render_anomalies(&*anomalies) }
                </section>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Efficiency" }</h2>
                    { render_efficiency(&*efficiency) }
                </section>
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Drill-down" }</h2>
                    { render_drill(&*drill) }
                </section>
            </>
        }
    }

    #[function_component(AlertRules)]
    pub fn alert_rules() -> Html {
        html! {
            <Layout title="Alert rules" subtitle="Threshold × metric definition → events + notifications.">
                <PermGate permission="alert.manage">
                    <AlertRulesBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(AlertRulesBody)]
    fn alert_rules_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<AlertRuleDto>>::Loading);
        // Audit #6 Issue #2: replace raw UUID text entry with a business-
        // facing selector backed by live metric definitions.
        let defs = use_state(|| Vec::<MetricDefinitionDto>::new());
        let metric_id = use_state(String::new);
        let threshold = use_state(String::new);
        let op = use_state(|| ">".to_string());
        // Audit #5 Issue #6: prompt-style threshold rules need
        // duration (seconds the condition must hold before firing)
        // and severity (info|warning|critical). Both already exist
        // in the backend CreateAlertRuleRequest contract.
        let duration_s = use_state(|| "0".to_string());
        let severity = use_state(|| "warning".to_string());

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_alert_rules().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        // Load metric definitions once for the dropdown selector.
        {
            let auth = auth.clone();
            let defs = defs.clone();
            let metric_id = metric_id.clone();
            use_effect_with((), move |_| {
                let api = auth.api();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_metric_definitions().await {
                        if metric_id.is_empty() {
                            if let Some(first) = v.first() { metric_id.set(first.id.to_string()); }
                        }
                        defs.set(v);
                    }
                });
                || ()
            });
        }

        let on_mid = {
            let metric_id = metric_id.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                metric_id.set(t.value());
            })
        };
        let on_th = {
            let threshold = threshold.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                threshold.set(t.value());
            })
        };
        let on_op = {
            let op = op.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                op.set(t.value());
            })
        };
        let on_duration = {
            let duration_s = duration_s.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                duration_s.set(t.value());
            })
        };
        let on_severity = {
            let severity = severity.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                severity.set(t.value());
            })
        };

        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let metric_id = metric_id.clone();
            let threshold = threshold.clone();
            let op = op.clone();
            let duration_s = duration_s.clone();
            let severity = severity.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let Ok(mid) = Uuid::parse_str(metric_id.trim()) else {
                    toast.error("metric_definition_id must be a UUID");
                    return;
                };
                let Ok(th) = threshold.parse::<f64>() else {
                    toast.error("threshold must be a number");
                    return;
                };
                let dur_parsed = (*duration_s).trim().parse::<i32>().unwrap_or(0);
                if dur_parsed < 0 {
                    toast.error("duration must be >= 0 seconds");
                    return;
                }
                let sev = (*severity).clone();
                if !["info", "warning", "critical"].contains(&sev.as_str()) {
                    toast.error("severity must be info|warning|critical");
                    return;
                }
                let req = CreateAlertRuleRequest {
                    metric_definition_id: mid,
                    threshold: th,
                    operator: (*op).clone(),
                    duration_seconds: Some(dur_parsed),
                    severity: Some(sev),
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_alert_rule(&req).await {
                        Ok(_) => { toast.success("Alert rule created."); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "New rule" }</h2>
                <form class="tx-form tx-form--row" onsubmit={on_create}>
                    <select class="tx-input" required=true onchange={on_mid}>
                        if defs.is_empty() {
                            <option value="" selected=true>{ "No metric definitions yet" }</option>
                        } else {
                            { for defs.iter().map(|d| {
                                let id_s = d.id.to_string();
                                let sel = *metric_id == id_s;
                                html! {
                                    <option value={id_s.clone()} selected={sel}>
                                        { format!("{} · {}", d.name, d.formula_kind) }
                                    </option>
                                }
                            }) }
                        }
                    </select>
                    <input class="tx-input" type="number" step="any"
                           placeholder="threshold" required=true
                           value={(*threshold).clone()} oninput={on_th}/>
                    <select class="tx-input" onchange={on_op}>
                        <option value=">"  selected={&*op == ">"}>{ ">" }</option>
                        <option value="<"  selected={&*op == "<"}>{ "<" }</option>
                        <option value=">=" selected={&*op == ">="}>{ ">=" }</option>
                        <option value="<=" selected={&*op == "<="}>{ "<=" }</option>
                        <option value="="  selected={&*op == "="}>{ "=" }</option>
                    </select>
                    <input class="tx-input" type="number" min="0"
                           placeholder="duration seconds (0 = fire immediately)"
                           value={(*duration_s).clone()} oninput={on_duration}/>
                    <select class="tx-input" onchange={on_severity}>
                        <option value="info"     selected={&*severity == "info"}>{ "info" }</option>
                        <option value="warning"  selected={&*severity == "warning"}>{ "warning" }</option>
                        <option value="critical" selected={&*severity == "critical"}>{ "critical" }</option>
                    </select>
                    <button class="tx-btn" type="submit">{ "Create" }</button>
                </form>
                <p class="tx-subtle">
                    { "Duration is the minimum time the threshold must hold before the rule \
                       fires — a prompt-style sustained-breach gate. Severity drives \
                       notification priority and the alert_digest report filter." }
                </p>
            </section>
        };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Metric"), AttrValue::from("Op"),
                    AttrValue::from("Threshold"),
                    AttrValue::from("Duration"),
                    AttrValue::from("Severity"),
                    AttrValue::from("Enabled"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { <span class="tx-mono tx-truncate">{ r.metric_definition_id.to_string() }</span> },
                    html! { <code>{ r.operator.clone() }</code> },
                    html! { { format!("{:.3}", r.threshold) } },
                    html! { <span class="tx-mono">{ format!("{}s", r.duration_seconds) }</span> },
                    html! { <span class="tx-chip">{ r.severity.clone() }</span> },
                    html! { if r.enabled { {"✔"} } else { {"—"} } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No alert rules."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
    }

    #[function_component(Reports)]
    pub fn reports() -> Html {
        html! {
            <Layout title="Report jobs" subtitle="Scheduled KPI + env exports.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("report.schedule"),
                    AttrValue::from("report.run"),
                ]}>
                    <ReportsBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(ReportsBody)]
    fn reports_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<ReportJobDto>>::Loading);

        // Create form state — `kind` × `format` × optional cron (RP1–RP2).
        // Audit #5 Issue #1: frontend kinds/formats now match the real
        // backend contract in `crates/backend/src/reports/handlers.rs`
        // (kpi_summary|env_series|alert_digest × pdf|csv|xlsx).
        // Audit #5 Issue #2: the form captures the prompt-required
        // filter params (since/until/limit/severity/source_id/
        // definition_id) and serialises them into the job's `params`
        // JSONB so the scheduler can honor them end-to-end.
        let kind = use_state(|| "kpi_summary".to_string());
        let fmt = use_state(|| "csv".to_string());
        let cron = use_state(String::new);
        let since = use_state(String::new);
        let until = use_state(String::new);
        let limit = use_state(|| "50".to_string());
        let severity = use_state(String::new);
        let source_id = use_state(String::new);
        let definition_id = use_state(String::new);
        // Audit #7 Issue #6: env_series now accepts site/department slicing.
        let report_site_id = use_state(String::new);
        let report_department_id = use_state(String::new);
        // Audit #6 Issue #2: selectors instead of UUID text entry.
        let env_sources = use_state(|| Vec::<EnvSourceDto>::new());
        let defs = use_state(|| Vec::<MetricDefinitionDto>::new());
        let sites_list = use_state(|| Vec::<terraops_shared::dto::ref_data::SiteRef>::new());
        let depts_list = use_state(|| Vec::<terraops_shared::dto::ref_data::DepartmentRef>::new());
        {
            let auth = auth.clone();
            let env_sources = env_sources.clone();
            let defs = defs.clone();
            let sites_list = sites_list.clone();
            let depts_list = depts_list.clone();
            use_effect_with((), move |_| {
                let api = auth.api();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_env_sources().await { env_sources.set(v); }
                    if let Ok(v) = api.list_metric_definitions().await { defs.set(v); }
                    if let Ok(v) = api.list_sites().await { sites_list.set(v); }
                    if let Ok(v) = api.list_departments().await { depts_list.set(v); }
                });
                || ()
            });
        }

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_report_jobs().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let run_now = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |id: Uuid| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.run_report_now(id).await {
                        Ok(_) => { toast.success("Report run queued."); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let on_submit = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let kind = kind.clone();
            let fmt = fmt.clone();
            let cron = cron.clone();
            let since = since.clone();
            let until = until.clone();
            let limit = limit.clone();
            let severity = severity.clone();
            let source_id = source_id.clone();
            let definition_id = definition_id.clone();
            let report_site_id = report_site_id.clone();
            let report_department_id = report_department_id.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let cron_val = {
                    let t = (*cron).trim();
                    if t.is_empty() { None } else { Some(t.to_string()) }
                };
                // Build a params JSON object honoring any fields the
                // analyst filled in. Missing fields are omitted so the
                // backend defaults apply.
                let mut obj = serde_json::Map::new();
                let s_since = (*since).trim().to_string();
                if !s_since.is_empty() {
                    obj.insert("since".into(), serde_json::Value::String(s_since));
                }
                let s_until = (*until).trim().to_string();
                if !s_until.is_empty() {
                    obj.insert("until".into(), serde_json::Value::String(s_until));
                }
                if let Ok(n) = (*limit).trim().parse::<i64>() {
                    obj.insert("limit".into(), serde_json::Value::Number(n.into()));
                }
                match (*kind).as_str() {
                    "alert_digest" => {
                        let s = (*severity).trim().to_string();
                        if !s.is_empty() {
                            obj.insert("severity".into(), serde_json::Value::String(s));
                        }
                    }
                    "env_series" => {
                        let s = (*source_id).trim().to_string();
                        if !s.is_empty() {
                            obj.insert("source_id".into(), serde_json::Value::String(s));
                        }
                        // Audit #7 Issue #6: site/department spatial slicing.
                        let sid = (*report_site_id).trim().to_string();
                        if !sid.is_empty() {
                            obj.insert("site_id".into(), serde_json::Value::String(sid));
                        }
                        let did = (*report_department_id).trim().to_string();
                        if !did.is_empty() {
                            obj.insert(
                                "department_id".into(),
                                serde_json::Value::String(did),
                            );
                        }
                    }
                    "kpi_summary" => {
                        let s = (*definition_id).trim().to_string();
                        if !s.is_empty() {
                            obj.insert("definition_id".into(), serde_json::Value::String(s));
                        }
                    }
                    _ => {}
                }
                let params_val = if obj.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Object(obj))
                };
                let req = terraops_shared::dto::report::CreateReportJobRequest {
                    kind: (*kind).clone(),
                    format: (*fmt).clone(),
                    params: params_val,
                    cron: cron_val,
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let cron = cron.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_report_job(&req).await {
                        Ok(_) => {
                            toast.success("Report scheduled.");
                            cron.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let download_artifact = {
            let auth = auth.clone();
            let toast = toast.clone();
            Callback::from(move |(id, fmt): (Uuid, String)| {
                let api = auth.api();
                let toast = toast.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.download_report_artifact(id).await {
                        Ok(bytes) => {
                            if let Err(e) = trigger_blob_download(
                                &bytes,
                                &format!("report-{id}.{fmt}"),
                            ) {
                                toast.error(&format!("Download failed: {e}"));
                            }
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        // Schedule form card.
        let bind_str = |s: UseStateHandle<String>| {
            Callback::from(move |e: Event| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };
        let bind_input = |s: UseStateHandle<String>| {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };

        // Kind-specific extra filter field (single optional UUID or
        // severity chip). Rendered conditionally so the analyst only
        // sees inputs that actually affect the chosen kind.
        let extra_filter = match (*kind).as_str() {
            "alert_digest" => html! {
                <label class="tx-field">
                    <span>{ "Severity" }</span>
                    <select class="tx-input" onchange={bind_str(severity.clone())}>
                        <option value="" selected={(*severity).is_empty()}>{ "— any —" }</option>
                        <option value="info"     selected={&*severity == "info"}>{ "info" }</option>
                        <option value="warning"  selected={&*severity == "warning"}>{ "warning" }</option>
                        <option value="critical" selected={&*severity == "critical"}>{ "critical" }</option>
                    </select>
                </label>
            },
            "env_series" => {
                let site_sel = (*report_site_id).clone();
                let dept_sel = (*report_department_id).clone();
                let depts_filtered: Vec<&terraops_shared::dto::ref_data::DepartmentRef> = depts_list
                    .iter()
                    .filter(|d| site_sel.is_empty() || d.site_id.to_string() == site_sel)
                    .collect();
                html! {
                    <>
                        <label class="tx-field">
                            <span>{ "Environmental source" }</span>
                            <select class="tx-input" onchange={bind_str(source_id.clone())}>
                                <option value="" selected={(*source_id).is_empty()}>{ "— any source —" }</option>
                                { for env_sources.iter().map(|s| {
                                    let id_s = s.id.to_string();
                                    let sel = *source_id == id_s;
                                    html! { <option value={id_s.clone()} selected={sel}>
                                        { format!("{} ({})", s.name, s.kind) }
                                    </option> }
                                }) }
                            </select>
                        </label>
                        <label class="tx-field">
                            <span>{ "Site" }</span>
                            <select class="tx-input" onchange={bind_str(report_site_id.clone())}>
                                <option value="" selected={site_sel.is_empty()}>{ "— any site —" }</option>
                                { for sites_list.iter().map(|s| {
                                    let id_s = s.id.to_string();
                                    let sel = site_sel == id_s;
                                    html! { <option value={id_s.clone()} selected={sel}>
                                        { s.name.clone() }
                                    </option> }
                                }) }
                            </select>
                        </label>
                        <label class="tx-field">
                            <span>{ "Department" }</span>
                            <select class="tx-input" onchange={bind_str(report_department_id.clone())}>
                                <option value="" selected={dept_sel.is_empty()}>{ "— any dept —" }</option>
                                { for depts_filtered.iter().map(|d| {
                                    let id_s = d.id.to_string();
                                    let sel = dept_sel == id_s;
                                    html! { <option value={id_s.clone()} selected={sel}>
                                        { d.name.clone() }
                                    </option> }
                                }) }
                            </select>
                        </label>
                    </>
                }
            },
            "kpi_summary" => html! {
                <label class="tx-field">
                    <span>{ "Metric definition" }</span>
                    <select class="tx-input" onchange={bind_str(definition_id.clone())}>
                        <option value="" selected={(*definition_id).is_empty()}>{ "— any definition —" }</option>
                        { for defs.iter().map(|d| {
                            let id_s = d.id.to_string();
                            let sel = *definition_id == id_s;
                            html! { <option value={id_s.clone()} selected={sel}>
                                { format!("{} · {}", d.name, d.formula_kind) }
                            </option> }
                        }) }
                    </select>
                </label>
            },
            _ => html! {},
        };

        let create_card = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Schedule a new report" }</h2>
                <form class="tx-form tx-form--inline" onsubmit={on_submit}>
                    <label class="tx-field">
                        <span>{ "Kind" }</span>
                        <select class="tx-input" onchange={bind_str(kind.clone())}>
                            <option value="kpi_summary"
                                selected={&*kind == "kpi_summary"}>{ "KPI summary" }</option>
                            <option value="env_series"
                                selected={&*kind == "env_series"}>{ "Env observations (series)" }</option>
                            <option value="alert_digest"
                                selected={&*kind == "alert_digest"}>{ "Alert digest" }</option>
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Format" }</span>
                        <select class="tx-input" onchange={bind_str(fmt.clone())}>
                            <option value="csv"  selected={&*fmt == "csv"}>{ "CSV" }</option>
                            <option value="pdf"  selected={&*fmt == "pdf"}>{ "PDF" }</option>
                            <option value="xlsx" selected={&*fmt == "xlsx"}>{ "XLSX" }</option>
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Since (RFC 3339, optional)" }</span>
                        <input class="tx-input" type="text" value={(*since).clone()}
                            placeholder="2026-04-01T00:00:00Z"
                            oninput={bind_input(since.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Until (RFC 3339, optional)" }</span>
                        <input class="tx-input" type="text" value={(*until).clone()}
                            placeholder="2026-04-20T23:59:59Z"
                            oninput={bind_input(until.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Row limit (1..=1000)" }</span>
                        <input class="tx-input" type="number" min="1" max="1000"
                            value={(*limit).clone()}
                            oninput={bind_input(limit.clone())}/>
                    </label>
                    { extra_filter }
                    <label class="tx-field">
                        <span>{ "Cron (optional)" }</span>
                        <input class="tx-input" type="text" value={(*cron).clone()}
                            placeholder="e.g. 0 8 * * *"
                            oninput={bind_input(cron.clone())}/>
                    </label>
                    <div class="tx-form__actions">
                        <button type="submit" class="tx-btn">{ "Schedule" }</button>
                    </div>
                </form>
                <p class="tx-subtle">
                    { "Leave cron empty for a one-off job. The scheduler honors the \
                       filter block as persisted params; only the fields you fill are sent." }
                </p>
            </section>
        };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Kind"), AttrValue::from("Format"),
                    AttrValue::from("Cron"),
                    AttrValue::from("Status"), AttrValue::from("Last run"),
                    AttrValue::from("Actions"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|j| {
                    let jid = j.id;
                    let jfmt = j.format.clone();
                    let has_artifact = j.last_artifact_path.is_some();
                    let run_now = run_now.clone();
                    let download_artifact = download_artifact.clone();
                    let onrun = Callback::from(move |_: MouseEvent| run_now.emit(jid));
                    let ondl = {
                        let jfmt = jfmt.clone();
                        Callback::from(move |_: MouseEvent| {
                            download_artifact.emit((jid, jfmt.clone()));
                        })
                    };
                    vec![
                        html! { { j.kind.clone() } },
                        html! { <code>{ j.format.clone() }</code> },
                        html! { <span class="tx-mono">{
                            j.cron.clone().unwrap_or_else(|| "—".into())
                        }</span> },
                        html! { <span class="tx-chip">{ j.status.clone() }</span> },
                        html! { <span class="tx-mono">{
                            j.last_run_at.map(|t| format_ts(t)).unwrap_or_else(|| "—".into())
                        }</span> },
                        html! {
                            <div class="tx-row-actions">
                                <button class="tx-btn tx-btn--ghost" onclick={onrun}>
                                    { "Run now" }
                                </button>
                                if has_artifact {
                                    <button class="tx-btn tx-btn--ghost" onclick={ondl}>
                                        { "Download" }
                                    </button>
                                }
                            </div>
                        },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No report jobs."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
    }

    /// Trigger a browser download from a raw byte buffer.
    ///
    /// The SPA uses this after `download_report_artifact()` returns to
    /// surface artifact bytes to the user as a real file rather than a
    /// navigation away from the app.
    fn trigger_blob_download(bytes: &[u8], filename: &str) -> Result<(), String> {
        use js_sys::{Array, Uint8Array};
        use wasm_bindgen::JsCast;
        use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};
        let u8a = Uint8Array::new_with_length(bytes.len() as u32);
        u8a.copy_from(bytes);
        let parts = Array::new();
        parts.push(&u8a.buffer());
        let bag = BlobPropertyBag::new();
        bag.set_type("application/octet-stream");
        let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &bag)
            .map_err(|_| "blob construction failed".to_string())?;
        let url = Url::create_object_url_with_blob(&blob)
            .map_err(|_| "url.createObjectURL failed".to_string())?;
        let win = web_sys::window().ok_or("no window")?;
        let doc = win.document().ok_or("no document")?;
        let a: HtmlAnchorElement = doc
            .create_element("a")
            .map_err(|_| "create <a> failed".to_string())?
            .dyn_into()
            .map_err(|_| "not an anchor".to_string())?;
        a.set_href(&url);
        a.set_download(filename);
        a.click();
        let _ = Url::revoke_object_url(&url);
        Ok(())
    }
}

// ===========================================================================
// user::* — End-user alerts feed (AL5 surface for consumers)
// ===========================================================================

pub mod user {
    use super::*;
    use terraops_shared::dto::alert::AlertEventDto;

    #[function_component(AlertsFeed)]
    pub fn alerts_feed() -> Html {
        html! {
            <Layout title="Alert events" subtitle="Fired events — ack to acknowledge.">
                <PermGate permission="alert.ack">
                    <AlertsFeedBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(AlertsFeedBody)]
    fn alerts_feed_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<AlertEventDto>>::Loading);
        // Audit #6 Issue #3 (server pagination) + Audit #11 Issue #4
        // (incremental Load more).
        let page = use_state(|| 1u32);
        let page_size: u32 = 50;
        let total = use_state(|| Option::<u64>::None);
        let loading_more = use_state(|| false);

        let fetch = {
            let auth = auth.clone();
            let list = list.clone();
            let total = total.clone();
            let page = page.clone();
            let loading_more = loading_more.clone();
            Callback::from(move |(target_page, append): (u32, bool)| {
                let api = auth.api();
                let list = list.clone();
                let total = total.clone();
                let page = page.clone();
                let loading_more = loading_more.clone();
                let existing: Vec<AlertEventDto> = if append {
                    match &*list {
                        LoadState::Loaded(v) => v.clone(),
                        _ => Vec::new(),
                    }
                } else { Vec::new() };
                page.set(target_page);
                if append { loading_more.set(true); } else { list.set(LoadState::Loading); }
                let qs = format!("page={target_page}&page_size={page_size}");
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_alert_events_page_query(&qs).await {
                        Ok(p) => {
                            total.set(Some(p.total));
                            let mut combined = existing;
                            combined.extend(p.items);
                            list.set(LoadState::Loaded(combined));
                            loading_more.set(false);
                        }
                        Err(e) => {
                            loading_more.set(false);
                            if !append {
                                list.set(LoadState::Failed(e.user_facing()));
                            }
                        }
                    }
                });
            })
        };
        let reload = {
            let fetch = fetch.clone();
            Callback::from(move |_: ()| { fetch.emit((1, false)); })
        };
        {
            let fetch = fetch.clone();
            use_effect_with((), move |_| { fetch.emit((1, false)); || () });
        }
        let on_more = {
            let fetch = fetch.clone();
            let page = page.clone();
            Callback::from(move |_: MouseEvent| { fetch.emit((*page + 1, true)); })
        };

        let ack = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |id: Uuid| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.ack_alert_event(id).await {
                        Ok(_) => { toast.success("Acknowledged."); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Fired"), AttrValue::from("Severity"),
                    AttrValue::from("Value"), AttrValue::from("Acked"),
                    AttrValue::from(""),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|ev| {
                    let eid = ev.id;
                    let ack = ack.clone();
                    let onclick = Callback::from(move |_: MouseEvent| ack.emit(eid));
                    let acked = ev.acked_at.is_some();
                    vec![
                        html! { <span class="tx-mono">{ format_ts(ev.fired_at) }</span> },
                        html! { <span class="tx-chip">{ ev.severity.clone() }</span> },
                        html! { { format!("{:.3}", ev.value) } },
                        html! { if acked { {"✔"} } else { {"—"} } },
                        html! {
                            if !acked {
                                <button class="tx-btn tx-btn--ghost" onclick={onclick}>{ "Ack" }</button>
                            }
                        },
                    ]
                }).collect();
                let loaded = rows.len() as u32;
                html! { <>
                    <DataTable headers={headers} rows={trows} empty_label="No alert events yet."/>
                    <LoadMore loaded={loaded} total={*total} loading={*loading_more}
                              on_more={on_more.clone()} />
                </> }
            }
        }
    }
}

// ===========================================================================
// recruiter::* — P-C Talent Intelligence
// ===========================================================================

pub mod recruiter {
    use super::*;
    use terraops_shared::dto::talent::{
        AddWatchlistItemRequest, CandidateDetail, CandidateListItem, CreateFeedbackRequest,
        CreateRoleRequest, CreateWatchlistRequest, RankedCandidate, RoleOpenItem,
        UpdateWeightsRequest, WatchlistEntry, WatchlistItem,
    };

    #[function_component(Candidates)]
    pub fn candidates() -> Html {
        html! {
            <Layout title="Candidates" subtitle="Talent pool — search, view, add to watchlist.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <CandidatesBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(CandidatesBody)]
    fn candidates_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<CandidateListItem>>::Loading);
        // Server pagination (Audit #6 Issue #3) still drives the backend
        // call shape (50 rows per request, `X-Total-Count` supplies `total`).
        // Audit #11 Issue #4: rows accumulate across fetches and the user
        // advances the list via "Load more" rather than Prev/Next.
        let page = use_state(|| 1u32);
        let page_size: u32 = 50;
        let total = use_state(|| Option::<u64>::None);
        let loading_more = use_state(|| false);

        // Search / filter state. The querystring is rebuilt from these
        // fields each time the user submits the search form.
        let q_text = use_state(String::new);
        let skills = use_state(String::new);
        let min_years = use_state(String::new);
        let location = use_state(String::new);
        let availability = use_state(String::new);
        let major = use_state(String::new);
        let min_education = use_state(String::new);
        // Audit #10 issue #3: user-selectable sort controls (whitelisted
        // column + direction). Defaults match the pre-audit implicit
        // order (last_active_at DESC) so recruiters see the freshest
        // profiles first when they land on the page.
        let sort_by = use_state(|| "last_active_at".to_string());
        let sort_dir = use_state(|| "desc".to_string());

        // Canonicalized querystring for the current form state (used by
        // tests + the reload callback).
        let build_query = {
            let q_text = q_text.clone();
            let skills = skills.clone();
            let min_years = min_years.clone();
            let location = location.clone();
            let availability = availability.clone();
            let major = major.clone();
            let min_education = min_education.clone();
            let sort_by = sort_by.clone();
            let sort_dir = sort_dir.clone();
            move |target_page: u32| -> String {
                let mut parts: Vec<String> = Vec::new();
                let push = |p: &mut Vec<String>, k: &str, v: &str| {
                    let v = v.trim();
                    if !v.is_empty() {
                        // naive but sufficient URL-encoding for this surface;
                        // backend Actix Query decoder tolerates spaces as `+`.
                        let enc: String = v
                            .chars()
                            .map(|c| match c {
                                ' ' => "+".into(),
                                '&' | '#' | '?' | '=' | '+' => {
                                    format!("%{:02X}", c as u8)
                                }
                                _ => c.to_string(),
                            })
                            .collect();
                        p.push(format!("{k}={enc}"));
                    }
                };
                push(&mut parts, "q", &*q_text);
                push(&mut parts, "skills", &*skills);
                push(&mut parts, "min_years", &*min_years);
                push(&mut parts, "location", &*location);
                push(&mut parts, "availability", &*availability);
                push(&mut parts, "major", &*major);
                push(&mut parts, "min_education", &*min_education);
                // Audit #10 issue #3: forward user-selected sort to
                // `GET /talent/candidates` so ORDER BY reflects the
                // recruiter's pick.
                push(&mut parts, "sort_by", &*sort_by);
                push(&mut parts, "sort_dir", &*sort_dir);
                parts.push(format!("page={target_page}"));
                parts.push(format!("page_size={}", 50));
                parts.join("&")
            }
        };

        let fetch = {
            let auth = auth.clone();
            let list = list.clone();
            let total = total.clone();
            let page = page.clone();
            let loading_more = loading_more.clone();
            let build_query = build_query.clone();
            Callback::from(move |(target_page, append): (u32, bool)| {
                let api = auth.api();
                let list = list.clone();
                let total = total.clone();
                let page = page.clone();
                let loading_more = loading_more.clone();
                let existing: Vec<CandidateListItem> = if append {
                    match &*list {
                        LoadState::Loaded(v) => v.clone(),
                        _ => Vec::new(),
                    }
                } else { Vec::new() };
                page.set(target_page);
                if append { loading_more.set(true); } else { list.set(LoadState::Loading); }
                let qs = build_query(target_page);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_candidates_query_paged(&qs).await {
                        Ok((v, tot)) => {
                            total.set(tot);
                            let mut combined = existing;
                            combined.extend(v);
                            list.set(LoadState::Loaded(combined));
                            loading_more.set(false);
                        }
                        Err(e) => {
                            loading_more.set(false);
                            if !append {
                                list.set(LoadState::Failed(e.user_facing()));
                            }
                        }
                    }
                });
            })
        };
        let reload = {
            let fetch = fetch.clone();
            Callback::from(move |_: ()| { fetch.emit((1, false)); })
        };
        {
            let fetch = fetch.clone();
            use_effect_with((), move |_| { fetch.emit((1, false)); || () });
        }

        let on_submit = {
            let reload = reload.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                // New search → reset accumulator to page 1.
                reload.emit(());
            })
        };
        let on_clear = {
            let q_text = q_text.clone();
            let skills = skills.clone();
            let min_years = min_years.clone();
            let location = location.clone();
            let availability = availability.clone();
            let major = major.clone();
            let min_education = min_education.clone();
            let sort_by = sort_by.clone();
            let sort_dir = sort_dir.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                q_text.set(String::new());
                skills.set(String::new());
                min_years.set(String::new());
                location.set(String::new());
                availability.set(String::new());
                major.set(String::new());
                min_education.set(String::new());
                // Audit #10 issue #3: reset sort to the safe default so
                // "Clear" really clears every selectable search control.
                sort_by.set("last_active_at".to_string());
                sort_dir.set("desc".to_string());
                reload.emit(());
            })
        };
        let on_more = {
            let fetch = fetch.clone();
            let page = page.clone();
            Callback::from(move |_: MouseEvent| { fetch.emit((*page + 1, true)); })
        };
        let bind = |s: UseStateHandle<String>| {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                s.set(t.value());
            })
        };

        let filters = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Search candidates" }</h2>
                <form class="tx-form tx-form--inline" onsubmit={on_submit}>
                    <label class="tx-field">
                        <span>{ "Search" }</span>
                        <input class="tx-input" type="text" value={(*q_text).clone()}
                            placeholder="name, email, bio"
                            oninput={bind(q_text.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Skills (CSV)" }</span>
                        <input class="tx-input" type="text" value={(*skills).clone()}
                            placeholder="rust,sql"
                            oninput={bind(skills.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Min years" }</span>
                        <input class="tx-input" type="number" min="0" value={(*min_years).clone()}
                            oninput={bind(min_years.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Location" }</span>
                        <input class="tx-input" type="text" value={(*location).clone()}
                            oninput={bind(location.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Availability" }</span>
                        <input class="tx-input" type="text" value={(*availability).clone()}
                            placeholder="immediate / 2_weeks / 1_month"
                            oninput={bind(availability.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Major" }</span>
                        <input class="tx-input" type="text" value={(*major).clone()}
                            oninput={bind(major.clone())}/>
                    </label>
                    <label class="tx-field">
                        <span>{ "Min education" }</span>
                        <select class="tx-input" onchange={{
                            let s = min_education.clone();
                            Callback::from(move |e: Event| {
                                let t: HtmlInputElement = e.target_unchecked_into();
                                s.set(t.value());
                            })
                        }}>
                            <option value="" selected={(*min_education).is_empty()}>{ "Any" }</option>
                            <option value="highschool" selected={&*min_education == "highschool"}>{ "High school" }</option>
                            <option value="associate"  selected={&*min_education == "associate"}>{ "Associate" }</option>
                            <option value="bachelor"   selected={&*min_education == "bachelor"}>{ "Bachelor" }</option>
                            <option value="master"     selected={&*min_education == "master"}>{ "Master" }</option>
                            <option value="phd"        selected={&*min_education == "phd"}>{ "PhD" }</option>
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Sort by" }</span>
                        <select class="tx-input" onchange={{
                            let s = sort_by.clone();
                            Callback::from(move |e: Event| {
                                let t: HtmlInputElement = e.target_unchecked_into();
                                s.set(t.value());
                            })
                        }}>
                            <option value="last_active_at"     selected={&*sort_by == "last_active_at"}>{ "Last active" }</option>
                            <option value="created_at"         selected={&*sort_by == "created_at"}>{ "Created" }</option>
                            <option value="updated_at"         selected={&*sort_by == "updated_at"}>{ "Updated" }</option>
                            <option value="full_name"          selected={&*sort_by == "full_name"}>{ "Name" }</option>
                            <option value="years_experience"   selected={&*sort_by == "years_experience"}>{ "Years of experience" }</option>
                            <option value="completeness_score" selected={&*sort_by == "completeness_score"}>{ "Profile completeness" }</option>
                        </select>
                    </label>
                    <label class="tx-field">
                        <span>{ "Direction" }</span>
                        <select class="tx-input" onchange={{
                            let s = sort_dir.clone();
                            Callback::from(move |e: Event| {
                                let t: HtmlInputElement = e.target_unchecked_into();
                                s.set(t.value());
                            })
                        }}>
                            <option value="desc" selected={&*sort_dir == "desc"}>{ "Descending" }</option>
                            <option value="asc"  selected={&*sort_dir == "asc"}>{ "Ascending" }</option>
                        </select>
                    </label>
                    <div class="tx-form__actions">
                        <button type="submit" class="tx-btn">{ "Search" }</button>
                        <button type="button" class="tx-btn tx-btn--ghost" onclick={on_clear}>
                            { "Clear" }
                        </button>
                    </div>
                </form>
            </section>
        };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Name"), AttrValue::from("Email"),
                    AttrValue::from("Years"), AttrValue::from("Skills"),
                    AttrValue::from("Completeness"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|c| {
                    let cid = c.id;
                    vec![
                        html! {
                            <Link<Route> to={Route::TalentCandidateDetail { id: cid }}
                                         classes={classes!("tx-link")}>{ c.full_name.clone() }</Link<Route>>
                        },
                        html! { <span class="tx-mono">{ c.email_mask.clone() }</span> },
                        html! { { c.years_experience } },
                        html! {
                            <span class="tx-chip-cloud">
                                { for c.skills.iter().take(6).map(|s| html!{
                                    <span class="tx-chip tx-chip--ghost">{ s.clone() }</span>
                                }) }
                            </span>
                        },
                        html! { { format!("{}%", c.completeness_score) } },
                    ]
                }).collect();
                let loaded = rows.len() as u32;
                let pager = html! {
                    <LoadMore loaded={loaded} total={*total} loading={*loading_more}
                              on_more={on_more.clone()} />
                };
                html! { <>
                    <DataTable headers={headers} rows={trows}
                               empty_label="No candidates match the filters."/>
                    { pager }
                </> }
            }
        };

        html! { <>{ filters }{ body }</> }
    }

    #[derive(Properties, PartialEq)]
    pub struct CandidateDetailProps {
        pub id: Uuid,
    }

    #[function_component(CandidateDetailPage)]
    pub fn candidate_detail_page(props: &CandidateDetailProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Candidate" subtitle="Full profile + thumb feedback (audited).">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <CandidateDetailBody {id} />
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(CandidateDetailBody)]
    fn candidate_detail_body(props: &CandidateDetailProps) -> Html {
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let detail = use_state(|| LoadState::<CandidateDetail>::Loading);
        let note = use_state(String::new);

        let reload = {
            let auth = auth.clone();
            let detail = detail.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let detail = detail.clone();
                detail.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_candidate(id).await {
                        Ok(v) => detail.set(LoadState::Loaded(v)),
                        Err(e) => detail.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let can_feedback = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("talent.feedback"))
            .unwrap_or(false);

        let on_note = {
            let note = note.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                note.set(t.value());
            })
        };

        let send_feedback = |thumb: &'static str| {
            let auth = auth.clone();
            let toast = toast.clone();
            let note = note.clone();
            Callback::from(move |_: MouseEvent| {
                let req = CreateFeedbackRequest {
                    candidate_id: id,
                    role_id: None,
                    thumb: thumb.to_string(),
                    note: if note.is_empty() { None } else { Some((*note).clone()) },
                };
                let api = auth.api();
                let toast = toast.clone();
                let note = note.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.post_talent_feedback(&req).await {
                        Ok(_) => { toast.success("Feedback recorded."); note.set(String::new()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        match &*detail {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(c) => html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ c.full_name.clone() }</h2>
                    <div class="tx-kv"><span>{ "Email" }</span>
                        <span class="tx-mono">{ c.email_mask.clone() }</span></div>
                    <div class="tx-kv"><span>{ "Location" }</span>
                        <span>{ c.location.clone().unwrap_or_else(|| "—".into()) }</span></div>
                    <div class="tx-kv"><span>{ "Experience" }</span>
                        <span>{ format!("{} years", c.years_experience) }</span></div>
                    <div class="tx-kv"><span>{ "Skills" }</span>
                        <span class="tx-chip-cloud">
                            { for c.skills.iter().map(|s| html!{
                                <span class="tx-chip">{ s.clone() }</span>
                            }) }
                        </span></div>
                    <div class="tx-kv"><span>{ "Completeness" }</span>
                        <span>{ format!("{}%", c.completeness_score) }</span></div>
                    <div class="tx-kv"><span>{ "Last active" }</span>
                        <span class="tx-mono">{ format_ts(c.last_active_at) }</span></div>
                    if can_feedback {
                        <div class="tx-form">
                            <label class="tx-subtle">{ "Note (optional)" }</label>
                            <input class="tx-input" value={(*note).clone()} oninput={on_note}/>
                            <div class="tx-toolbar">
                                <button class="tx-btn" onclick={send_feedback("up")}>{ "👍 Thumbs up" }</button>
                                <button class="tx-btn tx-btn--ghost" onclick={send_feedback("down")}>{ "👎 Thumbs down" }</button>
                            </div>
                        </div>
                    }
                </section>
            },
        }
    }

    #[function_component(Roles)]
    pub fn roles() -> Html {
        html! {
            <Layout title="Open roles" subtitle="Recruiter-managed job requisitions.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <RolesBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(RolesBody)]
    fn roles_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<RoleOpenItem>>::Loading);
        let title = use_state(String::new);
        let skills = use_state(String::new);
        let years = use_state(|| "0".to_string());
        // Audit #8 Issue #4: role creation now exposes the extended role
        // attributes so recruiters can capture the full requirement set.
        let create_major = use_state(String::new);
        let create_min_edu = use_state(String::new);
        let create_avail = use_state(String::new);
        // Audit #4 Issue #5: recruiter-side role search/filter state.
        // Audit #8 Issue #4 extends this with major / min_education /
        // availability filters and whitelisted sort column + direction.
        let search_q = use_state(String::new);
        let search_status = use_state(String::new);
        let search_min_years = use_state(String::new);
        let search_skills = use_state(String::new);
        let search_major = use_state(String::new);
        let search_min_edu = use_state(String::new);
        let search_avail = use_state(String::new);
        let sort_by = use_state(|| "created_at".to_string());
        let sort_dir = use_state(|| "desc".to_string());

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            let search_q = search_q.clone();
            let search_status = search_status.clone();
            let search_min_years = search_min_years.clone();
            let search_skills = search_skills.clone();
            let search_major = search_major.clone();
            let search_min_edu = search_min_edu.clone();
            let search_avail = search_avail.clone();
            let sort_by = sort_by.clone();
            let sort_dir = sort_dir.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                let q = (*search_q).clone();
                let status = (*search_status).clone();
                let min_years = search_min_years.parse::<i32>().ok();
                let skills_csv = (*search_skills).clone();
                let major = (*search_major).clone();
                let min_edu = (*search_min_edu).clone();
                let avail = (*search_avail).clone();
                let sort_by_s = (*sort_by).clone();
                let sort_dir_s = (*sort_dir).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    fn opt(s: &str) -> Option<&str> {
                        if s.trim().is_empty() { None } else { Some(s) }
                    }
                    match api
                        .search_talent_roles_ext(
                            opt(&q),
                            opt(&status),
                            min_years,
                            opt(&skills_csv),
                            opt(&major),
                            opt(&min_edu),
                            opt(&avail),
                            opt(&sort_by_s),
                            opt(&sort_dir_s),
                        )
                        .await
                    {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let can_manage = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("talent.manage"))
            .unwrap_or(false);

        let on_title = {
            let title = title.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                title.set(t.value());
            })
        };
        let on_skills = {
            let skills = skills.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                skills.set(t.value());
            })
        };
        let on_years = {
            let years = years.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                years.set(t.value());
            })
        };

        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let title = title.clone();
            let skills = skills.clone();
            let years = years.clone();
            let create_major = create_major.clone();
            let create_min_edu = create_min_edu.clone();
            let create_avail = create_avail.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let parsed: Vec<String> = skills
                    .split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let min_years: i32 = years.parse().unwrap_or(0);
                let trim_opt = |s: &str| {
                    let t = s.trim();
                    if t.is_empty() { None } else { Some(t.to_string()) }
                };
                let req = CreateRoleRequest {
                    title: (*title).clone(),
                    department_id: None,
                    required_skills: parsed,
                    min_years,
                    site_id: None,
                    required_major: trim_opt(&create_major),
                    min_education: trim_opt(&create_min_edu),
                    required_availability: trim_opt(&create_avail),
                    status: None,
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let title = title.clone();
                let skills = skills.clone();
                let create_major = create_major.clone();
                let create_min_edu = create_min_edu.clone();
                let create_avail = create_avail.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_talent_role(&req).await {
                        Ok(_) => {
                            toast.success("Role opened.");
                            title.set(String::new());
                            skills.set(String::new());
                            create_major.set(String::new());
                            create_min_edu.set(String::new());
                            create_avail.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = if can_manage {
            let on_major_in = {
                let s = create_major.clone();
                Callback::from(move |e: InputEvent| {
                    let t: HtmlInputElement = e.target_unchecked_into();
                    s.set(t.value());
                })
            };
            let on_avail_in = {
                let s = create_avail.clone();
                Callback::from(move |e: InputEvent| {
                    let t: HtmlInputElement = e.target_unchecked_into();
                    s.set(t.value());
                })
            };
            let on_min_edu_change = {
                let s = create_min_edu.clone();
                Callback::from(move |e: Event| {
                    let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                    s.set(t.value());
                })
            };
            html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Open a role" }</h2>
                    <form class="tx-form tx-form--row" onsubmit={on_create}>
                        <input class="tx-input" placeholder="Title" required=true
                               value={(*title).clone()} oninput={on_title}/>
                        <input class="tx-input" placeholder="Required skills (comma sep.)"
                               value={(*skills).clone()} oninput={on_skills}/>
                        <input class="tx-input" placeholder="Min years" required=true
                               value={(*years).clone()} oninput={on_years}/>
                        <input class="tx-input" placeholder="Required major"
                               value={(*create_major).clone()} oninput={on_major_in}/>
                        <select class="tx-input" onchange={on_min_edu_change}>
                            <option value="" selected={create_min_edu.is_empty()}>{ "Min education (any)" }</option>
                            <option value="highschool" selected={&*create_min_edu == "highschool"}>{ "High school" }</option>
                            <option value="associate"  selected={&*create_min_edu == "associate"}>{ "Associate" }</option>
                            <option value="bachelor"   selected={&*create_min_edu == "bachelor"}>{ "Bachelor" }</option>
                            <option value="master"     selected={&*create_min_edu == "master"}>{ "Master" }</option>
                            <option value="phd"        selected={&*create_min_edu == "phd"}>{ "PhD" }</option>
                        </select>
                        <input class="tx-input" placeholder="Required availability"
                               value={(*create_avail).clone()} oninput={on_avail_in}/>
                        <button class="tx-btn" type="submit">{ "Create" }</button>
                    </form>
                </section>
            }
        } else { html!() };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Title"), AttrValue::from("Min years"),
                    AttrValue::from("Required skills"), AttrValue::from("Status"),
                    AttrValue::from("Opened"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { { r.title.clone() } },
                    html! { { r.min_years } },
                    html! {
                        <span class="tx-chip-cloud">
                            { for r.required_skills.iter().map(|s| html!{
                                <span class="tx-chip tx-chip--ghost">{ s.clone() }</span>
                            }) }
                        </span>
                    },
                    html! { <span class="tx-chip">{ r.status.clone() }</span> },
                    html! { <span class="tx-mono">{ format_ts(r.opened_at) }</span> },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No open roles."/> }
            }
        };

        // Filter panel (audit #4 issue #5). Kept above the create-role
        // card so recruiters see the search surface first.
        let set_input = |st: UseStateHandle<String>| -> Callback<InputEvent> {
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                st.set(t.value());
            })
        };
        let on_apply = {
            let reload = reload.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                reload.emit(());
            })
        };
        let on_clear = {
            let sq = search_q.clone();
            let ss = search_status.clone();
            let smy = search_min_years.clone();
            let ssk = search_skills.clone();
            let sm = search_major.clone();
            let sme = search_min_edu.clone();
            let sa = search_avail.clone();
            let sb = sort_by.clone();
            let sd = sort_dir.clone();
            let reload = reload.clone();
            Callback::from(move |_: MouseEvent| {
                sq.set(String::new());
                ss.set(String::new());
                smy.set(String::new());
                ssk.set(String::new());
                sm.set(String::new());
                sme.set(String::new());
                sa.set(String::new());
                sb.set("created_at".to_string());
                sd.set("desc".to_string());
                reload.emit(());
            })
        };
        let filter_card = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Search roles" }</h2>
                <form class="tx-form tx-form--row" onsubmit={on_apply}>
                    <input class="tx-input" placeholder="Title contains…"
                           value={(*search_q).clone()} oninput={set_input(search_q.clone())}/>
                    <select class="tx-input" onchange={Callback::from({
                        let ss = search_status.clone();
                        move |e: Event| {
                            let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                            ss.set(t.value());
                        }
                    })}>
                        <option value="" selected={search_status.is_empty()}>{ "Any status" }</option>
                        <option value="open" selected={*search_status == "open"}>{ "open" }</option>
                        <option value="closed" selected={*search_status == "closed"}>{ "closed" }</option>
                        <option value="filled" selected={*search_status == "filled"}>{ "filled" }</option>
                    </select>
                    <input class="tx-input" placeholder="Min years"
                           value={(*search_min_years).clone()} oninput={set_input(search_min_years.clone())}/>
                    <input class="tx-input" placeholder="Skills (any, comma sep.)"
                           value={(*search_skills).clone()} oninput={set_input(search_skills.clone())}/>
                    <input class="tx-input" placeholder="Required major contains…"
                           value={(*search_major).clone()} oninput={set_input(search_major.clone())}/>
                    <select class="tx-input" onchange={Callback::from({
                        let sme = search_min_edu.clone();
                        move |e: Event| {
                            let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                            sme.set(t.value());
                        }
                    })}>
                        <option value="" selected={search_min_edu.is_empty()}>{ "Min education (any)" }</option>
                        <option value="highschool" selected={&*search_min_edu == "highschool"}>{ "High school" }</option>
                        <option value="associate"  selected={&*search_min_edu == "associate"}>{ "Associate" }</option>
                        <option value="bachelor"   selected={&*search_min_edu == "bachelor"}>{ "Bachelor" }</option>
                        <option value="master"     selected={&*search_min_edu == "master"}>{ "Master" }</option>
                        <option value="phd"        selected={&*search_min_edu == "phd"}>{ "PhD" }</option>
                    </select>
                    <input class="tx-input" placeholder="Required availability contains…"
                           value={(*search_avail).clone()} oninput={set_input(search_avail.clone())}/>
                    <select class="tx-input" onchange={Callback::from({
                        let sb = sort_by.clone();
                        move |e: Event| {
                            let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                            sb.set(t.value());
                        }
                    })}>
                        <option value="created_at" selected={&*sort_by == "created_at"}>{ "Sort: created" }</option>
                        <option value="opened_at"  selected={&*sort_by == "opened_at"}>{ "Sort: opened" }</option>
                        <option value="title"      selected={&*sort_by == "title"}>{ "Sort: title" }</option>
                        <option value="min_years"  selected={&*sort_by == "min_years"}>{ "Sort: min years" }</option>
                        <option value="status"     selected={&*sort_by == "status"}>{ "Sort: status" }</option>
                    </select>
                    <select class="tx-input" onchange={Callback::from({
                        let sd = sort_dir.clone();
                        move |e: Event| {
                            let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                            sd.set(t.value());
                        }
                    })}>
                        <option value="desc" selected={&*sort_dir == "desc"}>{ "desc" }</option>
                        <option value="asc"  selected={&*sort_dir == "asc"}>{ "asc" }</option>
                    </select>
                    <button class="tx-btn" type="submit">{ "Apply" }</button>
                    <button class="tx-btn tx-btn--ghost" type="button" onclick={on_clear}>{ "Clear" }</button>
                </form>
            </section>
        };

        html! { <>{ filter_card }{ create_card }{ body }</> }
    }

    #[function_component(Recommendations)]
    pub fn recommendations() -> Html {
        html! {
            <Layout title="Recommendations" subtitle="Cold-start by completeness → blended scoring after 10+ feedback.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <RecommendationsBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(RecommendationsBody)]
    fn recommendations_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let role_input = use_state(String::new);
        let result = use_state(|| LoadState::<Vec<RankedCandidate>>::Loading);
        let cold = use_state(|| None::<bool>);
        // Audit #6 Issue #2: replace the raw role_id UUID text box with a
        // selector backed by the live open-roles list.
        let roles = use_state(|| Vec::<RoleOpenItem>::new());
        {
            let auth = auth.clone();
            let roles = roles.clone();
            let role_input = role_input.clone();
            use_effect_with((), move |_| {
                let api = auth.api();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_talent_roles().await {
                        if role_input.is_empty() {
                            if let Some(first) = v.first() { role_input.set(first.id.to_string()); }
                        }
                        roles.set(v);
                    }
                });
                || ()
            });
        }

        let on_role = {
            let role_input = role_input.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                role_input.set(t.value());
            })
        };

        let run = {
            let auth = auth.clone();
            let role_input = role_input.clone();
            let result = result.clone();
            let cold = cold.clone();
            Callback::from(move |_: MouseEvent| {
                let Ok(rid) = Uuid::parse_str(role_input.trim()) else {
                    result.set(LoadState::Failed("Enter a valid role_id UUID.".into()));
                    return;
                };
                result.set(LoadState::Loading);
                let api = auth.api();
                let result = result.clone();
                let cold = cold.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_recommendations(rid).await {
                        Ok(v) => {
                            cold.set(Some(v.cold_start));
                            result.set(LoadState::Loaded(v.candidates));
                        }
                        Err(e) => result.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };

        let body = match &*result {
            LoadState::Loading => html! { <PlaceholderEmpty label="Pick a role above and click Rank."/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())} on_retry={None::<Callback<MouseEvent>>}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Candidate"), AttrValue::from("Score"),
                    AttrValue::from("Skills"), AttrValue::from("Why"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|rc| vec![
                    html! { { rc.candidate.full_name.clone() } },
                    html! { { format!("{:.3}", rc.score) } },
                    html! {
                        <span class="tx-chip-cloud">
                            { for rc.candidate.skills.iter().take(5).map(|s| html!{
                                <span class="tx-chip tx-chip--ghost">{ s.clone() }</span>
                            }) }
                        </span>
                    },
                    html! {
                        <ul class="tx-list">
                            { for rc.reasons.iter().map(|r| html!{ <li>{ r.clone() }</li> }) }
                        </ul>
                    },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No ranked candidates."/> }
            }
        };

        html! {
            <>
                <section class="tx-card">
                    <form class="tx-form tx-form--row" onsubmit={Callback::from(|e: SubmitEvent| e.prevent_default())}>
                        <select class="tx-input" onchange={on_role}>
                            if roles.is_empty() {
                                <option value="" selected=true>{ "No open roles yet" }</option>
                            } else {
                                { for roles.iter().map(|r| {
                                    let id_s = r.id.to_string();
                                    let sel = *role_input == id_s;
                                    html! { <option value={id_s.clone()} selected={sel}>
                                        { format!("{} · {}", r.title, r.status) }
                                    </option> }
                                }) }
                            }
                        </select>
                        <button class="tx-btn" onclick={run}>{ "Rank" }</button>
                    </form>
                    if let Some(cs) = *cold {
                        <p class="tx-subtle">
                            { if cs { "Cold-start ranking (fewer than 10 feedback signals)." }
                              else { "Blended ranking (10+ feedback signals in pool)." } }
                        </p>
                    }
                </section>
                { body }
            </>
        }
    }

    #[function_component(Weights)]
    pub fn weights() -> Html {
        html! {
            <Layout title="Ranking weights" subtitle="Personal tuning — applies to your ranked lists only.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <WeightsBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(WeightsBody)]
    fn weights_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let w = use_state(|| LoadState::<UpdateWeightsRequest>::Loading);

        let reload = {
            let auth = auth.clone();
            let w = w.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let w = w.clone();
                w.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_talent_weights().await {
                        Ok(v) => w.set(LoadState::Loaded(UpdateWeightsRequest {
                            skills_weight: v.skills_weight,
                            experience_weight: v.experience_weight,
                            recency_weight: v.recency_weight,
                            completeness_weight: v.completeness_weight,
                        })),
                        Err(e) => w.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let save = {
            let auth = auth.clone();
            let toast = toast.clone();
            let w = w.clone();
            Callback::from(move |_: MouseEvent| {
                let LoadState::Loaded(req) = (*w).clone() else { return };
                let api = auth.api();
                let toast = toast.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.put_talent_weights(&req).await {
                        Ok(_) => toast.success("Weights saved."),
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let set_field = |setter: fn(&mut UpdateWeightsRequest, i32)| {
            let w = w.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                let v: i32 = t.value().parse().unwrap_or(0);
                if let LoadState::Loaded(mut cur) = (*w).clone() {
                    setter(&mut cur, v);
                    w.set(LoadState::Loaded(cur));
                }
            })
        };

        match &*w {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(cur) => html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">{ "Ranking weights (0–100)" }</h2>
                    <div class="tx-form">
                        <label class="tx-subtle">{ "Skills" }</label>
                        <input class="tx-input" type="number" min="0" max="100"
                               value={cur.skills_weight.to_string()}
                               oninput={set_field(|c, v| c.skills_weight = v)}/>
                        <label class="tx-subtle">{ "Experience" }</label>
                        <input class="tx-input" type="number" min="0" max="100"
                               value={cur.experience_weight.to_string()}
                               oninput={set_field(|c, v| c.experience_weight = v)}/>
                        <label class="tx-subtle">{ "Recency" }</label>
                        <input class="tx-input" type="number" min="0" max="100"
                               value={cur.recency_weight.to_string()}
                               oninput={set_field(|c, v| c.recency_weight = v)}/>
                        <label class="tx-subtle">{ "Completeness" }</label>
                        <input class="tx-input" type="number" min="0" max="100"
                               value={cur.completeness_weight.to_string()}
                               oninput={set_field(|c, v| c.completeness_weight = v)}/>
                        <button class="tx-btn" onclick={save}>{ "Save" }</button>
                    </div>
                </section>
            },
        }
    }

    #[function_component(Watchlists)]
    pub fn watchlists() -> Html {
        html! {
            <Layout title="Watchlists" subtitle="Per-recruiter pinned candidate sets.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <WatchlistsBody/>
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(WatchlistsBody)]
    fn watchlists_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<WatchlistItem>>::Loading);
        let name = use_state(String::new);

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_watchlists().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let on_name = {
            let name = name.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                name.set(t.value());
            })
        };
        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let name = name.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let req = CreateWatchlistRequest { name: (*name).clone() };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let name = name.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_watchlist(&req).await {
                        Ok(_) => { toast.success("Watchlist created."); name.set(String::new()); reload.emit(()); }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "New watchlist" }</h2>
                <form class="tx-form tx-form--row" onsubmit={on_create}>
                    <input class="tx-input" placeholder="Name" required=true
                           value={(*name).clone()} oninput={on_name}/>
                    <button class="tx-btn" type="submit">{ "Create" }</button>
                </form>
            </section>
        };

        let body = match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Name"), AttrValue::from("Items"),
                    AttrValue::from("Created"), AttrValue::from("Updated"),
                    AttrValue::from(""),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|w| {
                    let wid = w.id;
                    vec![
                        html! { { w.name.clone() } },
                        html! { { w.item_count } },
                        html! { <span class="tx-mono">{ format_ts(w.created_at) }</span> },
                        html! { <span class="tx-mono">{ format_ts(w.updated_at) }</span> },
                        html! {
                            <Link<Route> to={Route::TalentWatchlistDetail { id: wid }}
                                         classes={classes!("tx-link")}>
                                { "Open →" }
                            </Link<Route>>
                        },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No watchlists yet."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
    }

    // ------------------------------------------------------------------
    // Audit #5 Issue #5: recruiter watchlist maintenance.
    // Detail page lets the recruiter see members of one watchlist, add
    // candidates by UUID, and remove them. This is the real
    // prompt-required maintenance surface.
    // ------------------------------------------------------------------

    #[derive(Properties, PartialEq)]
    pub struct WatchlistDetailProps {
        pub id: Uuid,
    }

    #[function_component(WatchlistDetail)]
    pub fn watchlist_detail(props: &WatchlistDetailProps) -> Html {
        let id = props.id;
        html! {
            <Layout title="Watchlist" subtitle="Pinned candidates for this watchlist.">
                <PermAnyGate permissions={vec![
                    AttrValue::from("talent.read"),
                    AttrValue::from("talent.manage"),
                ]}>
                    <WatchlistDetailBody {id} />
                </PermAnyGate>
            </Layout>
        }
    }

    #[function_component(WatchlistDetailBody)]
    fn watchlist_detail_body(props: &WatchlistDetailProps) -> Html {
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let items = use_state(|| LoadState::<Vec<WatchlistEntry>>::Loading);
        let new_cid = use_state(String::new);
        // Audit #6 Issue #2: candidate selector with a simple search box
        // that narrows the dropdown by name or skills.
        let candidates = use_state(|| Vec::<CandidateListItem>::new());
        let cand_filter = use_state(String::new);
        {
            let auth = auth.clone();
            let candidates = candidates.clone();
            let new_cid = new_cid.clone();
            use_effect_with((), move |_| {
                let api = auth.api();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(v) = api.list_candidates_query("page_size=200").await {
                        if new_cid.is_empty() {
                            if let Some(c) = v.first() { new_cid.set(c.id.to_string()); }
                        }
                        candidates.set(v);
                    }
                });
                || ()
            });
        }

        let reload = {
            let auth = auth.clone();
            let items = items.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let items = items.clone();
                items.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_watchlist_items(id).await {
                        Ok(v) => items.set(LoadState::Loaded(v)),
                        Err(e) => items.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with(id, move |_| { r.emit(()); || () }); }

        let on_new_cid = {
            let new_cid = new_cid.clone();
            Callback::from(move |e: Event| {
                let t: web_sys::HtmlSelectElement = e.target_unchecked_into();
                new_cid.set(t.value());
            })
        };
        let on_cand_filter = {
            let cand_filter = cand_filter.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                cand_filter.set(t.value());
            })
        };

        let on_add = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let new_cid = new_cid.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let Ok(cid) = Uuid::parse_str(new_cid.trim()) else {
                    toast.error("candidate id must be a UUID");
                    return;
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let new_cid = new_cid.clone();
                let req = AddWatchlistItemRequest { candidate_id: cid };
                wasm_bindgen_futures::spawn_local(async move {
                    match api.add_watchlist_item(id, &req).await {
                        Ok(_) => {
                            toast.success("Candidate added.");
                            new_cid.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let on_remove = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            Callback::from(move |cid: Uuid| {
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.remove_watchlist_item(id, cid).await {
                        Ok(_) => {
                            toast.success("Candidate removed.");
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let filter_lc = cand_filter.to_lowercase();
        let filtered: Vec<&CandidateListItem> = candidates.iter().filter(|c| {
            if filter_lc.is_empty() { return true; }
            c.full_name.to_lowercase().contains(&filter_lc)
                || c.email_mask.to_lowercase().contains(&filter_lc)
                || c.skills.iter().any(|s| s.to_lowercase().contains(&filter_lc))
        }).collect();
        let add_card = html! {
            <section class="tx-card">
                <h2 class="tx-title tx-title--sm">{ "Add candidate to this watchlist" }</h2>
                <form class="tx-form tx-form--row" onsubmit={on_add}>
                    <input class="tx-input" placeholder="Filter candidates (name / skill)"
                           value={(*cand_filter).clone()} oninput={on_cand_filter}/>
                    <select class="tx-input" required=true onchange={on_new_cid}>
                        if filtered.is_empty() {
                            <option value="" selected=true>{ "No matching candidates" }</option>
                        } else {
                            { for filtered.iter().take(200).map(|c| {
                                let id_s = c.id.to_string();
                                let sel = *new_cid == id_s;
                                html! { <option value={id_s.clone()} selected={sel}>
                                    { format!("{} · {}y · {}", c.full_name, c.years_experience,
                                        c.skills.iter().take(3).cloned().collect::<Vec<_>>().join(", ")) }
                                </option> }
                            }) }
                        }
                    </select>
                    <button class="tx-btn" type="submit">{ "Add" }</button>
                </form>
                <p class="tx-subtle">
                    { "Copy a candidate UUID from the Candidates page. \
                       Duplicate additions are silently ignored." }
                </p>
            </section>
        };

        let body = match &*items {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Candidate"),
                    AttrValue::from("Email"),
                    AttrValue::from("Years"),
                    AttrValue::from("Skills"),
                    AttrValue::from("Added"),
                    AttrValue::from(""),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|w| {
                    let cid = w.candidate.id;
                    let on_remove = on_remove.clone();
                    let onclk = Callback::from(move |_: MouseEvent| on_remove.emit(cid));
                    vec![
                        html! {
                            <Link<Route> to={Route::TalentCandidateDetail { id: cid }}
                                         classes={classes!("tx-link")}>
                                { w.candidate.full_name.clone() }
                            </Link<Route>>
                        },
                        html! { <span class="tx-mono">{ w.candidate.email_mask.clone() }</span> },
                        html! { { w.candidate.years_experience } },
                        html! { <span class="tx-mono tx-truncate">{ w.candidate.skills.join(", ") }</span> },
                        html! { <span class="tx-mono">{ format_ts(w.added_at) }</span> },
                        html! {
                            <button class="tx-btn tx-btn--ghost" onclick={onclk}>
                                { "Remove" }
                            </button>
                        },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows}
                                   empty_label="No candidates on this watchlist yet."/> }
            }
        };

        html! { <>{ add_card }{ body }</> }
    }
}

// ===========================================================================
// NotFound
// ===========================================================================

#[function_component(NotFound)]
pub fn not_found() -> Html {
    html! {
        <Layout title="Not found">
            <section class="tx-card">
                <p class="tx-subtle">{ "The page you requested does not exist." }</p>
                <Link<Route> to={Route::Dashboard} classes={classes!("tx-btn")}>{ "Go to dashboard" }</Link<Route>>
            </section>
        </Layout>
    }
}

// ---------------------------------------------------------------------------
// Shared load-state enum (used by every page above).
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
pub enum LoadState<T: Clone + PartialEq> {
    Loading,
    Loaded(T),
    Failed(String),
}

// Silence unused-import warnings when a sub-module doesn't reference a type.
#[allow(dead_code)]
fn _unused_uuid(_: Uuid) {}
#[allow(dead_code)]
fn _unused_rc<T>(_: Rc<T>) {}
