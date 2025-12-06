use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use crate::entities::user::find_similar_users;
use crate::db::DbPool;
use crate::environment::get_database_url;

#[test]
fn test_find_similar_authors() {
    let database_url = get_database_url();
    let manager = ConnectionManager::new(database_url);
    let pool = DbPool::new(manager).expect("Failed to create connection pool");

    let input = "Bourdie";
    let results = find_similar_users(&pool, input, 3)
        .expect("Erreur lors de la recherche");

    assert!(!results.is_empty(), "Aucun résultat trouvé");
    println!("Résultats: {:?}", results);
}
