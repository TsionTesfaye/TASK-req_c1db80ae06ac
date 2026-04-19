//! Yew SPA entrypoint. Mounts `App` (which wires auth/toast/notifications
//! context providers and the real router) at `#root`.

mod api;
mod app;
mod components;
mod pages;
mod router;
mod state;

fn main() {
    yew::Renderer::<app::App>::new().render();
}
