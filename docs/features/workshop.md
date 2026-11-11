# Wiring and mounting assistants

The panel layout as a checklist for the workshop, on a desktop or a tablet.

![The wiring assistant with the checklist, the current wire with its data and the wire highlighted in 3D](../images/wiring-assistant.png)

## Wiring assistant

**Wiring assistant** (in the spirit of EPLAN Smart Wiring): the *Wiring* workspace
turns the routed wires of the panel layout into a touch-friendly checklist for the
workshop – ordered by device, position, cross-section or wire number – with the
current wire highlighted in 3D, its colour, cross-section, length, stripping length
and the ferrule or terminal of each end (two wires sharing a twin ferrule one after
the other, *crimp together with the wire to …*), and the progress in wires and metres. Wires are marked as wired or as a
problem (with a note); marks are stored in the project, so wires changed in the
schematic after they were wired are flagged by the check. The web build opens the
assistant directly with `…/#wiring` (desktop: `--wiring`), which makes a tablet in
the workshop a cheap wiring station; checklist with state as CSV.

## Mounting assistant

**Mounting assistant** (in the spirit of EPLAN Smart Mounting): the *Mounting* switch
of the *Wiring* workspace lists the items of the panel layout in mounting order –
cutouts, rails, ducts and busbars, devices, accessories, then door and walls – with
the current item highlighted in 3D, its place, size and part, and the progress.
Items are marked as mounted or as a problem (with a note); items moved or given
another part after they were mounted are flagged by the check. Commands
`mounting_list` and `mark_mounted`; checklist with state as CSV.
