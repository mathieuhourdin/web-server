-- Restore legacy entity_type specialization from resource_type.
UPDATE resources
SET entity_type = CASE resource_type
    WHEN 'trmj' THEN 'trmj'
    WHEN 'trmb' THEN 'trmb'
    WHEN 'trmh' THEN 'trmh'
    ELSE entity_type
END
WHERE entity_type = 'trcm'
  AND resource_type IN ('trmj', 'trmb', 'trmh');

-- Roll subtype back to legacy generic resource_type.
UPDATE resources
SET resource_type = 'trcm'
WHERE resource_type IN ('trmj', 'trmb', 'trmh');
