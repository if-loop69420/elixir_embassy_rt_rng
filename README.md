# Elixir&Embassy Realtime RNG
This is a simple Realtime (and i think real as in not-pseudo) random number generator using Elixir and embassy-rs.
It runs on two devices (a RaspberryPi 4B reading values from UART) and an ESP32C3(sending the values it read).
The values are produced by reading voltage levels from an ADC on an unconnected pin which is acting as an antenna.
Only the first bit of each value is used and shifted into a u8. When that u8 is "full" the values are sent to the 
Raspberry Pi via UART and the registers are reset
