//! Client-side routing. Scaffold delivers only `/login` as a placeholder.
//! Full route surface (admin, monitoring, notifications, catalog, etc.)
//! lands in P1+ per `plan.md`.

use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
pub enum Route {
    #[at("/")]
    Root,
    #[at("/login")]
    Login,
    #[not_found]
    #[at("/404")]
    NotFound,
}

pub fn switch(route: Route) -> Html {
    match route {
        Route::Root => html! { <Redirect<Route> to={Route::Login} /> },
        Route::Login => html! { <LoginPlaceholder /> },
        Route::NotFound => html! { <NotFoundPage /> },
    }
}

#[function_component(LoginPlaceholder)]
fn login_placeholder() -> Html {
    html! {
        <main class="tx-shell">
            <section class="tx-card" aria-labelledby="login-title">
                <div class="tx-banner">
                    { "Scaffold preview. The real login flow (Argon2id + JWT + refresh rotation) lands in P1." }
                </div>
                <h1 id="login-title" class="tx-title">{ "TerraOps" }</h1>
                <p class="tx-subtle">{ "Offline Environmental & Catalog Intelligence Portal" }</p>
                <form onsubmit={Callback::from(|e: SubmitEvent| e.prevent_default())}>
                    <label for="email" class="tx-subtle">{ "Email" }</label>
                    <input id="email" class="tx-input" type="email" autocomplete="username" disabled=true placeholder="steward@terraops.local" />
                    <label for="password" class="tx-subtle">{ "Password" }</label>
                    <input id="password" class="tx-input" type="password" autocomplete="current-password" disabled=true placeholder="••••••••" />
                    <button class="tx-btn" type="submit" disabled=true>{ "Sign in (disabled in scaffold)" }</button>
                </form>
            </section>
        </main>
    }
}

#[function_component(NotFoundPage)]
fn not_found() -> Html {
    html! {
        <main class="tx-shell">
            <section class="tx-card">
                <h1 class="tx-title">{ "404" }</h1>
                <p class="tx-subtle">{ "No such route." }</p>
            </section>
        </main>
    }
}
