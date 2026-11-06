use super::*;
use diesel::{connection::SimpleConnection, result::Error as DieselError};

#[test]
fn device_analytics_window_and_safe_projection() {
    assert_eq!(DeviceAnalyticsQuery::default().days().unwrap(), 30);
    for days in [0, -1, 367, i64::MAX] {
        assert_eq!(
            DeviceAnalyticsQuery {
                active_within_days: Some(days)
            }
            .days()
            .unwrap_err()
            .status_code,
            400
        );
    }
    let now = Utc::now().naive_utc();
    let view: AdminDeviceView = AdminDeviceRow {
        id: Uuid::new_v4(),
        device_type: "IOS".into(),
        name: Some("Synthetic phone".into()),
        app_version: Some("1.2.3".into()),
        os_name: None,
        os_version: None,
        browser_name: None,
        browser_version: None,
        last_seen_at: Some(now),
        created_at: now,
        revoked_at: None,
        has_token: true,
        push_provider: Some("FCM".into()),
        token_updated_at: Some(now),
    }
    .into();
    let value = serde_json::to_value(view).unwrap();
    assert_eq!(value["device_type"], "ios");
    assert_eq!(value["push"]["provider"], "fcm");
    assert_eq!(value["push"]["has_token"], true);
    assert!(value.get("push_token").is_none());
    assert!(value.get("identifier").is_none());
    assert_eq!(value["push"].as_object().unwrap().len(), 3);
}

#[test]
#[ignore = "Requires explicitly configured disposable blog_test; synthetic fixtures only"]
fn device_analytics_snapshot_and_user_inspection() {
    use diesel_migrations::{embed_migrations, MigrationHarness};
    let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
    assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
    let mut conn = PgConnection::establish(&url).unwrap();
    const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
    conn.run_pending_migrations(MIGRATIONS).unwrap();
    conn.test_transaction::<_,DieselError,_>(|conn| {
        let now=Utc::now();
        let baseline=load_analytics(30,now,conn).unwrap();
        let (a,b,c,service,demo,client)=(Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4(),Uuid::new_v4());
        conn.batch_execute(&format!("INSERT INTO users(id,email,first_name,last_name,handle,password)
            SELECT id,'demo-'||id::text||'@automation.invalid','Synthetic','Device test',id::text,'unused'
            FROM unnest(ARRAY['{a}'::uuid,'{b}','{c}','{service}','{demo}']) id;
            UPDATE users SET principal_type='SERVICE',is_platform_user=FALSE WHERE id='{service}';
            INSERT INTO automation_clients(id,name,scopes,token_hash) VALUES('{client}','Synthetic devices',ARRAY[]::text[],'unused');
            INSERT INTO automation_demo_users(user_id,api_client_id) VALUES('{demo}','{client}');
            INSERT INTO devices(user_id,identifier,device_type,app_version,os_name,browser_name,last_seen_at,push_provider,push_token,revoked_at)
            VALUES('{a}','phone','IOS','1.2.3','iOS',NULL,NOW()-interval '1 minute','FCM','synthetic-never-return',NULL),
                ('{a}','browser','WEB',NULL,NULL,'Firefox',NOW()-interval '2 minutes',NULL,NULL,NULL),
                ('{a}','old','ANDROID',NULL,NULL,NULL,NOW()-interval '40 days',NULL,NULL,NULL),
                ('{a}','revoked','WEB',NULL,NULL,NULL,NOW()-interval '3 minutes','FCM','synthetic-revoked',NOW()),
                ('{b}','android','ANDROID','1.2.3','Android',NULL,NOW()-interval '4 minutes',NULL,NULL,NULL),
                ('{b}','unknown','UNKNOWN',' ',NULL,NULL,NOW()-interval '5 minutes',NULL,NULL,NULL),
                ('{b}','unseen','WEB',NULL,NULL,NULL,NULL,NULL,NULL,NULL),
                ('{service}','service','IOS',NULL,NULL,NULL,NOW()-interval '1 minute',NULL,NULL,NULL),
                ('{demo}','demo','IOS',NULL,NULL,NULL,NOW()-interval '1 minute',NULL,NULL,NULL);")).unwrap();
        let report=load_analytics(30,now,conn).unwrap();
        assert_eq!(report.total_users,baseline.total_users+3);
        assert_eq!(report.active_users,baseline.active_users+2);
        assert_eq!(report.active_devices,baseline.active_devices+4);
        assert_eq!(report.multi_platform_users,baseline.multi_platform_users+2);
        assert_eq!(report.users_without_recent_device,baseline.users_without_recent_device+1);
        assert!(report.platforms.iter().any(|p|p.device_type=="unknown"));
        assert!(report.app_versions.iter().any(|p|p.device_type=="unknown" && p.version.is_none()));
        let page=ValidatedPagination {limit:2,offset:0};
        let first=load_user_devices(a,page,conn).unwrap();
        assert_eq!(first.total,4);
        assert_eq!(first.items.len(),2);
        assert_eq!(first.items[0].device_type,"ios");
        assert!(first.items[0].push.has_token);
        let later=load_user_devices(a,ValidatedPagination {limit:2,offset:2},conn).unwrap();
        assert!(later.items.iter().any(|d|d.revoked_at.is_some() && d.push.has_token));
        assert_eq!(load_user_devices(c,page,conn).unwrap().total,0);
        assert_eq!(load_user_devices(Uuid::new_v4(),page,conn).err().unwrap().status_code,404);
        let json=serde_json::to_string(&first).unwrap();
        assert!(!json.contains("synthetic-never-return"));
        assert!(!json.contains("identifier"));
        Ok(())
    });
}
