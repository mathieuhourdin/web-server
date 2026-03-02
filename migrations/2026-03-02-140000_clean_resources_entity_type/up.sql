-- Canonical cleanup: derive entity_type from resource_type for every resource.
UPDATE resources
SET entity_type = CASE
    -- Public posts
    WHEN resource_type IN ('book', 'rdnt', 'list', 'pblm', 'ratc', 'natc', 'oatc', 'movi', 'vide', 'pcst', 'song', 'curs', 'idea', 'jrit', 'atcl') THEN 'ppst'

    -- Journals
    WHEN resource_type IN ('jrnl', 'wjnl', 'rjnl', 'meta') THEN 'jrnl'

    -- Traces
    WHEN resource_type IN ('trce', 'utrc', 'btrc', 'wtrc', 'hlpd') THEN 'trce'

    -- Trace mirrors (single entity type, subtype in resource_type)
    WHEN resource_type IN ('trcm', 'trmj', 'trmb', 'trmh') THEN 'trcm'

    -- Elements
    WHEN resource_type IN ('elmt', 'evnt', 'cmnt', 'feln') THEN 'elmt'

    -- Landmarks
    WHEN resource_type IN ('miss', 'task', 'qest', 'dlvr', 'proc', 'rsrc', 'them', 'autr', 'hlpr') THEN 'lndm'

    -- Analysis / Lens
    WHEN resource_type = 'anly' THEN 'lnds'
    WHEN resource_type = 'lens' THEN 'lens'

    ELSE entity_type
END;
