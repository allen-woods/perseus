mod error_pages;
mod templates;

use perseus::define_app;
define_app! {
    templates: [
        crate::templates::build_state::get_template::<G>(),
        crate::templates::build_paths::get_template::<G>(),
        crate::templates::request_state::get_template::<G>(),
        crate::templates::incremental_generation::get_template::<G>(),
        crate::templates::revalidation::get_template::<G>(),
        crate::templates::revalidation_and_incremental_generation::get_template::<G>(),
        crate::templates::amalgamation::get_template::<G>()
    ],
    error_pages: crate::error_pages::get_error_pages()
}
