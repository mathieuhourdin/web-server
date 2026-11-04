mod model;
pub mod routes;

pub(crate) use model::validate_trace_template;
pub use model::WritingTemplate;

#[cfg(test)]
mod tests;
