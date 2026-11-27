defmodule CoreElixir.Router do
  @moduledoc "Routes each command to the aggregate that decides it."
  use Commanded.Commands.Router

  alias CoreElixir.App.Files.Aggregate
  alias CoreElixir.App.Files.Commands.{AddFileToCommunity, CreateFile, RemoveFileFromCommunity, UpdateFile}

  identify(Aggregate, by: :file_id, prefix: "file-")

  dispatch([CreateFile, UpdateFile, AddFileToCommunity, RemoveFileFromCommunity],
    to: Aggregate,
    consistency: :strong
  )
end
