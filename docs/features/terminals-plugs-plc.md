# Terminal strips, plugs, PLC and marking

Terminal strips with multi-level terminals and accessories, plugs, PLC cards with the exchange to PLC programming tools, and the markers for the workshop.

## Terminal strip editor

Terminal order (move up / down, sort), renumbering 1 … n,
**jumpers** (bridged terminals share their potential), spare terminals (in the terminal
diagram and the BOM);
every terminal has its part: PE terminals take the strip's part with the tech data
`Terminal type: PE`, all others its feed-through part (bill of materials, panel
length of the strip, connection points per terminal); **automatic accessories**
from the tech data of the terminal parts (`End bracket: …`, `End plate: …`,
`Jumper: …` with jumper parts `Poles: n`): an end bracket at both ends, an end
plate after the last terminal of each part, plug-in jumpers of the shortest
fitting length (cut if longer, teeth broken off at terminals in between) – in the
bill of materials, the terminal diagram, the panel layout (2D / 3D) and the check;
the terminal strip window shows the strip on the rail with its jumpers and has a
switch for the automatic accessories.
**Multi-level terminals**: a terminal on level 2 or higher sits on the block of the
terminal before it (one part per block, tech data `Levels: 2`); the terminal
diagram lists every level, the rail layout and the bill of materials count blocks,
and the check reports more levels than the part has. **Jumper kinds**: plug-in
jumpers (tech data `Jumper`), saddle jumpers (`Saddle jumper`) and wire jumpers (any
level, no part); plug-in and saddle jumpers must stay on one level. Commands:
`list_strips`, `get_strip`, `set_terminal`, `set_jumper_kind`, `set_strip`,
`renumber_strip`.

## Plugs

**Plugs** (*Devices › Plugs…*, symbol group *Plugs*): pin and socket (mated pair, two
connection points), male pin and female socket (one each); tag = plug (`-X10`),
designation = pin. Male and female pins of the same designation are mated: the
potential passes through, the two sides are separate connections (wire numbers,
cables). Pins are numbered per plug when placed; the plug window and the *plug
diagram* report list the pins in plug order with the socket side, the pin side and
the potential; parts with `Plug pin` functions give the pin count (unused pins are
listed as spare). The check reports pins placed twice and more pins than the plug
parts have; the KBL export names the housing part of plug connectors with all
cavities of the part.

## Connector pin-outs and flat cables

**Pin-out** (plug window, export *Connector pin-outs (SVG)* `pinouts`, command
`get_pinout`): the mating faces of the pin side (male) and the socket side (female, the
mirror image) with every pin of the plug's part at its place, filled with the colour of
the wire or cable core at the pin (spares empty), the coding key and a pin table with
potential, targets and cable cores; hovering a pin shows its details, a click shows
its symbol. The face comes from the technical data of the connector part:
`Pin layout: circle 4` (pins on a circle, pin 1 right of the key at the top, counting
counter-clockwise on the male face as in IEC 61076-2-101), `circle 7+1` (centre pins
last), `circle 9+3` / `circle 11+5+1` (an inner ring, then the centre), `row 8`,
`zigzag 10` (two rows, odd pins above even pins: IDC), `grid 2x5` (numbered row by
row), `cols 20x2` (numbered column by column), `dsub 9`, each optionally `+PE`; for any
other face the pin centres in mm: `points -3.5,2 3.5,2 0,-2` (rectangular face) or
`round 0,9 -8,-3 8,-3` (circular face with key). `Pitch`, `Coding` and `Gender` (`male`,
`female`, `hermaphroditic`) complete it. Numbered pins of a smaller part sit at their
number (M8 3-pin: 1, 3, 4); plugs without a layout are shown in a row.

The catalog has, each male and female:

| Family | Parts |
| --- | --- |
| Sensor / actuator | M5, M8 (3 – 8 pins), M12 A, B (PROFIBUS), D, X (10 Gbit/s Ethernet) and the power codings S, K, T, L, 7/8" (3 – 6 pins) |
| Circular | M16 after IEC 61076-2-106 (3 – 24 pins), M23 signal (12, 17, 19 pins), M23 and M40 servo motor power connectors, GX12 / GX16 / GX20 aviation connectors |
| Valves | DIN EN 175301-803 forms A, B and C, 2 + PE and 3 + PE |
| Rectangular | heavy duty inserts of the A, D, E and Q series with PE, D-sub 9 – 37, DIN 41612 type C (32, 64, 96), PLC front connectors (20, 40) |
| Board | IDC sockets and box headers 10 – 64, single row 2.5 mm, dual row 3.0 and 4.2 mm, pluggable terminal blocks 3.5 / 5.08 / 7.62 mm |
| Data | RJ11, RJ12, RJ45, BNC |
| Mobile machines | sealed DT type (2 – 12 pins) and 1.5 mm system (1 – 6 pins) |
| Power | CEE IEC 60309 16 / 32 / 63 A with 3, 4 and 5 poles, appliance couplers IEC 60320 C5 – C20, Schuko, BS 1363, NEMA 5-15, L14-30, L21-30, installation plug connectors (3 – 5 poles), PV connectors, battery connectors (hermaphroditic, 50 – 175 A), safety banana connectors, thermocouple connectors |

**Flat cables** (`Cable form: Flat` in the cable part: ribbon cables 10 – 64 cores with
pitch 1.27 mm, flat cables for festoon systems): `connect_flat_cable` makes the wires at
one side of a plug cores of the cable, pin n taking core n. Ribbon cables (flat cables
with a `Pitch`) show grey cores with core 1 red on the faces. The check *Connector
pin-outs and flat cables* reports pins the plug's part does not have, cores of a flat
cable on an IDC connector (`Termination: IDC`) that land on another pin and flat cables
with another core count than the IDC connector has pins.

## Strip and plug navigator

**Strip and plug navigator** (navigator tab *Strips / plugs*): the terminal strips with
their terminals in rail order (levels of multi-level blocks) and spares, the plugs with
their pins and targets and the unused pins of the plug parts; a click shows a placed
one, spares and unused pins are dragged into the page.

## PLC navigator

**PLC navigator** (navigator tab *PLC*): the cards with their channels in address
order, placed ones with function text and connected devices (a click shows the
symbol), free channels of cards with a device description to drag into the page.

## PLC overview

Select the imported card once under **Insert center → Devices**, then click the sheet for each successive channel. For an EL1008, clicks one through eight share a card tag. Click nine starts channel 1 on another card. Escape ends placement. Expand **Representations** when a particular wiring variant is required.

Beckhoff analog macros with one identifiable signal and its companion terminals produce one PLC point. On the EL3174, voltage input `U1` uses terminal 1 for the signal and terminal 9 for its reference. Both terminals can be wired separately. Current-mode representations are alternatives for the same four channels. Use the Insert center to select a particular representation. The PLC overview reports the signal terminal, and readdressing the card leaves analog channel labels intact. TwinCAT variable declarations preserve the supplied data type. Channels without a declared data type remain available on the sheet and in the overview.

For Beckhoff EL1008 and EL2008, import the article XML with the matching EMA beside it. Place a part on the schematic to use its next available channel, or choose a particular multi-line channel in the Insert center. Channels from the same macro family share a card tag. Once all eight are placed, the next default drop starts another card. The PLC navigator lists the remaining channels after the first placement. The PLC overview edits addresses and function texts. Readdressing uses the imported channel order even when points are placed out of order. For example, channel 2 uses physical terminal 5 and bit 1. Set a different start byte for another card in the same address space. The EK1100 coupler retains its supply and bus connections.

All inputs / outputs per card with connected devices, editable
addresses and function texts, card start byte / rack / slot, readdressing (of the
digital points; analog ones keep their word addresses), **symbol
table export / import** (CSV: name, data type, address, comment) for PLC programming
tools, the tag table of **Siemens TIA Portal** (Excel sheet *PLC Tags*), the tag
import of **Rockwell Studio 5000** (aliases of the module channels), the symbol table
of **Siemens STEP 7 classic** (SDF), a global variable list for **Beckhoff TwinCAT 3**
(TcGVL, `AT %I*` / `AT %Q*`), variables for **Schneider Control Expert** (XSY, `EBOOL`
with topological addresses), device comments for **Mitsubishi GX Works** (CSV, `X` / `Y`
in octal) and IEC 61131-3 global variables (CODESYS); addresses
used on several cards are reported.

## Marking

**Marking** (*Connections › Marking…*: wire marker text, material per kind with piece
counts and export; *File › Export › Export marking (CSV)…*, report *Marking*): device markers (legend
plates for door devices; the enclosure's location left out of the tag), terminal markers
in rail order with the strip marker, wire markers (wire number on both ends, or
destination marking: each end shows the other end), cable markers; grouped by
marker material in the order of application, as CSV for marking software (UTF-8,
`;`, header `Material;Category;Text;Count;Enclosure;Group`); the material is in the
bill of materials. Texts and materials are project settings (`marking`).
**Labeling schemes** choose the markers, the columns and their order, the separator,
header and one row per printed label, for CSV / text and Excel export; built-in
import tables for Phoenix Contact CLIP PROJECT, Weidmüller M-Print PRO, Brady,
Murrplastik and Partex (starting points – copy and adjust to the program version),
own schemes are stored in the project (commands `marking_schemes`,
`set_marking_scheme`, exporters `marking` / `marking_xlsx` with `scheme`).
