use super::{model::*, persist};
use crate::db::DbPool;
use crate::entities_v2::{
    journal::Journal,
    post::Post,
    post_grant::{NewPostGrantDto, PostGrant},
    trace::{Trace, TraceComplementAudienceMode},
};
use diesel::{
    connection::SimpleConnection,
    prelude::*,
    r2d2::ConnectionManager,
    sql_types::{Array, Uuid as SqlUuid},
};
use std::sync::{Arc, Barrier};
use uuid::Uuid;

#[test]
fn validates_audience_names() {
    assert_eq!(persist::validate_name(" Friends ").unwrap(), "Friends");
    assert!(persist::validate_name(" ").is_err());
    assert!(persist::validate_name(&"a".repeat(101)).is_err());
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn trace_languages_persist_without_ai_and_reach_readers() {
    use crate::entities_v2::{
        feed::hydrate::{find_feed_items_paginated, find_my_feed_items_paginated},
        session::Session,
        trace::{language, model::{NewTrace, TraceStatus}, routes::get_trace_route},
    };
    use axum::extract::{Extension, Path};
    let f = Fixture::new();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE users SET ai_features_enabled=FALSE, ai_features_enabled_by_admin=FALSE WHERE id='{}'", f.owner)).unwrap();
    let french = "Je me suis promené dans la forêt ce matin. Les arbres étaient magnifiques et j'ai pris le temps de regarder les oiseaux.";
    let english = "I spent the afternoon walking through the countryside and thinking about the book I have been reading this week.";
    let make = || NewTrace::new("Language test".into(), "".into(), french.into(), chrono::Utc::now().naive_utc(), f.owner, f.journal);
    let draft = make().create(&f.pool).unwrap();
    assert_eq!(draft.language_code, None);
    let mut trace = draft;
    trace.status = TraceStatus::Finalized;
    trace.finalized_at = Some(chrono::Utc::now().naive_utc());
    let mut trace = trace.update(&f.pool).unwrap();
    assert_eq!(trace.language_code.as_deref(), Some("fr"));
    trace.content = english.into();
    assert_eq!(trace.update(&f.pool).unwrap().language_code.as_deref(), Some("en"));
    assert_eq!(make().create_finalized(&f.pool).unwrap().language_code.as_deref(), Some("fr"));
    let mut encrypted = make();
    encrypted.is_encrypted = true;
    encrypted.encryption_metadata = Some(serde_json::json!({}));
    assert!(encrypted.create_finalized(&f.pool).unwrap().language_code.is_none());
    let (id, post) = f.publication();
    let complement = Trace::create_complement(id, f.owner, "".into(), "".into(),
        french.into(), TraceComplementAudienceMode::Parent, &[], &f.pool).unwrap();
    assert_eq!(complement.trace.language_code.as_deref(), Some("fr"));
    let before = Trace::find_full_trace(id, &f.pool).unwrap();
    // The maintenance write must not look like an edit or increment a version.
    language::store(&mut f.pool.get().unwrap(), id, Some("fr")).unwrap();
    let after = Trace::find_full_trace(id, &f.pool).unwrap();
    assert_eq!(before.version_integer, after.version_integer);
    assert_eq!(before.updated_at, after.updated_at);
    PostGrant::create_or_update(&Post::find_full(post, &f.pool).unwrap(), f.owner,
        NewPostGrantDto { grantee_user_id: Some(f.reader), grantee_scope: None, access_level: None }, &f.pool).unwrap();
    let mut session = Session::anonymous(); session.user_id = Some(f.reader);
    let read = get_trace_route(Extension(f.pool.clone()), Extension(session), Path(id)).await.unwrap().0;
    assert_eq!(read.language_code.as_deref(), Some("fr"));
    let (journal, _) = Trace::get_shared_for_journal_paginated(f.reader, f.journal, 0, 20, None, &f.pool).unwrap();
    assert_eq!(journal.iter().find(|t| t.id == id).unwrap().language_code.as_deref(), Some("fr"));
    let (feed, _) = find_feed_items_paginated(f.reader, None, 0, 20, &f.pool).unwrap();
    assert_eq!(feed.iter().find(|t| t.source_id == id).unwrap().language_code.as_deref(), Some("fr"));
    let (feed, _) = find_my_feed_items_paginated(f.owner, 0, 20, &f.pool).unwrap();
    assert_eq!(feed.iter().find(|t| t.source_id == id).unwrap().language_code.as_deref(), Some("fr"));
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn continuation_previews_are_batched_and_follow_reader_permissions() {
    use crate::entities_v2::{
        album::Album, feed::hydrate::{find_feed_items_paginated,find_my_feed_items_paginated},
        session::Session, trace::{continuation::hydrate_continuations,routes::get_trace_route},
        trace_mention::TraceMention,
    };
    use axum::extract::{Extension,Path,Query};
    let f=Fixture::new();
    let (parent,parent_post)=f.publication();
    let (child,child_post)=f.publication();
    let mut linked=Trace::find_full_trace(child,&f.pool).unwrap();
    linked.continues_trace_id=Some(parent);linked.update(&f.pool).unwrap();
    fn share(f:&Fixture,post:Uuid) {
        PostGrant::create_or_update(&Post::find_full(post,&f.pool).unwrap(),f.owner,
            NewPostGrantDto { grantee_user_id:Some(f.reader),grantee_scope:None,access_level:None },&f.pool).unwrap();
    }
    async fn read(f:&Fixture,user:Uuid,id:Uuid)->crate::entities_v2::trace::model::TraceReadableView {
        let mut session=Session::anonymous();session.user_id=Some(user);
        get_trace_route(Extension(f.pool.clone()),Extension(session),Path(id)).await.unwrap().0
    }
    share(&f,child_post);
    let hidden=read(&f,f.reader,child).await;
    assert!(hidden.continues_trace_id.is_none() && hidden.continues_from.is_none());
    assert!(!serde_json::to_string(&hidden).unwrap().contains(&parent.to_string()));
    assert_eq!(read(&f,f.owner,child).await.continues_from.unwrap().id,parent);
    let (feed,_)=find_feed_items_paginated(f.reader,None,0,20,&f.pool).unwrap();
    assert!(feed.iter().find(|i|i.source_id==child).unwrap().continues_from.is_none());
    share(&f,parent_post);
    let visible=read(&f,f.reader,child).await;
    assert_eq!(visible.continues_trace_id,Some(parent));
    assert_eq!(visible.continues_from.unwrap().title,"Test");
    let (feed,_)=find_feed_items_paginated(f.reader,None,0,20,&f.pool).unwrap();
    assert_eq!(feed.iter().find(|i|i.source_id==child).unwrap().continues_from.as_ref().unwrap().id,parent);
    let (own_feed,_)=find_my_feed_items_paginated(f.owner,0,20,&f.pool).unwrap();
    assert_eq!(own_feed.iter().find(|i|i.source_id==child).unwrap().continues_trace_id,Some(parent));
    let mut session=Session::anonymous();session.user_id=Some(f.reader);
    let query=serde_json::from_value(serde_json::json!({})).unwrap();
    let journal=crate::entities_v2::trace::routes::get_traces_for_journal_route(
        Extension(f.pool.clone()),Extension(session),Path(f.journal),Query(query)).await.unwrap().0;
    assert_eq!(journal.items.iter().find(|i|i.id==child).unwrap().continues_from.as_ref().unwrap().id,parent);
    let album=Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO albums(id,owner_user_id,title,visibility,completion_status) VALUES('{album}','{}','Test','PUBLISHED','COMPLETE');
        INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES('{album}','{child}',0);",f.owner)).unwrap();
    let items=Album::find(album,&f.pool).unwrap().find_items_for_viewer(f.reader,&f.pool).unwrap();
    assert_eq!(items[0].trace.continues_from.as_ref().unwrap().id,parent);
    f.pool.get().unwrap().batch_execute(&format!("UPDATE post_grants SET status='REVOKED' WHERE post_id='{parent_post}'")).unwrap();
    assert!(read(&f,f.reader,child).await.continues_from.is_none());
    TraceMention::replace_active(parent,&[f.reader],&f.pool).unwrap();
    assert_eq!(read(&f,f.reader,child).await.continues_trace_id,Some(parent));
    TraceMention::replace_active(parent,&[],&f.pool).unwrap();
    assert!(read(&f,f.reader,child).await.continues_from.is_none());
    TraceMention::replace_active(parent,&[f.reader],&f.pool).unwrap();
    for sql in [
        format!("UPDATE traces SET status='ARCHIVED' WHERE id='{parent}'"),
        format!("UPDATE traces SET status='FINALIZED',is_encrypted=TRUE,encryption_metadata='{{}}'::jsonb WHERE id='{parent}'"),
    ] {
        f.pool.get().unwrap().batch_execute(&sql).unwrap();
        let result=read(&f,f.reader,child).await;
        assert!(result.continues_trace_id.is_none() && result.continues_from.is_none());
    }
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET is_encrypted=FALSE,encryption_metadata=NULL WHERE id='{parent}';
        INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{}','{}');",f.owner,f.reader)).unwrap();
    let mut items=vec![Trace::find_full_trace(child,&f.pool).unwrap();3];
    hydrate_continuations(f.reader,items.iter_mut(),&f.pool).unwrap();
    assert!(items.iter().all(|t|t.continues_from.is_none() && t.continues_trace_id.is_none()));
}


#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn translation_messages_are_batched_personal_and_completed_only() {
    use axum::extract::{Extension,Path,Query};
    use crate::entities_v2::{
        message::{Message,MessageReaction,NewMessage,NewMessageDto,MessageProcessingState},
        session::Session,
        trace::{routes::{get_trace_route,get_traces_for_journal_route},translation::hydrate_translations},
        journal::routes::get_journal_audience_user_traces_route,
        feed::hydrate::{find_feed_items_paginated,find_my_feed_items_paginated},
    };
    let f=Fixture::new();
    let (trace,post)=f.publication();
    let (without_translation,_)=f.publication();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
         VALUES('{}','{}','FOLLOW','ACCEPTED',NOW())",f.stranger,f.owner)).unwrap();
    let post=Post::find_full(post,&f.pool).unwrap();
    for user in [f.reader,f.stranger] {
        PostGrant::create_or_update(&post,f.owner,NewPostGrantDto {
            grantee_user_id:Some(user),grantee_scope:None,access_level:None },&f.pool).unwrap();
    }
    fn reply(f:&Fixture,reader:Uuid,mentor:Uuid,trace:Uuid,kind:&str,state:MessageProcessingState,content:&str)->Message {
        let mut payload=serde_json::json!({"recipient_user_id":mentor,"trace_id":trace,"message_type":kind,"content":"Translate please"});
        if kind=="shared_trace_translation_request" {
            payload["attachment_type"]=serde_json::json!("shared_trace_translation");
            payload["attachment"]=serde_json::json!({"target_locale":"fr"});
        }
        let request=NewMessage::new(serde_json::from_value::<NewMessageDto>(payload).unwrap(),reader).create(&f.pool).unwrap();
        let mut response=NewMessage::new(serde_json::from_value::<NewMessageDto>(serde_json::json!({
            "recipient_user_id":reader,"trace_id":trace,"reply_to_message_id":request.id,
            "message_type":"mentor_reply","content":content})).unwrap(),mentor);
        response.processing_state=state;
        response.create(&f.pool).unwrap()
    }
    let old=reply(&f,f.reader,f.stranger,trace,"shared_trace_translation_request",MessageProcessingState::Processed,"Old translation");
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE messages SET created_at=NOW()-INTERVAL '1 hour' WHERE id='{}'",old.id)).unwrap();
    let latest=reply(&f,f.reader,f.stranger,trace,"shared_trace_translation_request",MessageProcessingState::Processed,"Personal mentor comments\n\nFull translation");
    let other=reply(&f,f.stranger,f.owner,trace,"shared_trace_translation_request",MessageProcessingState::Processed,"Other reader private translation");
    for state in [MessageProcessingState::Pending,MessageProcessingState::Running,MessageProcessingState::Failed] {
        reply(&f,f.reader,f.stranger,trace,"shared_trace_translation_request",state,"Not completed");
    }
    reply(&f,f.reader,f.stranger,trace,"shared_trace_explanation_request",MessageProcessingState::Processed,"Not a translation");
    reply(&f,f.reader,f.stranger,trace,"shared_trace_translation_request",MessageProcessingState::Processed,"  ");
    MessageReaction::upsert(latest.id,f.reader,"❤️",&f.pool).unwrap();
    let mut items=vec![Trace::find_full_trace(trace,&f.pool).unwrap(),
        Trace::find_full_trace(trace,&f.pool).unwrap(),Trace::find_full_trace(without_translation,&f.pool).unwrap()];
    hydrate_translations(f.reader,items.iter_mut(),&f.pool).unwrap();
    assert_eq!(items[0].translation_message.as_ref().unwrap().id,latest.id);
    assert_eq!(items[1].translation_message.as_ref().unwrap().id,latest.id);
    assert!(items[2].translation_message.is_none());
    assert!(items[0].translation_message.as_ref().unwrap().reactions[0].reacted_by_me);
    hydrate_translations(f.owner,items.iter_mut(),&f.pool).unwrap();
    assert!(items.iter().all(|t|t.translation_message.is_none()));
    hydrate_translations(f.stranger,items.iter_mut(),&f.pool).unwrap();
    assert_eq!(items[0].translation_message.as_ref().unwrap().id,other.id);
    let session=|user| {let mut session=Session::anonymous();session.user_id=Some(user);session};
    let read=get_trace_route(Extension(f.pool.clone()),Extension(session(f.reader)),Path(trace)).await.unwrap().0;
    assert_eq!(read.translation_message.unwrap().id,latest.id);
    let query=serde_json::from_value(serde_json::json!({})).unwrap();
    let journal=get_traces_for_journal_route(Extension(f.pool.clone()),Extension(session(f.reader)),Path(f.journal),Query(query)).await.unwrap().0;
    assert_eq!(journal.items.iter().find(|t|t.id==trace).unwrap().translation_message.as_ref().unwrap().id,latest.id);
    let (feed,_)=find_feed_items_paginated(f.reader,None,0,20,&f.pool).unwrap();
    assert_eq!(feed.iter().find(|t|t.source_id==trace).unwrap().translation_message.as_ref().unwrap().id,latest.id);
    let (own_feed,_)=find_my_feed_items_paginated(f.owner,0,20,&f.pool).unwrap();
    assert!(own_feed.iter().all(|item|item.translation_message.is_none()));
    let query=serde_json::from_value(serde_json::json!({})).unwrap();
    let inspection=get_journal_audience_user_traces_route(Extension(f.pool.clone()),Extension(session(f.owner)),Path((f.journal,f.reader)),Query(query)).await.unwrap().0;
    assert!(inspection.items.iter().all(|item|item.trace.translation_message.is_none()));
    assert!(!serde_json::to_string(&inspection).unwrap().contains("Personal mentor comments"));
    assert!(Message::find(latest.id,&f.pool).unwrap().seen_at.is_none());
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{}','{}')",f.reader,f.stranger)).unwrap();
    let read=get_trace_route(Extension(f.pool.clone()),Extension(session(f.reader)),Path(trace)).await.unwrap().0;
    assert!(read.translation_message.is_none());
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE post_grants SET status='REVOKED' WHERE post_id='{}' AND grantee_user_id='{}'",post.id,f.reader)).unwrap();
    assert!(get_trace_route(Extension(f.pool.clone()),Extension(session(f.reader)),Path(trace)).await.is_err());
}



#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn publication_notification_selection_preserves_access_and_filters_delivery() {
    use axum::extract::{Extension,Json,Path};
    use crate::entities_v2::{session::Session,post::{notifications::{NotificationOptions,PublicationNotificationMode as Mode,get_notification_settings},
        routes::{put_trace_post_route,PutTracePostDto,enqueue_post_published_notification_emails}}};
    let f=Fixture::new();
    let (trace,id)=f.publication();
    let mut post=Post::find_full(id,&f.pool).unwrap();
    assert_eq!(post.notification_mode,Mode::All);
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
        VALUES('{}','{}','FOLLOW','ACCEPTED',NOW());
        UPDATE users SET shared_journal_activity_email_mode='instant' WHERE id IN ('{}','{}')",
        f.stranger,f.owner,f.reader,f.stranger)).unwrap();
    for reader in [f.reader,f.stranger] {
        PostGrant::create_or_update(&post,f.owner,NewPostGrantDto{
            grantee_user_id:Some(reader),grantee_scope:None,access_level:None},&f.pool).unwrap();
    }
    assert!(post.notification_allows(f.reader) && post.notification_allows(f.stranger));
    f.pool.get().unwrap().batch_execute(&format!("UPDATE posts SET status='DRAFT' WHERE id='{id}'")).unwrap();
    post=Post::find_full(id,&f.pool).unwrap();
    post.apply_notification_options(NotificationOptions{notification_mode:Some(Mode::Selected),
        notification_user_ids:Some(vec![f.reader,f.reader])},&f.pool).unwrap();
    post=post.update(&f.pool).unwrap();
    assert_eq!(post.notification_user_ids,vec![f.reader]);
    assert!(post.notification_allows(f.reader) && !post.notification_allows(f.stranger));
    assert!(post.apply_notification_options(NotificationOptions{notification_mode:Some(Mode::Selected),
        notification_user_ids:Some(vec![Uuid::new_v4()])},&f.pool).is_err());
    let mut session=Session::anonymous();session.user_id=Some(f.owner);
    let settings=get_notification_settings(Extension(f.pool.clone()),Extension(session.clone()),Path(id)).await.unwrap().0;
    assert_eq!(settings.notification_user_ids,vec![f.reader]);
    let mut other=Session::anonymous();other.user_id=Some(f.reader);
    assert_eq!(get_notification_settings(Extension(f.pool.clone()),Extension(other),Path(id)).await.err().unwrap().status_code,404);
    assert!(serde_json::to_value(&post).unwrap().get("notification_user_ids").is_none());
    // Publish silently through the real route: no asynchronous push/mail tasks run.
    let payload:PutTracePostDto=serde_json::from_value(serde_json::json!({
        "status":"published","notification_mode":"none"})).unwrap();
    post=put_trace_post_route(Extension(f.pool.clone()),Extension(session.clone()),Path(trace),Json(payload)).await.unwrap().0;
    assert_eq!(post.notification_mode,Mode::None);
    assert!(post.notification_user_ids.is_empty());
    assert!(enqueue_post_published_notification_emails(&post,&[],&f.pool).unwrap().is_empty());
    assert!(PostGrant::user_can_read_post(&post,f.reader,&f.pool).unwrap());
    assert!(PostGrant::user_can_read_post(&post,f.stranger,&f.pool).unwrap());
    let (feed,_)=crate::entities_v2::feed::hydrate::find_feed_items_paginated(f.reader,None,0,20,&f.pool).unwrap();
    assert!(feed.iter().any(|item|item.source_id==trace));
    // Updating an already-published selection must not create another publication event.
    let payload:PutTracePostDto=serde_json::from_value(serde_json::json!({
        "notification_mode":"selected","notification_user_ids":[f.reader]})).unwrap();
    post=put_trace_post_route(Extension(f.pool.clone()),Extension(session),Path(trace),Json(payload)).await.unwrap().0;
    let emails=enqueue_post_published_notification_emails(&post,&[],&f.pool).unwrap();
    assert_eq!(emails.len(),1);
    #[derive(QueryableByName)]
    struct Recipient { #[diesel(sql_type=SqlUuid)] recipient_user_id:Uuid }
    let recipient=diesel::sql_query("SELECT recipient_user_id FROM outbound_emails WHERE id=$1")
        .bind::<SqlUuid,_>(emails[0]).get_result::<Recipient>(&mut f.pool.get().unwrap()).unwrap();
    assert_eq!(recipient.recipient_user_id,f.reader);
    assert!(enqueue_post_published_notification_emails(&post,&[f.reader],&f.pool).unwrap().is_empty());
    // Revoked grants cannot be revived by the selection list.
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE post_grants SET status='REVOKED' WHERE post_id='{id}' AND grantee_user_id='{}'",f.reader)).unwrap();
    assert!(enqueue_post_published_notification_emails(&post,&[],&f.pool).unwrap().is_empty());
}


#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn draft_mentor_context_is_owned_versioned_and_immutable() {
    use crate::entities_v2::message::{draft_context, NewMessage, NewMessageDto};
    let f = Fixture::new();
    let (parent, _) = f.publication();
    let draft = Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,continues_trace_id)
        VALUES('{draft}','{}','{}','Draft','','An unfinished thought',NOW(),'USER_TRACE','DRAFT',NOW(),'{parent}')",
        f.owner,f.journal)).unwrap();
    let version = Trace::find_full_trace(draft,&f.pool).unwrap().version_integer;
    let prepare = |owner,expected,thread,question: &str| draft_context::prepare(
        owner,Some(draft),Some(f.reader),expected,thread,question,&f.pool);
    assert_eq!(prepare(f.owner,None,None,"Where next?").err().unwrap().status_code,400);
    assert_eq!(prepare(f.owner,Some(version+1),None,"Where next?").err().unwrap().status_code,409);
    assert_eq!(prepare(f.stranger,Some(version),None,"Where next?").err().unwrap().status_code,404);
    assert_eq!(prepare(f.owner,Some(version),Some(Uuid::new_v4()),"Where next?").err().unwrap().status_code,404);
    let prepared = prepare(f.owner,Some(version),None,"Where next?").unwrap().unwrap();
    assert!(prepared.source.writing_thread_id.is_some());
    assert_eq!(prepared.input["draft"]["content"],"An unfinished thought");
    assert_eq!(prepared.input["thread_traces"][0]["id"],parent.to_string());
    assert_eq!(prepared.source.omitted_thread_trace_count,0);
    let dto: NewMessageDto = serde_json::from_value(serde_json::json!({
        "recipient_user_id":f.reader,"trace_id":draft,"content":"Where next?","message_type":"question"
    })).unwrap();
    let mut new = NewMessage::new(dto,f.owner);
    new.metadata = Some(prepared.metadata());
    let message = new.create_with_draft_context(Some(&prepared),&f.pool).unwrap();
    let public = serde_json::to_string(&message).unwrap();
    assert!(!public.contains("An unfinished thought"));
    assert_eq!(message.metadata.unwrap().draft_trace.unwrap().trace_version_integer,version);
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET content='Later edited writing' WHERE id IN ('{draft}','{parent}')")).unwrap();
    assert_eq!(draft_context::load(message.id,f.owner,&f.pool).unwrap(),prepared.input);
    assert_eq!(draft_context::load(message.id,f.stranger,&f.pool).err().unwrap().status_code,409);
    let current = Trace::find_full_trace(draft,&f.pool).unwrap().version_integer;
    let huge = "x".repeat(100_001);
    assert_eq!(prepare(f.owner,Some(current),None,&huge).err().unwrap().status_code,413);
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET is_encrypted=TRUE,encryption_metadata='{{}}'::jsonb WHERE id='{draft}'")).unwrap();
    let current = Trace::find_full_trace(draft,&f.pool).unwrap().version_integer;
    assert_eq!(prepare(f.owner,Some(current),None,"Where next?").err().unwrap().status_code,403);
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn writing_threads_chain_branch_drafts_and_projection_fences() {
    use crate::entities_v2::{journal::Journal,writing_thread::{persist as threads,service}};
    let f=Fixture::new();
    let (a,_)=f.publication();
    fn assert_cache(f:&Fixture) {
        #[derive(diesel::QueryableByName)]
        struct Check { #[diesel(sql_type=diesel::sql_types::Bool)] valid:bool }
        let result=diesel::sql_query("SELECT COALESCE(bool_and(
            w.trace_count=(SELECT count(*) FROM writing_thread_trace_ids(w.id) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED')
            AND w.character_count=(SELECT COALESCE(sum(char_length(t.title)::bigint+char_length(t.content)::bigint),0)
                FROM writing_thread_trace_ids(w.id) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED' AND NOT t.is_encrypted)
            AND w.last_activity_at IS NOT DISTINCT FROM (SELECT max(t.finalized_at)
                FROM writing_thread_trace_ids(w.id) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED')
            AND COALESCE((SELECT array_agg(m.trace_id ORDER BY m.position) FROM writing_thread_traces m WHERE m.thread_id=w.id),'{}'::uuid[])
                = COALESCE((SELECT array_agg(c.trace_id ORDER BY c.depth DESC) FROM writing_thread_trace_ids(w.id) c),'{}'::uuid[])
            ),TRUE) AS valid FROM writing_threads w WHERE w.user_id=$1")
            .bind::<SqlUuid,_>(f.owner).get_result::<Check>(&mut f.pool.get().unwrap()).unwrap();
        assert!(result.valid,"Memberships and cached metrics must match the authoritative chain");
    }
    fn fresh_draft(f:&Fixture)->Trace {
        let mut journal=Journal::find_full(f.journal,&f.pool).unwrap();
        let draft=Trace::create_blank_draft_for_journal(&journal,&f.pool).unwrap();
        journal.current_draft_id=Some(draft.id);journal.update(&f.pool).unwrap();draft
    }
    fn finalize(mut draft:Trace,content:&str,pool:&DbPool)->Trace {
        draft.content=content.to_string();draft.status=crate::entities_v2::trace::TraceStatus::Finalized;
        draft.finalized_at=Some(chrono::Utc::now().naive_utc());draft.update(pool).unwrap()
    }
    fn link(mut draft:Trace,source:Uuid,f:&Fixture)->(Trace,Uuid) {
        draft.continues_trace_id=Some(source);
        let version=draft.version_integer;
        let draft=draft.update_with_expected_version(version,&f.pool).unwrap();
        let id=threads::list(f.owner,None,Some(source),"",0,20,&f.pool).unwrap().0
            .into_iter().find(|w|w.thread.pending_trace_id==Some(draft.id)).unwrap().thread.id;
        (draft,id)
    }
    let draft=fresh_draft(&f);
    let (mut writing,thread)=link(draft,a,&f);
    writing.content="I am thinking about attention and the places where I write.".into();
    let version=writing.version_integer;
    writing=writing.update_with_expected_version(version,&f.pool).unwrap();
    assert_eq!(threads::view(f.owner,thread,&f.pool).unwrap().thread.pending_trace_id,Some(writing.id));
    assert_cache(&f);
    let b=finalize(writing,"The library gives me a different kind of attention.",&f.pool).id;
    assert_eq!(threads::view(f.owner,thread,&f.pool).unwrap().thread.latest_trace_id,Some(b));
    let (next,next_thread)=link(fresh_draft(&f),b,&f);
    assert_eq!(next_thread,thread);
    let c=finalize(next,"I want to explore this relation between space and attention.",&f.pool).id;
    let (branch,branch_id)=link(fresh_draft(&f),b,&f);
    assert_ne!(thread,branch_id);
    let d=finalize(branch,"What makes the library a shared space, rather than just a quiet place?",&f.pool).id;
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![a,b,c]);
    assert_eq!(threads::traces(f.owner,branch_id,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![b,d]);
    assert_eq!(threads::view(f.owner,branch_id,&f.pool).unwrap().trace_count,2);
    assert_cache(&f);
    let publication=Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!("INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
        VALUES('{publication}','{}','{b}','','','','OUTPUT','IDEA','PUBLISHED',NOW());",f.owner)).unwrap();
    PostGrant::create_or_update(&Post::find_full(publication,&f.pool).unwrap(),f.owner,NewPostGrantDto {
        grantee_user_id:Some(f.reader),grantee_scope:None,access_level:None,
    },&f.pool).unwrap();
    assert!(!Trace::find_full_trace(a,&f.pool).unwrap().user_can_read(f.reader,&f.pool).unwrap());
    let mut reader_session=crate::entities_v2::session::Session::anonymous();reader_session.user_id=Some(f.reader);
    let readable=crate::entities_v2::trace::routes::get_trace_route(axum::extract::Extension(f.pool.clone()),
        axum::extract::Extension(reader_session),axum::extract::Path(b)).await.unwrap().0;
    assert!(readable.continues_trace_id.is_none());
    assert!(threads::view(f.reader,thread,&f.pool).is_err());
    assert_eq!(threads::list(f.reader,None,None,"",0,20,&f.pool).unwrap().1,0);
    assert_eq!(threads::list(f.owner,None,Some(b),"",0,20,&f.pool).unwrap().1,2);
    let mut unrelated=fresh_draft(&f);unrelated.content="Unrelated existing writing".into();
    let unrelated=unrelated.update(&f.pool).unwrap();
    let (mut linked,linked_thread)=link(unrelated.clone(),c,&f);
    assert_eq!(linked_thread,thread);
    assert_eq!(linked.content,unrelated.content);
    linked.continues_trace_id=Some(d);
    let linked=linked.update(&f.pool).unwrap();
    assert!(threads::view(f.owner,thread,&f.pool).unwrap().thread.pending_trace_id.is_none());
    let mut linked=linked;
    linked.continues_trace_id=None;
    linked.update(&f.pool).unwrap();
    assert!(threads::view(f.owner,branch_id,&f.pool).unwrap().thread.pending_trace_id.is_none());
    assert_eq!(Trace::find_full_trace(unrelated.id,&f.pool).unwrap().content,unrelated.content);
    assert!(f.pool.get().unwrap().batch_execute(&format!("UPDATE traces SET continues_trace_id='{c}' WHERE id='{a}'")).is_err());

    f.pool.get().unwrap().batch_execute("UPDATE writing_threads SET generation_due_at=NOW() WHERE generation_status='PENDING'").unwrap();
    let claimed=threads::claim(&f.pool).unwrap().unwrap();
    assert!(threads::finish(&claimed,"Lieux et attention",None,"Réflexion sur les lieux et l’attention",&["réflexion".into()],&f.pool).unwrap());
    let output=serde_json::to_value(threads::view(f.owner,claimed.id,&f.pool).unwrap()).unwrap();
    assert_eq!(output["title"],"Lieux et attention");
    assert!(output["ongoing_question"].is_null());
    assert_eq!(output["journey_summary"],"Réflexion sur les lieux et l’attention");
    assert_eq!(output["search_tags"],serde_json::json!(["réflexion"]));
    assert!(output.get("description").is_none());
    assert_eq!(threads::list(f.owner,None,None,"reflexion",0,20,&f.pool).unwrap().1,1);
    let mut edited=Trace::find_full_trace(b,&f.pool).unwrap();edited.content.push_str(" The question is also social.");
    edited.update(&f.pool).unwrap();
    assert!(threads::view(f.owner,thread,&f.pool).unwrap().thread.journey_summary.is_none());
    assert!(threads::view(f.owner,branch_id,&f.pool).unwrap().thread.journey_summary.is_none());
    assert_cache(&f);
    f.pool.get().unwrap().batch_execute("UPDATE writing_threads SET generation_due_at=NOW() WHERE generation_status='PENDING'").unwrap();
    let stale=threads::claim(&f.pool).unwrap().unwrap();
    let mut edited=Trace::find_full_trace(b,&f.pool).unwrap();edited.content.push_str(" Another revision.");edited.update(&f.pool).unwrap();
    assert!(!threads::finish(&stale,"Stale title",Some("Stale question?"),"Stale journey",&[],&f.pool).unwrap());
    f.pool.get().unwrap().batch_execute("UPDATE writing_threads SET generation_due_at=NOW() WHERE generation_status='PENDING'").unwrap();
    let disabled=threads::claim(&f.pool).unwrap().unwrap();let disabled_id=disabled.id;
    f.pool.get().unwrap().batch_execute(&format!("UPDATE users SET ai_features_enabled=FALSE WHERE id='{}'",f.owner)).unwrap();
    service::process_claimed(disabled,f.pool.clone()).await;
    assert_eq!(threads::view(f.owner,disabled_id,&f.pool).unwrap().thread.generation_status,"DISABLED");
    let mut archived=Trace::find_full_trace(c,&f.pool).unwrap();archived.status=crate::entities_v2::trace::TraceStatus::Archived;archived.update(&f.pool).unwrap();
    assert_eq!(threads::view(f.owner,thread,&f.pool).unwrap().thread.latest_trace_id,Some(c));
    assert!(!threads::view(f.owner,thread,&f.pool).unwrap().can_continue);
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().1,2);
    assert_cache(&f);
    let mut restored=Trace::find_full_trace(c,&f.pool).unwrap();restored.status=crate::entities_v2::trace::TraceStatus::Finalized;restored.update(&f.pool).unwrap();
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().1,3);
    assert_cache(&f);
    // Public PATCH uses the same versioned write, including explicit null.
    let draft=Trace::find_full_trace(unrelated.id,&f.pool).unwrap();
    let mut session=crate::entities_v2::session::Session::anonymous();
    session.user_id=Some(f.owner);
    let payload=serde_json::from_value(serde_json::json!({
        "continues_trace_id":d,"content":"Existing draft text",
        "expected_version_integer":draft.version_integer
    })).unwrap();
    let linked=crate::entities_v2::trace::routes::patch_trace_route(
        axum::extract::Extension(f.pool.clone()),axum::extract::Extension(session.clone()),
        axum::extract::Path(draft.id),axum::extract::Json(payload)).await.unwrap().0;
    assert_eq!(linked.continues_trace_id,Some(d));
    let payload=serde_json::from_value(serde_json::json!({
        "continues_trace_id":null,"expected_version_integer":linked.version_integer
    })).unwrap();
    let unlinked=crate::entities_v2::trace::routes::patch_trace_route(
        axum::extract::Extension(f.pool.clone()),axum::extract::Extension(session),
        axum::extract::Path(draft.id),axum::extract::Json(payload)).await.unwrap().0;
    assert!(unlinked.continues_trace_id.is_none());
    assert_eq!(unlinked.content,"Existing draft text");
    // Assigning the link and finalizing in one database update also advances the end.
    let mut combined=unlinked;
    combined.continues_trace_id=Some(d);
    let end=finalize(combined,"Continuation finalized together with its link.",&f.pool);
    assert_eq!(threads::view(f.owner,branch_id,&f.pool).unwrap().thread.latest_trace_id,Some(end.id));

    // An already finalized root can acquire its first link, even with descendants.
    let (source,_)=f.publication();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE writing_threads SET journey_summary='Old projection',title='Old title',generation_status='READY',compiled_revision=input_revision WHERE id='{thread}'"
    )).unwrap();
    let root=Trace::find_full_trace(a,&f.pool).unwrap();
    let mut session=crate::entities_v2::session::Session::anonymous();
    session.user_id=Some(f.owner);
    let payload=serde_json::from_value(serde_json::json!({
        "continues_trace_id":source,"expected_version_integer":root.version_integer
    })).unwrap();
    let attached=crate::entities_v2::trace::routes::patch_trace_route(
        axum::extract::Extension(f.pool.clone()),axum::extract::Extension(session.clone()),
        axum::extract::Path(a),axum::extract::Json(payload)).await.unwrap().0;
    assert_eq!(attached.continues_trace_id,Some(source));
    assert_eq!(attached.status,crate::entities_v2::trace::TraceStatus::Finalized);
    assert_eq!(attached.content,root.content);
    assert!(threads::view(f.owner,thread,&f.pool).unwrap().thread.journey_summary.is_none());
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![source,a,b,c]);
    let incoming=threads::list(f.owner,None,Some(source),"",0,20,&f.pool).unwrap().0;
    assert_eq!(incoming.len(),1);
    assert_eq!(incoming[0].thread.id,thread);
    assert_eq!(incoming[0].thread.latest_trace_id,Some(c));
    assert_eq!(incoming[0].thread.generation_status,"PENDING");
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![source,a,b,c]);
    for replacement in [None,Some(d)] {
        let payload=serde_json::from_value(serde_json::json!({
            "continues_trace_id":replacement,"expected_version_integer":attached.version_integer
        })).unwrap();
        let error=crate::entities_v2::trace::routes::put_trace_route(
            axum::extract::Extension(f.pool.clone()),axum::extract::Extension(session.clone()),
            axum::extract::Path(a),axum::extract::Json(payload)).await.err().unwrap();
        assert_eq!(error.status_code,400);
    }
    // Adding another predecessor keeps one thread, regardless of link creation order.
    let count=threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1;
    let (p,_)=f.publication();
    let (q,_)=f.publication();
    let mut upstream=Trace::find_full_trace(p,&f.pool).unwrap();
    upstream.continues_trace_id=Some(q);
    upstream.update(&f.pool).unwrap();
    let upstream_id=threads::list(f.owner,None,Some(q),"",0,20,&f.pool).unwrap().0[0].thread.id;
    assert_eq!(threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1,count+1);
    let mut join=Trace::find_full_trace(source,&f.pool).unwrap();
    join.continues_trace_id=Some(p);
    join.update(&f.pool).unwrap();
    assert_eq!(threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1,count);
    assert!(threads::view(f.owner,upstream_id,&f.pool).is_err());
    assert_eq!(threads::traces(f.owner,thread,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![q,p,source,a,b,c]);
    assert_eq!(threads::traces(f.owner,branch_id,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![b,d,end.id]);
    assert_cache(&f);

    // Reproduce the old split, then apply the production repair migration.
    let base=threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1;
    f.pool.get().unwrap().batch_execute(include_str!("../../../../migrations/2026-10-14-110000_merge_writing_thread_chains/down.sql")).unwrap();
    let (m,_)=f.publication();
    let (n,_)=f.publication();
    let (o,_)=f.publication();
    let mut tail=Trace::find_full_trace(n,&f.pool).unwrap();tail.continues_trace_id=Some(m);tail.update(&f.pool).unwrap();
    let retained=threads::list(f.owner,None,Some(m),"",0,20,&f.pool).unwrap().0[0].thread.id;
    let mut head=Trace::find_full_trace(m,&f.pool).unwrap();head.continues_trace_id=Some(o);head.update(&f.pool).unwrap();
    assert_eq!(threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1,base+2);
    f.pool.get().unwrap().batch_execute(include_str!("../../../../migrations/2026-10-14-110000_merge_writing_thread_chains/up.sql")).unwrap();
    assert_eq!(threads::list(f.owner,None,None,"",0,20,&f.pool).unwrap().1,base+1);
    assert_eq!(threads::traces(f.owner,retained,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![o,m,n]);
    assert_eq!(threads::view(f.owner,retained,&f.pool).unwrap().thread.generation_status,"PENDING");
    assert_eq!(threads::traces(f.owner,branch_id,0,20,&f.pool).unwrap().0.iter().map(|t|t.id).collect::<Vec<_>>(),vec![b,d,end.id]);
    assert_cache(&f);
    let mut shared=Trace::find_full_trace(b,&f.pool).unwrap();
    shared.title="café 📚".into();shared.content.push_str(" Écriture et réflexion.");
    shared=shared.update(&f.pool).unwrap();assert_cache(&f);
    let characters=threads::view(f.owner,branch_id,&f.pool).unwrap().character_count;
    shared.is_encrypted=true;shared.encryption_metadata=Some(serde_json::json!({}));
    shared=shared.update(&f.pool).unwrap();assert_cache(&f);
    assert_eq!(threads::view(f.owner,branch_id,&f.pool).unwrap().character_count,
        characters-(shared.title.chars().count()+shared.content.chars().count()) as i64);
    shared.is_encrypted=false;shared.encryption_metadata=None;shared.update(&f.pool).unwrap();assert_cache(&f);
    let barrier=Arc::new(Barrier::new(2));
    let jobs=[b,c].into_iter().map(|id| {
        let pool=f.pool.clone();let barrier=barrier.clone();
        std::thread::spawn(move|| {
            let mut trace=Trace::find_full_trace(id,&pool).unwrap();
            barrier.wait();trace.content.push_str(" Concurrent edit.");trace.update(&pool).unwrap();
        })
    }).collect::<Vec<_>>();
    for job in jobs { job.join().unwrap(); }
    assert_cache(&f);
    // Backfill from the graph repairs deliberately damaged caches without new AI calls.
    let revision=threads::view(f.owner,thread,&f.pool).unwrap().thread.input_revision;
    f.pool.get().unwrap().batch_execute(&format!("DELETE FROM writing_thread_traces WHERE thread_id='{thread}';
        UPDATE writing_threads SET trace_count=0,character_count=0,last_activity_at=NULL WHERE id='{thread}';
        SELECT rebuild_writing_thread_memberships('{thread}');")).unwrap();
    assert_cache(&f);
    assert_eq!(threads::view(f.owner,thread,&f.pool).unwrap().thread.input_revision,revision);
    f.pool.get().unwrap().batch_execute(&format!("DELETE FROM traces WHERE id='{m}';")).unwrap();
    assert_cache(&f);
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn my_feed_batches_seen_previews_and_private_trace_seen_by_is_empty() {
    use crate::entities_v2::{
        feed::hydrate::{find_my_feed_items_paginated,find_feed_items_paginated},
        session::Session, trace::routes::get_trace_seen_by_route, user_post_state::UserPostState,
    };
    use axum::extract::{Extension,Path};
    let f=Fixture::new();
    let (published,post)=f.publication();
    let (private,private_post)=f.publication();
    let (staged,staged_post)=f.publication();
    for pid in [post,staged_post] {
        PostGrant::create_or_update(&Post::find_full(pid,&f.pool).unwrap(),f.owner,NewPostGrantDto {
            grantee_user_id:Some(f.reader),grantee_scope:None,access_level:None,
        },&f.pool).unwrap();
        UserPostState::create_or_mark_seen(f.reader,pid,&f.pool).unwrap();
    }
    UserPostState::create_or_mark_seen(f.owner,post,&f.pool).unwrap();
    f.pool.get().unwrap().batch_execute(&format!("DELETE FROM posts WHERE id='{private_post}'; UPDATE posts SET status='DRAFT' WHERE id='{staged_post}';")).unwrap();
    let (items,total)=find_my_feed_items_paginated(f.owner,0,20,&f.pool).unwrap();
    assert_eq!(total,3);
    let preview=items.iter().find(|i|i.source_id==published).unwrap().seen_by_preview.as_ref().unwrap();
    assert_eq!(preview.seen_count,1);
    assert_eq!(preview.users[0].user_id,f.reader);
    for id in [private,staged] {
        let preview=items.iter().find(|i|i.source_id==id).unwrap().seen_by_preview.as_ref().unwrap();
        assert_eq!(preview.seen_count,0);
        assert!(preview.users.is_empty());
        let mut session=Session::anonymous();session.user_id=Some(f.owner);
        assert!(get_trace_seen_by_route(Extension(f.pool.clone()),Extension(session),Path(id)).await.unwrap().0.is_empty());
    }
    let mut stranger=Session::anonymous();stranger.user_id=Some(f.stranger);
    assert!(get_trace_seen_by_route(Extension(f.pool.clone()),Extension(stranger),Path(private)).await.is_err());
    let (shared,_)=find_feed_items_paginated(f.reader,None,0,20,&f.pool).unwrap();
    assert!(shared.iter().all(|i|i.seen_by_preview.is_none()));
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn message_sends_require_an_accepted_relationship_except_for_mentors() {
    use crate::entities_v2::message::routes::ensure_can_send_message;
    let f = Fixture::new();
    assert!(ensure_can_send_message(f.reader, f.owner, &f.pool).is_ok());
    assert!(ensure_can_send_message(f.owner, f.reader, &f.pool).is_ok());
    assert_eq!(ensure_can_send_message(f.owner, f.stranger, &f.pool).unwrap_err().status_code, 403);
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE relationships SET status='PENDING' WHERE requester_user_id='{}' AND target_user_id='{}';",
        f.reader, f.owner)).unwrap();
    assert!(ensure_can_send_message(f.reader, f.owner, &f.pool).is_err());
    assert!(ensure_can_send_message(f.owner, f.reader, &f.pool).is_err());
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO user_roles(user_id,role) VALUES('{}','MENTOR');", f.stranger)).unwrap();
    assert!(ensure_can_send_message(f.owner, f.stranger, &f.pool).is_ok());
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{}','{}');", f.stranger, f.owner)).unwrap();
    assert!(ensure_can_send_message(f.owner, f.stranger, &f.pool).is_err());
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn overlapping_history_adds_sources_and_reviews_only_new_access() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Original", &f.pool).unwrap();
    let b = persist::create(f.owner, "Overlapping", &f.pool).unwrap();
    let c = persist::create(f.owner, "Mixed", &f.pool).unwrap();
    let (_, shared) = f.publication();
    let (_, private) = f.publication();
    for audience in [&a, &b, &c] {
        persist::attach(f.owner, audience.id, shared, &f.pool).unwrap();
    }
    persist::attach(f.owner, c.id, private, &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::apply_history(f.owner, a.id, f.reader, HistoryDecisionDto {
        decision: JournalHistoryDecision::AllIncludingSensitive, post_ids: None,
    }, &f.pool).unwrap();

    let (member, notify) = persist::add_member(f.owner, b.id, f.reader, true, &f.pool).unwrap();
    assert!(!notify);
    assert_eq!(member.history_review_state, "reviewed");
    let summary = persist::history_summary(f.owner, b.id, f.reader, &f.pool).unwrap();
    assert_eq!(summary.eligible_trace_count, 1);
    assert_eq!(summary.already_accessible_trace_count, 1);
    assert_eq!(summary.newly_shareable_trace_count, 0);
    assert!(!summary.review_required);
    assert_eq!(f.grant(shared).sources.iter().filter(|s| s.status == "active").count(), 2);
    persist::remove_member(f.owner, a.id, f.reader, true, true, &f.pool).unwrap();
    assert!(f.can_read(shared), "The overlapping audience must preserve access");

    let (_, notify) = persist::add_member(f.owner, c.id, f.reader, true, &f.pool).unwrap();
    assert!(notify);
    let summary = persist::history_summary(f.owner, c.id, f.reader, &f.pool).unwrap();
    assert_eq!(summary.eligible_trace_count, 2);
    assert_eq!(summary.already_accessible_trace_count, 1);
    assert_eq!(summary.newly_shareable_normal_trace_count, 1);
    assert!(summary.review_required);
    assert!(!f.can_read(private));
    persist::remove_member(f.owner, b.id, f.reader, true, true, &f.pool).unwrap();
    assert!(f.can_read(shared));
    persist::apply_history(f.owner, c.id, f.reader, HistoryDecisionDto {
        decision: JournalHistoryDecision::None, post_ids: None,
    }, &f.pool).unwrap();
    assert!(f.can_read(shared), "Declining new history must preserve automatically added sources");
    assert!(!f.can_read(private));
    persist::remove_member(f.owner, c.id, f.reader, true, true, &f.pool).unwrap();
    assert!(!f.can_read(shared));
    assert!(persist::history_summary(f.stranger, c.id, f.reader, &f.pool).is_err());
}

#[test]
fn removed_audience_role_is_rejected_instead_of_silently_broadening_public_links() {
    use crate::entities_v2::post::{routes::PutTracePostDto, NewPostDto};
    assert!(serde_json::from_str::<PutTracePostDto>(r#"{"status":"published"}"#).is_ok());
    for role in [r#""restricted""#, r#""default""#, "null"] {
        let payload = format!(r#"{{"status":"published","audience_role":{role}}}"#);
        assert!(serde_json::from_str::<PutTracePostDto>(&payload).is_err());
        assert!(serde_json::from_str::<NewPostDto>(&payload).is_err());
    }
}

#[test]
fn member_removal_defaults_are_future_only() {
    let query: RemoveMemberQuery = serde_json::from_str("{}").unwrap();
    assert!(query.stop_future);
    assert!(!query.revoke_history);
    let query: RemoveMemberQuery =
        serde_json::from_str(r#"{"stop_future":false,"revoke_history":true}"#).unwrap();
    assert!(!query.stop_future);
    assert!(query.revoke_history);
}

struct Fixture {
    pool: DbPool,
    owner: Uuid,
    reader: Uuid,
    stranger: Uuid,
    journal: Uuid,
}

impl Fixture {
    fn new() -> Self {
        let url = std::env::var("AUDIENCE_TEST_DATABASE_URL").expect(
            "Set AUDIENCE_TEST_DATABASE_URL to a disposable migrated audience_test database",
        );
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(
            parsed.path(),
            "/audience_test",
            "Integration fixtures must only use the disposable audience_test database"
        );
        let pool = DbPool::builder()
            .max_size(8)
            .build(ConnectionManager::new(url))
            .unwrap();
        let owner = Uuid::new_v4();
        let reader = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let journal = Uuid::new_v4();
        pool.get().unwrap().batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at)
            VALUES('{owner}','{owner}@test.invalid','Owner','Test','@{owner}','unused',NOW()),
                ('{reader}','{reader}@test.invalid','Reader','Test','@{reader}','unused',NOW()),
                ('{stranger}','{stranger}@test.invalid','Stranger','Test','@{stranger}','unused',NOW());
            INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status) VALUES('{journal}','{owner}','Test','','','USER_JOURNAL','ACTIVE');
            INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
            VALUES('{reader}','{owner}','FOLLOW','ACCEPTED',NOW());")).unwrap();
        Self {
            pool,
            owner,
            reader,
            stranger,
            journal,
        }
    }

    fn publication(&self) -> (Uuid, Uuid) {
        let trace = Uuid::new_v4();
        let post = Uuid::new_v4();
        self.pool.get().unwrap().batch_execute(&format!("INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
            VALUES('{trace}','{}','{}','Test','','Content',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW());
            INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
            VALUES('{post}','{}','{trace}','','','','OUTPUT','IDEA','PUBLISHED',NOW());",self.owner,self.journal,self.owner)).unwrap();
        (trace, post)
    }

    fn grant(&self, post: Uuid) -> PostGrant {
        PostGrant::find_for_post_paginated(post, 0, 100, &self.pool)
            .unwrap()
            .0
            .into_iter()
            .find(|g| g.grantee_user_id == Some(self.reader))
            .unwrap()
    }

    fn can_read(&self, post: Uuid) -> bool {
        PostGrant::user_can_read_post(
            &Post::find_full(post, &self.pool).unwrap(),
            self.reader,
            &self.pool,
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Ok(mut conn) = self.pool.get() {
            let _ = diesel::sql_query("DELETE FROM users WHERE id=ANY($1)")
                .bind::<Array<SqlUuid>, _>(vec![self.owner, self.reader, self.stranger])
                .execute(&mut conn);
        }
    }
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn trace_albums_route_respects_trace_and_album_access_and_pagination() {
    use crate::{
        entities_v2::{
            album::{routes::get_trace_albums_route, Album},
            session::Session,
        },
        pagination::{PaginatedResponse, PaginationParams},
    };
    use axum::{
        extract::{Extension, Path, Query},
        Json,
    };
    let f = Fixture::new();
    let (tid, pid) = f.publication();
    let private = Uuid::new_v4();
    let published = Uuid::new_v4();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO albums(id,owner_user_id,title,visibility) VALUES
            ('{private}','{}','Private album','PRIVATE'),
            ('{published}','{}','Published album','PUBLISHED');
         INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES
            ('{private}','{tid}',0),('{published}','{tid}',0);",
            f.owner, f.owner
        ))
        .unwrap();
    async fn page(
        f: &Fixture,
        user: Uuid,
        trace: Uuid,
        offset: i64,
        limit: i64,
    ) -> Result<Json<PaginatedResponse<Album>>, crate::entities_v2::error::PpdcError> {
        let mut session = Session::anonymous();
        session.user_id = Some(user);
        get_trace_albums_route(
            Extension(f.pool.clone()),
            Extension(session),
            Path(trace),
            Query(PaginationParams::from_parts(Some(offset), Some(limit))),
        )
        .await
    }
    let first = page(&f, f.owner, tid, 0, 1).await.unwrap().0;
    let second = page(&f, f.owner, tid, 1, 1).await.unwrap().0;
    assert_eq!(first.total, 2);
    assert_eq!(first.items.len(), 1);
    assert_eq!(second.items.len(), 1);
    assert_ne!(first.items[0].id, second.items[0].id);
    assert_eq!(
        page(&f, f.reader, tid, 0, 20)
            .await
            .err()
            .unwrap()
            .status_code,
        401
    );
    PostGrant::create_or_update(
        &Post::find_full(pid, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    let reader = page(&f, f.reader, tid, 0, 20).await.unwrap().0;
    assert_eq!(reader.total, 1);
    assert_eq!(reader.items[0].id, published);
    assert_eq!(
        page(&f, f.stranger, tid, 0, 20)
            .await
            .err()
            .unwrap()
            .status_code,
        401
    );
    let (empty, _) = f.publication();
    assert_eq!(page(&f, f.owner, empty, 0, 20).await.unwrap().0.total, 0);
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn documents_read_access_follows_posts_and_trace_attachments() {
    use crate::entities_v2::document::{
        routes::{get_document_route, put_document_route},
        Document, DocumentRole, UpdateDocumentDto,
    };
    use crate::entities_v2::session::Session;
    use axum::{
        extract::{Extension, Path},
        Json,
    };
    let f = Fixture::new();
    let shared = Uuid::new_v4();
    let private = Uuid::new_v4();
    let standalone = Uuid::new_v4();
    let document_post = Uuid::new_v4();
    let (trace_a, post_a) = f.publication();
    let (trace_b, post_b) = f.publication();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO documents(id,owner_user_id,document_role,content_source,title) VALUES
            ('{shared}','{}','REFERENCE','REFERENCE_ONLY','Shared'),
            ('{private}','{}','CREATION','REFERENCE_ONLY','Private'),
            ('{standalone}','{}','CREATION','REFERENCE_ONLY','Published');
         INSERT INTO trace_attachments(trace_id,document_id,attachment_name) VALUES
            ('{trace_a}','{shared}','Reference'),('{trace_b}','{shared}','Reference');
         INSERT INTO posts(id,user_id,source_document_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
            VALUES('{document_post}','{}','{standalone}','','','','OUTPUT','IDEA','PUBLISHED',NOW());",
        f.owner, f.owner, f.owner, f.owner
    )).unwrap();
    let list = |viewer, role, offset, limit| {
        Document::find_for_user_filtered_paginated(
            viewer, f.owner, role, None, None, None, offset, limit, &f.pool,
        )
        .unwrap()
    };
    assert_eq!(list(f.owner, None, 0, 20).1, 3);
    assert_eq!(list(f.reader, None, 0, 20).1, 0);
    for post in [post_a, post_b, document_post] {
        PostGrant::create_or_update(
            &Post::find_full(post, &f.pool).unwrap(),
            f.owner,
            NewPostGrantDto {
                grantee_user_id: Some(f.reader),
                grantee_scope: None,
                access_level: None,
            },
            &f.pool,
        )
        .unwrap();
    }
    let first = list(f.reader, None, 0, 1);
    let second = list(f.reader, None, 1, 1);
    assert_eq!(first.1, 2, "Two attachments must not duplicate a document");
    assert_ne!(first.0[0].id, second.0[0].id);
    assert_eq!(list(f.reader, Some(DocumentRole::Reference), 0, 20).1, 1);
    assert_eq!(list(f.stranger, None, 0, 20).1, 0);
    let mut session = Session::anonymous();
    session.user_id = Some(f.reader);
    assert!(get_document_route(
        Extension(f.pool.clone()),
        Extension(session.clone()),
        Path(shared)
    )
    .await
    .is_ok());
    assert!(get_document_route(
        Extension(f.pool.clone()),
        Extension(session.clone()),
        Path(standalone)
    )
    .await
    .is_ok());
    assert_eq!(
        get_document_route(
            Extension(f.pool.clone()),
            Extension(session.clone()),
            Path(private)
        )
        .await
        .err()
        .unwrap()
        .status_code,
        401
    );
    assert_eq!(
        put_document_route(
            Extension(f.pool.clone()),
            Extension(session),
            Path(shared),
            Json(UpdateDocumentDto::default())
        )
        .await
        .err()
        .unwrap()
        .status_code,
        401
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id IN ('{post_a}','{post_b}');"
        ))
        .unwrap();
    assert_eq!(list(f.reader, None, 0, 20).1, 1);
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO trace_mentions(trace_id,mentioned_user_id) VALUES('{trace_a}','{}');",
            f.reader
        ))
        .unwrap();
    assert_eq!(
        list(f.reader, None, 0, 20).1,
        2,
        "Mention-only trace access also shares attachments"
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_encrypted=TRUE WHERE id='{trace_a}';"
        ))
        .unwrap();
    assert_eq!(list(f.reader, None, 0, 20).1, 1);
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET is_encrypted=FALSE,status='DRAFT' WHERE id='{trace_a}';"
        ))
        .unwrap();
    assert_eq!(list(f.reader, None, 0, 20).1, 1);
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{}','{}');",
            f.owner, f.reader
        ))
        .unwrap();
    assert_eq!(list(f.reader, None, 0, 20).1, 0);
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn content_counts_track_viewer_acl_and_oldest_finalization() {
    use crate::entities_v2::{session::Session, user::get_user_content_counts_route};
    use axum::extract::{Extension, Path};
    let f = Fixture::new();
    let (private, _) = f.publication();
    let (mentioned, _) = f.publication();
    let (trace_a, post_a) = f.publication();
    let (trace_b, post_b) = f.publication();
    let doc = Uuid::new_v4();
    let album_a = Uuid::new_v4();
    let album_b = Uuid::new_v4();
    let album_private = Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET finalized_at='2026-01-01',is_encrypted=TRUE WHERE id='{private}';
         UPDATE traces SET finalized_at='2026-02-01' WHERE id='{mentioned}';
         UPDATE traces SET finalized_at='2026-03-01' WHERE id='{trace_a}';
         UPDATE traces SET finalized_at='2026-05-01' WHERE id='{trace_b}';
         INSERT INTO traces(user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at)
             VALUES('{}','{}','Draft','','',NOW(),'USER_TRACE','DRAFT',NOW());
         INSERT INTO documents(id,owner_user_id,document_role,content_source,title)
             VALUES('{doc}','{}','REFERENCE','REFERENCE_ONLY','Shared');
         INSERT INTO documents(owner_user_id,document_role,content_source,title,status) VALUES
             ('{}','CREATION','REFERENCE_ONLY','Private','ACTIVE'),
             ('{}','CREATION','REFERENCE_ONLY','Archived','ARCHIVED');
         INSERT INTO trace_attachments(trace_id,document_id,attachment_name) VALUES
             ('{trace_a}','{doc}','Reference'),('{trace_b}','{doc}','Reference');
         INSERT INTO albums(id,owner_user_id,title,visibility) VALUES
             ('{album_a}','{}','First','PUBLISHED'),('{album_b}','{}','Second','PUBLISHED'),
             ('{album_private}','{}','Private','PRIVATE');
         INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES
             ('{album_a}','{trace_a}',0),('{album_b}','{trace_b}',0),('{album_private}','{trace_a}',0);",
        f.owner, f.journal, f.owner, f.owner, f.owner, f.owner, f.owner, f.owner
    )).unwrap();
    async fn counts(f: &Fixture, viewer: Uuid) -> crate::entities_v2::user::UserContentCounts {
        let mut session = Session::anonymous();
        session.user_id = Some(viewer);
        get_user_content_counts_route(Extension(f.pool.clone()), Extension(session), Path(f.owner))
            .await
            .unwrap()
            .0
    }
    let empty = counts(&f, f.reader).await;
    assert_eq!(
        (
            empty.traces_count,
            empty.albums_count,
            empty.documents_count
        ),
        (0, 0, 0)
    );
    assert!(empty.oldest_trace_finalized_at.is_none());
    let owner = counts(&f, f.owner).await;
    assert_eq!(
        (
            owner.traces_count,
            owner.albums_count,
            owner.documents_count
        ),
        (4, 3, 2)
    );
    assert_eq!(
        owner.oldest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-01-01T00:00:00+00:00"
    );
    for post in [post_a, post_b] {
        PostGrant::create_or_update(
            &Post::find_full(post, &f.pool).unwrap(),
            f.owner,
            NewPostGrantDto {
                grantee_user_id: Some(f.reader),
                grantee_scope: None,
                access_level: None,
            },
            &f.pool,
        )
        .unwrap();
    }
    Trace::create_complement(
        trace_a,
        f.owner,
        String::new(),
        String::new(),
        "Excluded complement".to_string(),
        TraceComplementAudienceMode::Parent,
        &[],
        &f.pool,
    )
    .unwrap();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO trace_mentions(trace_id,mentioned_user_id) VALUES('{mentioned}','{}');",
            f.reader
        ))
        .unwrap();
    let shared = counts(&f, f.reader).await;
    assert_eq!(
        (
            shared.traces_count,
            shared.albums_count,
            shared.documents_count
        ),
        (3, 2, 1)
    );
    assert_eq!(
        shared.oldest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-02-01T00:00:00+00:00"
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='ARCHIVED' WHERE id='{mentioned}';
         UPDATE post_grants SET status='REVOKED' WHERE post_id='{post_a}';"
        ))
        .unwrap();
    let revoked = counts(&f, f.reader).await;
    assert_eq!(
        (
            revoked.traces_count,
            revoked.albums_count,
            revoked.documents_count
        ),
        (1, 1, 1)
    );
    assert_eq!(
        revoked.oldest_trace_finalized_at.unwrap().to_rfc3339(),
        "2026-05-01T00:00:00+00:00"
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_blocks(blocker_user_id,blocked_user_id) VALUES('{}','{}');",
            f.owner, f.reader
        ))
        .unwrap();
    let blocked = counts(&f, f.reader).await;
    assert_eq!(
        (
            blocked.traces_count,
            blocked.albums_count,
            blocked.documents_count
        ),
        (0, 0, 0)
    );
    assert!(blocked.oldest_trace_finalized_at.is_none());
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn feed_merges_album_creation_cards_without_seen_tracking_or_duplicates() {
    use crate::entities_v2::feed::{
        hydrate::{count_recent_unread_feed_items, find_feed_items_paginated},
        model::FeedSourceKind,
    };
    use chrono::NaiveDateTime;
    let f = Fixture::new();
    let (trace_a, post_a) = f.publication();
    let (_, post_b) = f.publication();
    let (hidden_trace, _) = f.publication();
    let album = Uuid::new_v4();
    let private_album = Uuid::new_v4();
    let archived_album = Uuid::new_v4();
    let hidden_album = Uuid::new_v4();
    let album_post = Uuid::new_v4();
    let hidden_album_post = Uuid::new_v4();
    let document = Uuid::new_v4();
    let document_post = Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE posts SET publishing_date='2026-10-05' WHERE id='{post_a}';
         UPDATE posts SET publishing_date='2026-10-01' WHERE id='{post_b}';
         INSERT INTO albums(id,owner_user_id,title,visibility,completion_status,created_at,updated_at) VALUES
            ('{album}','{}','Album','PUBLISHED','IN_PROGRESS','2026-10-04','2026-10-10'),
            ('{private_album}','{}','Private','PRIVATE','IN_PROGRESS','2026-10-09','2026-10-09'),
            ('{archived_album}','{}','Archived','PUBLISHED','ARCHIVED','2026-10-09','2026-10-09'),
            ('{hidden_album}','{}','Hidden','PUBLISHED','IN_PROGRESS','2026-10-09','2026-10-09');
         INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES
            ('{album}','{trace_a}',0),('{private_album}','{trace_a}',0),
            ('{archived_album}','{trace_a}',0),('{hidden_album}','{hidden_trace}',0);
         INSERT INTO documents(id,owner_user_id,document_role,content_source,title)
            VALUES('{document}','{}','CREATION','REFERENCE_ONLY','Document');
         INSERT INTO posts(id,user_id,source_album_id,source_document_id,title,subtitle,content,interaction_type,post_type,status,publishing_date) VALUES
            ('{album_post}','{}','{album}',NULL,'','','','OUTPUT','IDEA','PUBLISHED','2026-10-09'),
            ('{hidden_album_post}','{}','{hidden_album}',NULL,'','','','OUTPUT','IDEA','PUBLISHED','2026-10-09'),
            ('{document_post}','{}',NULL,'{document}','','','','OUTPUT','IDEA','PUBLISHED','2026-10-03');",
        f.owner, f.owner, f.owner, f.owner, f.owner, f.owner, f.owner, f.owner
    )).unwrap();
    for post in [post_a, post_b, album_post, hidden_album_post, document_post] {
        PostGrant::create_or_update(
            &Post::find_full(post, &f.pool).unwrap(),
            f.owner,
            NewPostGrantDto {
                grantee_user_id: Some(f.reader),
                grantee_scope: None,
                access_level: None,
            },
            &f.pool,
        )
        .unwrap();
    }
    let (items, total) = find_feed_items_paginated(f.reader, None, 0, 20, &f.pool).unwrap();
    assert_eq!(total, 4);
    assert_eq!(
        items.iter().map(|item| item.source_id).collect::<Vec<_>>(),
        vec![
            trace_a,
            album,
            document,
            Post::find_full(post_b, &f.pool)
                .unwrap()
                .source_trace_id
                .unwrap()
        ]
    );
    let card = &items[1];
    assert_eq!(card.source_kind, FeedSourceKind::Album);
    assert!(card.post_id.is_none());
    assert!(card.publishing_date.is_none());
    assert_eq!(card.feed_date.to_string(), "2026-10-04 00:00:00");
    assert_eq!(card.updated_at.to_string(), "2026-10-10 00:00:00");
    let (page, page_total) = find_feed_items_paginated(f.reader, None, 1, 1, &f.pool).unwrap();
    assert_eq!(page_total, 4);
    assert_eq!(page[0].source_id, album);
    assert_eq!(
        find_feed_items_paginated(f.reader, None, 50, 1, &f.pool)
            .unwrap()
            .1,
        4
    );
    assert_eq!(
        find_feed_items_paginated(f.stranger, None, 0, 20, &f.pool)
            .unwrap()
            .1,
        0
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_post_states(user_id,post_id,first_seen_at,last_seen_at)
            VALUES('{}','{post_b}',NOW(),NOW());",
            f.reader
        ))
        .unwrap();
    let (unseen, unseen_total) =
        find_feed_items_paginated(f.reader, Some(false), 0, 20, &f.pool).unwrap();
    assert_eq!(unseen_total, 2);
    assert!(unseen
        .iter()
        .all(|item| item.source_kind != FeedSourceKind::Album));
    assert_eq!(
        find_feed_items_paginated(f.reader, Some(true), 0, 20, &f.pool)
            .unwrap()
            .1,
        1
    );
    let cutoff = "2026-10-01T00:00:00".parse::<NaiveDateTime>().unwrap();
    assert_eq!(
        count_recent_unread_feed_items(f.reader, cutoff, &f.pool).unwrap(),
        2
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id='{album_post}';"
        ))
        .unwrap();
    assert_eq!(
        find_feed_items_paginated(f.reader, None, 0, 20, &f.pool)
            .unwrap()
            .1,
        4,
        "Album cards require child access, not a backing album post grant"
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE post_grants SET status='REVOKED' WHERE post_id='{post_a}';"
        ))
        .unwrap();
    assert_eq!(
        find_feed_items_paginated(f.reader, None, 0, 20, &f.pool)
            .unwrap()
            .1,
        2
    );
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn own_feed_includes_private_traces_and_albums_without_changing_trace_collection() {
    use crate::entities_v2::{
        feed::{get_my_feed_route, model::FeedSourceKind},
        session::Session,
    };
    use crate::pagination::PaginationParams;
    use axum::extract::{Extension, Query};
    let f = Fixture::new();
    let (published, published_post) = f.publication();
    let private_trace = Uuid::new_v4();
    let private_album = Uuid::new_v4();
    let published_album = Uuid::new_v4();
    let archived_album = Uuid::new_v4();
    let foreign_album = Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET finalized_at='2026-10-05' WHERE id='{published}';
         UPDATE posts SET publishing_date='2026-10-10' WHERE id='{published_post}';
         INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at) VALUES
            ('{private_trace}','{}','{}','Private','','Owner only','2026-01-01','USER_TRACE','FINALIZED',NOW(),'2026-10-01');
         INSERT INTO traces(user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at)
            VALUES('{}','{}','Draft','','',NOW(),'USER_TRACE','DRAFT',NOW());
         INSERT INTO albums(id,owner_user_id,title,visibility,completion_status,created_at,updated_at) VALUES
            ('{private_album}','{}','Private album','PRIVATE','IN_PROGRESS','2026-10-04','2026-10-11'),
            ('{published_album}','{}','Published album','PUBLISHED','COMPLETE','2026-10-03','2026-10-03'),
            ('{archived_album}','{}','Archived album','PRIVATE','ARCHIVED','2026-10-09','2026-10-09'),
            ('{foreign_album}','{}','Other user','PRIVATE','IN_PROGRESS','2026-10-09','2026-10-09');",
        f.owner, f.journal, f.owner, f.journal, f.owner, f.owner, f.owner, f.reader
    )).unwrap();
    Trace::create_complement(
        published,
        f.owner,
        String::new(),
        String::new(),
        "Excluded complement".to_string(),
        TraceComplementAudienceMode::Parent,
        &[],
        &f.pool,
    )
    .unwrap();
    async fn page(
        f: &Fixture,
        user: Uuid,
        offset: i64,
        limit: i64,
    ) -> crate::pagination::PaginatedResponse<crate::entities_v2::feed::FeedItem> {
        let mut session = Session::anonymous();
        session.user_id = Some(user);
        get_my_feed_route(
            Extension(f.pool.clone()),
            Extension(session),
            Query(PaginationParams::from_parts(Some(offset), Some(limit))),
        )
        .await
        .unwrap()
        .0
    }
    let all = page(&f, f.owner, 0, 20).await;
    assert_eq!(all.total, 4);
    assert_eq!(
        all.items
            .iter()
            .map(|item| item.source_id)
            .collect::<Vec<_>>(),
        vec![published, private_album, published_album, private_trace]
    );
    assert_eq!(all.items[0].post_id, Some(published_post));
    assert_eq!(
        all.items[0].feed_date.to_string(),
        "2026-10-05 00:00:00",
        "Owned traces use finalization, not their publication date"
    );
    assert!(all.items[1].post_id.is_none());
    assert_eq!(all.items[1].source_kind, FeedSourceKind::Album);
    assert!(all.items[3].post_id.is_none());
    assert_eq!(all.items[3].content, "Owner only");
    assert!(all.items.iter().all(|item| item.owner_user_id == f.owner));
    let second = page(&f, f.owner, 1, 1).await;
    assert_eq!(second.total, 4);
    assert_eq!(second.items[0].source_id, private_album);
    assert!(page(&f, f.owner, 100, 1).await.items.is_empty());
    let other = page(&f, f.reader, 0, 20).await;
    assert_eq!(other.total, 1);
    assert_eq!(other.items[0].source_id, foreign_album);
    let (_, trace_total) = Trace::find_list_items_for_owner_paginated(
        f.owner,
        0,
        20,
        crate::entities_v2::trace::TraceStatus::Finalized,
        None,
        None,
        None,
        None,
        &f.pool,
    )
    .unwrap();
    assert_eq!(
        trace_total, 2,
        "The existing trace collection still contains traces only"
    );
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn chronological_album_ranges_use_visible_finalization_dates() {
    use crate::entities_v2::{
        album::{routes::get_album_route, Album},
        feed::hydrate::find_feed_items_paginated,
        session::Session,
    };
    use axum::extract::{Extension, Path};
    let f = Fixture::new();
    let (hidden, _) = f.publication();
    let (late, late_post) = f.publication();
    let (early, early_post) = f.publication();
    let album = Uuid::new_v4();
    let empty = Uuid::new_v4();
    f.pool.get().unwrap().batch_execute(&format!(
        "UPDATE traces SET finalized_at='2026-01-01' WHERE id='{hidden}';
         UPDATE traces SET finalized_at='2026-05-01',interaction_date='2025-01-01' WHERE id='{late}';
         UPDATE traces SET finalized_at='2026-03-01',interaction_date='2027-01-01' WHERE id='{early}';
         INSERT INTO albums(id,owner_user_id,title,visibility) VALUES
             ('{album}','{}','Chronological','PUBLISHED'),('{empty}','{}','Empty','PRIVATE');
         INSERT INTO album_items(album_id,trace_id,ordering_index) VALUES
             ('{album}','{hidden}',0),('{album}','{late}',1),('{album}','{early}',2);", f.owner, f.owner
    )).unwrap();
    for post in [late_post, early_post] {
        PostGrant::create_or_update(
            &Post::find_full(post, &f.pool).unwrap(),
            f.owner,
            NewPostGrantDto {
                grantee_user_id: Some(f.reader),
                grantee_scope: None,
                access_level: None,
            },
            &f.pool,
        )
        .unwrap();
    }
    let mut session = Session::anonymous();
    session.user_id = Some(f.reader);
    let response = get_album_route(Extension(f.pool.clone()), Extension(session), Path(album))
        .await
        .unwrap()
        .0;
    assert_eq!(
        response
            .items
            .iter()
            .map(|i| i.trace.id)
            .collect::<Vec<_>>(),
        vec![early, late]
    );
    let reader_range = response.album.visible_trace_date_range.unwrap();
    assert_eq!(reader_range.from.to_rfc3339(), "2026-03-01T00:00:00+00:00");
    assert_eq!(reader_range.to.to_rfc3339(), "2026-05-01T00:00:00+00:00");
    let owner_album = Album::find(album, &f.pool)
        .unwrap()
        .with_visible_date_range(f.owner, &f.pool)
        .unwrap();
    assert_eq!(
        owner_album
            .visible_trace_date_range
            .unwrap()
            .from
            .to_rfc3339(),
        "2026-01-01T00:00:00+00:00"
    );
    assert!(Album::find(album, &f.pool)
        .unwrap()
        .with_visible_date_range(f.stranger, &f.pool)
        .unwrap()
        .visible_trace_date_range
        .is_none());
    assert!(Album::find(empty, &f.pool)
        .unwrap()
        .with_visible_date_range(f.owner, &f.pool)
        .unwrap()
        .visible_trace_date_range
        .is_none());
    let card = find_feed_items_paginated(f.reader, None, 0, 20, &f.pool)
        .unwrap()
        .0
        .into_iter()
        .find(|item| item.source_id == album)
        .unwrap();
    assert_eq!(card.visible_trace_date_range.unwrap(), reader_range);
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE albums SET ordering_mode='MANUAL' WHERE id='{album}';"
        ))
        .unwrap();
    assert!(Album::find(album, &f.pool)
        .unwrap()
        .with_visible_date_range(f.reader, &f.pool)
        .unwrap()
        .visible_trace_date_range
        .is_none());
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn mobile_update_policy_is_publicly_readable_but_admin_writable() {
    use crate::entities_v2::{platform_infra::platform_settings::*, session::Session};
    use axum::extract::{Extension, Json, Query};
    let f = Fixture::new();
    let read = || {
        get_mobile_version_policy_route(
            Extension(f.pool.clone()),
            Query(MobilePolicyQuery {
                platform: MobilePlatform::Ios,
            }),
        )
    };
    let default = read().await.unwrap().0;
    assert_eq!(default.policy.minimum_required.build, 0);
    assert!(default.updated_at.is_none());
    let mut member = Session::anonymous();
    member.user_id = Some(f.reader);
    assert_eq!(
        put_admin_mobile_update_policy_route(
            Extension(f.pool.clone()),
            Extension(member.clone()),
            Json(MobileUpdatePolicy::default())
        )
        .await
        .err()
        .unwrap()
        .status_code,
        403
    );
    assert_eq!(
        get_admin_mobile_update_policy_route(
            Extension(f.pool.clone()),
            Extension(Session::anonymous())
        )
        .await
        .err()
        .unwrap()
        .status_code,
        401
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO user_roles(user_id,role) VALUES('{}','ADMIN');",
            f.owner
        ))
        .unwrap();
    let mut admin = Session::anonymous();
    admin.user_id = Some(f.owner);
    let mut policy = MobileUpdatePolicy::default();
    policy.ios.minimum_required = MobileBuildVersion {
        version: "1.0.2".to_string(),
        build: 40,
    };
    policy.ios.recommended = MobileBuildVersion {
        version: "1.0.3".to_string(),
        build: 46,
    };
    policy.ios.store_url = Some("https://apps.apple.com/fr/app/hupo/id123".to_string());
    let saved = put_admin_mobile_update_policy_route(
        Extension(f.pool.clone()),
        Extension(admin.clone()),
        Json(policy.clone()),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(saved.updated_by, Some(f.owner));
    assert!(saved.updated_at.is_some());
    let public = read().await.unwrap().0;
    assert_eq!(public.policy.recommended.build, 46);
    assert!(!serde_json::to_value(public)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("updated_by"));
    let android = get_mobile_version_policy_route(
        Extension(f.pool.clone()),
        Query(MobilePolicyQuery {
            platform: MobilePlatform::Android,
        }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(android.policy.minimum_required.build, 0);
    policy.ios.minimum_required.build = 100;
    assert_eq!(
        put_admin_mobile_update_policy_route(
            Extension(f.pool.clone()),
            Extension(admin),
            Json(policy)
        )
        .await
        .err()
        .unwrap()
        .status_code,
        400
    );
    assert_eq!(read().await.unwrap().0.policy.minimum_required.build, 40);
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn shared_journal_follower_automation_uses_audiences_only() {
    let f = Fixture::new();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE journals SET sharing_mode='SHARED' WHERE id='{}';",
            f.journal
        ))
        .unwrap();
    let (_, historical) = f.publication();
    let journal = Journal::find_full(f.journal, &f.pool).unwrap();
    let pending =
        persist::create_missing_members_for_existing_followers(&journal, &f.pool).unwrap();
    assert_eq!(pending.len(), 1);
    let (_, aid, reader) = pending[0];
    assert_eq!(reader, f.reader);
    assert!(
        !f.can_read(historical),
        "Auto membership must not share history"
    );
    assert!(
        persist::create_missing_members_for_existing_followers(&journal, &f.pool)
            .unwrap()
            .is_empty()
    );
    let (_, future) = f.publication();
    assert!(!f.can_read(future));
    persist::attach(f.owner, aid, future, &f.pool).unwrap();
    assert!(f.can_read(future));
    persist::remove_member(f.owner, aid, reader, true, false, &f.pool).unwrap();
    let mut conn = f.pool.get().unwrap();
    assert!(
        persist::create_members_for_new_follower_pair_with_conn(f.owner, reader, &mut conn)
            .unwrap()
            .is_empty()
    );
    drop(conn);
    assert!(
        !persist::member(&mut f.pool.get().unwrap(), aid, reader)
            .unwrap()
            .future_access_enabled
    );
    persist::replace_defaults(f.owner, f.journal, &[], &f.pool).unwrap();
    assert!(
        persist::create_missing_members_for_existing_followers(&journal, &f.pool)
            .unwrap()
            .is_empty()
    );
    use crate::schema::journal_sharing_policies;
    let policies = journal_sharing_policies::table
        .filter(journal_sharing_policies::journal_id.eq(f.journal))
        .count()
        .get_result::<i64>(&mut f.pool.get().unwrap())
        .unwrap();
    assert_eq!(
        policies, 0,
        "No retired policies may be created by automation"
    );
}

#[tokio::test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
async fn public_links_use_sensitivity_independently_of_grants() {
    use crate::entities_v2::records::journal_share_link::{JournalShareLink, get_shared_journal_traces_route};
    use axum::extract::{Extension, Path, Query};
    let f = Fixture::new();
    let (tid, normal) = f.publication();
    assert!(
        !f.can_read(normal),
        "Public links are independent of individual grants"
    );
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        1
    );
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET sharing_sensitivity='SENSITIVE' WHERE id='{tid}';"
        ))
        .unwrap();
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        0
    );
    let journal = Journal::find_full(f.journal, &f.pool).unwrap();
    let scoped = JournalShareLink::create(&journal, f.owner,
        chrono::Utc::now() + chrono::Duration::hours(1), Some(normal), &f.pool).unwrap();
    let token = url::Url::parse(&scoped.url).unwrap().query_pairs()
        .find(|(key, _)| key == "token").unwrap().1.into_owned();
    let response = get_shared_journal_traces_route(Extension(f.pool.clone()), Path(scoped.id),
        Query(serde_json::from_value(serde_json::json!({"token": token})).unwrap())).await.unwrap().0;
    assert_eq!(response.items.len(), 1, "Explicit links can read sensitive writing");
    assert_eq!(response.items[0].id, tid);
    assert!(JournalShareLink::create(&journal, f.reader,
        chrono::Utc::now() + chrono::Duration::hours(1), Some(normal), &f.pool).is_err());
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET sharing_sensitivity='NORMAL',is_encrypted=TRUE WHERE id='{tid}';"
        ))
        .unwrap();
    let error = f
        .pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='PUBLISHED' WHERE id='{normal}';"
        ))
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("Source does not permit publication"));
    assert_eq!(
        Post::find_public_default_for_journal_paginated(f.journal, 0, 20, &f.pool)
            .unwrap()
            .1,
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn only_me_deletes_preparation_but_preserves_publication_history() {
    use crate::entities_v2::post::PostStatus;
    let f = Fixture::new();
    let (_, published) = f.publication();
    assert_eq!(
        Post::find_full(published, &f.pool)
            .unwrap()
            .delete_draft_trace_post(&f.pool)
            .unwrap_err()
            .status_code,
        400
    );
    let mut history = Post::find_full(published, &f.pool).unwrap();
    history.status = PostStatus::Draft;
    history = history.update(&f.pool).unwrap();
    assert_eq!(
        history
            .delete_draft_trace_post(&f.pool)
            .unwrap_err()
            .status_code,
        400
    );
    let (tid, prepared) = f.publication();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='DRAFT',publishing_date=NULL WHERE id='{prepared}';"
        ))
        .unwrap();
    let a = persist::create(f.owner, "Delete preparation", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::attach(f.owner, a.id, prepared, &f.pool).unwrap();
    assert_eq!(
        PostGrant::find_for_post_paginated(prepared, 0, 100, &f.pool)
            .unwrap()
            .1,
        1
    );
    Post::find_full(prepared, &f.pool)
        .unwrap()
        .delete_draft_trace_post(&f.pool)
        .unwrap();
    assert!(Post::find_for_trace_and_publisher(tid, f.owner, &f.pool)
        .unwrap()
        .is_none());
    assert_eq!(
        PostGrant::find_for_post_paginated(prepared, 0, 100, &f.pool)
            .unwrap()
            .1,
        0
    );
    assert!(
        Trace::find_full_trace(tid, &f.pool).is_ok(),
        "Deleting preparation must preserve the trace"
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn publication_uses_only_prepared_grants() {
    use crate::entities_v2::post::PostStatus;
    let f = Fixture::new();
    let a = persist::create(f.owner, "Prepared", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::replace_defaults(f.owner, f.journal, &[a.id], &f.pool).unwrap();
    let (tid, pid) = f.publication();
    assert!(
        !f.can_read(pid),
        "Publication never applies journal defaults"
    );
    assert!(persist::post_audiences(f.owner, pid, &f.pool)
        .unwrap()
        .is_empty());
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='DRAFT',finalized_at=NULL WHERE id='{tid}';
         UPDATE posts SET status='DRAFT',publishing_date=NULL WHERE id='{pid}';"
        ))
        .unwrap();
    let attached = persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    assert_eq!(attached.newly_shared_grant_count, 1);
    assert_eq!(f.grant(pid).audience_id, Some(a.id));
    assert!(!f.can_read(pid), "Prepared grants never expose drafts");
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='DRAFT',content='Autosaved draft' WHERE id='{tid}';"
        ))
        .unwrap();
    assert_eq!(
        Post::find_full(pid, &f.pool).unwrap().status,
        PostStatus::Draft,
        "Autosaving a draft must preserve its staged publication"
    );
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
         VALUES('{}','{}','FOLLOW','ACCEPTED',NOW());", f.stranger,f.owner
    )).unwrap();
    persist::add_member(f.owner, a.id, f.stranger, true, &f.pool).unwrap();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE traces SET status='FINALIZED',finalized_at=NOW() WHERE id='{tid}';"
        ))
        .unwrap();
    assert!(!f.can_read(pid));
    let mut post = Post::find_full(pid, &f.pool).unwrap();
    post.status = PostStatus::Published;
    post = post.update(&f.pool).unwrap();
    assert!(f.can_read(pid));
    assert_eq!(
        PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
            .unwrap()
            .1,
        1,
        "Publication must not expand audience membership"
    );
    PostGrant::revoke(pid, f.grant(pid).id, f.owner, &f.pool).unwrap();
    assert!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .is_empty(),
        "Removing the last audience-origin reader also removes its history scope"
    );
    assert!(!f.can_read(pid));
    post.status = PostStatus::Archived;
    post = post.update(&f.pool).unwrap();
    post.status = PostStatus::Published;
    post.update(&f.pool).unwrap();
    assert!(
        !f.can_read(pid),
        "Republishing never restores withdrawn grants"
    );
    persist::detach(f.owner, a.id, pid, true, &f.pool).unwrap();
    assert_eq!(
        PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
            .unwrap()
            .0
            .iter()
            .filter(|g| g.status == crate::entities_v2::post_grant::PostGrantStatus::Active)
            .count(),
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn audience_trace_counts_match_the_eligible_collection() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Counted", &f.pool).unwrap();
    let empty = persist::create(f.owner, "Empty", &f.pool).unwrap();
    assert_eq!(
        persist::with_trace_counts(vec![a.clone()], &f.pool).unwrap()[0].trace_count,
        0
    );
    assert!(persist::with_trace_counts(Vec::new(), &f.pool)
        .unwrap()
        .is_empty());

    let (_, published) = f.publication();
    persist::attach(f.owner, a.id, published, &f.pool).unwrap();
    let (_, staged) = f.publication();
    let (_, archived) = f.publication();
    let (encrypted_trace, encrypted_post) = f.publication();
    let (archived_trace, archived_trace_post) = f.publication();
    for pid in [staged, archived, encrypted_post, archived_trace_post] {
        persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    }
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "UPDATE posts SET status='DRAFT' WHERE id='{staged}';
         UPDATE posts SET status='ARCHIVED' WHERE id='{archived}';
         UPDATE traces SET is_encrypted=TRUE WHERE id='{encrypted_trace}';
         UPDATE traces SET status='ARCHIVED' WHERE id='{archived_trace}';"
        ))
        .unwrap();

    let (audiences, total) = persist::list(f.owner, false, 0, 20, &f.pool).unwrap();
    assert_eq!(total, 2);
    let views = persist::with_trace_counts(audiences, &f.pool).unwrap();
    assert_eq!(
        views
            .iter()
            .find(|v| v.audience.id == a.id)
            .unwrap()
            .trace_count,
        1
    );
    assert_eq!(
        views
            .iter()
            .find(|v| v.audience.id == empty.id)
            .unwrap()
            .trace_count,
        0
    );
    let listed_total = persist::list_posts(f.owner, a.id, None, 0, 20, &f.pool)
        .unwrap()
        .1;
    assert_eq!(listed_total, 1);
    let owned = persist::owned(&mut *f.pool.get().unwrap(), f.owner, a.id).unwrap();
    let detail = persist::with_trace_counts(vec![owned], &f.pool)
        .unwrap()
        .remove(0);
    assert_eq!(detail.trace_count, listed_total);
    let json = serde_json::to_value(detail).unwrap();
    assert_eq!(json["id"], serde_json::json!(a.id));
    assert_eq!(json["trace_count"], 1);
    assert!(json.get("audience").is_none(), "Read projection stays flat");
    assert_eq!(
        persist::owned(&mut *f.pool.get().unwrap(), f.stranger, a.id)
            .unwrap_err()
            .status_code,
        404
    );

    // Removing a post association updates the query-time count immediately.
    persist::detach(f.owner, a.id, published, false, &f.pool).unwrap();
    assert_eq!(
        persist::with_trace_counts(vec![a], &f.pool).unwrap()[0].trace_count,
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn individual_removal_revokes_all_sources_and_detaches_empty_origins() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Origin", &f.pool).unwrap();
    let b = persist::create(f.owner, "Overlapping scope", &f.pool).unwrap();
    f.pool.get().unwrap().batch_execute(&format!(
        "INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status,accepted_at)
         VALUES('{}','{}','FOLLOW','ACCEPTED',NOW())", f.stranger, f.owner)).unwrap();
    for user in [f.reader, f.stranger] {
        persist::add_member(f.owner, a.id, user, true, &f.pool).unwrap();
        persist::add_member(f.owner, b.id, user, true, &f.pool).unwrap();
    }
    let (_, pid) = f.publication();
    persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    persist::attach(f.owner, b.id, pid, &f.pool).unwrap();
    let reader = f.grant(pid);
    assert!(PostGrant::revoke(pid, reader.id, f.stranger, &f.pool).is_err());
    PostGrant::revoke(pid, reader.id, f.owner, &f.pool).unwrap();
    assert_eq!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .len(),
        2
    );
    // Finish removing the two readers concurrently, including an idempotent DELETE.
    let other = PostGrant::find_for_post_paginated(pid, 0, 100, &f.pool)
        .unwrap()
        .0
        .into_iter()
        .find(|g| g.grantee_user_id == Some(f.stranger))
        .unwrap();
    let results = std::thread::scope(|s| {
        let first = s.spawn(|| PostGrant::revoke(pid, reader.id, f.owner, &f.pool));
        let last = s.spawn(|| PostGrant::revoke(pid, other.id, f.owner, &f.pool));
        (first.join().unwrap(), last.join().unwrap())
    });
    assert!(results.0.is_ok() && results.1.is_ok());
    let remaining = persist::post_audiences(f.owner, pid, &f.pool).unwrap();
    assert!(
        remaining.is_empty(),
        "Both active origins lose their last reader"
    );
    assert!(f.grant(pid).sources.iter().all(|s| s.status == "revoked"));
    // Explicit selection again restores grants, rather than publication doing so.
    persist::attach(f.owner, a.id, pid, &f.pool).unwrap();
    assert!(f.can_read(pid));
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|s| {
        for id in [reader.id, other.id] {
            let barrier = barrier.clone();
            let f = &f;
            s.spawn(move || {
                barrier.wait();
                PostGrant::revoke(pid, id, f.owner, &f.pool).unwrap();
            });
        }
    });
    assert_eq!(
        persist::post_audiences(f.owner, pid, &f.pool)
            .unwrap()
            .len(),
        0
    );
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn audience_access_and_history_integration() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "Friends", &f.pool).unwrap();
    let b = persist::create(f.owner, "Family", &f.pool).unwrap();
    let (_, past) = f.publication();
    persist::attach(f.owner, a.id, past, &f.pool).unwrap();
    persist::attach(f.owner, b.id, past, &f.pool).unwrap();
    assert_eq!(
        persist::rename(f.stranger, a.id, "Stolen", &f.pool)
            .unwrap_err()
            .status_code,
        404
    );
    assert_eq!(
        persist::add_member(f.owner, a.id, f.stranger, true, &f.pool)
            .unwrap_err()
            .status_code,
        403
    );
    assert!(
        persist::add_member(f.owner, a.id, f.reader, true, &f.pool)
            .unwrap()
            .1
    );
    persist::add_member(f.owner, b.id, f.reader, true, &f.pool).unwrap();
    assert!(
        !f.can_read(past),
        "Joining must not share history automatically"
    );
    let (_, pending) = persist::pending(f.owner, 0, 100, &f.pool).unwrap();
    assert_eq!(pending, 2);
    let (preview, total) =
        persist::list_posts(f.owner, a.id, Some(f.reader), 0, 1, &f.pool).unwrap();
    assert_eq!(total, 1);
    assert!(!preview[0].has_access);
    let history = || HistoryDecisionDto {
        decision: JournalHistoryDecision::AllNormal,
        post_ids: None,
    };
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        1
    );
    assert_eq!(
        persist::apply_history(f.owner, b.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        0
    );
    assert_eq!(f.grant(past).audience_id, Some(a.id));
    assert!(f.can_read(past));
    let removal = persist::remove_member(f.owner, a.id, f.reader, true, false, &f.pool).unwrap();
    assert_eq!(removal.revoked_post_count, 0);
    assert!(
        f.can_read(past),
        "Future-only removal must preserve existing access"
    );
    persist::change_future(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    assert_eq!(
        persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        0
    );
    assert!(
        f.can_read(past),
        "Removing one audience preserves the other active source"
    );
    assert_eq!(
        persist::remove_member(f.owner, a.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        1
    );
    assert!(!f.can_read(past));
    f.pool.get().unwrap().batch_execute(&format!("UPDATE posts SET status='ARCHIVED' WHERE id='{past}'; UPDATE posts SET status='PUBLISHED' WHERE id='{past}';")).unwrap();
    assert!(
        !f.can_read(past),
        "Automatic audience processing must not restore revoked history"
    );
    persist::restart_history_review(f.owner, b.id, f.reader, &f.pool).unwrap();
    persist::apply_history(f.owner, b.id, f.reader, history(), &f.pool).unwrap();
    assert_eq!(
        f.grant(past).audience_id,
        Some(a.id),
        "Legacy origin metadata is not rewritten by new source decisions"
    );
    assert_eq!(
        persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool)
            .unwrap()
            .revoked_post_count,
        1
    );
    assert!(!f.can_read(past));
    persist::remove_member(f.owner, a.id, f.reader, false, true, &f.pool).unwrap();
    persist::remove_member(f.owner, a.id, f.reader, true, false, &f.pool).unwrap();
    persist::replace_defaults(f.owner, f.journal, &[a.id, b.id], &f.pool).unwrap();
    let (future_trace, future) = f.publication();
    assert!(!f.can_read(future));
    persist::attach(f.owner, b.id, future, &f.pool).unwrap();
    persist::attach(f.owner, a.id, future, &f.pool).unwrap();
    assert!(f.can_read(future));
    assert_eq!(f.grant(future).audience_id, Some(b.id));
    let (readers, total) =
        Journal::find_audience_paginated(f.journal, f.owner, 0, 20, &f.pool).unwrap();
    assert_eq!(total, 1);
    assert!(readers[0].has_future_default_access);
    assert_eq!(readers[0].default_audience_ids, vec![b.id]);
    assert!(!f.can_read(past));
    assert_eq!(
        persist::post_audiences(f.owner, future, &f.pool)
            .unwrap()
            .len(),
        2
    );
    let (_, manual) = f.publication();
    PostGrant::create_or_update(
        &Post::find_full(manual, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    persist::attach(f.owner, b.id, manual, &f.pool).unwrap();
    assert_eq!(
        f.grant(manual).audience_id,
        None,
        "Manual first-source must remain manual"
    );
    let child = Trace::create_complement(
        future_trace,
        f.owner,
        String::new(),
        String::new(),
        "PS".to_string(),
        TraceComplementAudienceMode::Parent,
        &[],
        &f.pool,
    )
    .unwrap();
    assert_eq!(f.grant(child.post.id).audience_id, Some(b.id));
    assert!(f.can_read(child.post.id));
    persist::remove_member(f.owner, b.id, f.reader, false, true, &f.pool).unwrap();
    assert!(!f.can_read(future));
    assert!(!f.can_read(child.post.id));
    assert!(f.can_read(manual));
    persist::restart_history_review(f.owner, a.id, f.reader, &f.pool).unwrap();
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        2
    );
    assert_eq!(
        persist::apply_history(f.owner, a.id, f.reader, history(), &f.pool)
            .unwrap()
            .newly_shared_post_count,
        0
    );
    persist::restart_history_review(f.owner, a.id, f.reader, &f.pool).unwrap();
    assert_eq!(
        persist::apply_history(
            f.owner,
            a.id,
            f.reader,
            HistoryDecisionDto {
                decision: JournalHistoryDecision::UserSelected,
                post_ids: Some(vec![Uuid::new_v4()])
            },
            &f.pool
        )
        .unwrap_err()
        .status_code,
        400
    );
    // Detachment future-only leaves its already materialized grants untouched.
    persist::detach(f.owner, a.id, past, false, &f.pool).unwrap();
    assert!(f.can_read(past));
    // Unfollow uses the same transactional cleanup path as blocking.
    let mut conn = f.pool.get().unwrap();
    conn.batch_execute(&format!(
        "DELETE FROM relationships WHERE requester_user_id='{}' AND target_user_id='{}'",
        f.reader, f.owner
    ))
    .unwrap();
    PostGrant::revoke_all_direct_between_users_with_conn(f.owner, f.reader, &mut conn).unwrap();
    drop(conn);
    assert!(!f.can_read(past));
    assert!(!f.can_read(manual));
    assert!(
        !persist::member(&mut *f.pool.get().unwrap(), a.id, f.reader)
            .unwrap()
            .future_access_enabled
    );
    assert_eq!(
        persist::add_member(f.owner, a.id, f.reader, true, &f.pool)
            .unwrap_err()
            .status_code,
        403
    );
    persist::archive(f.owner, a.id, false, &f.pool).unwrap();
    assert!(persist::default_audiences(f.owner, f.journal, &f.pool)
        .unwrap()
        .iter()
        .all(|audience| audience.id != a.id));
}

#[test]
#[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
fn concurrent_audiences_keep_one_grant_with_multiple_sources() {
    let f = Fixture::new();
    let a = persist::create(f.owner, "A", &f.pool).unwrap();
    let b = persist::create(f.owner, "B", &f.pool).unwrap();
    persist::add_member(f.owner, a.id, f.reader, true, &f.pool).unwrap();
    persist::add_member(f.owner, b.id, f.reader, true, &f.pool).unwrap();
    let (_, post) = f.publication();
    f.pool
        .get()
        .unwrap()
        .batch_execute(&format!(
            "INSERT INTO post_audiences(post_id,audience_id)
        VALUES('{post}','{}'),('{post}','{}')",
            a.id, b.id
        ))
        .unwrap();
    #[derive(QueryableByName)]
    struct Changed {
        #[diesel(sql_type=diesel::sql_types::Bool)]
        changed: bool,
    }
    let barrier = Arc::new(Barrier::new(2));
    let handles = [a.id, b.id]
        .into_iter()
        .map(|aid| {
            let pool = f.pool.clone();
            let reader = f.reader;
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                diesel::sql_query("SELECT grant_audience_post($1,$2,$3,TRUE) AS changed")
                    .bind::<SqlUuid, _>(aid)
                    .bind::<SqlUuid, _>(post)
                    .bind::<SqlUuid, _>(reader)
                    .get_result::<Changed>(&mut pool.get().unwrap())
                    .unwrap()
                    .changed
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|changed| *changed)
            .count(),
        1
    );
    let grants = PostGrant::find_for_post_paginated(post, 0, 100, &f.pool)
        .unwrap()
        .0;
    assert_eq!(grants.len(), 1);
    assert!(matches!(grants[0].audience_id,Some(id) if id==a.id||id==b.id));
    assert_eq!(grants[0].sources.len(), 2);
    assert!(grants[0].sources.iter().all(|s| s.status == "active"));
    let removed = persist::detach(f.owner, a.id, post, true, &f.pool).unwrap();
    assert_eq!(removed.revoked_grant_count, 0);
    assert!(f.can_read(post), "Other audience still grants access");
    let manual = PostGrant::create_or_update(
        &Post::find_full(post, &f.pool).unwrap(),
        f.owner,
        NewPostGrantDto {
            grantee_user_id: Some(f.reader),
            grantee_scope: None,
            access_level: None,
        },
        &f.pool,
    )
    .unwrap();
    assert!(manual
        .sources
        .iter()
        .any(|s| s.source_type == "manual" && s.status == "active"));
    persist::detach(f.owner, b.id, post, true, &f.pool).unwrap();
    assert!(
        f.can_read(post),
        "Manual access survives removing both audiences"
    );
    let revoked = PostGrant::revoke(post, manual.id, f.owner, &f.pool).unwrap();
    assert!(revoked.sources.iter().all(|s| s.status == "revoked"));
    assert!(!f.can_read(post));
    let pool = f.pool.clone();
    let owner = f.owner;
    drop(f);
    #[derive(QueryableByName)]
    struct Exists {
        #[diesel(sql_type=diesel::sql_types::Bool)]
        exists: bool,
    }
    assert!(
        !diesel::sql_query("SELECT EXISTS(SELECT 1 FROM users WHERE id=$1) AS exists")
            .bind::<SqlUuid, _>(owner)
            .get_result::<Exists>(&mut pool.get().unwrap())
            .unwrap()
            .exists,
        "Audience-origin grants must not prevent account deletion"
    );
}
