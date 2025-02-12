use crate::models::*;
use crate::AppState;
use axum::{extract::*, http::StatusCode, response::*};
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::process::Command;

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
	pub song_info: Option<pv_db::SongInfo>,
	pub song_info_en: Option<pv_db::SongInfo>,
	pub levels: [Option<pv_db::Level>; 5],
}

#[derive(Serialize, Deserialize)]
pub struct MeilisearchModule {
	pub uid: u64,
	pub post_id: i32,
	pub module_id: i32,
	#[serde(flatten)]
	pub module: module_db::Module,
}

#[derive(Serialize, Deserialize)]
pub struct MeilisearchCstmItem {
	pub uid: u64,
	pub post_id: i32,
	pub customize_item_id: i32,
	#[serde(flatten)]
	pub customize_item: module_db::CustomizeItem,
}

#[derive(Serialize, Deserialize)]
struct Config {
	include: Option<Vec<String>>,
}

pub const ROM_DIRS: [&'static str; 31] = [
	"rom",
	"rom_ps4",
	"rom_ps4_dlc",
	"rom_ps4_fix",
	"rom_ps4_patch",
	"rom_steam",
	"rom_steam_cn",
	"rom_steam_dlc",
	"rom_steam_en",
	"rom_steam_fr",
	"rom_steam_ge",
	"rom_steam_it",
	"rom_steam_kr",
	"rom_steam_region",
	"rom_steam_region_cn",
	"rom_steam_region_cn",
	"rom_steam_region_dlc",
	"rom_steam_region_dlc_kr",
	"rom_steam_region_en",
	"rom_steam_region_fr",
	"rom_steam_region_ge",
	"rom_steam_region_kr",
	"rom_steam_region_sp",
	"rom_steam_region_tw",
	"rom_steam_sp",
	"rom_steam_tw",
	"rom_switch",
	"rom_switch_cn",
	"rom_switch_en",
	"rom_switch_kr",
	"rom_switch_tw",
];

pub const DB_PREFIXES: [&'static str; 21] = [
	"mod_",
	"",
	"end_",
	"mdata_",
	"patch2_",
	"patch_",
	"dlc13_",
	"dlc12_",
	"dlc14_",
	"dlc9_",
	"dlc8_",
	"dlc11_",
	"dlc10_",
	"dlc4_",
	"dlc3B_",
	"dlc7_",
	"privilege_",
	"dlc2A_",
	"dlc1_",
	"dlc3A_",
	"dlc2B_",
];

pub async fn extract_post_data(post_id: i32, state: AppState) -> Option<()> {
	let post = Post::get_short(post_id, &state.db).await?;
	if post.post_type == PostType::Plugin
		|| post.post_type == PostType::Cover
		|| post.post_type == PostType::Ui
	{
		return None;
	}

	for file in &post.local_files {
		let file = format!("/pixeldrain/{file}");
		let file = Path::new(&file);
		let extension = file.extension()?.to_str()?;

		let dir = temp_dir::TempDir::new().ok()?;
		let dir = dir.path().to_str()?;

		match extension {
			"zip" => {
				Command::new("unzip")
					.arg(file)
					.arg("-d")
					.arg(dir)
					.output()
					.await
					.ok()?;
				()
			}
			"rar" => {
				Command::new("unrar")
					.arg("x")
					.arg(file)
					.arg(dir)
					.output()
					.await
					.ok()?;
				()
			}
			"7z" => {
				Command::new("7z")
					.arg("x")
					.arg(file)
					.arg(format!("-o{dir}"))
					.output()
					.await
					.ok()?;
				()
			}
			_ => {
				continue;
			}
		}

		for file in walkdir::WalkDir::new(dir).into_iter().filter(|file| {
			if let Ok(file) = &file {
				file.path().ends_with("config.toml")
			} else {
				false
			}
		}) {
			let file = file.ok()?;
			let file = file.path();
			let data = tokio::fs::read_to_string(file).await.ok()?;
			let config: Config = toml::from_str(&data).ok()?;
			let Some(include) = config.include else {
				continue;
			};

			for include in &include {
				for rom in &ROM_DIRS {
					let folder = format!("{}/{include}/{rom}", file.parent()?.to_str()?);
					let path = Path::new(&folder);
					if !path.exists() {
						continue;
					}
					for prefix in &DB_PREFIXES {
						let pv_db = format!("{folder}/{prefix}pv_db.txt");
						let path = Path::new(&pv_db);
						if path.exists() {
							if let Ok(data) = tokio::fs::read_to_string(&path).await {
								parse_pv_db(&data, post_id, state.clone()).await;
							}
						}

						let module_tbl = format!("{folder}/{prefix}gm_module_tbl.farc");
						let module_tbl = Path::new(&module_tbl);
						let customize_item_tbl =
							format!("{folder}/{prefix}gm_customize_item_tbl.farc");
						let customize_item_tbl = Path::new(&customize_item_tbl);
						if module_tbl.exists() || customize_item_tbl.exists() {
							let chritm_prop = format!("{folder}/{prefix}chritm_prop.farc");
							let chritm_prop = Path::new(&chritm_prop);
							let str_array = format!("{folder}/lang2/mod_str_array.toml");
							let str_array = Path::new(&str_array);

							let module_tbl = if module_tbl.exists() {
								Some(module_tbl)
							} else {
								None
							};
							let customize_item_tbl = if customize_item_tbl.exists() {
								Some(customize_item_tbl)
							} else {
								None
							};
							let chritm_prop = if chritm_prop.exists() {
								Some(chritm_prop)
							} else {
								None
							};
							let str_array = if str_array.exists() {
								Some(str_array)
							} else {
								None
							};

							parse_module_db(
								module_tbl,
								customize_item_tbl,
								chritm_prop,
								str_array,
								post_id,
								state.clone(),
							)
							.await;
						}
					}
				}
			}
		}
	}

	Some(())
}

async fn parse_module_db<P: AsRef<Path>>(
	module_tbl: Option<P>,
	customize_item_tbl: Option<P>,
	chritm_prop: Option<P>,
	str_array: Option<P>,
	post_id: i32,
	state: AppState,
) -> Option<()> {
	let module_db =
		module_db::ModuleDb::from_files(module_tbl, customize_item_tbl, chritm_prop, str_array)
			.await?;

	let modules = module_db
		.modules
		.into_iter()
		.map(|(id, module)| MeilisearchModule {
			uid: (post_id as u64) << 32 | (id as u64),
			post_id,
			module_id: id,
			module: module,
		})
		.collect::<Vec<_>>();

	let cstm_items = module_db
		.cstm_items
		.into_iter()
		.map(|(id, cstm_item)| MeilisearchCstmItem {
			uid: (post_id as u64) << 32 | (id as u64),
			post_id,
			customize_item_id: id,
			customize_item: cstm_item,
		})
		.collect::<Vec<_>>();

	let base = meilisearch_sdk::search::SearchQuery::new(&state.meilisearch.index("modules"))
		.with_filter("post_id=-1")
		.with_limit(2000)
		.execute::<MeilisearchModule>()
		.await
		.ok()?;

	let modules = modules
		.into_iter()
		.filter(|module| {
			!base.hits.iter().any(|base| {
				base.result.module_id == module.module_id
					&& base.result.module.name_jp == module.module.name_jp
			})
		})
		.collect::<Vec<_>>();

	state
		.meilisearch
		.index("modules")
		.add_or_update(&modules, Some("uid"))
		.await
		.ok()?;

	let base = meilisearch_sdk::search::SearchQuery::new(&state.meilisearch.index("cstm_items"))
		.with_filter("post_id=-1")
		.with_limit(2000)
		.execute::<MeilisearchCstmItem>()
		.await
		.ok()?;

	let cstm_items = cstm_items
		.into_iter()
		.filter(|cstm_item| {
			!base.hits.iter().any(|base| {
				base.result.customize_item_id == cstm_item.customize_item_id
					&& base.result.customize_item.name_jp == cstm_item.customize_item.name_jp
			})
		})
		.collect::<Vec<_>>();

	state
		.meilisearch
		.index("cstm_items")
		.add_or_update(&cstm_items, Some("uid"))
		.await
		.ok()?;

	Some(())
}

async fn parse_pv_db(data: &str, post_id: i32, state: AppState) -> Option<()> {
	let pv_db = pv_db::PvDb::from_str(data)?;

	let mut documents = Vec::new();
	for (id, entry) in pv_db.pvs.iter() {
		let mut levels = [const { None }; 5];
		if let Some(difficulties) = &entry.difficulty {
			if let Some(easys) = &difficulties.easy {
				for easy in easys {
					if easy.edition == Some(0) {
						levels[0] = easy.level.clone();
					}
				}
			}
			if let Some(normals) = &difficulties.normal {
				for normal in normals {
					if normal.edition == Some(0) {
						levels[1] = normal.level.clone();
					}
				}
			}
			if let Some(hards) = &difficulties.hard {
				for hard in hards {
					if hard.edition == Some(0) {
						levels[2] = hard.level.clone();
					}
				}
			}
			if let Some(extremes) = &difficulties.extreme {
				for extreme in extremes {
					if extreme.edition == Some(0) {
						levels[3] = extreme.level.clone();
					} else if extreme.edition == Some(1) {
						levels[4] = extreme.level.clone();
					}
				}
			}
		}
		documents.push(MeilisearchPv {
			uid: (post_id as u64) << 32 | (*id as u64),
			post: post_id,
			pv_id: *id as i32,
			song_name: entry.song_name.clone(),
			song_name_en: entry.song_name_en.clone(),
			song_info: entry.songinfo.clone(),
			song_info_en: entry.songinfo_en.clone(),
			levels,
		});
	}

	let base = meilisearch_sdk::search::SearchQuery::new(&state.meilisearch.index("pvs"))
		.with_filter("post=-1")
		.with_limit(300)
		.execute::<MeilisearchPv>()
		.await
		.unwrap();

	let pvs = documents
		.into_iter()
		.filter(|pv| {
			!base.hits.iter().any(|base| {
				base.result.pv_id == pv.pv_id
					&& base.result.song_name == pv.song_name
					&& base.result.song_name_en == pv.song_name_en
			})
		})
		.collect::<Vec<_>>();

	state
		.meilisearch
		.index("pvs")
		.add_or_update(&pvs, Some("uid"))
		.await
		.unwrap();

	Some(())
}

pub async fn search_pvs(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<Vec<Pv>>, (StatusCode, String)> {
	let index = state.meilisearch.index("pvs");
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
	let mut posts: BTreeMap<i32, Post> = BTreeMap::new();
	for pv in pvs {
		let post = if pv.post == -1 {
			None
		} else if let Some(post) = posts.get(&pv.post) {
			Some(post.clone())
		} else if let Some(mut post) = Post::get_full(pv.post, &state.db).await {
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
			posts.insert(post.id, post.clone());
			Some(post)
		} else if pv.post != -1 {
			let pvs = state.meilisearch.index("pvs");
			_ = meilisearch_sdk::documents::DocumentDeletionQuery::new(&pvs)
				.with_filter(&format!("post={}", pv.post))
				.execute::<crate::api::ids::MeilisearchPv>()
				.await;
			None
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

pub async fn search_modules(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<Vec<Module>>, (StatusCode, String)> {
	let index = state.meilisearch.index("modules");
	let mut search = meilisearch_sdk::search::SearchQuery::new(&index);

	search.query = query.query.as_ref().map(|query| query.as_str());

	search.limit = query.limit;
	search.offset = query.offset;

	search.sort = Some(&["module_id:asc"]);

	let filter = if let Some(filter) = &query.filter {
		format!("{filter}")
	} else {
		String::new()
	};

	search.filter = Some(meilisearch_sdk::search::Filter::new(sqlx::Either::Left(
		filter.as_str(),
	)));

	let modules = search
		.execute::<MeilisearchModule>()
		.await
		.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

	let modules = modules
		.hits
		.into_iter()
		.map(|module| module.result)
		.collect::<Vec<_>>();

	let mut vec = Vec::with_capacity(modules.len());
	let mut posts: BTreeMap<i32, Post> = BTreeMap::new();
	for module in modules {
		let post = if module.post_id == -1 {
			None
		} else if let Some(post) = posts.get(&module.post_id) {
			Some(post.clone())
		} else if let Some(mut post) = Post::get_full(module.post_id, &state.db).await {
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
			posts.insert(post.id, post.clone());
			Some(post)
		} else if module.post_id != -1 {
			let modules = state.meilisearch.index("modules");
			_ = meilisearch_sdk::documents::DocumentDeletionQuery::new(&modules)
				.with_filter(&format!("post_id={}", module.post_id))
				.execute::<crate::api::ids::MeilisearchModule>()
				.await;
			None
		} else {
			None
		};

		vec.push(Module {
			uid: BASE64_STANDARD.encode(module.uid.to_ne_bytes()),
			post,
			id: module.module_id,
			module: module.module,
		})
	}

	Ok(Json(vec))
}

pub async fn search_cstm_items(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<Vec<CstmItem>>, (StatusCode, String)> {
	let index = state.meilisearch.index("cstm_items");
	let mut search = meilisearch_sdk::search::SearchQuery::new(&index);

	search.query = query.query.as_ref().map(|query| query.as_str());

	search.limit = query.limit;
	search.offset = query.offset;

	search.sort = Some(&["customize_item_id:asc"]);

	let filter = if let Some(filter) = &query.filter {
		format!("{filter}")
	} else {
		String::new()
	};

	search.filter = Some(meilisearch_sdk::search::Filter::new(sqlx::Either::Left(
		filter.as_str(),
	)));

	let cstm_items = search
		.execute::<MeilisearchCstmItem>()
		.await
		.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

	let cstm_items = cstm_items
		.hits
		.into_iter()
		.map(|cstm_item| cstm_item.result)
		.collect::<Vec<_>>();

	let mut vec = Vec::with_capacity(cstm_items.len());
	let mut posts: BTreeMap<i32, Post> = BTreeMap::new();
	for cstm_item in cstm_items {
		let post = if cstm_item.post_id == -1 {
			None
		} else if let Some(post) = posts.get(&cstm_item.post_id) {
			Some(post.clone())
		} else if let Some(mut post) = Post::get_full(cstm_item.post_id, &state.db).await {
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
			posts.insert(post.id, post.clone());
			Some(post)
		} else if cstm_item.post_id != -1 {
			let cstm_items = state.meilisearch.index("cstm_items");
			_ = meilisearch_sdk::documents::DocumentDeletionQuery::new(&cstm_items)
				.with_filter(&format!("post_id={}", cstm_item.post_id))
				.execute::<crate::api::ids::MeilisearchCstmItem>()
				.await;
			None
		} else {
			None
		};

		let bind_module = if let Some(bind_module) = cstm_item.customize_item.bind_module {
			if bind_module != -1 {
				let Json(modules) = crate::api::ids::search_modules(
					axum_extra::extract::Query(crate::api::ids::SearchParams {
						query: None,
						filter: Some(format!("module_id={bind_module}")),
						limit: Some(1),
						offset: Some(0),
					}),
					State(state.clone()),
				)
				.await
				.unwrap_or(Json(Vec::new()));
				modules.first().map(|module| Module {
					uid: module.uid.clone(),
					post: post.clone(),
					id: module.id,
					module: module.module.clone(),
				})
			} else {
				None
			}
		} else {
			None
		};

		vec.push(CstmItem {
			uid: BASE64_STANDARD.encode(cstm_item.uid.to_ne_bytes()),
			post,
			id: cstm_item.customize_item_id,
			cstm_item: cstm_item.customize_item,
			bind_module,
		})
	}

	Ok(Json(vec))
}
