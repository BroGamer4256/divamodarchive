use crate::AppState;
use axum::{extract::*, routing::*, Router};

pub mod ids;
pub mod posts;

async fn extract_post(Path(id): Path<i32>, State(state): State<AppState>) {
	ids::extract_post_data(id, state).await;
}

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/api/v1/posts", get(posts::search_posts))
		.route("/api/v1/posts/count", get(posts::count_posts))
		.route(
			"/api/v1/posts/:id",
			get(posts::get_post).delete(posts::delete_post),
		)
		.route("/api/v1/posts/posts", get(posts::get_multiple_posts))
		.route("/api/v1/posts/edit", post(posts::edit))
		.route("/api/v1/posts/upload_image", get(posts::upload_image))
		.route("/api/v1/posts/upload", get(posts::upload_ws))
		.route("/api/v1/posts/:id/download/:variant", get(posts::download))
		.route("/api/v1/posts/:id/like", post(posts::like))
		.route("/api/v1/posts/:id/comment", post(posts::comment))
		.route("/api/v1/posts/:id/author", post(posts::add_author))
		.route("/api/v1/posts/:id/dependency", post(posts::add_dependency))
		.route("/api/v1/posts/:id/report", post(posts::report))
		.route(
			"/api/v1/posts/:post/comment/:comment",
			delete(posts::delete_comment),
		)
		.route("/api/v1/users/settings", post(posts::user_settings))
		.route("/api/v1/ids/pvs", get(ids::search_pvs))
		.route("/api/v1/ids/modules", get(ids::search_modules))
		.route("/api/v1/ids/cstm_items", get(ids::search_cstm_items))
		.route("/api/v1/ids/extract/:id", get(extract_post))
		.layer(tower_http::cors::CorsLayer::permissive())
		.with_state(state)
}
