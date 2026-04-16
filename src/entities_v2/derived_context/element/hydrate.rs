use std::collections::HashSet;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::{ErrorType, PpdcError};
use crate::entities_v2::landmark::Landmark;
use crate::schema::{element_landmarks, element_relations, elements};

use super::model::{
    Element, ElementRelationWithRelatedElement, ElementStatus, ElementSubtype, ElementType,
};

type ElementTuple = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
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
        status_raw,
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
    let status = status_raw.as_deref().and_then(ElementStatus::from_db);

    Element {
        id,
        title,
        subtitle,
        content,
        extended_content,
        element_type,
        element_subtype,
        status,
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
    let mut conn = pool.get()?;
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
    elements::status,
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
        elements::status,
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
        let mut conn = pool.get()?;
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
        let mut conn = pool.get()?;
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
        let mut conn = pool.get()?;
        let rows = elements::table
            .inner_join(element_landmarks::table.on(element_landmarks::element_id.eq(elements::id)))
            .filter(element_landmarks::landmark_id.eq(landmark_id))
            .select(select_element_columns())
            .order(elements::created_at.asc())
            .load::<ElementTuple>(&mut conn)?;
        load_elements_from_query(rows, pool)
    }

    pub fn find_for_trace(trace_id: Uuid, pool: &DbPool) -> Result<Vec<Element>, PpdcError> {
        let mut conn = pool.get()?;
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
        Self::find_for_analysis_scope_filtered(
            user_id,
            analysis_ids,
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
            pool,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_for_analysis_scope_filtered(
        user_id: Uuid,
        analysis_ids: Vec<Uuid>,
        element_types: Vec<ElementType>,
        element_subtypes: Vec<ElementSubtype>,
        statuses: Vec<ElementStatus>,
        interaction_date_from: Option<NaiveDateTime>,
        interaction_date_to: Option<NaiveDateTime>,
        created_at_from: Option<NaiveDateTime>,
        created_at_to: Option<NaiveDateTime>,
        pool: &DbPool,
    ) -> Result<Vec<Element>, PpdcError> {
        let (items, _) = Self::find_for_analysis_scope_filtered_paginated(
            user_id,
            analysis_ids,
            element_types,
            element_subtypes,
            statuses,
            interaction_date_from,
            interaction_date_to,
            created_at_from,
            created_at_to,
            0,
            i64::MAX / 4,
            pool,
        )?;
        Ok(items)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_for_analysis_scope_filtered_paginated(
        user_id: Uuid,
        analysis_ids: Vec<Uuid>,
        element_types: Vec<ElementType>,
        element_subtypes: Vec<ElementSubtype>,
        statuses: Vec<ElementStatus>,
        interaction_date_from: Option<NaiveDateTime>,
        interaction_date_to: Option<NaiveDateTime>,
        created_at_from: Option<NaiveDateTime>,
        created_at_to: Option<NaiveDateTime>,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Element>, i64), PpdcError> {
        if analysis_ids.is_empty() {
            return Ok((vec![], 0));
        }

        let mut conn = pool.get()?;
        let mut count_query = elements::table
            .filter(elements::user_id.eq(user_id))
            .filter(elements::analysis_id.eq_any(&analysis_ids))
            .into_boxed();
        if !element_types.is_empty() {
            let values = element_types
                .iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            count_query = count_query.filter(elements::element_type.eq_any(values));
        }
        if !element_subtypes.is_empty() {
            let values = element_subtypes
                .iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            count_query = count_query.filter(elements::element_subtype.eq_any(values));
        }
        if !statuses.is_empty() {
            let values = statuses
                .iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            count_query = count_query.filter(elements::status.eq_any(values));
        }
        if let Some(from) = interaction_date_from {
            count_query = count_query.filter(elements::interaction_date.ge(Some(from)));
        }
        if let Some(to) = interaction_date_to {
            count_query = count_query.filter(elements::interaction_date.le(Some(to)));
        }
        if let Some(from) = created_at_from {
            count_query = count_query.filter(elements::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            count_query = count_query.filter(elements::created_at.le(to));
        }

        let total = count_query.count().get_result::<i64>(&mut conn)?;

        let mut query = elements::table
            .filter(elements::user_id.eq(user_id))
            .filter(elements::analysis_id.eq_any(analysis_ids))
            .into_boxed();
        if !element_types.is_empty() {
            let values = element_types
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(elements::element_type.eq_any(values));
        }
        if !element_subtypes.is_empty() {
            let values = element_subtypes
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(elements::element_subtype.eq_any(values));
        }
        if !statuses.is_empty() {
            let values = statuses
                .into_iter()
                .map(|value| value.to_db())
                .collect::<Vec<_>>();
            query = query.filter(elements::status.eq_any(values));
        }
        if let Some(from) = interaction_date_from {
            query = query.filter(elements::interaction_date.ge(Some(from)));
        }
        if let Some(to) = interaction_date_to {
            query = query.filter(elements::interaction_date.le(Some(to)));
        }
        if let Some(from) = created_at_from {
            query = query.filter(elements::created_at.ge(from));
        }
        if let Some(to) = created_at_to {
            query = query.filter(elements::created_at.le(to));
        }

        let rows = query
            .select(select_element_columns())
            .order(elements::interaction_date.desc())
            .offset(offset)
            .limit(limit)
            .load::<ElementTuple>(&mut conn)?;
        Ok((load_elements_from_query(rows, pool)?, total))
    }

    pub fn find_related_elements(
        &self,
        pool: &DbPool,
    ) -> Result<Vec<ElementRelationWithRelatedElement>, PpdcError> {
        let mut conn = pool.get()?;

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
