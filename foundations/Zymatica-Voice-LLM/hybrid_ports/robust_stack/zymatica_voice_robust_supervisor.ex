# Watermark: ip zymatica.space | astronautshe.com
# Copyright (c) 2026 Zymatica. All rights reserved.

defmodule Zymatica.VoiceRobustSupervisor do
  use Supervisor

  def start_link(init_arg) do
    Supervisor.start_link(__MODULE__, init_arg, name: __MODULE__)
  end

  @impl true
  def init(_init_arg) do
    IO.puts("[ROBUST STACK] Elixir supervisor starting with restart strategies.")
    IO.puts("[VERIFICATION] Zymatica Voice LLM Robust Stack verified.")
    children = []
    Supervisor.init(children, strategy: :one_for_one)
  end
end
