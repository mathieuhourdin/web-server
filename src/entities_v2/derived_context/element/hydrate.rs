use std::collections::HashSet;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landmark::Landmark;
use crate::schema::{element_landmarks, element_relations, elements};

use super::model::{Element, ElementRelationWithRelatedElement, ElementSubtype, ElementType};

type ElementTuple = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<NaiveDateTime>,
    Uuid,
    Uuid,
    Uuid,
    Option<Uuid>,
    NaiveDateTime,
    NaiveDateTime,
);

fn tuple_to_element(row: ElementTuple, landmark_id: Option<Uuid>) -> Element {
    let (
        id,
        title,
        subtitle,
        content,
        extended_content,
        verb,
        element_type_raw,
        element_subtype_raw,
        interaction_date,
        user_id,
        analysis_id,
        trace_id,
        trace_mirror_id,
        created_at,
        updated_at,
    ) = row;

    let element_subtype = ElementSubtype::from_db(&element_subtype_raw).unwrap_or_else(|| {
        ElementSubtype::default_for_type(
            ElementType::from_db(&element_type_raw).unwrap_or(ElementType::Transaction),
        )
    });
    let element_type = element_subtype.element_type();

    Element {
        id,
        title,
        subtitle,
        content,
        extended_content,
        element_type,
        element_subtype,
        verb,
        interaction_date,
        user_id,
        analysis_id,
        trace_id,
        trace_mirror_id,
        landmark_id,
        created_at,
        updated_at,
    }
}

fn load_first_landmark_id_for_element(
    element_id: Uuid,
    pool: &DbPool,
) -> Result<Option<Uuid>, PpdcError> {
    let mut conn = pool
        .get()
        .expect("Failed to get a connection from the pool");
    element_landmarks::table
        .filter(element_landmarks::element_id.eq(element_id))
        .order(element_landmarks::created_at.asc())
        .select(element_landmarks::landmark_id)
        .first::<Uuid>(&mut conn)
        .optional()
        .map_err(Into::into)
}

fn load_elements_from_query(
    rows: Vec<ElementTuple>,
    pool: &DbPool,
) -> Result<Vec<Element>, PpdcError> {
    rows.into_iter()
        .map(|row| {
            let landmark_id = load_first_landmark_id_for_element(row.0, pool)?;
            Ok(tuple_to_element(row, landmark_id))
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

fn select_element_columns() -> (
    elements::id,
    elements::title,
    elements::subtitle,
    elements::content,
    elements::extended_content,
    elements::verb,
    elements::element_type,
    elements::element_subtype,
    elements::interaction_date,
    elements::user_id,
    elements::analysis_id,
    elements::trace_id,
    elements::trace_mirror_id,
    elements::created_at,
    elements::updated_at,
) {
    (
        elements::id,
        elements::title,
        elements::subtitle,
        elements::content,
        elements::extended_content,
        elements::verb,
        elements::element_type,
        elements::element_subtype,
        elements::interaction_date,
        elements::user_id,
        elements::analysis_id,
        elements::trace_id,
        elements::trace_mirror_id,
        elements::created_at,
        elements::updated_at,
    )
}

impl Element {
    pub fn find(id: Uuid, pool: &DbPool) -> Result<Element, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let row = elements::table
            .filter(elements::id.eq(id))
            .select(select_element_columns())
            .first::<ElementTuple>(&mut conn)
            .optional()?;
        let Some(row) = row else {
            return Err(PpdcError::new(
                404,
                ErrorType::ApiError,
                "Element not found".to_string(),
            ));
        };
        let landmark_id = load_first_landmark_id_for_element(row.0, pool)?;
        Ok(tuple_to_element(row, landmark_id))
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
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = elements::table
            .inner_join(element_landmarks::table.on(element_landmarks::element_id.eq(elements::id)))
            .filter(element_landmarks::landmark_id.eq(landmark_id))
            .select(select_element_columns())
            .order(elements::created_at.asc())
            .load::<ElementTuple>(&mut conn)?;
        load_elements_from_query(rows, pool)
    }

    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = elements::table
            .filter(elements::trace_id.eq(trace_id))
            .select(select_element_columns())
            .order(elements::created_at.asc())
            .load::<ElementTuple>(&mut conn)?;
        load_elements_from_query(rows, pool)
    }

    pub fn find_for_analysis_scope(
        user_id: Uuid,
        analysis_ids: Vec<Uuid>,
        pool: &DbPool,
    ) -> Result<Vec<Element>, PpdcError> {
        if analysis_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let rows = elements::table
            .filter(elements::user_id.eq(user_id))
            .filter(elements::analysis_id.eq_any(analysis_ids))
            .select(select_element_columns())
            .order(elements::created_at.asc())
            .load::<ElementTuple>(&mut conn)?;
        load_elements_from_query(rows, pool)
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
            .select((
                element_relations::target_element_id,
                element_relations::relation_type,
            ))
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
            .select((
                element_relations::origin_element_id,
                element_relations::relation_type,
            ))
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
