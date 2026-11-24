defmodule App.Repo.Migrations.AddLastSyncedAtIndexes do
  use Ecto.Migration

  # The pull cursor orders and filters every synced table by last_synced_at
  def change do
    for table <- [
          :app_users,
          :app_communities,
          :app_users_communities,
          :app_posts,
          :app_posts_communities
        ] do
      create index(table, [:last_synced_at])
    end
  end
end
