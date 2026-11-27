defmodule Core.App.Files.Events do
  @moduledoc """
  The facts of a file's aggregate. Each carries what the projector needs
  to write the rows, including the versions the aggregate decided. Times
  are RFC 3339.
  """

  defmodule FileCreated do
    @moduledoc "A file was created, in the communities of `links`."
    @derive Jason.Encoder
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

  defmodule FileUpdated do
    @moduledoc "Fields of a file changed, `changes`, to `version`; `conflicts` the fields that did not."
    @derive Jason.Encoder
    defstruct [:file_id, :request_id, :actor_id, :version, :at, changes: %{}, conflicts: %{}]
  end

  defmodule FileAddedToCommunity do
    @moduledoc "The file is in a community, under a new link or one revived."
    @derive Jason.Encoder
    defstruct [:file_id, :request_id, :actor_id, :link_id, :community_id, :link_version, :inserted_at, :at]
  end

  defmodule FileRemovedFromCommunity do
    @moduledoc "The file was taken out of a community."
    @derive Jason.Encoder
    defstruct [:file_id, :request_id, :actor_id, :link_id, :community_id, :link_version, :deleted_at, :at]
  end

  defmodule FileDeleted do
    @moduledoc "The file was taken out of its last community, so it is gone."
    @derive Jason.Encoder
    defstruct [:file_id, :request_id, :actor_id, :version, :deleted_at, :at]
  end
end
