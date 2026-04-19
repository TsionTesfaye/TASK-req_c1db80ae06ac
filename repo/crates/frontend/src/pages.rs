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

        let sku = use_state(String::new);
        let name = use_state(String::new);

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_products().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

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

        let can_create = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("product.manage"))
            .unwrap_or(false);

        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let sku = sku.clone();
            let name = name.clone();
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let req = CreateProductRequest {
                    sku: (*sku).clone(),
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
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_product(&req).await {
                        Ok(p) => {
                            toast.success(&format!("Created product {}", p.sku));
                            sku.set(String::new());
                            name.set(String::new());
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
                    AttrValue::from("SKU"), AttrValue::from("Name"),
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
                        html! { { p.name.clone() } },
                        html! { { p.category_name.clone().unwrap_or_else(|| "—".into()) } },
                        html! { { p.brand_name.clone().unwrap_or_else(|| "—".into()) } },
                        html! {
                            { format!("{} {:.2}",
                                p.currency,
                                (p.price_cents as f64) / 100.0) }
                        },
                        html! { if p.on_shelf { {"✔"} } else { {"—"} } },
                        html! { <span class="tx-mono">{ p.updated_at.to_rfc3339() }</span> },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No products."/> }
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
        let id = props.id;
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let detail = use_state(|| LoadState::<ProductDetail>::Loading);

        let reload = {
            let auth = auth.clone();
            let detail = detail.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let detail = detail.clone();
                detail.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.get_product(id).await {
                        Ok(v) => detail.set(LoadState::Loaded(v)),
                        Err(e) => detail.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        let can_manage = auth
            .state
            .as_ref()
            .map(|s| s.has_permission("product.manage"))
            .unwrap_or(false);

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
            LoadState::Loaded(p) => html! {
                <section class="tx-card">
                    <h2 class="tx-title tx-title--sm">
                        <code>{ p.sku.clone() }</code>{ " — " }{ p.name.clone() }
                    </h2>
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
                    <div class="tx-kv"><span>{ "Tax rates" }</span>
                        <span>{ format!("{} rate(s)", p.tax_rates.len()) }</span></div>
                    <div class="tx-kv"><span>{ "Images" }</span>
                        <span>{ format!("{} image(s)", p.images.len()) }</span></div>
                    <div class="tx-kv"><span>{ "Updated" }</span>
                        <span class="tx-mono">{ p.updated_at.to_rfc3339() }</span></div>
                    if can_manage {
                        <button class="tx-btn tx-btn--ghost" onclick={toggle_shelf}>
                            { if p.on_shelf { "Take off shelf" } else { "Put on shelf" } }
                        </button>
                    }
                </section>
            },
        }
    }

    #[function_component(ImportsList)]
    pub fn imports_list() -> Html {
        html! {
            <Layout title="Import batches" subtitle="Upload → validate → commit CSV product batches.">
                <PermGate permission="product.manage">
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
                        html! { <span class="tx-mono">{ b.created_at.to_rfc3339() }</span> },
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
                <PermGate permission="product.manage">
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
    use terraops_shared::dto::metric::{MetricDefinitionDto, MetricSeriesResponse};
    use terraops_shared::dto::report::ReportJobDto;

    #[function_component(Sources)]
    pub fn sources() -> Html {
        html! {
            <Layout title="Environmental sources" subtitle="Sensors, meters, and manual kiosks.">
                <PermGate permission="env.read">
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
            .map(|s| s.has_permission("env.manage"))
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
                    html! { <span class="tx-mono">{ s.updated_at.to_rfc3339() }</span> },
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
                <PermGate permission="env.read">
                    <ObservationsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ObservationsBody)]
    fn observations_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<ObservationDto>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_observations("").await {
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
                    AttrValue::from("Observed"), AttrValue::from("Source"),
                    AttrValue::from("Value"), AttrValue::from("Unit"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|o| vec![
                    html! { <span class="tx-mono">{ o.observed_at.to_rfc3339() }</span> },
                    html! { <span class="tx-mono tx-truncate">{ o.source_id.to_string() }</span> },
                    html! { { format!("{:.3}", o.value) } },
                    html! { { o.unit.clone() } },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No observations yet."/> }
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

        match &*list {
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
                let headers = vec![AttrValue::from("At"), AttrValue::from("Value")];
                let trows: Vec<Vec<Html>> = s.points.iter().map(|p| vec![
                    html! { <span class="tx-mono">{ p.at.to_rfc3339() }</span> },
                    html! { { format!("{:.3}", p.value) } },
                ]).collect();
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
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let summary = use_state(|| LoadState::<terraops_shared::dto::kpi::KpiSummary>::Loading);
        let reload = {
            let auth = auth.clone();
            let summary = summary.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let summary = summary.clone();
                summary.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.kpi_summary().await {
                        Ok(v) => summary.set(LoadState::Loaded(v)),
                        Err(e) => summary.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

        match &*summary {
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
                            { format!("Generated {}", s.generated_at.to_rfc3339()) }
                        </p>
                    </article>
                </section>
            },
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
        let metric_id = use_state(String::new);
        let threshold = use_state(String::new);
        let op = use_state(|| ">".to_string());

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

        let on_mid = {
            let metric_id = metric_id.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
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
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
                op.set(t.value());
            })
        };

        let on_create = {
            let auth = auth.clone();
            let toast = toast.clone();
            let reload = reload.clone();
            let metric_id = metric_id.clone();
            let threshold = threshold.clone();
            let op = op.clone();
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
                let req = CreateAlertRuleRequest {
                    metric_definition_id: mid,
                    threshold: th,
                    operator: (*op).clone(),
                    duration_seconds: None,
                    severity: None,
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
                    <input class="tx-input" placeholder="metric_definition_id (UUID)"
                           required=true value={(*metric_id).clone()} oninput={on_mid}/>
                    <input class="tx-input" placeholder="threshold" required=true
                           value={(*threshold).clone()} oninput={on_th}/>
                    <input class="tx-input" placeholder="&gt; &lt; &gt;= &lt;="
                           required=true value={(*op).clone()} oninput={on_op}/>
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
                    AttrValue::from("Metric"), AttrValue::from("Op"),
                    AttrValue::from("Threshold"), AttrValue::from("Severity"),
                    AttrValue::from("Enabled"),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|r| vec![
                    html! { <span class="tx-mono tx-truncate">{ r.metric_definition_id.to_string() }</span> },
                    html! { <code>{ r.operator.clone() }</code> },
                    html! { { format!("{:.3}", r.threshold) } },
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
                <PermGate permission="report.manage">
                    <ReportsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(ReportsBody)]
    fn reports_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let toast = use_context::<ToastContext>().expect("ToastContext");
        let list = use_state(|| LoadState::<Vec<ReportJobDto>>::Loading);

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

        match &*list {
            LoadState::Loading => html! { <PlaceholderLoading/> },
            LoadState::Failed(m) => html! {
                <PlaceholderError message={AttrValue::from(m.clone())}
                    on_retry={Some({ let r = reload.clone(); Callback::from(move |_| r.emit(())) })}/>
            },
            LoadState::Loaded(rows) => {
                let headers = vec![
                    AttrValue::from("Kind"), AttrValue::from("Format"),
                    AttrValue::from("Status"), AttrValue::from("Last run"),
                    AttrValue::from(""),
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|j| {
                    let jid = j.id;
                    let run_now = run_now.clone();
                    let onclick = Callback::from(move |_: MouseEvent| run_now.emit(jid));
                    vec![
                        html! { { j.kind.clone() } },
                        html! { <code>{ j.format.clone() }</code> },
                        html! { <span class="tx-chip">{ j.status.clone() }</span> },
                        html! { <span class="tx-mono">{
                            j.last_run_at.map(|t| t.to_rfc3339()).unwrap_or_else(|| "—".into())
                        }</span> },
                        html! { <button class="tx-btn tx-btn--ghost" onclick={onclick}>{ "Run now" }</button> },
                    ]
                }).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No report jobs."/> }
            }
        }
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
                <PermGate permission="alert.read">
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

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_alert_events().await {
                        Ok(v) => list.set(LoadState::Loaded(v)),
                        Err(e) => list.set(LoadState::Failed(e.user_facing())),
                    }
                });
            })
        };
        { let r = reload.clone(); use_effect_with((), move |_| { r.emit(()); || () }); }

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
                        html! { <span class="tx-mono">{ ev.fired_at.to_rfc3339() }</span> },
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
                html! { <DataTable headers={headers} rows={trows} empty_label="No alert events yet."/> }
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
        CandidateDetail, CandidateListItem, CreateFeedbackRequest,
        CreateRoleRequest, CreateWatchlistRequest, RankedCandidate, RoleOpenItem,
        UpdateWeightsRequest, WatchlistItem,
    };

    #[function_component(Candidates)]
    pub fn candidates() -> Html {
        html! {
            <Layout title="Candidates" subtitle="Talent pool — search, view, add to watchlist.">
                <PermGate permission="talent.read">
                    <CandidatesBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(CandidatesBody)]
    fn candidates_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let list = use_state(|| LoadState::<Vec<CandidateListItem>>::Loading);
        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_candidates().await {
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
                html! { <DataTable headers={headers} rows={trows} empty_label="No candidates."/> }
            }
        }
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
                <PermGate permission="talent.read">
                    <CandidateDetailBody {id} />
                </PermGate>
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
                        <span class="tx-mono">{ c.last_active_at.to_rfc3339() }</span></div>
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
                <PermGate permission="talent.read">
                    <RolesBody/>
                </PermGate>
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

        let reload = {
            let auth = auth.clone();
            let list = list.clone();
            Callback::from(move |_: ()| {
                let api = auth.api();
                let list = list.clone();
                list.set(LoadState::Loading);
                wasm_bindgen_futures::spawn_local(async move {
                    match api.list_talent_roles().await {
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
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let parsed: Vec<String> = skills
                    .split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                let min_years: i32 = years.parse().unwrap_or(0);
                let req = CreateRoleRequest {
                    title: (*title).clone(),
                    department_id: None,
                    required_skills: parsed,
                    min_years,
                    site_id: None,
                    status: None,
                };
                let api = auth.api();
                let toast = toast.clone();
                let reload = reload.clone();
                let title = title.clone();
                let skills = skills.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match api.create_talent_role(&req).await {
                        Ok(_) => {
                            toast.success("Role opened.");
                            title.set(String::new());
                            skills.set(String::new());
                            reload.emit(());
                        }
                        Err(e) => toast.error(&e.user_facing()),
                    }
                });
            })
        };

        let create_card = if can_manage {
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
                    html! { <span class="tx-mono">{ r.opened_at.to_rfc3339() }</span> },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No open roles."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
    }

    #[function_component(Recommendations)]
    pub fn recommendations() -> Html {
        html! {
            <Layout title="Recommendations" subtitle="Cold-start by completeness → blended scoring after 10+ feedback.">
                <PermGate permission="talent.read">
                    <RecommendationsBody/>
                </PermGate>
            </Layout>
        }
    }

    #[function_component(RecommendationsBody)]
    fn recommendations_body() -> Html {
        let auth = use_context::<AuthContext>().expect("AuthContext");
        let role_input = use_state(String::new);
        let result = use_state(|| LoadState::<Vec<RankedCandidate>>::Loading);
        let cold = use_state(|| None::<bool>);

        let on_role = {
            let role_input = role_input.clone();
            Callback::from(move |e: InputEvent| {
                let t: HtmlInputElement = e.target_unchecked_into();
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
            LoadState::Loading => html! { <PlaceholderEmpty label="Enter a role_id above and click Rank."/> },
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
                        <input class="tx-input" placeholder="role_id (UUID)" value={(*role_input).clone()} oninput={on_role}/>
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
                <PermGate permission="talent.read">
                    <WeightsBody/>
                </PermGate>
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
                <PermGate permission="talent.read">
                    <WatchlistsBody/>
                </PermGate>
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
                ];
                let trows: Vec<Vec<Html>> = rows.iter().map(|w| vec![
                    html! { { w.name.clone() } },
                    html! { { w.item_count } },
                    html! { <span class="tx-mono">{ w.created_at.to_rfc3339() }</span> },
                    html! { <span class="tx-mono">{ w.updated_at.to_rfc3339() }</span> },
                ]).collect();
                html! { <DataTable headers={headers} rows={trows} empty_label="No watchlists yet."/> }
            }
        };

        html! { <>{ create_card }{ body }</> }
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
