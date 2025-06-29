/// copied wholesale from http://githupb.com/p2panda/reflection
/// until i better understand why this complex async shit is required to load 2 text files off the disk
use std::future::Future;
use std::pin::Pin;

use sqlx::error::BoxDynError;
use sqlx::migrate::{Migration, MigrationSource, Migrator};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Combine multiple `sqlx::migrate::Migrator` into a single `sqlx::migrate::MigrationSource`
///
/// See for more details: https://github.com/launchbadge/sqlx/discussions/3407
#[derive(Debug)]
pub struct CombinedMigrationSource {
    migrators: Vec<Migrator>,
}

impl CombinedMigrationSource {
    pub fn new(migrators: Vec<Migrator>) -> CombinedMigrationSource {
        Self { migrators }
    }
}

impl<'s> MigrationSource<'s> for CombinedMigrationSource {
    fn resolve(self) -> BoxFuture<'s, Result<Vec<Migration>, BoxDynError>> {
        Box::pin(async move {
            Ok(self
                .migrators
                .iter()
                .flat_map(|migrator| migrator.iter())
                .cloned()
                .collect())
        })
    }
}
