use std::fmt;

use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use uuid::Uuid;

macro_rules! entity_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(Uuid);

        impl $name {
            pub(super) fn generate() -> Self {
                Self(Uuid::now_v7())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.hyphenated().fmt(formatter)
            }
        }

        impl ToSql for $name {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                Ok(ToSqlOutput::from(self.to_string()))
            }
        }

        impl FromSql for $name {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                Uuid::parse_str(value.as_str()?)
                    .map(Self)
                    .map_err(|error| FromSqlError::Other(Box::new(error)))
            }
        }
    };
}

entity_id!(AssetId);
entity_id!(CollectionId);
