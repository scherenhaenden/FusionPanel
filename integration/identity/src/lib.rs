pub mod adapter;
pub mod extractors;
pub mod middleware;
pub mod routes;

pub use adapter::SqliteIdentityAdapter;
pub use extractors::{AuthenticatedUser, ExtractorError, RequirePermission, RequireRole};
pub use middleware::{auth_middleware, IdentityStore};
pub use routes::{auth_routes, AuthState};
