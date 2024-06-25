use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NistRandEntry::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NistRandEntry::ChainIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NistRandEntry::PulseIndex)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(NistRandEntry::Uri).string().not_null())
                    .col(
                        ColumnDef::new(NistRandEntry::Timestamp)
                            .date_time()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NistRandEntry::OutputValue)
                            .binary_len(512)
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .primary()
                            .name("nist-rand-entry-primary")
                            .col(NistRandEntry::ChainIndex)
                            .col(NistRandEntry::PulseIndex),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NistRandEntry::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum NistRandEntry {
    Table,
    ChainIndex,
    PulseIndex,
    Uri,
    Timestamp,
    OutputValue,
}
