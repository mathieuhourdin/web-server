UPDATE posts
SET content_source = 'CUSTOM'
FROM trace_versions
WHERE posts.trace_version_id = trace_versions.id
  AND posts.content_source = 'TRACE_VERSION'
  AND (
      posts.content IS DISTINCT FROM trace_versions.content
      OR posts.image_asset_id IS DISTINCT FROM trace_versions.image_asset_id
  );
