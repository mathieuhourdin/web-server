use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

use web_server::openai_handler::gpt_responses_handler::{
    make_gpt_request,
}; // ou le bon module
// use your_crate_name::environment; // si besoin pour la clé, etc.

#[derive(Debug, Deserialize)]
struct ExtractOutput {
    elements: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ExtractedElement {
    resource_identifier: String,
    theme: Option<String>,
    author: Option<String>,
    evidence: Vec<String>,
}

fn read_to_string<P: AsRef<Path>>(path: P) -> anyhow::Result<String> {
    Ok(fs::read_to_string(path)?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1) Charger les fichiers
    let system_prompt = read_to_string("src/work_analyzer/trace_mirror/prompts/primary_resource_suggestion/system.md")?;
    let user_trace = read_to_string("src/work_analyzer/trace_mirror/prompts/primary_resource_suggestion/test_trace.md")?;
    let schema_str = read_to_string("src/work_analyzer/trace_mirror/prompts/primary_resource_suggestion/schema.json")?;
    let schema_json: serde_json::Value = serde_json::from_str(&schema_str)?;

    // 2) Construire le "user_prompt"
    // Ici tu peux juste mettre le texte brut, ou wrapper dans un JSON comme en prod
    let user_prompt = format!(
        r#"{{"known_resources": [], "trace_text": {trace_json} }}"#,
        trace_json = serde_json::to_string(&user_trace)?
    );

    // 3) Appeler ton helper générique
    let result: ExtractedElement = make_gpt_request::<ExtractedElement>(
        system_prompt,
        user_prompt,
        Some(schema_json),
        None,
        None,
    ).await?;

    // 4) Afficher le résultat joliment
    println!("=== RAW RESULT ===");
    println!("{}", serde_json::to_string_pretty(&result.clone())?);

    // 5) (optionnel) Valider rapidement les evidences
    println!("\n=== QUICK EVIDENCE CHECK ===");
    for (i, el) in result.evidence.iter().enumerate() {
        let check_result = user_trace.contains(el.as_str());
        println!("- element #{i}: value = {:?}, check_result = {:?}", serde_json::to_string_pretty(el).unwrap(), check_result);
    }

    Ok(())
}
