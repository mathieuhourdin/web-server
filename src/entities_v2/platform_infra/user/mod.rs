mod admin;
mod enums;
mod model;
mod purge;
mod routes;

pub use admin::{
    delete_admin_user_route,
    get_admin_platform_overview_route, get_admin_recent_user_activity_route,
    get_admin_service_user_route, get_admin_service_users_route, get_admin_users_route,
    patch_admin_user_ai_features_route, post_admin_service_user_route,
    put_admin_service_user_route, AdminPlatformCurrentHealth, AdminPlatformDailyOverview,
    AdminPlatformOverview, AdminUserAiFeaturesResponse, AdminUserDailyActivity, AdminUserListItem,
    AdminUserRecentActivity,
};
pub use enums::{
    EmailNotificationMode, HomeFocusView, JournalTheme, UserPrincipalType, UserRole,
    WeekAnalysisWeekday,
};
pub use model::{
    ensure_user_has_any_lens, ensure_user_has_default_journals, ensure_user_has_meta_journal,
    find_similar_users, NewServiceUserDto, NewUser, User, UserListParams, UserMatch,
    UserPseudonymizedAuthentifiedResponse, UserPseudonymizedResponse, UserPublicResponse,
    UserResponse, UserRoleAssignment, UserSearchParams, UserSearchResult,
};
pub(crate) use routes::{cleanup_failed_user_registration, finalize_new_human_user_registration};
pub(crate) use purge::purge_user;
pub use routes::{
    get_closest_followers_route, get_me_unread_counts_route, get_mentors_route,
    get_suggested_users_route, get_user_route, get_user_search_route, get_users, patch_user_route,
    post_user, post_user_profile_picture_asset_route, put_user_route, delete_my_account_route,
    delete_user_account_route,
    MeUnreadCountsResponse,
};
