pub mod posts;
pub mod users;

#[get("/v1.json")]
pub fn get_spec() -> &'static str {
	include_str!("v1.json")
}
