use crate::db::DbPool;
use crate::entities::session::Session;
use crate::entities::resource::{NewResource};
use crate::entities::resource::resource_type::ResourceType;
use crate::entities::resource::maturing_state::MaturingState;
use crate::entities::interaction::model::{Interaction, NewInteraction, InteractionWithResource};
use crate::entities::user::{User, NewUser, find_similar_users};
use crate::entities::error::PpdcError;
use chrono::Utc;
use diesel::RunQueryDsl;
use crate::entities::process_audio::routes::ResourceInfo;

pub fn find_or_create_user_by_author_name(
    pool: &DbPool,
    author_name: &str,
    threshold: f32,
) -> Result<User, Box<dyn std::error::Error>> {
    // First, try to find similar users
    let similar_users = find_similar_users(pool, author_name, 5)
        .map_err(|e| format!("Failed to search for similar users: {}", e))?;
    
    // Check if we have a good match (score above threshold)
    if let Some(best_match) = similar_users.first() {
        if best_match.score >= threshold {
            // Found a good match, return the user
            return User::find(&best_match.id, pool)
                .map_err(|e| format!("Failed to find user: {}", e).into());
        }
    }
    
    // No good match found, create a new user
    create_user_from_author_name(pool, author_name)
}

pub fn create_user_from_author_name(pool: &DbPool, author_name: &str) -> Result<User, Box<dyn std::error::Error>> {
    // Parse the author name - assume it's in "First Last" format
    let name_parts: Vec<&str> = author_name.trim().split_whitespace().collect();
    
    let (first_name, last_name) = if name_parts.len() >= 2 {
        let first = name_parts[0].to_string();
        let last = name_parts[1..].join(" ");
        (first, last)
    } else {
        // If only one name, use it as last name
        ("".to_string(), author_name.to_string())
    };
    
    // Generate a unique handle and email
    let handle = format!("@{}", author_name.to_lowercase().replace(" ", "_"));
    let email = format!("{}@generated.local", handle.trim_start_matches('@'));
    
    let mut new_user = NewUser {
        email,
        first_name,
        last_name,
        handle,
        password: None,
        profile_picture_url: None,
        is_platform_user: Some(false), // Generated users are not platform users
        biography: Some(format!("Auteur généré automatiquement: {}", author_name)),
        pseudonym: Some(author_name.to_string()),
        pseudonymized: Some(true),
    };
    
    // Hash the password (empty password for generated users)
    new_user.hash_password()
        .map_err(|e| format!("Failed to hash password: {}", e))?;
    
    new_user.create(pool)
        .map_err(|e| format!("Failed to create user: {}", e).into())
}

pub async fn create_resource_and_interaction(
    pool: &DbPool, 
    session: &Session, 
    resource_info: &ResourceInfo
) -> Result<InteractionWithResource, Box<dyn std::error::Error>> {
    // Get user ID from session
    let user_id = session.user_id.ok_or("User not authenticated")?;
    
    // Find or create user based on author name
    let author_user = if resource_info.auteur != "N/A" && !resource_info.auteur.is_empty() {
        match find_or_create_user_by_author_name(pool, &resource_info.auteur, 0.7) {
            Ok(user) => Some(user),
            Err(e) => {
                println!("Failed to find or create author user: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Determine resource type based on the content
    let resource_type = if resource_info.titre != "Contenu Général" {
        ResourceType::Book // Default to Book, could be enhanced with better detection
    } else {
        ResourceType::Idea
    };
    
    
    // Create new resource
    let new_resource = NewResource {
        title: resource_info.titre.clone(),
        subtitle: format!("{} - {}", resource_info.projet.clone(), resource_info.auteur.clone()),
        content: Some(resource_info.ressource_summary.clone()),
        external_content_url: None,
        comment: None,
        image_url: None,
        resource_type: Some(resource_type),
        maturing_state: Some(MaturingState::Finished),
        publishing_state: Some("pbsh".to_string()),
        category_id: None,
        is_external: Some(true),
    };
    
    // Save resource to database
    let created_resource = new_resource.create(pool)
        .map_err(|e| format!("Failed to create resource: {}", e))?;
    
    // Create input interaction (when user records the voice message)
    let new_input_interaction = NewInteraction {
        interaction_user_id: Some(user_id),
        interaction_progress: 0,
        interaction_comment: Some(format!("Ressource créée à partir d'un message vocal: {}", resource_info.projet)),
        interaction_date: Some(Utc::now().naive_utc()),
        interaction_type: Some("inpt".to_string()), // Input type
        interaction_is_public: Some(true),
        resource_id: Some(created_resource.id),
    };
    
    // Save input interaction to database
    let _created_input_interaction = save_interaction(pool, &new_input_interaction)
        .map_err(|e| format!("Failed to create input interaction: {}", e))?;
    
    // Create output interaction (representing the resource itself)
    // Use author user if available, otherwise use session user
    let output_user_id = if let Some(author) = &author_user {
        author.id
    } else {
        user_id
    };
    
    let output_interaction_date = if resource_info.estimated_publishing_date != "N/A" {
        // Try to parse the estimated publishing date
        match chrono::NaiveDate::parse_from_str(&resource_info.estimated_publishing_date, "%Y-%m-%d") {
            Ok(date) => date.and_hms_opt(12, 0, 0).unwrap_or_else(|| Utc::now().naive_utc()),
            Err(_) => Utc::now().naive_utc(), // Fallback to current time if parsing fails
        }
    } else {
        Utc::now().naive_utc() // Use current time if no date provided
    };
    
    let new_output_interaction = NewInteraction {
        interaction_user_id: Some(output_user_id),
        interaction_progress: 100, // Resource is complete/finished
        interaction_comment: Some({
            let author_name = if let Some(author) = &author_user { 
                author.display_name()
            } else { 
                "utilisateur".to_string()
            };
            format!("Ressource créée par {}: {}", author_name, resource_info.titre)
        }),
        interaction_date: Some(output_interaction_date),
        interaction_type: Some("outp".to_string()), // Output type
        interaction_is_public: Some(true),
        resource_id: Some(created_resource.id),
    };
    
    // Save output interaction to database
    let created_output_interaction = save_interaction(pool, &new_output_interaction)
        .map_err(|e| format!("Failed to create output interaction: {}", e))?;
    
    // Create InteractionWithResource for response (using the output interaction)
    let interaction_with_resource = InteractionWithResource {
        interaction: created_output_interaction,
        resource: created_resource,
    };
    
    Ok(interaction_with_resource)
}

pub fn save_interaction(pool: &DbPool, new_interaction: &NewInteraction) -> Result<Interaction, Box<dyn std::error::Error>> {
    use diesel::insert_into;
    use crate::schema::interactions;
    
    let mut conn = pool.get().expect("Failed to get a connection from the pool");
    
    let result = insert_into(interactions::table)
        .values(new_interaction)
        .get_result(&mut conn)
        .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(result)
}