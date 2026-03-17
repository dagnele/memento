use crate::service::cat::CatResult;

pub fn render(result: &CatResult) -> String {
    result.content.clone()
}
