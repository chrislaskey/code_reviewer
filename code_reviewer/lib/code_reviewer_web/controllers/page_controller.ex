defmodule CodeReviewerWeb.PageController do
  use CodeReviewerWeb, :controller

  def home(conn, _params) do
    render(conn, :home)
  end
end
