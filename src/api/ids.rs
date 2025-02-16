use crate::models::*;
use crate::AppState;
use axum::{extract::*, http::StatusCode, response::*};
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::*;
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

#[derive(Serialize, Deserialize)]
pub struct Pv {
	pub uid: String,
	pub post: Option<i32>,
	pub id: i32,
	pub name: String,
	pub name_en: String,
	pub song_info: Option<pv_db::SongInfo>,
	pub song_info_en: Option<pv_db::SongInfo>,
	pub levels: [Option<pv_db::Level>; 5],
}

impl Pv {
	pub fn has_music(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(music) = &song_info.music {
				if !music.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(music) = &song_info.music {
				if !music.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn has_lyrics(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(lyrics) = &song_info.lyrics {
				if !lyrics.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(lyrics) = &song_info.lyrics {
				if !lyrics.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn has_arranger(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(arranger) = &song_info.arranger {
				if !arranger.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(arranger) = &song_info.arranger {
				if !arranger.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn has_manipulator(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(manipulator) = &song_info.manipulator {
				if manipulator.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(manipulator) = &song_info.manipulator {
				if !manipulator.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn has_editor(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(pv_editor) = &song_info.pv_editor {
				if !pv_editor.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(pv_editor) = &song_info.pv_editor {
				if !pv_editor.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn has_guitar(&self) -> bool {
		if let Some(song_info) = &self.song_info {
			if let Some(guitar_player) = &song_info.guitar_player {
				if !guitar_player.trim().is_empty() {
					return true;
				}
			}
		}
		if let Some(song_info) = &self.song_info_en {
			if let Some(guitar_player) = &song_info.guitar_player {
				if !guitar_player.trim().is_empty() {
					return true;
				}
			}
		}
		return false;
	}

	pub fn song_info_count(&self) -> isize {
		self.has_music() as isize
			+ self.has_lyrics() as isize
			+ self.has_arranger() as isize
			+ self.has_manipulator() as isize
			+ self.has_editor() as isize
			+ self.has_guitar() as isize
	}
}

#[derive(Serialize, Deserialize)]
pub struct Module {
	pub uid: String,
	pub post: Option<i32>,
	pub id: i32,
	pub module: module_db::Module,
}

#[derive(Serialize, Deserialize)]
pub struct CstmItem {
	pub uid: String,
	pub post: Option<i32>,
	pub id: i32,
	pub cstm_item: module_db::CustomizeItem,
}

#[derive(Serialize, Deserialize)]
pub struct PvSearch {
	pub pvs: Vec<Pv>,
	pub posts: BTreeMap<i32, Post>,
}

pub async fn search_pvs(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<PvSearch>, (StatusCode, String)> {
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
			Some(post.id)
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
			Some(post.id)
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
			id: pv.pv_id,
			name: pv.song_name,
			name_en: pv.song_name_en,
			song_info: pv.song_info,
			song_info_en: pv.song_info_en,
			levels: pv.levels,
			post,
		})
	}

	Ok(Json(PvSearch { pvs: vec, posts }))
}

#[derive(Serialize, Deserialize)]
pub struct ModuleSearch {
	pub modules: Vec<Module>,
	pub posts: BTreeMap<i32, Post>,
}

pub async fn search_modules(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<ModuleSearch>, (StatusCode, String)> {
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
			Some(post.id)
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
			Some(post.id)
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

	Ok(Json(ModuleSearch {
		modules: vec,
		posts,
	}))
}

#[derive(Serialize, Deserialize)]
pub struct CstmItemSearch {
	pub cstm_items: Vec<CstmItem>,
	pub bound_modules: BTreeMap<i32, Module>,
	pub posts: BTreeMap<i32, Post>,
}

pub async fn search_cstm_items(
	axum_extra::extract::Query(query): axum_extra::extract::Query<SearchParams>,
	State(state): State<AppState>,
) -> Result<Json<CstmItemSearch>, (StatusCode, String)> {
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
	let mut pending_bound_modules: BTreeSet<i32> = BTreeSet::new();

	for cstm_item in cstm_items {
		let post = if cstm_item.post_id == -1 {
			None
		} else if let Some(post) = posts.get(&cstm_item.post_id) {
			Some(post.id)
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
			Some(post.id)
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

		if let Some(bind_module) = cstm_item.customize_item.bind_module {
			if bind_module != -1 {
				pending_bound_modules.insert(bind_module);
			}
		}

		let customize_item = if cstm_item.customize_item.bind_module == Some(-1) {
			module_db::CustomizeItem {
				bind_module: None,
				chara: cstm_item.customize_item.chara,
				part: cstm_item.customize_item.part,
				name: cstm_item.customize_item.name,
				name_jp: cstm_item.customize_item.name_jp,
				name_en: cstm_item.customize_item.name_en,
				name_cn: cstm_item.customize_item.name_cn,
				name_fr: cstm_item.customize_item.name_fr,
				name_ge: cstm_item.customize_item.name_ge,
				name_it: cstm_item.customize_item.name_it,
				name_kr: cstm_item.customize_item.name_kr,
				name_sp: cstm_item.customize_item.name_sp,
				name_tw: cstm_item.customize_item.name_tw,
			}
		} else {
			cstm_item.customize_item
		};

		vec.push(CstmItem {
			uid: BASE64_STANDARD.encode(cstm_item.uid.to_ne_bytes()),
			post,
			id: cstm_item.customize_item_id,
			cstm_item: customize_item,
		})
	}

	let mut bound_modules = BTreeMap::new();

	if pending_bound_modules.len() > 0 {
		let filter = pending_bound_modules
			.iter()
			.map(|id| format!("module_id={id}"))
			.collect::<Vec<_>>()
			.join(" OR ");

		let Json(modules) = crate::api::ids::search_modules(
			axum_extra::extract::Query(crate::api::ids::SearchParams {
				query: None,
				filter: Some(filter),
				limit: Some(pending_bound_modules.len()),
				offset: Some(0),
			}),
			State(state.clone()),
		)
		.await
		.unwrap_or(Json(ModuleSearch {
			modules: Vec::new(),
			posts: BTreeMap::new(),
		}));

		for module in modules.modules {
			if let Some(post_id) = &module.post {
				if let Some(post) = modules.posts.get(post_id) {
					if !posts.contains_key(post_id) {
						posts.insert(*post_id, post.clone());
					}
				}
			}

			bound_modules.insert(
				module.id,
				Module {
					uid: module.uid,
					post: module.post,
					id: module.id,
					module: module.module,
				},
			);
		}
	}

	Ok(Json(CstmItemSearch {
		cstm_items: vec,
		bound_modules,
		posts,
	}))
}
