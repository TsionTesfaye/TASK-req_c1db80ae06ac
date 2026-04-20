//! Shared components: Layout, Nav, RoleGate, Toast, Placeholders, Table helpers.

use yew::prelude::*;
use yew_router::prelude::*;

use crate::router::Route;
use crate::state::{AuthContext, NotificationsContext, Toast, ToastContext};

// ---------------------------------------------------------------------------
// Layout + Nav shell
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct LayoutProps {
    pub children: Children,
    #[prop_or_default]
    pub title: AttrValue,
    #[prop_or_default]
    pub subtitle: AttrValue,
}

#[function_component(Layout)]
pub fn layout(props: &LayoutProps) -> Html {
    html! {
        <div class="tx-app">
            <Nav/>
            <main class="tx-main" role="main">
                if !props.title.is_empty() {
                    <header class="tx-page-head">
                        <h1 class="tx-page-title">{ props.title.clone() }</h1>
                        if !props.subtitle.is_empty() {
                            <p class="tx-subtle">{ props.subtitle.clone() }</p>
                        }
                    </header>
                }
                { for props.children.iter() }
            </main>
            <ToastRack/>
        </div>
    }
}

#[function_component(Nav)]
pub fn nav() -> Html {
    let auth = use_context::<AuthContext>().expect("AuthContext");
    let notifs = use_context::<NotificationsContext>().expect("NotificationsContext");
    let navigator = use_navigator().unwrap();

    let Some(state) = auth.state.clone() else {
        return html!();
    };

    let logout = {
        let auth = auth.clone();
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            let api = auth.api();
            let auth = auth.clone();
            let navigator = navigator.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = api.logout().await;
                auth.set.emit(None);
                navigator.push(&Route::Login);
            });
        })
    };

    let has = |code: &str| state.has_permission(code);

    let unread = notifs.snapshot.unread;

    html! {
        <aside class="tx-nav" aria-label="Primary">
            <div class="tx-nav-brand">
                <div class="tx-nav-logo">{ "TO" }</div>
                <div class="tx-nav-brand-text">
                    <div class="tx-nav-brand-title">{ "TerraOps" }</div>
                    <div class="tx-subtle tx-nav-brand-sub">{ "Offline Ops Portal" }</div>
                </div>
            </div>
            <nav class="tx-nav-links">
                <NavItem to={Route::Dashboard} label="Dashboard" />
                <NavItem to={Route::Notifications} label="Notifications" badge={unread} />
                <NavItem to={Route::ChangePassword} label="Change Password" />

                if has("product.read") || has("product.write") {
                    <div class="tx-nav-section">{ "Catalog" }</div>
                    <NavItem to={Route::Products} label="Products" />
                    if has("product.import") {
                        <NavItem to={Route::Imports} label="Import batches" />
                    }
                }

                // Environmental section — gated by the real backend
                // permission vocabulary (`metric.read` covers /env/sources +
                // /env/observations + /metrics/*, `alert.ack` covers the
                // events feed, `report.schedule`/`report.run` cover reports).
                if has("metric.read") || has("kpi.read")
                   || has("alert.ack") || has("alert.manage")
                   || has("report.schedule") || has("report.run")
                {
                    <div class="tx-nav-section">{ "Environmental" }</div>
                    if has("kpi.read") {
                        <NavItem to={Route::Kpi} label="KPIs" />
                    }
                    if has("metric.read") {
                        <NavItem to={Route::EnvSources} label="Sources" />
                        <NavItem to={Route::EnvObservations} label="Observations" />
                        <NavItem to={Route::MetricDefinitions} label="Metric definitions" />
                    }
                    if has("alert.manage") {
                        <NavItem to={Route::AlertRules} label="Alert rules" />
                    }
                    if has("alert.ack") || has("kpi.read") {
                        <NavItem to={Route::AlertEvents} label="Alerts feed" />
                    }
                    if has("report.schedule") || has("report.run") {
                        <NavItem to={Route::Reports} label="Report jobs" />
                    }
                }

                if has("talent.read") {
                    <div class="tx-nav-section">{ "Talent" }</div>
                    <NavItem to={Route::TalentCandidates} label="Candidates" />
                    <NavItem to={Route::TalentRoles} label="Open roles" />
                    <NavItem to={Route::TalentRecommendations} label="Recommendations" />
                    <NavItem to={Route::TalentWatchlists} label="Watchlists" />
                    <NavItem to={Route::TalentWeights} label="Ranking weights" />
                }

                if has("user.manage") || has("role.assign") || has("monitoring.read") {
                    <div class="tx-nav-section">{ "Admin" }</div>
                }
                if has("user.manage") {
                    <NavItem to={Route::AdminUsers} label="Users" />
                }
                if has("allowlist.manage") {
                    <NavItem to={Route::AdminAllowlist} label="IP Allowlist" />
                }
                if has("mtls.manage") {
                    <NavItem to={Route::AdminMtls} label="Device mTLS" />
                }
                if has("retention.manage") {
                    <NavItem to={Route::AdminRetention} label="Retention" />
                }
                if has("monitoring.read") {
                    <div class="tx-nav-section">{ "Monitoring" }</div>
                    <NavItem to={Route::MonLatency} label="Latency" />
                    <NavItem to={Route::MonErrors} label="Errors" />
                    <NavItem to={Route::MonCrashes} label="Crashes" />
                    <NavItem to={Route::AdminAudit} label="Audit Log" />
                }
            </nav>
            <div class="tx-nav-footer">
                <div class="tx-nav-user">
                    <div class="tx-nav-user-name">{ &state.user.display_name }</div>
                    <div class="tx-subtle tx-mono">{ &state.user.email_mask }</div>
                    <div class="tx-nav-user-roles">
                        { for state.user.roles.iter().map(|r| html!{
                            <span class="tx-chip">{ r.display() }</span>
                        }) }
                    </div>
                </div>
                <button class="tx-btn tx-btn--ghost" onclick={logout}>{ "Sign out" }</button>
            </div>
        </aside>
    }
}

#[derive(Properties, PartialEq)]
pub struct NavItemProps {
    pub to: Route,
    pub label: AttrValue,
    #[prop_or(0)]
    pub badge: i64,
}

#[function_component(NavItem)]
pub fn nav_item(props: &NavItemProps) -> Html {
    html! {
        <Link<Route> to={props.to.clone()} classes={classes!("tx-nav-link")}>
            <span>{ props.label.clone() }</span>
            if props.badge > 0 {
                <span class="tx-badge">{ props.badge }</span>
            }
        </Link<Route>>
    }
}

// ---------------------------------------------------------------------------
// RoleGate + PermGate
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct PermGateProps {
    pub permission: AttrValue,
    pub children: Children,
    #[prop_or_default]
    pub fallback: Option<Html>,
}

/// Renders children only if the current user holds `permission`. When the
/// user is not authenticated at all (no `AuthContext.state`), this
/// redirects to `/login` — unauthenticated access to a permission-gated
/// surface must never dead-end on an in-page denial card. When the user
/// is authenticated but missing the permission, renders the fallback
/// (default: a friendly "not authorized" card).
#[function_component(PermGate)]
pub fn perm_gate(props: &PermGateProps) -> Html {
    let auth = use_context::<AuthContext>().expect("AuthContext");
    let Some(state) = auth.state.as_ref() else {
        return html! { <Redirect<Route> to={Route::Login} /> };
    };
    if state.has_permission(&props.permission) {
        html! { <>{ for props.children.iter() }</> }
    } else if let Some(fb) = &props.fallback {
        fb.clone()
    } else {
        html! {
            <div class="tx-card tx-card--warn">
                <h2 class="tx-title">{ "Not authorized" }</h2>
                <p class="tx-subtle">
                    { format!("Your role does not include the \"{}\" permission.", props.permission) }
                </p>
            </div>
        }
    }
}

// ---------------------------------------------------------------------------
// Placeholders
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct PlaceholderProps {
    #[prop_or_default]
    pub label: AttrValue,
}

#[function_component(PlaceholderLoading)]
pub fn placeholder_loading(props: &PlaceholderProps) -> Html {
    let label = if props.label.is_empty() { AttrValue::from("Loading…") } else { props.label.clone() };
    html! {
        <div class="tx-placeholder tx-placeholder--loading">
            <span class="tx-spinner" aria-hidden="true" />
            <span>{ label }</span>
        </div>
    }
}

#[function_component(PlaceholderEmpty)]
pub fn placeholder_empty(props: &PlaceholderProps) -> Html {
    let label = if props.label.is_empty() { AttrValue::from("Nothing here yet.") } else { props.label.clone() };
    html! {
        <div class="tx-placeholder tx-placeholder--empty">{ label }</div>
    }
}

#[derive(Properties, PartialEq)]
pub struct PlaceholderErrorProps {
    pub message: AttrValue,
    #[prop_or_default]
    pub on_retry: Option<Callback<MouseEvent>>,
}

#[function_component(PlaceholderError)]
pub fn placeholder_error(props: &PlaceholderErrorProps) -> Html {
    html! {
        <div class="tx-placeholder tx-placeholder--error">
            <div>{ props.message.clone() }</div>
            if let Some(cb) = props.on_retry.clone() {
                <button class="tx-btn tx-btn--ghost" onclick={cb}>{ "Retry" }</button>
            }
        </div>
    }
}

// ---------------------------------------------------------------------------
// Toast rack (reads ToastContext)
// ---------------------------------------------------------------------------

#[function_component(ToastRack)]
pub fn toast_rack() -> Html {
    let ctx = use_context::<ToastContext>().expect("ToastContext");
    html! {
        <div class="tx-toasts" role="region" aria-live="polite">
            { for ctx.toasts.iter().map(|t| render_toast(t, ctx.dismiss.clone())) }
        </div>
    }
}

fn render_toast(t: &Toast, dismiss: Callback<u64>) -> Html {
    let id = t.id;
    let onclick = Callback::from(move |_| dismiss.emit(id));
    html! {
        <div class={t.level.class()}>
            <span>{ &t.message }</span>
            <button class="tx-toast-x" onclick={onclick} aria-label="Dismiss">{ "✕" }</button>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Server-side pager — Prev/Next + position readout for list surfaces that
// request incremental pages from the backend (Audit #6 Issue #3).
//
// The owning body holds a `page: u32` state and a `page_size: u32` state
// and refetches whenever either changes. This component only renders the
// controls; all state mutation flows back through the supplied callbacks.
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct ServerPagerProps {
    /// 1-based current page.
    pub page: u32,
    pub page_size: u32,
    /// `None` when the backend did not report a total (e.g. missing
    /// `X-Total-Count` header). In that case we still render Next on the
    /// assumption that more rows may exist; Prev is gated by `page > 1`.
    #[prop_or_default]
    pub total: Option<u64>,
    pub on_prev: Callback<MouseEvent>,
    pub on_next: Callback<MouseEvent>,
}

#[function_component(ServerPager)]
pub fn server_pager(props: &ServerPagerProps) -> Html {
    let page = props.page.max(1);
    let page_size = props.page_size.max(1) as u64;
    let (label, at_end) = match props.total {
        Some(total) => {
            let last_page = ((total + page_size - 1) / page_size).max(1) as u32;
            let start = ((page as u64 - 1) * page_size + 1).min(total.max(1));
            let end = ((page as u64) * page_size).min(total);
            (
                format!("{}–{} of {} (page {}/{})", start, end, total, page, last_page),
                page >= last_page,
            )
        }
        None => (format!("Page {}", page), false),
    };
    html! {
        <div class="tx-pager">
            <button class="tx-btn tx-btn--ghost" onclick={props.on_prev.clone()} disabled={page <= 1}>
                { "Prev" }
            </button>
            <span class="tx-subtle">{ label }</span>
            <button class="tx-btn tx-btn--ghost" onclick={props.on_next.clone()} disabled={at_end}>
                { "Next" }
            </button>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Simple paginated table primitive
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct DataTableProps {
    pub headers: Vec<AttrValue>,
    pub rows: Vec<Vec<Html>>,
    #[prop_or_default]
    pub empty_label: AttrValue,
    #[prop_or(25)]
    pub page_size: usize,
}

#[function_component(DataTable)]
pub fn data_table(props: &DataTableProps) -> Html {
    let page = use_state(|| 0usize);
    let total = props.rows.len();
    let page_size = props.page_size.max(1);
    let num_pages = (total + page_size - 1) / page_size;
    let current = *page;
    let start = current * page_size;
    let end = (start + page_size).min(total);

    let on_prev = {
        let page = page.clone();
        Callback::from(move |_: MouseEvent| {
            if *page > 0 {
                page.set(*page - 1);
            }
        })
    };
    let on_next = {
        let page = page.clone();
        let num_pages = num_pages;
        Callback::from(move |_: MouseEvent| {
            if *page + 1 < num_pages {
                page.set(*page + 1);
            }
        })
    };

    if total == 0 {
        let label = if props.empty_label.is_empty() {
            AttrValue::from("No rows.")
        } else {
            props.empty_label.clone()
        };
        return html! { <PlaceholderEmpty label={label} /> };
    }

    html! {
        <div class="tx-table-wrap">
            <table class="tx-table">
                <thead>
                    <tr>
                        { for props.headers.iter().map(|h| html!{ <th>{ h.clone() }</th> }) }
                    </tr>
                </thead>
                <tbody>
                    { for props.rows[start..end].iter().map(|cells| html!{
                        <tr>{ for cells.iter().map(|c| html!{ <td>{ c.clone() }</td> }) }</tr>
                    }) }
                </tbody>
            </table>
            if num_pages > 1 {
                <div class="tx-pager">
                    <button class="tx-btn tx-btn--ghost" onclick={on_prev} disabled={current == 0}>{ "Prev" }</button>
                    <span class="tx-subtle">
                        { format!("Page {} of {} ({} rows)", current + 1, num_pages, total) }
                    </span>
                    <button class="tx-btn tx-btn--ghost" onclick={on_next} disabled={current + 1 >= num_pages}>{ "Next" }</button>
                </div>
            }
        </div>
    }
}
