use crate::entities::error::PpdcError;
use crate::entities::resource::resource_type::ResourceType;
use crate::entities_v2::landmark::Landmark;
use crate::openai_handler::GptRequestConfig;
use crate::work_analyzer::trace_broker::{
    traits::{
        ProcessorContext,
        LandmarkProcessor,
        ExtractedElementForLandmark, MatchedExtractedElementForLandmark, NewLandmarkForExtractedElement
    }
};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementForResource {
    pub resource_label: String,
    pub extracted_content: String,
    pub generated_context: String,
}

impl ExtractedElementForLandmark for ExtractedElementForResource {
    fn reference(&self) -> String {
        self.resource_label.clone()
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedExtractedElementForResource {
    pub resource_label: String,
    pub extracted_content: String,
    pub generated_context: String,
    pub resource_id: Option<String>,
}

impl MatchedExtractedElementForLandmark for MatchedExtractedElementForResource {
    fn title(&self) -> String {
        self.resource_label.clone()
    }
    fn subtitle(&self) -> String {
        "".to_string()
    }
    fn extracted_content(&self) -> String {
        self.extracted_content.clone()
    }
    fn generated_context(&self) -> String {
        self.generated_context.clone()
    }
    fn landmark_id(&self) -> Option<String> {
        self.resource_id.clone()
    }
    fn landmark_type(&self) -> ResourceType {
        ResourceType::Resource
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct MatchedElementsForResources {
    pub matched_elements: Vec<MatchedExtractedElementForResource>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractedElementsForResources {
    pub elements: Vec<ExtractedElementForResource>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewResourceForExtractedElement {
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub author: String,
    pub identified: bool,
    pub resource_type: String,
}

impl NewLandmarkForExtractedElement for NewResourceForExtractedElement {
    fn title(&self) -> String {
        self.title.clone()
    }
    fn subtitle(&self) -> String {
        format!("Par {} - {}", self.author, self.resource_type)
    }
    fn content(&self) -> String {
        self.content.clone()
    }
    fn identified(&self) -> bool {
        self.identified
    }
    fn landmark_type(&self) -> ResourceType {
        ResourceType::Resource
    }
}

pub struct ResourceProcessor {
}

#[async_trait]
impl LandmarkProcessor for ResourceProcessor {
    type ExtractedElement = ExtractedElementForResource;
    type MatchedElement = MatchedExtractedElementForResource;
    type NewLandmark = NewResourceForExtractedElement;

    fn new() -> Self {
        Self {
        }
    }

    fn extract_system_prompt(&self) -> String {
        let system_prompt = format!("
            Tu es un extracteur de ressources mentionnées dans une trace utilisateur (plateforme de suivi de travail).

            Définition (IMPORTANT) :
            - Une 'ressource' est un artefact externe identifiable : livre, article, papier, billet, film, série, podcast, outil/logiciel, site web, service en ligne.
            - Ne considère PAS comme ressource des objets internes du système (ex: Analyse de la trace ..., IDs, noms de runs, landmark, analysis, etc.).

            Objectif :
            - Repérer toutes les ressources explicitement mentionnées dans la trace.
            - Pour chaque ressource trouvée, produire un élément JSON avec :
              - resource_label : nom/titre de la ressource (chaîne courte, normalisée si possible)
              - extracted_content : extrait exact de la trace (copié/collé) qui justifie la mention (1 à 3 phrases max)
              - generated_context : 1 à 2 phrases décrivant le rôle de la ressource DANS CETTE TRACE UNIQUEMENT.
                - Interdit : ajouter des faits externes (auteur, date, résumé du livre, contexte hors trace).
                - Autorisé : reformuler ce que la trace dit (ex: 'mentionné comme lecture', 'utilisé pour explorer un sujet', 'envie de lire').

            Règles strictes :
            - Si AU MOINS une ressource est explicitement mentionnée, la liste 'elements' NE DOIT PAS être vide.
            - Si aucune ressource n'est mentionnée, renvoyer {{'elements': []}} .
            - Ne renvoyer que du JSON valide, sans texte autour, et respectant exactement le schéma.

            Schéma de sortie (à respecter) :
            {{
              'elements': [
                {{
                  'resource_label': 'string',
                  'extracted_content': 'string',
                  'generated_context': 'string'
                }}
              ]
            }}"
        );

        system_prompt
    }

    fn extract_schema(&self) -> serde_json::Value {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "elements": {
                    "type": "array", 
                    "items": {
                        "type": "object", 
                        "properties": {
                            "resource_label": {"type": "string"},
                            "extracted_content": {"type": "string"},
                            "generated_context": {"type": "string"}
                        },
                        "required": ["resource_label", "extracted_content", "generated_context"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["elements"],
            "additionalProperties": false
        });
        schema
    }

    fn create_new_landmark_system_prompt(&self) -> String {
        let system_prompt = format!("
        Tu es un moteur de création de nouvelles ressources à partir d'éléments simples.
        Tu dois créer une nouvelle ressource à partir d'un élément simple.
        Si la ressource n'est pas totalement identifiée, indique le champ identified à false.
        Réponds uniquement avec du JSON valide respectant le schéma donné.
        "
        );
        system_prompt
    }
    fn create_new_landmark_schema(&self) -> serde_json::Value {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "subtitle": {"type": "string"},
                "content": {"type": "string"},
                "author": {"type": "string"},
                "identified": {"type": "boolean"},
                "resource_type": {"type": "string", "enum": ["Book", "Article", "Movie", "Website", "Other"]},
            },
            "required": ["title", "subtitle", "content", "author", "identified", "resource_type"],
            "additionalProperties": false
        });
        schema
    }

    fn match_elements_system_prompt(&self) -> String {
        let system_prompt = format!("
        Tu es un moteur de correspondance entre éléments simples et ressources.
        Des éléments simples ont été extraits d'un texte utilisateur, et ils sont censés faire référence à des ressources.
        Les ressources actuellement explorées par l'utilisateur sont fournies.
        Pour chaque élément simple, tu dois chercher s'il correspond à une ressource.
        Si un élément simple correspond à une ressource, renvoie le avec l'ID de la ressource.
        Si un élément simple ne correspond à aucune ressource, renvoie le quand même, avec une valeur null pour le resource_id.
        conserve les champs existants de l'élément simple.
        Renvoie tous les éléments simples fournis, même s'ils ne correspondent pas à une ressource.
        Réponds uniquement avec du JSON valide respectant le schéma donné.
        "
        );
        system_prompt
    }
    fn match_elements_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "matched_elements": { 
                    "type": "array", 
                    "items": {
                        "type": "object", 
                        "properties": {
                            "resource_label": {"type": "string"},
                            "extracted_content": {"type": "string"},
                            "generated_context": {"type": "string"},
                            "resource_id": {
                                "anyOf": [
                                    {
                                        "type": "string",
                                        "minLength": 1
                                    },
                                    {
                                        "type": "null"
                                    }
                                ]
                            }
                        },
                        "required": [
                            "resource_label",
                            "extracted_content",
                            "generated_context",
                            "resource_id"
                        ],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["matched_elements"],
            "additionalProperties": false
        })
    }

    async fn extract_elements(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<ExtractedElementForResource>, PpdcError> {

        println!("work_analyzer::trace_broker::split_trace_in_elements_gpt_request");
        let resources_string = context.landmarks
            .iter()
            .filter(|resource| resource.landmark_type == ResourceType::Resource)
            .map(|resource| format!(
                " Title: {}, Subtitle: {}, Content: {}", 
                resource.title.clone(), 
                resource.subtitle.clone(),
                resource.content.clone()))
                .collect::<Vec<String>>()
                .join("\n");
        let user_prompt = format!("
        Ressources déjà explorées :Ressources déjà connues (peut aider à reconnaître des abréviations) :
         {}\n\n
        Trace à analyser : 
        {}",
        resources_string, context.trace.content.as_str());
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.extract_system_prompt(),
            &user_prompt,
            self.extract_schema()
        );
        let elements: ExtractedElementsForResources = gpt_request_config.execute().await?;
        Ok(elements.elements)
    }

    async fn match_elements(
        &self,
        elements: &Vec<ExtractedElementForResource>,
        context: &ProcessorContext,
    ) -> Result<Vec<MatchedExtractedElementForResource>, PpdcError> {
        println!("work_analyzer::trace_broker::match_elements_and_resources");

        let elements_string = serde_json::to_string(&elements)?;
        let resources = context.landmarks
            .iter()
            .filter(|resource| resource.landmark_type == ResourceType::Resource)
            .collect::<Vec<&Landmark>>();
        let resources_string = serde_json::to_string(&resources)?;

        let user_prompt = format!("
        Éléments simples : {}\n\n
        Ressources : {}\n\n", elements_string, resources_string);
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.match_elements_system_prompt(),
            &user_prompt,
            self.match_elements_schema()
        );
        let matched_elements: MatchedElementsForResources = gpt_request_config.execute().await?;
        Ok(matched_elements.matched_elements)
    }
    async fn get_new_landmark_for_extracted_element(
        &self,
        element: &MatchedExtractedElementForResource,
        _context: &ProcessorContext,
    ) -> Result<Self::NewLandmark, PpdcError> {

        println!("work_analyzer::trace_broker::get_new_resource_for_extracted_element_from_gpt_request");

        let extracted_element_string = serde_json::to_string(&element)?;
        let user_prompt = format!("User:
        Élément simple : {}\n\n", extracted_element_string);
        let gpt_request_config = GptRequestConfig::new(
            "gpt-4.1-nano".to_string(),
            &self.create_new_landmark_system_prompt(),
            &user_prompt,
            self.create_new_landmark_schema()
        );
        let new_resource_for_extracted_element: NewResourceForExtractedElement = gpt_request_config.execute().await?;
        Ok(new_resource_for_extracted_element)
    }

    async fn process(
        &self,
        context: &ProcessorContext,
    ) -> Result<Vec<Landmark>, PpdcError> {
        let elements = self.extract_elements(context).await?;
        let mut matched_elements: Vec<MatchedExtractedElementForResource> = vec![];
        if !elements.is_empty() {
            matched_elements = self.match_elements(&elements, context).await?;
        }
        let matched_elements = matched_elements;
        let new_landmarks = self.create_new_landmarks_and_elements(matched_elements, context).await?;
        Ok(new_landmarks)
        
    }
}