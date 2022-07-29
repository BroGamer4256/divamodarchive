pub mod posts;
pub mod users;

#[must_use]
#[get("/v1.json")]
pub const fn get_spec() -> &'static str {
	include_str!("v1.json")
}
