# init_dawn.d

> A custom ESP32-C3 alarm clock, designed from the PCB up.

>**Status: Waiting for design review**
---

## Overview

*init_dawn.d* is a custom alarm clock built around the **XIAO ESP32-C3**, with a custom PCB and a 3D printed enclosure.
the PCB, enclosure and hardware layout are being designed specifically for this project instead of using a ready-made board.
currently the project is at the **hardware/design review stage**. The PCB has been designed and routed in KiCad and the
enclosure has been adapted around the actual board and components.the firmware is there as the project structure, but
actual firmware development will happen after the hardware design is reviewed.

## Hardware

The current design uses:

- XIAO ESP32-C3
- SPI TFT LCD
- Passive buzzer
- 7 physical switches
- Custom PCB
- M3 × 5 × 4 mm heat-set brass inserts
- 3D printed enclosure

### PCB

Current PCB dimensions:

```text
116 × 84 × 1.6 mm
````

Designed and routed using KiCad.

KiCad project files:

```text
kicad/
├── init_dawn.d.kicad_pcb
├── init_dawn.d.kicad_sch
└── init_dawn.d.kicad_pro
```

## Controls

The clock has **7 physical switches**:

- 4 main switches
- 1 SHIFT switch
- 2 alarm switches

the shift switch gives the main buttons another set of functions, so more controls can be handled without needing a physical button for every single action.there was an earlier idea to use binary encoding for the switches, but that ended up making the hardware and input logic more complicated than needed. The current design keeps the switches much simpler and uses SHIFT instead.

## Display

The TFT uses an SPI interface with the ESP32-C3.

the enclosure has a cutout for the **LCD viewing area only**. The rest of the display module stays inside the enclosure.

## Buzzer

A passive buzzer is included for alarm and notification sounds.
it is controlled directly by the ESP32-C3.

## Enclosure

The enclosure started from a parametric electronics enclosure and has been modified to fit this PCB and the actual components.
current modifications include:
- Custom PCB mounting points
- M3 heat-set insert bosses
- LCD cutout
- USB-C access
- Buzzer clearance / ventilation
- Switch openings
The PCB is mounted using **M3 × 5 × 4 mm heat-set brass inserts**.

CAD files:

```text
cad/
├── Cadsucks.FCStd
└── Cadsucks.3mf
```

`Cadsucks.FCStd` is the editable FreeCAD file.

`Cadsucks.3mf` is the current 3D-printable version.

## Repository Structure

```text
.
├── cad/
│   ├── Cadsucks.3mf
│   └── Cadsucks.FCStd
│
├── gerber/
│   ├── gerber.zip
│   ├── *.gbr
│   └── *.gbrjob
│
├── init_dawn/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
│
├── kicad/
│   ├── init_dawn.d.kicad_pcb
│   ├── init_dawn.d.kicad_prl
│   ├── init_dawn.d.kicad_pro
│   └── init_dawn.d.kicad_sch
│
└── LICENSE
```

## Current Status

**Waiting for design review**

### Completed

* [x] PCB schematic
* [x] PCB layout and routing
* [x] Gerber generation
* [x] PCB 3D model
* [x] Initial enclosure
* [x] PCB mounting bosses
* [x] LCD cutout
* [x] Hardware layout

### In progress

* [ ] Design review
* [ ] Final hardware revisions
* [ ] Prototype
* [ ] Firmware
* [ ] Final enclosure revisions

## Firmware

The firmware directory is already set up, but firmware development has not started yet.
Planned firmware features:

- Clock functionality
- TFT interface
- Button input
- SHIFT functionality
- Alarm scheduling
- Buzzer control

## License

See [`LICENSE`](LICENSE) for licensing information.
