use crate::models::*;
use crate::AppState;
use axum::{extract::*, http::StatusCode, response::*};
use base64::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SearchParams {
	pub query: Option<String>,
	pub filter: Option<String>,
	pub limit: Option<usize>,
	pub offset: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct MeilisearchPv {
	pub uid: u64,
	pub post: i32,
	pub pv_id: i32,
	pub song_name: String,
	pub song_name_en: String,
	pub song_info: Option<SongInfo>,
	pub song_info_en: Option<SongInfo>,
	pub levels: [Option<Level>; 5],
}

pub async fn search_pvs(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<Vec<Pv>>, (StatusCode, String)> {
	let index = state.meilisearch.index("pv");
	let mut search = meilisearch_sdk::search::SearchQuery::new(&index);

	search.query = query.query.as_ref().map(|query| query.as_str());

	search.limit = query.limit;
	search.offset = query.offset;

	search.sort = Some(&["pv_id:asc"]);

	let filter = if let Some(filter) = &query.filter {
		format!("{filter}")
	} else {
		String::new()
	};

	search.filter = Some(meilisearch_sdk::search::Filter::new(sqlx::Either::Left(
		filter.as_str(),
	)));

	let pvs = search
		.execute::<MeilisearchPv>()
		.await
		.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

	let pvs = pvs.hits.into_iter().map(|p| p.result).collect::<Vec<_>>();

	let mut vec = Vec::with_capacity(pvs.len());
	for pv in pvs {
		let post = if let Some(mut post) = Post::get_full(pv.post, &state.db).await {
			for i in 0..post.files.len() {
				post.files[i] = format!(
					"https://divamodarchive.com/api/v1/posts/{}/download/{i}",
					post.id
				);
				post.local_files[i] = post.local_files[i]
					.split("/")
					.last()
					.map(|s| String::from(s))
					.unwrap_or(String::new());
			}
			Some(post)
		} else {
			None
		};
		vec.push(Pv {
			uid: BASE64_STANDARD.encode(pv.uid.to_ne_bytes()),
			post,
			id: pv.pv_id,
			name: pv.song_name,
			name_en: pv.song_name_en,
			song_info: pv.song_info,
			song_info_en: pv.song_info_en,
			levels: pv.levels,
		})
	}

	Ok(Json(vec))
}
