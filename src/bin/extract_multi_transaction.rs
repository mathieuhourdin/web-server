use web_server::entities::error::PpdcError;
use web_server::work_analyzer::elements_pipeline::extraction_multi_transaction::extract_multi_transaction_from_test_trace;

#[tokio::main]
async fn main() -> Result<(), PpdcError> {
    let extraction = extract_multi_transaction_from_test_trace().await?;
    println!("{}", serde_json::to_string_pretty(&extraction)?);
    Ok(())
}
