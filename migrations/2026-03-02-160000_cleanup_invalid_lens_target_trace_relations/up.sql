DELETE FROM resource_relations rr
USING resources origin_resource, resources target_resource
WHERE rr.relation_type = 'trgt'
  AND rr.origin_resource_id = origin_resource.id
  AND rr.target_resource_id = target_resource.id
  AND origin_resource.entity_type = 'lens'
  AND target_resource.entity_type <> 'trce';
