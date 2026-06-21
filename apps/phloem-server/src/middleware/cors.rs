//! CORS configuration.

use tower_http::cors::CorsLayer;

/// Create a CORS layer that allows the SvelteKit dev server origin.
pub fn cors_layer() -> CorsLayer {
    CorsLayer::permissive()
}
