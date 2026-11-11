# Wires, cables, potentials and numbering

How connections get their cable, colour, cross-section and number, and how devices, pages and wires are numbered.

## Potential definitions

**Potential definitions** (*Connections › Potentials…*): kind (L, N, PE, DC +/−, signal),
voltage, wire colour (IEC 60757 codes) and cross-section per potential; presets
IEC 60204-1 (machinery, new projects) and IEC 60445 (phase colours). Wires without
their own colour / cross-section take them from the potential of their connection
(also behind terminals), otherwise from the project default for connections without
a defined potential; cable cores keep the cable's values. The wire properties show
the automatic value and where it comes from; the wire lists, panel wiring, duct fill,
ferrules, harnesses and marking use it. The check reports GNYE outside protective
conductors and PE in other colours, manual values that differ from the definition
and potentials without definition. *View › Wire colours* (saved with the project)
draws the wires in their colour on the schematic and in PDF / SVG export; two-colour
wires such as GNYE are drawn dashed in their second colour.

## Cables

Cable definitions (tag, type, cores, cross-section, length, part from the
product group *Cables*). Wires are assigned to cores (several wires at once get the free
cores in order, the wires of one connection share a core); cores and the cable tag are
shown on the schematic, with a stroke across the wire where a cable definition line of
the cable crosses it (else at 30 % of the wire). The core designation and the cable tag
of a selected wire can be dragged into place and keep their offset from there; *Reset*
in the wire properties (or `update_wire` with an empty `core_offset` / `cable_offset`)
places them automatically again. Cable overview and
cable diagram reports, cables in the BOM by length, checks for unknown cables, cores
used twice or not in the cable.

## Connection and cable definitions

A connection definition point on a wire sets
number, colour and cross-section of the whole connection and the core of its wire; a
cable definition line drawn across wires makes them cores of a cable (new cable with
numbered cores, or a chosen one). The check reports points beside wires, contradicting
or not applied points, and lines of unknown cables.

## Connection symbols: T-junctions, corners, crossings and bridges

Three devices on one potential can be wired as A–B and A–C or as A–B and B–C; the
netlist alone cannot tell them apart. A **T-junction** (*Insert › Connections ›
T-junction*, placed where three wires meet) stores this choice with the project, in the
spirit of the connection symbols of EPLAN: its three ports are the stem (the direction
of the junction: left, right, up or down) and the two arms of the bar across it. Four
variants per direction name the *source* port and the order of the two *targets*:

| Variant | Source | First target | Second target |
| --- | --- | --- | --- |
| 1 | left / upper arm | the other arm (straight on) | the stem |
| 2 | right / lower arm | the other arm (straight on) | the stem |
| 3 | stem | left / upper arm | right / lower arm |
| 4 | stem | right / lower arm | left / upper arm |

The two physical wires of the connection run from the device at the source to the
device at the first target and to the device at the second target. The device of a
port is the first connection point along its wire, also across interruption points;
where the wire reaches another T-junction it hands on its targets (when the wire comes
in at its source) or its source. The connection list, the connection diagrams, the
panel wiring, the harnesses and every other consumer of the connections use this
result. Connections without a T-junction keep the schematic order as before, and a
junction that does not fit its wires (not three wires, not one in its direction, a
connection point on the spot) is ignored and reported by the check *Connection
symbols*, as are two symbols on one point and a connection whose symbols wire it in a
loop (more wires than its devices need, e.g. the sources of two T-junctions facing each
other across a wire).

The symbol is a short diagonal from the source to the second target (the first target
lies straight on); *Representation › Dot* draws the usual connection dot instead and
keeps the routing. The properties pane shows the four variants as previews with the
targets numbered, the direction, and the wires of the connection. Rotating or mirroring
the drawing turns the junction with its wires; a moved device stretches its wire and
leaves the junction where the wires meet, and when the whole node moves the junction
follows.

A **corner** (*Insert › Connections › Corner*) is the second connection symbol: two
wires meeting at a right angle, with four orientations (R turns it while placing). It
changes no routing; its use is automatic wiring: with *Auto-connect* on, placing a
corner (or a T-junction) draws wires from connection points that face its free arms
straight on, and a symbol placed or moved so that a connection point faces a free arm
is wired to it. So a corner at the meeting point of two connections wires both of them
in. A corner that fits its wires is drawn as nothing but the bend; one without its two
wires shows its arms in the error colour and is reported by the check.

A **crossing** (*Insert › Connections › Crossing*) connects the two wires crossing at
its point, which stay unconnected without one. It is two T-junctions joined: each end of
its straight line (vertical or horizontal) is wired straight on to the other end first
and across to one arm second; the two variants decide which end takes the left / upper
arm. In EPLAN's example, a vertical crossing of variant 2 leads a connection from above
to the bottom first and to the left second, and one from below to the top first and to
the right second. A **bridge** (*Insert › Connections › Bridge*) has four wires as well
and is a star from the side the connection comes from to the three others: the opposite
side first, then the left or upper, then the right or lower. Both can be drawn as a
dot, keep their routing then, and turn with the drawing. Every symbol is a list of
links between its arms; where a wire reaches another symbol, that symbol hands on the
devices of the arms linked with the one the wire comes in at.

Commands: `add_t_junction` (direction from the wires there when not given), `add_corner`
(arms from the wires there, e.g. `down left`), `add_crossing` (`line` vertical or
horizontal, `variant` 1 or 2), `add_bridge` (`direction` the connection comes from), all
with `autoconnect` to draw the wires to facing connection points, `update_connection_symbol`,
`list_connection_symbols` (kind, routing, links and the wires of each). `autoconnect`
takes the ids of connection symbols too.

## Wire numbers

One number per connection (all its wire segments; the two sides of a
terminal are separate connections). Automatic numbering: sequential, page/column
(`3.4.1`) or by potential name (`L1`, `+24V`), with prefix and start value; manually
set numbers are kept. Shown on the schematic, in the connection list and the panel
wire list; checks for conflicting and duplicate numbers.

## Numbering

**Numbering** (in the spirit of EPLAN's offline numbering): *Devices › Device
numbering…* renumbers device tags per structure and identifying letter in schematic
order (page, column, top to bottom) after a format: `{letter}{n}` (`-K1`),
`{letter}{page}.{n}` (`-K3.1`), `{letter}{page}{column}` (`-K34`) or NFPA style
`{page}{column}{letter}` (`-34K`), with start value, step, letters and scope, and a
preview of the new tags; everything that refers to a tag follows (parts, strips,
plugs, PLC data, panel layout, harnesses); tags of devices that are not placed
(device list, panel layout) are not given to others. *Home › Renumber pages…* numbers pages
with start and step, restarting per structure and keeping sub-pages (`3.a`; *Home › New page › New
sub-page* inserts `3.1`, `3.2`, … after the current page). Page sections hold pages
and subsections like directories (*Home › New section*, drag pages onto them in the
*Pages* navigator); the page names carry the section numbers (`2.3.1`), follow when a
section is renumbered or moved, and renumbering counts per section. The wire
numbering gets a *Format* scheme with placeholders (`{page}`, `{column}`, `{row}`,
`{potential}`, `{kind}`, `{color}`, `{cross_section}`, counter `{n}` counted per
value of the rest), counter digits and step, and potential kinds that get no number
(e.g. PE). Commands: `renumber_devices`, `renumber_pages`, `set_wire_numbering`.
