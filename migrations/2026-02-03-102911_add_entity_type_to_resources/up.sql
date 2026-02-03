ALTER TABLE resources ADD COLUMN entity_type TEXT;

UPDATE resources
SET entity_type = CASE
    WHEN resource_type IN (
        'book',
        'rdnt',
        'list',
        'pblm',
        'ratc',
        'natc',
        'oatc',
        'atcl',
        'movi',
        'vide',
        'pcst',
        'song',
        'curs',
        'idea',
        'jrit'
    ) THEN 'ppst'
    WHEN resource_type = 'jrnl' THEN 'jrnl'
    WHEN resource_type = 'trce' THEN 'trce'
    WHEN resource_type = 'trcm' THEN 'trcm'
    WHEN resource_type IN ('elmt', 'evnt', 'cmnt', 'feln') THEN 'elmt'
    WHEN resource_type IN ('miss', 'task', 'qest', 'dlvr', 'proc', 'rsrc', 'them') THEN 'lndm'
    WHEN resource_type = 'anly' THEN 'lnds'
    WHEN resource_type = 'lens' THEN 'lens'
    ELSE 'ppst'
END;

ALTER TABLE resources ALTER COLUMN entity_type SET NOT NULL;
