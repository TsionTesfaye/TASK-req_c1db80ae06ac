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
    DataTable, Layout, PermGate, PlaceholderEmpty, PlaceholderError, PlaceholderLoading,
};
use crate::router::Route;
use crate::state::{AuthContext, AuthState, ToastContext};

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
                        <label for="email" class="tx-subtle">{ "Email" }</label>
                        <input id="email" class="tx-input" type="email" autocomplete="username"
                               required=true value={(*email).clone()} oninput={on_email}
                               placeholder="admin@terraops.local" />
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
// dashboard::Home (minimal P1 placeholder, owned by main; P-B takes over later)
// ===========================================================================

pub mod dashboard {
    use super::*;

    #[function_component(Home)]
    pub fn home() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let Some(state) = auth.state.as_ref().cloned() else {
            return html! { <Redirect<Route> to={Route::Login} /> };
        };
        html! {
            <Layout title="Dashboard" subtitle="Signed in to the offline ops portal.">
                <section class="tx-grid">
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
                        <h2 class="tx-title tx-title--sm">{ "Permissions" }</h2>
                        <div class="tx-chip-cloud">
                            { for state.user.permissions.iter().map(|p| html!{
                                <span class="tx-chip tx-chip--ghost">{ p.clone() }</span>
                            }) }
                        </div>
                    </article>
                    <article class="tx-card tx-card--hint">
                        <h2 class="tx-title tx-title--sm">{ "What's here" }</h2>
                        <p class="tx-subtle">
                            { "Use the left nav to reach admin, monitoring, and notifications surfaces. KPI and environmental dashboards land in P-B." }
                        </p>
                        if state.has_role(Role::Administrator) {
                            <Link<Route> to={Route::AdminUsers} classes={classes!("tx-btn", "tx-btn--ghost")}>
                                { "Manage users" }
                            </Link<Route>>
                        }
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
            let password = password.clone();
            let selected_role = selected_role.clone();
            let submitting = submitting.clone();
            let auth = auth.clone();
            let toast = toast.clone();
            let on_created = props.on_created.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                submitting.set(true);
                let body = CreateUserRequest {
                    display_name: (*name).clone(),
                    email: (*email).clone(),
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
                let password = password.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_user(&body).await {
                        Ok(_) => {
                            name.set(String::new());
                            email.set(String::new());
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
            .map(|d| d.to_rfc3339())
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
                                <div class="tx-subtle">{ format!("Last updated: {}", m.updated_at.to_rfc3339()) }</div>
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
                    html!{ <span class="tx-mono">{ e.at.to_rfc3339() }</span> },
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
                    <div class="tx-subtle tx-mono">{ n.created_at.to_rfc3339() }</div>
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
                    html!{ <span class="tx-mono">{ c.reported_at.to_rfc3339() }</span> },
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
