CREATE TABLE albums (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    subtitle TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL DEFAULT '',
    ordering_mode TEXT NOT NULL DEFAULT 'CHRONOLOGICAL',
    completion_status TEXT NOT NULL DEFAULT 'IN_PROGRESS',
    visibility TEXT NOT NULL DEFAULT 'PRIVATE',
    cover_asset_id UUID NULL REFERENCES assets(id) ON DELETE SET NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT albums_ordering_mode_check
        CHECK (ordering_mode IN ('CHRONOLOGICAL', 'MANUAL', 'ADDED_AT')),
    CONSTRAINT albums_completion_status_check
        CHECK (completion_status IN ('IN_PROGRESS', 'COMPLETE', 'ARCHIVED')),
    CONSTRAINT albums_visibility_check
        CHECK (visibility IN ('PRIVATE', 'PUBLISHED'))
);

CREATE TABLE album_items (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    album_id UUID NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    trace_id UUID NOT NULL REFERENCES traces(id) ON DELETE CASCADE,
    ordering_index INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT album_items_unique_trace_per_album UNIQUE (album_id, trace_id)
);

CREATE INDEX idx_albums_owner_user_id ON albums(owner_user_id);
CREATE INDEX idx_albums_visibility ON albums(visibility);
CREATE INDEX idx_album_items_album_id ON album_items(album_id);
CREATE INDEX idx_album_items_trace_id ON album_items(trace_id);

SELECT diesel_manage_updated_at('albums');
