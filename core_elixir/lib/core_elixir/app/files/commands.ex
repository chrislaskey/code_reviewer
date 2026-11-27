defmodule CoreElixir.App.Files.Commands do
  @moduledoc """
  The commands of a file's aggregate. Each carries the id of the request
  it answers, the sync event's id, so a retry is recognized, and `at`,
  the server's time of the decision, as RFC 3339.
  """

  defmodule CreateFile do
    @moduledoc "Creates a file in its first communities, `links`: `[%{id, community_id}]`."
    defstruct [
      :file_id,
      :request_id,
      :actor_id,
      :name,
      :content_type,
      :size,
      :sha256,
      :content,
      :inserted_at,
      :at,
      links: []
    ]
  end

  defmodule UpdateFile do
    @moduledoc "Changes a file's fields, merged against the version and values the device edited."
    defstruct [:file_id, :request_id, :actor_id, :base_version, :at, base: %{}, changes: %{}]
  end

  defmodule AddFileToCommunity do
    @moduledoc "Puts the file in a community, under `link_id` when it was never there."
    defstruct [:file_id, :request_id, :actor_id, :link_id, :community_id, :inserted_at, :at]
  end

  defmodule RemoveFileFromCommunity do
    @moduledoc "Takes the file out of a community, at `deleted_at`."
    defstruct [:file_id, :request_id, :actor_id, :community_id, :deleted_at, :at]
  end
end
