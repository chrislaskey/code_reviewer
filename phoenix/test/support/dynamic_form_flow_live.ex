defmodule App.DynamicFormFlowLive do
  @moduledoc """
  A LiveView holding one `DynamicForm.form`, for the `flows` of
  native/dynamic_form/tests/cases.json: the form from a definition and
  data given in the session, and the data of the last valid submit shown
  in `#result` as JSON, after how many there have been. See `App.Web.DynamicFormCasesTest`.
  """

  use Phoenix.LiveView

  @impl true
  def mount(_params, %{"definition" => definition, "data" => data}, socket) do
    {:ok,
     assign(socket,
       instance: DynamicForm.Parser.FromData.parse!(definition),
       data: data,
       successes: 0,
       result: nil
     )}
  end

  @impl true
  def render(assigns) do
    ~H"""
    <DynamicForm.form id="f" instance={@instance} data={@data} form_name="f" components={App.Web.CoreComponents} />
    <pre id="result">{@successes} {@result}</pre>
    """
  end

  @impl true
  def handle_info({:dynamic_form, :success, %DynamicForm.Payload{data: data}}, socket) do
    {:noreply, socket |> update(:successes, &(&1 + 1)) |> assign(:result, Jason.encode!(data))}
  end
end
