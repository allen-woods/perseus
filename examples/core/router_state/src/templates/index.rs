use perseus::{templates::RouterLoadState, Html, Template};
use sycamore::prelude::{cloned, create_memo, view, View};

#[perseus::template_rx(RouterStatePage)]
pub fn router_state_page() -> View<G> {
    let load_state = perseus::get_render_ctx!().router.get_load_state();
    // This uses Sycamore's `create_memo` to create a state that will update whenever the router state changes
    let load_state_str = create_memo(
        cloned!(load_state => move || match (*load_state.get()).clone() {
            RouterLoadState::Loaded { template_name, path } => format!("Loaded {} (template: {}).", path, template_name),
            RouterLoadState::Loading { template_name, path } => format!("Loading {} (template: {}).", path, template_name),
            RouterLoadState::Server => "We're on the server.".to_string()
        }),
    );

    view! {
        a(href = "about", id = "about-link") { "About!" }

        p { (load_state_str.get()) }
    }
}

pub fn get_template<G: Html>() -> Template<G> {
    Template::new("index").template(router_state_page)
}
