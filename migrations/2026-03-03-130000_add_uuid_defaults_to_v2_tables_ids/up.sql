BEGIN;

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

ALTER TABLE journals ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE traces ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE landscape_analyses ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE landmarks ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE trace_mirrors ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE elements ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE "references" ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE lenses ALTER COLUMN id SET DEFAULT uuid_generate_v4();
ALTER TABLE posts ALTER COLUMN id SET DEFAULT uuid_generate_v4();

COMMIT;
