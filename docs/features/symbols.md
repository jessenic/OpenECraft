# Symbols and libraries

The built-in symbol libraries (IEC, NFPA / JIC, GB, GOST, JIS, fluid power, P&ID, single-line) and the symbol editor for own symbols.

## IEC 60617 symbol library

**IEC 60617 symbol library** (~330 symbols): relay coils (AC, latching, polarized,
timers, time switches, counters, over- / undervoltage relays), NO/NC/changeover contacts
(early / late, passing, on- / off-delay), push / pull / key / foot / selector switches
with spring return or latching and position diagrams, emergency stops with turn / pull /
key reset, cam / roller / pressure / flow / level / bimetal switches, inductive,
capacitive, magnetic and photoelectric sensors, fuses (LV HBC), fuse switches and
disconnectors, disconnectors and switch-disconnectors, circuit breakers, RCDs, motor
protection switches, overload relays, AC / DC / EC motors and fans, signal lamps,
beacons, sirens, indicating, recording and integrating meters, current transformers,
1- and 3-phase transformers (vector groups) and autotransformers, rectifiers and
converters, terminals, PLC inputs/outputs, potentials, interruption points, and
electronics: resistors, capacitors (also polarized), diodes, Zener and photodiodes, LEDs,
NPN / PNP transistors, N-channel MOSFET, thyristor, triac, NTC thermistor,
photoresistor, optocoupler and bridge rectifier.

## Installation plan symbols

**Installation plan symbols** (IEC 60617-11): 1-pole and two-way switches, dimmer, push
button, lighting outlet, fluorescent luminaire, double socket outlet with PE, junction box,
motion detector and distribution board, each with one connection point for the cable.

## Symbol editor and libraries

Besides the IEC symbols there are NFPA / JIC
symbols (30: coils, timer coils, contacts, timed and overload contacts, main
contacts, push buttons, emergency stop, selector, foot, limit, float, pressure,
temperature, flow and proximity switches, breakers, disconnects, fuses, pilot lights,
horns, solenoids, control transformers, motors), fluid power symbols (ISO 1219: pumps, compressor, hydraulic motor, single and
double acting cylinders, 3/2, 4/2 and 5/2 valves, non-return, relief, ball and flow
control valves, pressure regulator, service unit, filter, accumulator, cooler,
gauge, tank, silencer), single-line / overview symbols (cross references to the device, not part of
the netlist) and black boxes / PLC boxes. National sets for China (GB), Russia and
the CIS (GOST) and Japan (JIS) carry the device letters of GB/T 7159 (`KM`, `KA`,
`QF`, `SB`, `HL` …), GOST 2.710 (`KM`, `KK`, `QF`, `SB`, `HL` …) and JIS C 0401
(`MC`, `MCCB`, `PBS`, `PL`, `THR` …), so placed devices are numbered `-KM1`,
`-MCCB1`; they are drawn like the IEC symbols, which GB/T 4728 and JIS C 0617 adopt
(the ESKD proportions of the GOST drawings are not reproduced). Own symbols are drawn in the symbol editor
(lines, rectangles, circles, texts, tag / label fields, connection points on the 4 mm
grid, function such as main function, contact, terminal, PLC point), or copied from a
built-in symbol; they are stored in the project and exchanged as library files.
A placed symbol of the library (an own or an imported EPLAN symbol) can be drawn larger
or smaller: **Scale** in its properties (or `update_symbol` with `scale`, 0.1 to 10)
scales its graphics and connection points, while its texts keep their size, so that
imported symbols fit the drawing; the wires at its connection points follow.
