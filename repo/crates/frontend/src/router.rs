//! Client-side routing for the P1 SPA.
//!
//! Every route below is backed by a real page component in `crate::pages`.
//! Pages themselves handle auth gating (redirecting unauthenticated users to
//! `/login`) and permission gating (rendering `PermGate` fallbacks when the
//! signed-in user lacks the required permission).

use yew::prelude::*;
use yew_router::prelude::*;

use crate::pages;

#[derive(Clone, Routable, PartialEq, Debug)]
pub enum Route {
    #[at("/")]
    Root,
    #[at("/login")]
    Login,
    #[at("/dashboard")]
    Dashboard,
    #[at("/change-password")]
    ChangePassword,
    #[at("/notifications")]
    Notifications,
    #[at("/admin/users")]
    AdminUsers,
    #[at("/admin/allowlist")]
    AdminAllowlist,
    #[at("/admin/mtls")]
    AdminMtls,
    #[at("/admin/retention")]
    AdminRetention,
    #[at("/admin/audit")]
    AdminAudit,
    #[at("/monitoring/latency")]
    MonLatency,
    #[at("/monitoring/errors")]
    MonErrors,
    #[at("/monitoring/crashes")]
    MonCrashes,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(route: Route) -> Html {
    match route {
        Route::Root => html! { <Redirect<Route> to={Route::Dashboard} /> },
        Route::Login => html! { <pages::auth::Login/> },
        Route::Dashboard => html! { <pages::dashboard::Home/> },
        Route::ChangePassword => html! { <pages::auth::ChangePassword/> },
        Route::Notifications => html! { <pages::notifications::Center/> },
        Route::AdminUsers => html! { <pages::admin::Users/> },
        Route::AdminAllowlist => html! { <pages::admin::Allowlist/> },
        Route::AdminMtls => html! { <pages::admin::Mtls/> },
        Route::AdminRetention => html! { <pages::admin::Retention/> },
        Route::AdminAudit => html! { <pages::admin::Audit/> },
        Route::MonLatency => html! { <pages::monitoring::Latency/> },
        Route::MonErrors => html! { <pages::monitoring::Errors/> },
        Route::MonCrashes => html! { <pages::monitoring::Crashes/> },
        Route::NotFound => html! { <pages::NotFound/> },
    }
}
