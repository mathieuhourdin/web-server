-- Backfill trace mirror subtype into resource_type from legacy entity_type values.
UPDATE resources
SET resource_type = CASE entity_type
    WHEN 'trmj' THEN 'trmj'
    WHEN 'trmb' THEN 'trmb'
    WHEN 'trmh' THEN 'trmh'
    ELSE resource_type
END
WHERE entity_type IN ('trmj', 'trmb', 'trmh');

-- Normalize all trace mirror entity types to a single value.
UPDATE resources
SET entity_type = 'trcm'
WHERE entity_type IN ('trmj', 'trmb', 'trmh');
