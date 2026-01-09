-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE llm_calls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    status TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt TEXT NOT NULL,
    schema TEXT NOT NULL,
    request TEXT NOT NULL,
    request_url TEXT NOT NULL,
    response TEXT NOT NULL,
    output TEXT NOT NULL,
    input_tokens_used INTEGER NOT NULL,
    reasoning_tokens_used INTEGER NOT NULL,
    output_tokens_used INTEGER NOT NULL,
    price DOUBLE PRECISION NOT NULL,
    currency TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp
);

SELECT diesel_manage_updated_at('llm_calls');

