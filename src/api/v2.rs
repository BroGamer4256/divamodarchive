// In spite of convention this is not a straight upgrade to v1
// This has several important missing features mainly related to writing data
// V2 is designed specifically for apps which simply want to read posts

pub mod details;
pub mod posts;

#[get("/v2.json")]
pub const fn get_spec() -> &'static str {
	include_str!("v2.json")
}
