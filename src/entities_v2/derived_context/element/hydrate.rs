use std::collections::HashSet;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Nullable, Text, Timestamp, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities::error::{ErrorType, PpdcError};
use crate::schema::{element_landmarks, element_relations};
use crate::entities_v2::landmark::Landmark;

use super::model::{Element, ElementRelationWithRelatedElement, ElementSubtype, ElementType};

#[derive(QueryableByName)]
struct ElementRow {
    #[diesel(sql_type = SqlUuid)]
    id: Uuid,
    #[diesel(sql_type = Text)]
    title: String,
    #[diesel(sql_type = Text)]
    subtitle: String,
    #[diesel(sql_type = Text)]
    content: String,
    #[diesel(sql_type = Nullable<Text>)]
    extended_content: Option<String>,
    #[diesel(sql_type = Text)]
    verb: String,
    #[diesel(sql_type = Text)]
    element_type: String,
    #[diesel(sql_type = Text)]
    element_subtype: String,
    #[diesel(sql_type = Nullable<Timestamp>)]
    interaction_date: Option<NaiveDateTime>,
    #[diesel(sql_type = SqlUuid)]
    user_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    analysis_id: Uuid,
    #[diesel(sql_type = SqlUuid)]
    trace_id: Uuid,
    #[diesel(sql_type = Nullable<SqlUuid>)]
    trace_mirror_id: Option<Uuid>,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
}

fn row_to_element(row: ElementRow, landmark_id: Option<Uuid>) -> Element {
    let element_subtype = ElementSubtype::from_db(&row.element_subtype).unwrap_or_else(|| {
        ElementSubtype::default_for_type(
            ElementType::from_db(&row.element_type).unwrap_or(ElementType::Transaction),
        )
    });
    let element_type = element_subtype.element_type();

    Element {
        id: row.id,
        title: row.title,
        subtitle: row.subtitle,
        content: row.content,
        extended_content: row.extended_content,
        element_type,
        element_subtype,
        verb: row.verb,
        interaction_date: row.interaction_date,
        user_id: row.user_id,
        analysis_id: row.analysis_id,
        trace_id: row.trace_id,
        trace_mirror_id: row.trace_mirror_id,
        landmark_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn get_first_landmark_id_for_element(element_id: Uuid, pool: &DbPool) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    let maybe_id = element_landmarks::table
        .filter(element_landmarks::element_id.eq(element_id))
        .order(element_landmarks::created_at.asc())
        .select(element_landmarks::landmark_id)
        .first::<Uuid>(&mut conn)
        .optional()?;
    Ok(maybe_id)
}

fn load_elements_by_uuid(sql: &str, id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    let rows = sql_query(sql)
        .bind::<SqlUuid, _>(id)
        .load::<ElementRow>(&mut conn)?;
    rows.into_iter()
        .map(|row| {
            let landmark_id = get_first_landmark_id_for_element(row.id, pool)?;
            Ok(row_to_element(row, landmark_id))
        })
        .collect()
}

fn normalize_relation_type(raw: &str) -> String {
    match raw {
        "APPLIES_TO" => "applies_to".to_string(),
        "SUBTASK_OF" => "subtask_of".to_string(),
        "THEME_OF" => "theme_of".to_string(),
        _ => raw.to_lowercase(),
    }
}

impl Element {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        let mut result = load_elements_by_uuid(
            r#"
            SELECT id, title, subtitle, content, extended_content, verb, element_type, element_subtype,
                   interaction_date, user_id, analysis_id, trace_id, trace_mirror_id, created_at, updated_at
            FROM elements
            WHERE id = $1
            "#,
            id,
            pool,
        )?;
        result.pop().ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Element not found".to_string())
        })
    }

    pub fn find_full(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        Self::find(id, pool)
    }

    pub fn with_interaction_date(self, _pool: &DbPool) -> Result<Element, PpdcError> {
        Ok(self)
    }

    pub fn find_landmarks(&self, pool: &DbPool) -> Result<Vec<Landmark>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let landmark_ids = element_landmarks::table
            .filter(element_landmarks::element_id.eq(self.id))
            .select(element_landmarks::landmark_id)
            .load::<Uuid>(&mut conn)?;
        landmark_ids
            .into_iter()
            .map(|id| Landmark::find(id, pool))
            .collect()
    }

    pub fn find_for_landmark(landmark_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        load_elements_by_uuid(
            r#"
            SELECT e.id, e.title, e.subtitle, e.content, e.extended_content, e.verb, e.element_type, e.element_subtype,
                   e.interaction_date, e.user_id, e.analysis_id, e.trace_id, e.trace_mirror_id, e.created_at, e.updated_at
            FROM elements e
            JOIN element_landmarks el ON el.element_id = e.id
            WHERE el.landmark_id = $1
            ORDER BY e.created_at ASC
            "#,
            landmark_id,
            pool,
        )
    }

    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        load_elements_by_uuid(
            r#"
            SELECT id, title, subtitle, content, extended_content, verb, element_type, element_subtype,
                   interaction_date, user_id, analysis_id, trace_id, trace_mirror_id, created_at, updated_at
            FROM elements
            WHERE trace_id = $1
            ORDER BY created_at ASC
            "#,
            trace_id,
            pool,
        )
    }

    pub fn find_related_elements(
        &self,
        pool: &DbPool,
    ) -> Result<Vec<ElementRelationWithRelatedElement>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");

        let mut relations = Vec::new();
        let mut seen = HashSet::<(Uuid, String)>::new();

        let outgoing = element_relations::table
            .filter(element_relations::origin_element_id.eq(self.id))
            .select((element_relations::target_element_id, element_relations::relation_type))
            .load::<(Uuid, String)>(&mut conn)?;
        for (related_element_id, relation_type_raw) in outgoing {
            if related_element_id == self.id {
                continue;
            }
            let relation_type = normalize_relation_type(&relation_type_raw);
            if !seen.insert((related_element_id, relation_type.clone())) {
                continue;
            }
            let related_element = Element::find_full(related_element_id, pool)?;
            relations.push(ElementRelationWithRelatedElement {
                relation_type,
                related_element,
            });
        }

        let incoming = element_relations::table
            .filter(element_relations::target_element_id.eq(self.id))
            .select((element_relations::origin_element_id, element_relations::relation_type))
            .load::<(Uuid, String)>(&mut conn)?;
        for (related_element_id, relation_type_raw) in incoming {
            if related_element_id == self.id {
                continue;
            }
            let relation_type = normalize_relation_type(&relation_type_raw);
            if !seen.insert((related_element_id, relation_type.clone())) {
                continue;
            }
            let related_element = Element::find_full(related_element_id, pool)?;
            relations.push(ElementRelationWithRelatedElement {
                relation_type,
                related_element,
            });
        }

        Ok(relations)
    }
}
