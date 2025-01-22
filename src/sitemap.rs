use crate::AppState;
use axum::{extract::*, http::HeaderMap, response::*};
use reqwest::{header, StatusCode};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename = "loc")]
pub struct Loc {
	#[serde(rename = "$value")]
	pub loc: String,
}

#[derive(Serialize)]
#[serde(rename = "changefreq")]
pub struct Changefreq {
	#[serde(rename = "$value")]
	pub changefreq: String,
}

#[derive(Serialize)]
#[serde(rename = "priority")]
pub struct Priority {
	#[serde(rename = "$value")]
	pub priority: String,
}

#[derive(Serialize)]
#[serde(rename = "lastmod")]
pub struct Lastmod {
	#[serde(rename = "$value")]
	pub lastmod: String,
}

#[derive(Serialize)]
#[serde(rename = "url")]
pub struct Url {
	pub loc: Loc,
	pub changefreq: Changefreq,
	pub priority: Priority,
	pub lastmod: Option<Lastmod>,
}

#[derive(Serialize)]
#[serde(rename = "urlset")]
pub struct Urlset {
	pub url: Vec<Url>,
	pub xmlns: String,
}

#[axum::debug_handler]
pub async fn sitemap(State(state): State<AppState>) -> Result<(HeaderMap, String), StatusCode> {
	let mut urls = Vec::new();
	let latest_date = sqlx::query!("SELECT time FROM posts WHERE explicit = false ORDER BY time")
		.fetch_one(&state.db)
		.await;

	let lastmod = if let Ok(latest_date) = latest_date {
		Some(Lastmod {
			lastmod: latest_date.time.date().to_string(),
		})
	} else {
		None
	};

	let base_url = Url {
		loc: Loc {
			loc: String::from("https://divamodarchive.com/"),
		},
		changefreq: Changefreq {
			changefreq: String::from("daily"),
		},
		priority: Priority {
			priority: String::from("1.0"),
		},
		lastmod,
	};
	urls.push(base_url);

	let about_url = Url {
		loc: Loc {
			loc: String::from("https://divamodarchive.com/about"),
		},
		changefreq: Changefreq {
			changefreq: String::from("monthly"),
		},
		priority: Priority {
			priority: String::from("0.25"),
		},
		lastmod: None,
	};
	urls.push(about_url);

	let posts = sqlx::query!("SELECT id, time FROM posts WHERE explicit = false ORDER BY time")
		.fetch_all(&state.db)
		.await;
	if let Ok(posts) = posts {
		for post in posts {
			let url = Url {
				loc: Loc {
					loc: format!("https://divamodarchive.com/posts/{}", post.id),
				},
				changefreq: Changefreq {
					changefreq: String::from("weekly"),
				},
				priority: Priority {
					priority: String::from("1.0"),
				},
				lastmod: Some(Lastmod {
					lastmod: post.time.date().to_string(),
				}),
			};
			urls.push(url);
		}
	};

	let users = sqlx::query!("SELECT DISTINCT u.id FROM users u LEFT JOIN post_authors pa ON pa.user_id = u.id WHERE pa.post_id IS NOT NULL ORDER BY id;")
		.fetch_all(&state.db)
		.await;
	if let Ok(users) = users {
		for user in users {
			let url = Url {
				loc: Loc {
					loc: format!("https://divamodarchive.com/user/{}", user.id),
				},
				changefreq: Changefreq {
					changefreq: String::from("monthly"),
				},
				priority: Priority {
					priority: String::from("0.5"),
				},
				lastmod: None,
			};
			urls.push(url);
		}
	};

	let xml = Urlset {
		url: urls,
		xmlns: String::from("http://www.sitemaps.org/schemas/sitemap/0.9"),
	};
	let xml = quick_xml::se::to_string(&xml).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

	let mut headers = HeaderMap::new();
	headers.insert(header::CONTENT_TYPE, "application/xml".parse().unwrap());

	Ok((headers, xml))
}
