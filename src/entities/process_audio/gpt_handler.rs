use crate::environment;
use reqwest::Client;
use serde::{Serialize, Deserialize};
use crate::entities::interaction::model::InteractionWithResource;
//use crate::entities::process_audio::process_audio_routes::{GPTRequest, GPTMessage, GPTResponse, ExtractionProperties, GenerationProperties};

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    file: String,
    response_format: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    text: String,
}

#[derive(Debug, Serialize)]
struct GPTRequest {
    model: String,
    messages: Vec<GPTMessage>,
    max_tokens: u32,
    temperature: f32,
    response_format: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct GPTResponseFormat {
    #[serde(rename = "type")]
    response_type: String,
    json_schema: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractionProperties {
    pub titre: String,
    pub auteur: String,
    pub projet: String,
    pub commentaire: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationProperties {
    pub ressource_summary: String,
    pub estimated_publishing_date: String,
    pub interaction_comment: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Properties {
    pub extraction_props: ExtractionProperties,
    pub generation_props: GenerationProperties,
}

#[derive(Debug, Serialize)]
struct GPTMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct GPTResponse {
    choices: Vec<GPTChoice>,
}

#[derive(Debug, Deserialize)]
struct GPTChoice {
    message: GPTMessageResponse,
}

#[derive(Debug, Deserialize)]
struct GPTMessageResponse {
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SelectedProblems {
    pub problem_id: String,
    pub relation_comment: String,
}

pub async fn generate_resource_info_with_gpt(transcription: &str) -> Result<Properties, Box<dyn std::error::Error>> {
    let extraction_props = extract_resource_info_with_gpt(transcription).await?;
    let generation_props = generate_resource_summary_with_gpt(&extraction_props.titre, &extraction_props.auteur, &extraction_props.projet, &extraction_props.commentaire).await?;

    let properties = Properties {
        extraction_props,
        generation_props,
    };

    Ok(properties)
}

async fn extract_resource_info_with_gpt(transcription: &str) -> Result<ExtractionProperties, Box<dyn std::error::Error>> {
    // Get OpenAI API key and base URL from environment
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();

    let system_prompt = format!("System:
    Tu es un expert en extraction de ressources académiques et littéraires.
    Corrige activement les erreurs de transcription dans les noms propres. 
    Si un auteur ou un titre ressemble fortement à un auteur/ouvrage connu, propose la version corrigée.
    Exemple : 'Raymond Gruyère' → 'Raymond Ruyer'.
    Si un commentaire permet de déterminer un auteur ou un titre, utilise-le.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    Si une information est trop incertaine ou absente, mets NA.
    Ne fais pas de résumé à ce stade.
    ");

    let user_prompt = format!("User:
    Basé sur la transcription audio suivante, extrais uniquement les champs:
    - titre (string)
    - auteur (string)
    - projet (string)
    - commentaire (string)

    Transcription: {}\n\n", transcription);

    let gpt_request = GPTRequest {
        model: "gpt-4o".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 800,
        temperature: 0.5,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "titre": {"type": "string"},
                        "auteur": {"type": "string"},
                        "projet": {"type": "string"},
                        "commentaire": {"type": "string"}
                    },
                    "required": ["titre", "auteur"]
                }
            }
        }),
    };

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;
        
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }
    
    let gpt_response: GPTResponse = response.json().await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;

    // Extract the JSON content from the response
    let json_content = gpt_response.choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;
    
    // Parse the JSON response into ExtractionProperties
    let extraction_props: ExtractionProperties = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    if extraction_props.titre == "NA" || extraction_props.auteur == "NA" {
        return Err("No title or author found".into());
    }

    Ok(extraction_props)
}   

async fn generate_resource_summary_with_gpt(title: &str, author_name: &str, _project: &str, _comment: &str) -> Result<GenerationProperties, Box<dyn std::error::Error>> {
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();
    
    let system_prompt = format!("System:
    Tu es un expert en sciences humaines et sociales. 
    À partir d’un titre et d’un auteur fournis, rédige un résumé détaillé (200 à 300 mots, en français).
    Le résumé doit expliquer les idées principales, les découvertes clés et la signification de l’ouvrage.");

    let user_prompt = format!("User:
    Rédige un résumé détaillé pour l’ouvrage suivant:
    - Titre: {title}
    - Auteur: {author_name}

    ");


    let gpt_request = GPTRequest {
        model: "gpt-4o".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 800,
        temperature: 0.8,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "ressource_summary": {"type": "string"},
                        "estimated_publishing_date": {"type": "string"},
                        "interaction_comment": {"type": "string"}
                    },
                    "required": ["ressource_summary", "estimated_publishing_date", "interaction_comment"]
                }
            }
        }),
    };

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;
        
    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }
    
    let gpt_response: GPTResponse = response.json().await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;

    // Extract the JSON content from the response
    let json_content = gpt_response.choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;
    
    // Parse the JSON response into GenerationProperties
    let generation_props: GenerationProperties = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    Ok(generation_props)
}

pub async fn select_problems_with_gpt(resource_title: &str, resource_author: &str, resource_content: &str, user_comment: &str, problems_descriptions: &Vec<InteractionWithResource>) -> Result<Vec<SelectedProblems>, Box<dyn std::error::Error>> {
    let api_key = environment::get_openai_api_key();
    let base_url = environment::get_openai_api_base_url();
    
    let client = Client::new();
    
    let problems_descriptions : String = problems_descriptions.iter().map(|interaction| format!("Problem_id: {}, Problem_description: {} {} {}", interaction.resource.id, interaction.resource.title, interaction.resource.subtitle, interaction.resource.content.clone())).collect::<Vec<String>>().join("\n");

    let system_prompt = format!("System:
    Tu es un expert en sélection de problèmes.
    À partir d'un ouvrage et de son résumé, du commentaire d'un utilisateur, 
    d’une liste de problèmes décrits, sélectionne les id des problèmes qui sont les plus pertinents en relation avec
    l'ouvrage et le commentaire de l'utilisateur.
    Redige un bref commentaire pour expliquer pourquoi l'ouvrage est pertinent pour ce problème.
    Réponds uniquement avec du JSON valide respectant le schéma donné.
    Si un problème est trop incertain ou absente, mets NA.
    Ne fais pas de résumé à ce stade.
    ");

    let user_prompt = format!("User:
    Sélectionne les id des problèmes qui sont les plus pertinents en relation avec
    l'ouvrage et le commentaire de l'utilisateur suivants : 

    Ouvrage: {resource_title} {resource_content} par {resource_author}
    Commentaire de l'utilisateur: {user_comment}

    Liste des problèmes: {problems_descriptions}
    ");

    let gpt_request = GPTRequest {
        model: "gpt-4o".to_string(),
        messages: vec![
            GPTMessage {
                role: "system".to_string(),
                content: system_prompt,
            },
            GPTMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        max_tokens: 1200,
        temperature: 0.8,
        response_format: serde_json::json!({
            "type": "json_schema",
            "json_schema": {
                "name": "reference_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "selected_problems": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "problem_id": {"type": "string"},
                                    "relation_comment": {"type": "string"},
                                },
                                "required": ["problem_id", "relation_comment"]
                            }
                        }
                    },
                    "required": ["selected_problems"]
                }
            }
        }),
    };
    println!("GPT request: {}", serde_json::to_string(&gpt_request).unwrap());

    let response = client
        .post(&format!("{}/v1/chat/completions", base_url))
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&gpt_request)
        .send()
        .await
        .map_err(|e| format!("Failed to send request to GPT: {}", e))?;

    let status = response.status();
    
    if !status.is_success() {
        let error_text = response.text().await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("GPT API error ({}): {}", status, error_text).into());
    }

    
    let gpt_response: GPTResponse = response.json().await
        .map_err(|e| format!("Failed to parse GPT response: {}", e))?;

    println!("GPT response: {}", gpt_response.choices[0].message.content);

    // Extract the JSON content from the response
    let json_content = gpt_response.choices
        .first()
        .and_then(|choice| Some(choice.message.content.clone()))
        .ok_or("No response content from GPT")?;
    
    // Parse the JSON response into the wrapper object
    let response_wrapper: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| format!("Failed to parse GPT JSON response: {}", e))?;

    // Extract the selected_problems array from the wrapper
    let selected_problems_array = response_wrapper.get("selected_problems")
        .ok_or("Missing 'selected_problems' field in response")?
        .as_array()
        .ok_or("'selected_problems' field is not an array")?;

    // Convert each item to SelectedProblems
    let selected_problems: Vec<SelectedProblems> = selected_problems_array
        .iter()
        .map(|item| {
            let problem_id = item.get("problem_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing or invalid 'problem_id' field")?
                .to_string();
            let relation_comment = item.get("relation_comment")
                .and_then(|v| v.as_str())
                .ok_or("Missing or invalid 'relation_comment' field")?
                .to_string();
            Ok(SelectedProblems { problem_id, relation_comment })
        })
        .collect::<Result<Vec<_>, &str>>()
        .map_err(|e| format!("Error parsing selected problems: {}", e))?;

    Ok(selected_problems)
}


#[cfg(test)]
mod tests {
    use super::*;

    /*#[test]
    fn test_find_or_create_user_by_author_name() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        
        let author_name = "Hourdin Matthieu";
        let threshold = 0.7;
        let user = find_or_create_user_by_author_name(&pool, author_name, threshold);
        
        // Assert that we get a result (either success or error)
        assert!(user.is_ok() || user.is_err());
    }*/

    /*#[tokio::test]
    fn test_generate_resource_info_with_gpt() {
        use crate::environment::get_database_url;
        let database_url = get_database_url();
        let manager = diesel::r2d2::ConnectionManager::new(database_url);
        let pool = DbPool::new(manager).expect("Failed to create connection pool");
        
        let transcription = "L'animal, l'homme, la fonction symbolique de Raymond Ruyère.";
        let gpt_response = generate_resource_info_with_gpt(&transcription);

        println!("GPT response: {:?}", gpt_response.clone().unwrap());

        assert!(gpt_response.is_ok());
        assert!(gpt_response.clone().unwrap().choices.len() > 0);
    }*/
}