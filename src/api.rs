use crate::AppState;
use axum::{extract::*, routing::*, Router};
use ids::*;
use posts::*;

pub mod ids;
pub mod posts;

async fn extract_post(Path(id): Path<i32>, State(state): State<AppState>) {
	ids::extract_post_data(id, state).await;
}

pub fn route(state: AppState) -> Router {
	Router::new()
		.route("/api/v1/posts", get(search_posts))
		.route("/api/v1/posts/count", get(count_posts))
		.route("/api/v1/posts/:id", get(get_post).delete(delete_post))
		.route("/api/v1/posts/posts", get(get_multiple_posts))
		.route("/api/v1/posts/edit", post(edit))
		.route("/api/v1/posts/upload_image", get(upload_image))
		.route("/api/v1/posts/upload", get(upload_ws))
		.route("/api/v1/posts/:id/download/:variant", get(download))
		.route("/api/v1/posts/:id/like", post(like))
		.route("/api/v1/posts/:id/comment", post(comment))
		.route("/api/v1/posts/:id/author", post(add_author))
		.route("/api/v1/posts/:id/dependency", post(add_dependency))
		.route("/api/v1/posts/:id/report", post(report))
		.route(
			"/api/v1/posts/:post/comment/:comment",
			delete(delete_comment),
		)
		.route("/api/v1/users/settings", post(user_settings))
		.route("/api/v1/ids/pvs", get(search_pvs))
		.route("/api/v1/ids/modules", get(search_modules))
		.route("/api/v1/ids/cstm_items", get(search_cstm_items))
		.route("/api/v1/ids/extract/:id", get(extract_post))
		.layer(tower_http::cors::CorsLayer::permissive())
		.with_state(state)
}
