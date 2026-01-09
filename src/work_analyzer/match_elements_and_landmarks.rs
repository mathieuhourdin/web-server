use crate::entities::error::PpdcError;
use crate::entities::resource::Resource;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use crate::work_analyzer::context_builder::{get_ontology_context, get_user_biography};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchElementsAndLandmarksResult {
    pub matched_elements: Vec<MatchedElement>,
    pub suggested_landmarks: Vec<SuggestedLandmark>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedElement {
    pub simple_element_id: String,
    pub landmark_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuggestedLandmark {
    pub temporary_landmark_id: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub entity_type: String,
    pub simple_element_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LandmarkPojectionWithElements {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub entity_type: String,
    pub is_new: bool,
    pub progress: Option<i32>,
    pub closed: bool,
    pub simple_element_ids: Vec<String>,
}

pub async fn match_elements_and_landmarks(
    simple_elements: &Vec<Resource>,
    existing_landmarks: &Vec<Resource>,
    full_context: &String,
) -> Result<Vec<LandmarkPojectionWithElements>, PpdcError> {
    println!("work_analyzer::match_elements_and_landmarks::match_elements_and_landmarks: Matching elements and landmarks for simple elements: {:?}, existing landmarks: {:?}, full context: {}", simple_elements, existing_landmarks, full_context);
    let match_elements_and_landmarks_result =
        get_chatgpt_suggestion_for_match_elements_and_landmarks_and_new_landmarks(
            simple_elements,
            existing_landmarks,
            full_context,
        )
        .await?;

    let new_landmarks_projections = match_elements_and_landmarks_result
        .suggested_landmarks
        .iter()
        .map(|suggested_landmark| LandmarkPojectionWithElements {
            id: suggested_landmark.temporary_landmark_id.clone(),
            title: suggested_landmark.title.clone(),
            subtitle: suggested_landmark.subtitle.clone(),
            content: suggested_landmark.content.clone(),
            entity_type: suggested_landmark.entity_type.clone(),
            is_new: true,
            progress: None,
            closed: false,
            simple_element_ids: suggested_landmark.simple_element_ids.clone(),
        })
        .collect::<Vec<_>>();

    let existing_landmarks_projections = existing_landmarks
        .iter()
        .map(|landmark| {
            let mut landmark_projection = LandmarkPojectionWithElements {
                id: landmark.id.to_string(),
                title: landmark.title.clone(),
                subtitle: landmark.subtitle.clone(),
                content: landmark.content.clone(),
                entity_type: landmark.resource_type.to_code().to_string(),
                is_new: false,
                progress: Some(0),
                closed: false,
                simple_element_ids: vec![],
            };
            let simple_element_ids = match_elements_and_landmarks_result
                .matched_elements
                .iter()
                .filter(|matched_element| {
                    matched_element
                        .landmark_ids
                        .contains(&landmark.id.to_string())
                })
                .map(|matched_element| matched_element.simple_element_id.clone())
                .collect::<Vec<_>>();
            landmark_projection.simple_element_ids = simple_element_ids;
            landmark_projection
        })
        .collect::<Vec<_>>();

    let full_landmarks: Vec<LandmarkPojectionWithElements> = new_landmarks_projections
        .into_iter()
        .chain(existing_landmarks_projections.into_iter())
        .collect();
    Ok(full_landmarks)
}

pub async fn get_chatgpt_suggestion_for_match_elements_and_landmarks_and_new_landmarks(
    simple_elements: &Vec<Resource>,
    existing_landmarks: &Vec<Resource>,
    full_context: &String,
) -> Result<MatchElementsAndLandmarksResult, PpdcError> {
    println!("work_analyzer::match_elements_and_landmarks::get_chatgpt_suggestion_for_match_elements_and_landmarks_and_new_landmarks: Getting chatgpt suggestion for match elements and landmarks and new landmarks for simple elements: {:?}, existing landmarks: {:?}, full context: {}", simple_elements, existing_landmarks, full_context);
    let string_simple_elements = simple_elements
        .iter()
        .map(|element| {
            format!(
                "Id : {}\nTitre : {}\nSous titre : {}\nContenu : {}",
                element.id.to_string(),
                element.title,
                element.subtitle,
                element.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let string_existing_landmarks = existing_landmarks
        .iter()
        .map(|element| {
            format!(
                "Id : {}\nTitre : {}\nSous titre : {}\nContenu : {}",
                element.id.to_string(),
                element.title,
                element.subtitle,
                element.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let ontology_context = get_ontology_context();
    let system_prompt = format!("System:
    Contexte de l'ontologie : {}\n\n

    Ici tu es dans l'étape 2 du pipeline de traitement des traces : le matching entre les éléments simples nouvellement créés et les landmarks existants.
    
    Ton rôle :

    1. Pour chaque élément simple : 
      - déterminer à quels landmarks existants il se rattache (0, 1 ou plusieurs, une idée peut correspondre à un livrable et à une ressource où on l'a trouvée par exemple),
      - utiliser exactement les identifiants de landmarks fournis en entrée,
      - ne pas inventer d'identifiants pour les landmarks existants.

    2. Si un élément simple ne correspond clairement à aucun landmark existant, tu peux proposer un ou plusieurs nouveaux landmarks :
      - crée pour chacun un identifiant temporaire (par ex. 'temp_lm_1', 'temp_lm_2'...),
      - regroupe plusieurs éléments similaires sous un même nouveau landmark,
      - ne crée un nouveau landmark que si cela représente un repère utile et relativement stable, pas pour des détails ponctuels.
      - fais attention à ne pas dupliquer les landmarks, si un landmark proche existe déjà réutilise le plutôt.
      - fais attention à ce que les landmarks soient bien distincts et bien différents les uns des autres.
      - N'hésite pas à créer ou conserver un landmark task générique pour les éléments qui ont trait à la vie quotidienne de l'utilisateur.
      - Utilise le code de l'entité pour qualifier le nouveau landmark.

      Déifinis le type de landmark parmis les suivants : 
          'Task',       // Tâche, chose plus ou moins générique que l'utilisateur se donne à faire explicitement ou implicitement
          'Question',   // question de recherche / problématique ouverte
          'Deliverable',// production attendue (chapitre, article, fiche de lecture, rapport…)
          'Resource',   // ressource mobilisée (livre, article, film… ex : High Output Management par Andy Grove)
          'Process'     // façon de travailler, routine, méthode

      Ne crée pas que des landmarks de type 'Task'.

     3. Chaque élément associé à un nouveau landmark temporaire doit apparaître à la fois :
      - dans 'matched_elements' (avec l'ID temporaire),
      - et dans 'suggested_landmarks.element_ids' (pour ce landmark).

    Pour la compréhension, il t'es aussi fourni les traces brutes de l'utilisateur, ainsi qu'un texte descriptif de l'utilisateur.
    Ils te servent à mieux comprendre le contexte de travail de l'utilisateur, mais c'est bien les éléments simples que tu dois mapper.

    Réponds uniquement avec du JSON valide respectant le schéma donné.
    ", ontology_context);

    let user_prompt = format!(
        "User:
    Contexte de l'utilisateur : {}\n\n
    Éléments simples : {}\n\n
    Landmarks : {}\n\n",
        full_context, string_simple_elements, string_existing_landmarks
    );

    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "matched_elements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "simple_element_id": {"type": "string"},
                        "landmark_ids": {"type": "array", "items": {"type": "string"}},
                    },
                    "required": ["simple_element_id", "landmark_ids"],
                    "additionalProperties": false
                }
            },
            "suggested_landmarks": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "temporary_landmark_id": {"type": "string"},
                        "title": {"type": "string"},
                        "subtitle": {"type": "string"},
                        "content": {"type": "string"},
                        "entity_type": {"type": "string", "enum": ["Task", "Question", "Deliverable", "Process", "Resource"]},
                        "simple_element_ids": {"type": "array", "items": {"type": "string"}},
                    },
                    "required": ["temporary_landmark_id", "title", "subtitle", "content", "entity_type", "simple_element_ids"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["matched_elements", "suggested_landmarks"],
        "additionalProperties": false
    });

    let match_elements_and_landmarks_result: MatchElementsAndLandmarksResult =
        make_gpt_request(system_prompt, user_prompt, schema).await?;
    Ok(match_elements_and_landmarks_result)
}
