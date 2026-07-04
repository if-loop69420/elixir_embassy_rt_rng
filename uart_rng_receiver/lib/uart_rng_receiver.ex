defmodule UartRngReceiver do
  @moduledoc """
  Documentation for `UartRngReceiver`.
  """
  def start_listening do
    {:ok, pid} = Circuits.UART.start_link()

    Circuits.UART.open(pid, "ttyS0", speed: 115200, active: true)

    IO.puts("Listening to hardware RNG")

    loop()
  end

  defp loop do
    receive do
      {:circuits_uart, "ttyS0", raw_binary_data} ->
        IO.puts("Received Chaos: #{inspect(raw_binary_data)}")
    end
  end
end
