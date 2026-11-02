/// Current external access, shared by own-feed filtering and admin analytics.
/// The caller supplies a trace row with SQL alias `t`; no user input is interpolated.
pub(crate) const TRACE_IS_SHARED_SQL: &str = r#"
NOT t.is_encrypted AND (
    EXISTS (
        SELECT 1 FROM posts shared JOIN post_grants g ON g.post_id=shared.id AND g.status='ACTIVE'
        WHERE shared.publication_channel_id IS NULL AND shared.source_trace_id=t.id
          AND shared.status='PUBLISHED'
          AND ((g.grantee_user_id IS NOT NULL AND g.grantee_user_id<>t.user_id
                AND trace_publication_readable(shared.id,g.grantee_user_id))
            OR (g.grantee_scope='ALL_PLATFORM_USERS' AND trace_publication_eligible(shared.id,t.user_id)))
    ) OR EXISTS (
        SELECT 1 FROM trace_mentions m WHERE m.trace_id=t.id AND m.removed_at IS NULL
          AND m.mentioned_user_id<>t.user_id AND automation_same_sandbox(t.user_id,m.mentioned_user_id)
          AND NOT EXISTS(SELECT 1 FROM user_blocks b WHERE
            (b.blocker_user_id=t.user_id AND b.blocked_user_id=m.mentioned_user_id)
            OR (b.blocked_user_id=t.user_id AND b.blocker_user_id=m.mentioned_user_id))
    ) OR EXISTS (
        SELECT 1 FROM posts public_post
        JOIN publication_channels c ON c.id=public_post.publication_channel_id AND c.enabled AND c.kind='BLOG'
        JOIN blogs blog ON blog.channel_id=c.id
        WHERE public_post.source_trace_id=t.id AND public_post.user_id=t.user_id
          AND public_post.status='PUBLISHED'
          AND NOT EXISTS(SELECT 1 FROM automation_demo_users d WHERE d.user_id=c.user_id)
    ) OR EXISTS (
        SELECT 1 FROM journal_share_links link JOIN journals j ON j.id=link.journal_id
        JOIN posts linked ON linked.source_trace_id=t.id AND linked.user_id=t.user_id
          AND linked.publication_channel_id IS NULL AND linked.status='PUBLISHED'
        WHERE link.journal_id=t.journal_id AND j.status<>'ARCHIVED' AND NOT j.is_encrypted
          AND link.revoked_at IS NULL AND link.expires_at>(NOW() AT TIME ZONE 'UTC')
          AND (link.scoped_post_id=linked.id OR (link.scoped_post_id IS NULL AND t.sharing_sensitivity='NORMAL'))
    )
)
"#;
