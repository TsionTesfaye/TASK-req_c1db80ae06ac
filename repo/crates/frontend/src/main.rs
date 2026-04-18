//! Yew SPA entrypoint. Scaffold-level: mounts the router with a single
//! `/login` placeholder route. Real auth flows, page content, and state
//! plumbing land in P1 per `plan.md`.

mod api;
mod app;
mod router;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
