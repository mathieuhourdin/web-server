use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::db::DbPool;
use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::entities::resource::Resource;
use crate::work_analyzer::context_builder::create_work_context;

pub async fn get_high_level_analysis(
    simple_elements: &Vec<Resource>,
) -> Result<HighLevelAnalysis, PpdcError> {
    let work_context = create_work_context(simple_elements);
    let high_level_analysis = get_high_level_analysis_from_gpt(&work_context).await?;
    Ok(high_level_analysis)

}

#[derive(Debug, Serialize, Deserialize)]
pub struct HighLevelAnalysis {
    pub title: String,
    pub subtitle: String,
    pub content: String,
}


pub async fn get_high_level_analysis_from_gpt(
    work_context: &String,
) -> Result<HighLevelAnalysis, PpdcError> {
    println!("work_analyzer::high_level_analysis::get_high_level_analysis_from_gpt: Getting high level analysis from gpt");

    let system_prompt = format!("System:
    Contexte de l'ontologie : {}\n\n
    
    Tu es un mentor qui accompagne un utilisateur dans son travail.
    A partir du travail de l'utilisateur, tu dois fournir une analyse globale de son travail.
    Donne d'abord une vue d'ensemble de son travail, puis donne des conseils.
    Donne surtout des feedbacks de haut niveau, des feedbacks qui l'aident à situer la qualité réelle de son travail.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    ", String::new());

    let user_prompt = format!("User:
    Contexte de travail de l'utilisateur : {}\n\n",
        work_context
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "title": {"type": "string"},
            "subtitle": {"type": "string"},
            "content": {"type": "string"},
        },
        "required": ["title", "subtitle", "content"],
        "additionalProperties": false
    });

    let high_level_analysis: HighLevelAnalysis = make_gpt_request(system_prompt, user_prompt, schema).await?;
    Ok(high_level_analysis)
}