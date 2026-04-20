//! Client-side routing for the SPA.
//!
//! Every route below is backed by a real page component in `crate::pages`.
//! Pages themselves handle auth gating (redirecting unauthenticated users to
//! `/login`) and permission gating (rendering `PermGate` fallbacks when the
//! signed-in user lacks the required permission).

use uuid::Uuid;
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

    // Admin
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

    // Monitoring
    #[at("/monitoring/latency")]
    MonLatency,
    #[at("/monitoring/errors")]
    MonErrors,
    #[at("/monitoring/crashes")]
    MonCrashes,

    // P-A Catalog & Governance (Data Steward)
    #[at("/products")]
    Products,
    #[at("/products/:id")]
    ProductDetail { id: Uuid },
    #[at("/imports")]
    Imports,
    #[at("/imports/:id")]
    ImportDetail { id: Uuid },

    // P-B Environmental Intelligence / KPI / Alerts / Reports
    #[at("/env/sources")]
    EnvSources,
    #[at("/env/observations")]
    EnvObservations,
    #[at("/metrics/definitions")]
    MetricDefinitions,
    #[at("/metrics/definitions/:id")]
    MetricDefinitionDetail { id: Uuid },
    #[at("/metrics/computations/:id/lineage")]
    MetricComputationLineage { id: Uuid },
    #[at("/kpi")]
    Kpi,
    #[at("/alerts/rules")]
    AlertRules,
    #[at("/alerts/events")]
    AlertEvents,
    #[at("/reports")]
    Reports,

    // P-C Talent Intelligence (Recruiter)
    #[at("/talent/candidates")]
    TalentCandidates,
    #[at("/talent/candidates/:id")]
    TalentCandidateDetail { id: Uuid },
    #[at("/talent/roles")]
    TalentRoles,
    #[at("/talent/recommendations")]
    TalentRecommendations,
    #[at("/talent/weights")]
    TalentWeights,
    #[at("/talent/watchlists")]
    TalentWatchlists,
    #[at("/talent/watchlists/:id")]
    TalentWatchlistDetail { id: Uuid },

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

        Route::Products => html! { <pages::data_steward::ProductsList/> },
        Route::ProductDetail { id } => {
            html! { <pages::data_steward::ProductDetailPage {id} /> }
        }
        Route::Imports => html! { <pages::data_steward::ImportsList/> },
        Route::ImportDetail { id } => {
            html! { <pages::data_steward::ImportDetailPage {id} /> }
        }

        Route::EnvSources => html! { <pages::analyst::Sources/> },
        Route::EnvObservations => html! { <pages::analyst::Observations/> },
        Route::MetricDefinitions => html! { <pages::analyst::Definitions/> },
        Route::MetricDefinitionDetail { id } => {
            html! { <pages::analyst::DefinitionSeries {id} /> }
        }
        Route::MetricComputationLineage { id } => {
            html! { <pages::analyst::ComputationLineagePage {id} /> }
        }
        Route::Kpi => html! { <pages::analyst::Kpi/> },
        Route::AlertRules => html! { <pages::analyst::AlertRules/> },
        Route::AlertEvents => html! { <pages::user::AlertsFeed/> },
        Route::Reports => html! { <pages::analyst::Reports/> },

        Route::TalentCandidates => html! { <pages::recruiter::Candidates/> },
        Route::TalentCandidateDetail { id } => {
            html! { <pages::recruiter::CandidateDetailPage {id} /> }
        }
        Route::TalentRoles => html! { <pages::recruiter::Roles/> },
        Route::TalentRecommendations => html! { <pages::recruiter::Recommendations/> },
        Route::TalentWeights => html! { <pages::recruiter::Weights/> },
        Route::TalentWatchlists => html! { <pages::recruiter::Watchlists/> },
        Route::TalentWatchlistDetail { id } => {
            html! { <pages::recruiter::WatchlistDetail {id} /> }
        }

        Route::NotFound => html! { <pages::NotFound/> },
    }
}
