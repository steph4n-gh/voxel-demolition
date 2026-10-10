# Voxel Demolition: Oblivion Protocol

**Player Handbook · Version 0.2.4**

**Read the structure. Choose the cut. Watch the consequences.**

Voxel Demolition is a first-person demolition sandbox about turning standing structures into falling sections, rubble, and excavated ground. Ten handheld weapons, coordinated C4 charges, and four siege batteries give you different ways to attack the same problem. The campaign adds deadlines, spending limits, and two contracts that judge where your rubble lands.

This handbook starts with a short first session, then becomes a reference you can keep beside the game. Key names in **bold** refer to the default bindings. **Primary** means left mouse button; **ALT** means right mouse button. Distances are metres, and campaign prices are virtual ordnance costs.

### Find your next step

1. [Your first demolition](#1-your-first-demolition)
2. [Complete controls](#2-complete-controls)
3. [Movement, timing, and the HUD](#3-movement-timing-and-the-hud)
4. [How structures fail](#4-how-structures-fail)
5. [The ten-weapon arsenal](#5-the-ten-weapon-arsenal)
6. [The workbench](#6-the-workbench)
7. [C4 and coordinated cuts](#7-c4-and-coordinated-cuts)
8. [The siege fleet](#8-the-siege-fleet)
9. [Districts and demolition contracts](#9-districts-and-demolition-contracts)
10. [A tested approach to Corporate Drop](#10-a-tested-approach-to-corporate-drop)
11. [Replay and the flight camera](#11-replay-and-the-flight-camera)
12. [Installation, session state, and troubleshooting](#12-installation-session-state-and-troubleshooting)
13. [Glossary and pocket reference](#13-glossary-and-pocket-reference)

## 1. Your first demolition

### A five-minute sandbox session

The game opens in the Corporate Mega-Block district, in sandbox mode, with the Havoc Cannon selected. Sandbox lets you learn without a contract deadline or spending failure.

1. **Get comfortable.** Move with **W/A/S/D** and look with the mouse. Hold **Left Shift** to sprint. Tap **Space** to jump. Press **Esc** to release the mouse and pause the world; press **Esc** again to resume.
2. **Read the scene.** Press **V** to turn on the structural scanner. Look for the columns and lower supports holding up a larger section. Scan colours are hints about local support; they are not a guarantee of which way a tower will fall.
3. **Make a small opening.** Click the left mouse button to fire the Havoc Cannon at a lower wall or column. Watch the impact, then give falling sections time to land before firing again.
4. **Try a precise cut.** Press **4** for the Solstice Lance. Hold left mouse to drill a static wall or detached chunk. A narrow opening can be more useful than a wide explosion when you want to remove a particular support.
5. **Change the experiment.** Press **Tab** for the workbench. Select a weapon or preset, try a modest change to blast radius or force, then close the panel with **Tab** or **Esc**. The world and contract timer pause while the panel is open.
6. **Review the result.** Press **F10** after you have played for a moment. The replay opens paused at the latest recorded moment. Hold **Left Arrow** to look back, drag to orbit, and press **F10** again to return to the intact live session.
7. **Start fresh.** Press **R** or **Backspace** to regenerate the current district. That clears its destruction, deployed equipment, placed charges, fluids, and replay history.

For a smaller introductory scene, use **F7** to visit the Chemical Refinery in sandbox. Its volatile materials make cause and effect easy to see. Reset before each experiment so you can compare cuts on a fresh structure.

### A useful first habit

Aim, fire once, and watch. A detached section can strike another floor or tower after your shot has finished. If you immediately carpet the scene with ordnance, you lose the opportunity to see which cut caused the collapse.

Keep **F1** close. It opens the in-game controls manual. While that manual is open, **F1** and **Esc** are the active keyboard controls; close it before trying another action.

## 2. Complete controls

### Walking and handheld equipment

| Input | Action | When to use it |
| --- | --- | --- |
| Mouse | Look and aim | During live play with the cursor captured |
| **W / A / S / D** | Forward / left / backward / right | Ground and air movement |
| **Left Shift** | Sprint | Hold while moving |
| **Space** | Jump | Also releases a grapple with a slingshot boost |
| **Q** | Grapple, release grapple, or jet dash | Grapple a static surface within 60 m; otherwise dash if ready |
| **I** | Invert vertical mouse look | Toggle during live play |
| **1-9, 0** | Select a handheld weapon | See the arsenal table |
| Left mouse | Primary fire | Hold for Solstice Lance and Plasma Flamer; click for other weapons |
| Right mouse | ALT fire | Hold and release for a charged Apex shot; see sticky-charge exception below |
| **E** | Place C4 on the aimed surface | Requires an aim hit and available charge capacity |
| **F** | Detonate all placed C4 | Also activates available links between those charges |

### Siege and tactical tools

| Input | Action | Important detail |
| --- | --- | --- |
| **Z** | Cycle siege type | Changes the selected battery, not your handheld weapon |
| **B** | Deploy or move the selected battery | Copies its current siege workbench settings |
| **G** | Fire the selected deployed battery | Aim at a target; wait for its cooldown |
| **Y** | Recall every deployed battery | Clears the deployed fleet |
| **T** | Toggle slow motion | Does not stop the campaign clock |
| Mouse wheel | Tune slow-motion speed | Active slow motion only; 0.05x to 1.00x |
| **V** | Toggle structural scanner | A visual aid for reading nearby supports |
| **H** | Discharge a fluid hazard | Water with Frostbite, slag with Flamer, volatile gas with other weapons |
| **M** | Mute or unmute music | Sound effects have separate workbench gain controls |
| **X** | Toggle live projectile flight camera | Tracks a suitable live projectile; does not pause play |

### World, campaign, and panels

| Input | Action | Important detail |
| --- | --- | --- |
| **F1** | Open or close the controls manual | Reserved for help, not level selection |
| **Tab** | Open or close the workbench | Pauses live simulation and the contract timer |
| **Esc** | Close a panel, or free/capture the cursor | With no panel open, free cursor pauses the world |
| **[ / ]** | Previous / next district | Cycles all nine districts |
| **F2-F9** | Jump directly to districts 2-9 | Exact map appears in section 9 |
| **R / Backspace** | Regenerate the current district | Restarts its active contract when in campaign mode |
| **C** | Switch campaign / sandbox | Entering campaign loads and starts the selected contract |
| **Enter** | Start, advance, or retry a contract | Depends on whether it is ready, passed, or failed |
| **N / P** | Next / previous contract | Loads a fresh contract scene; these are not weapon keys |
| **F10** | Enter or leave cinematic replay | Requires recorded gameplay |

### Replay-only controls

| Input | Action |
| --- | --- |
| **Space** | Play / pause the replay |
| Hold **Left / Right Arrow** | Scrub backward / forward |
| **Up / Down Arrow** | Increase / decrease playback speed, from 0.1x to 4x |
| Hold either mouse button and drag | Orbit the replay camera |
| Mouse wheel | Zoom the replay camera, from 5 m to 120 m |
| **F10** | Return to live play |

### Function keys on Mac and laptop keyboards

If **F1** changes display brightness, or **F10** affects system audio, hold **Fn** or the **Globe** key while pressing the function key. On recent macOS versions, the standard-key option is under **System Settings  ->  Keyboard  ->  Keyboard Shortcuts  ->  Function Keys**. Keyboard firmware may also provide an Fn-lock setting. [Apple's function-key guide](https://support.apple.com/en-us/102439) explains the platform setting.

The game has fixed keyboard and mouse bindings. There is no in-game key-rebinding menu or documented gamepad control scheme. On a trackpad, configure a secondary click so that ALT fire is available.

## 3. Movement, timing, and the HUD

### Ground movement and jumps

Normal movement targets 7 m/s; sprint targets 12.5 m/s. You accelerate toward that speed rather than instantly changing velocity. Small obstacles up to approximately 0.55 m can be stepped over, while taller blocks require a jump or a new route.

Jumps have a short grace period after leaving an edge and a short input buffer before landing, both about 0.15 seconds. You do not have to hit the precise frame when a foot touches the floor. Air steering lets you correct a jump, but it does not turn the operative into a free-flying camera.

Excavated ground and settled rubble become part of your route. Watch the **ALT** altitude readout when entering a crater. If you fall to the bottom void boundary, the player returns to the district spawn; the district itself is not reset.

### Grapple or dash: the same key, two outcomes

**Q** first looks for a static voxel surface within 60 m. If it finds one, a winch pulls you toward that anchor. Press **Q** again to release, or press **Space** while attached to add a forward/upward slingshot boost and let go. The winch automatically releases very near the anchor or if you are pulled too far away.

An intact wall or floor can be an anchor. A detached falling chunk cannot. If your aimed surface has already become moving rubble, the same key may produce a dash instead.

When there is no valid anchor and the dash is ready, **Q** adds a horizontal burst and a small upward boost. With a movement key held, the dash follows your movement direction; otherwise it follows your horizontal facing. Its recharge is approximately 0.8 seconds. **JET: READY / CHARGING** reports that recharge, not a fuel tank.

### Slow motion and pausing

**T** slows the physics and weapon simulation. The wheel adjusts the rate in 0.05x steps. Player movement and the campaign clock continue on normal live time. Projectiles, delayed explosions, and cooldowns can therefore take longer to complete before the deadline. Slow motion is useful for studying a collapse; normal speed is usually better when finishing a timed contract.

A large structural detachment can briefly trigger automatic slow motion. The speed shown on the HUD still matters during that effect.

For a genuine pause, open **Tab**, open **F1**, free the cursor with **Esc**, or enter **F10** replay. These stop live simulation and contract timing. A muted soundtrack is not a pause command.

### Read the HUD as a set of instruments

| Readout | What it tells you |
| --- | --- |
| District title and dimensions | Which generated scene is loaded |
| **INTEGRITY** | Remaining static voxel fraction; detached chunks have left that static structure |
| **REBAR** | Remaining static reinforcement fraction |
| **PULVERIZED / COLLAPSES** | Destruction and detachment counters for the current scene |
| **RUBBLE: DYN / SETTLED** | Moving chunks versus chunks resting in the scene |
| **ALT** | Player height in metres |
| **JET** | Dash availability |
| Compass and battery markers | Heading and directions to deployed batteries |
| Seismic / fault meter | Activity indicator derived from moving chunks, effects, and lights |
| Reticle prompt | Whether Q can grapple or dash, C4 placement availability, and nearby actions |
| Weapon panel | Current weapon, heat/activity, and ALT guidance |
| C4 pips and siege panel | Armed-charge count, selected battery, and deployment/cooldown state |
| Campaign card | Objective progress, elapsed time, spending, collateral, and outcome |

Campaign destruction measures the original above-surface structure removed from its standing position. Detached pieces already count toward that target while they fall. The contract can still require them to settle inside a drop zone and can still fail on its other limits; a low integrity bar alone does not prove completion.

The scanner marks sampled nearby static structure: green suggests support below, yellow a local gap, orange/red a deeper gap, and pulsing magenta a more extreme unsupported span. Fuel and fusion-core highlights use their own colours. Treat the scanner as an inspection aid, then observe the actual movement of the building.

## 4. How structures fail

### Attack the path to ground

Structures stand because some connected material leads down to supported terrain. When you remove that connection, unsupported sections can detach and become falling chunks. Those chunks retain their material and remaining damage; detachment does not make an armored piece harmless.

That gives you three distinct jobs: breach a surface to expose the frame, cut the supports carrying a larger section, then manage or destroy the detached material. A tower falling against another tower can create more destruction than its first impact crater. In a precision contract, the same sideways motion can create expensive collateral.

For a broad collapse, cut several separated lower supports. For a controlled drop, work through more of the structure's height and use gentler force. A partial cut can leave a section hanging from reinforcement you have not yet reached.

### Match the tool to the material

The Material Telemetry tab provides each material's health, resistance, density, and conductivity. These are properties of individual blocks, not whole-building hit points. A material can resist one kind of attack and respond strongly to another.

| Material | What to expect | Useful approach |
| --- | --- | --- |
| Concrete | Common structural mass | Breach walls, then sever the supports behind them |
| Brick | Lighter masonry | Small kinetic or thermal cuts can remove it efficiently |
| Glass | Brittle facade material | Clear it to see the frame; it is not usually the strongest support |
| Steel Rebar | Dense, conductive reinforcement | Tesla attacks or focused cuts into the frame |
| Composite Carbon Rebar | Strong reinforcement with high conductivity | Avoid assuming a damaged facade means the frame is gone |
| Heavy Armor | Tough, dense plates | Repeated drilling, penetrating shots, or concentrated high-energy attacks |
| Cryo Ice | Brittle frozen material | Freeze first, then strike or shatter |
| Basalt | Resistant geological/solidified material | Use a heavier cut when excavating it |
| Bedrock | Reinforced foundation with 800 health per block | Strong enough to survive many ordinary attacks, but destructible |
| Explosive fuel material | Concussive chain-reaction source | One destroyed fuel block can trigger nearby volatile material |
| Glow Core | Powerful thermal chain-reaction source | Plan the breach before setting off a core |
| Power Capacitor | Electrical discharge source | Nearby conductors can carry the damage farther |

Bedrock is not an immutable world boundary. The destructible district includes foundations and subterranean material. The wide surrounding industrial slab is a separate support surface around that district; distant skyline scenery is a backdrop, not another district to demolish.

### Volatiles and fluid consequences

Fuel, fusion cores, and capacitors have different detonation effects. Chain reactions can remove material beyond your original cut, including detached volatile chunks. They are useful in the refinery and reactor, and difficult to confine near a precision drop-zone boundary.

Severed reinforcement can release fluid effects. The refinery uses volatile gas feedback; the fusion reactor uses molten slag; other districts use water. Water can extinguish nearby fire jets and quench slag. Hot slag can damage local material and eventually solidify as basalt. Gas drifts and can ignite near sparks or fire.

**H** adds a new discharge ahead of you. With Frostbite selected it creates water; with the Flamer it creates hot slag; with every other weapon it creates volatile gas.

### Three experiments worth trying

1. Reset a district, then compare one facade hit with one exposed-column hit.
2. Freeze a modest section with Frostbite, then strike it with another weapon.
3. In sandbox, place C4 across more than one support and compare the linked cut with a single blast.

Watch from outside the intended drop footprint, and use replay to compare the sequence of detachments. These are sandbox experiments, not structural-engineering predictions for real buildings.

## 5. The ten-weapon arsenal

The factory descriptions below assume stock tuning. Components and sliders can change eligible primary attacks; ALT attacks generally keep their own fixed size, force, and distance. The fire-rate setting changes cooldowns. The workbench does not turn every weapon into a projectile launcher.

Only Solstice Lance and Plasma Flamer fire continuously while left mouse is held. Click again for another shot with the other eight weapons. Let the current cooldown finish before trying ALT.

### Breach and bombard

| Key / weapon | Primary at factory tuning | ALT and tactical use |
| --- | --- | --- |
| **1 · Titan Maul** | A short forward breach, beginning 1.5 m ahead and extending another 6 m. Pushes loose debris forward. 0.60 s cooldown. | **Seismic Ground Ring:** attacks nearby foundation columns around the player, approximately 5.5 m out. Close in for a local breach; use the ring when supports surround you. |
| **2 · Havoc Cannon** | A falling explosive shell at 34 m/s, with a 3 m blast radius. 0.70 s cooldown. | **Airburst Cluster:** releases six bomblets when an upward lob reaches its apex, or on impact if it hits first. Aim upward to spread damage over a roof. |
| **3 · Event Horizon** | A gravity orb that creates a 2.4 s singularity on impact. It pulls loose matter within 35 m, erodes its centre, then ends in a large outward blast. Default collapse radius is 18 m; tuning keeps that radius within 16-22 m. 1.00 s cooldown. | **Grav Pulse:** immediately collapses all active singularities. With none active, attacks a point 12 m ahead with a 4.5 m radius. Delay or accelerate the final blast to suit the falling structure. |

Event Horizon can collect rubble and then throw it outward. A stronger or larger vortex can clear a broad scene quickly, but the outward impulse matters when the contract charges for spilled rubble. More simultaneous vortices are not automatically more precise.

### Cut and control

| Key / weapon | Primary at factory tuning | ALT and tactical use |
| --- | --- | --- |
| **4 · Solstice Lance** | Hold for a thermal drill up to 75 m, with a 0.65 m cutting radius and 0.06 s ticks. Cuts standing structure and loose chunks. The primary laser preserves bedrock. | **Molten Searing Slice:** seven overlapping thermal cuts centred 14 m ahead. Get the intended surface to that distance before firing. |
| **5 · Apex Railgun** | An immediate bore through standing structure up to 100 m, with a 0.75 m tunnel radius. 0.85 s cooldown. | **Hyper-Slug:** hold right mouse and release after at least 0.25 s; automatically fires at 0.85 s. Makes repeated 2.2 m radius cuts over up to 80 m, stopping after 14 damage-producing steps. |
| **6 · Frostbite** | A falling cryo shell at 36 m/s, with a 3.5 m outer freeze radius. Shatters the inner impact region and freezes nearby eligible material. 0.55 s cooldown. | **Cryo Blizzard:** a fixed burst 16 m ahead, with a 4 m damage radius. Freeze a section, then use another attack to break it. |
| **0 · Plasma Wire-Saw** | Click to cut standing structure and split loose chunks along the selected plane. Static reach is 28 m. 0.18 s cooldown. | Cycles **horizontal -> vertical -> diagonal -> horizontal**. Cycling does no damage and costs nothing. Select a plane, then click primary to cut. |

Use Lance for a sustained narrow opening, Railgun for an immediate long bore, and Wire-Saw for a deliberate separation plane. The saw can divide a detached chunk as well as a wall. Check the plane indicator before each cut; its ALT is a selector rather than a second attack.

### Fracture and thermal attacks

| Key / weapon | Primary at factory tuning | ALT and tactical use |
| --- | --- | --- |
| **7 · Seismic Disrupter** | A falling torpedo at 30 m/s, with a 3.6 m fracture radius. Cuts inward from the struck surface and heaves nearby rubble. 0.75 s cooldown. | **Tectonic Fault Rupture:** attacks 20 m ahead with a 3.5 m damage radius and a wider debris blast. Position its attack point inside the intended foundation. |
| **8 · Tesla Caster** | An aimed discharge up to 80 m. Damages the impact area and seeks up to four nearby conductive nodes. 0.24 s cooldown. | **Overload Arc Storm:** thermal damage 18 m ahead, with a 3 m radius and electrical visuals. Primary is useful near exposed rebar and capacitors. |
| **9 · Plasma Flamer** | Hold for a rapid stream of falling thermal globes at 40 m/s, with a 1.8 m blast radius and 0.08 s pulses. | **Thermobaric Flame Wall:** attacks 10 m ahead with a 3 m radius. Sweep primary across a facade or fuel conduit; close the distance for ALT. |

### ALT distance is part of aiming

Lance, Frostbite, Seismic, Tesla, and Flamer ALT attacks act at fixed points in front of you. Their visible traces can extend farther than their damage. A wall far beyond the stated point will not receive that fixed burst merely because the effect appears to point at it. Move closer, or use primary for a distant target.

The other important exception is **Remote Sticky**. When sticky projectile charges exist, right mouse detonates them all before using the selected weapon's ordinary ALT. This also takes priority over charging Apex. Clear the sticky charges before expecting a normal ALT attack.

### Campaign ordnance prices

Sandbox has no spending failure. In an active contract, accepted attacks add the following virtual costs. Holding a continuous weapon spends per accepted tick or pulse, so a faster cadence increases spending per second.

| Primary | Cost | Other action | Cost |
| --- | ---: | --- | ---: |
| Titan Maul | $25 | Damaging ALT, charged Apex, or sticky detonation | $75 |
| Havoc Cannon | $150 | Wire-Saw plane cycle | $0 |
| Event Horizon | $500 | C4 placement | $2,500 |
| Solstice Lance | $3.60 per tick | C4 detonation | No additional spend |
| Apex Railgun | $300 | Accepted siege shot | $600 |
| Frostbite | $120 | | |
| Seismic Disrupter | $400 | | |
| Tesla Caster | $80 | | |
| Plasma Flamer | $4 per pulse | | |
| Plasma Wire-Saw | $100 | | |

There is no separate ammunition pickup, reload, or weapon-purchase routine. Your practical limits are cooldowns, placement capacity, the contract allowance, and the consequences of the attack.

## 6. The workbench

Press **Tab** to open the workbench. The scene and contract timer pause, so you can compare settings without losing the attempt. Five tabs separate handheld equipment, ballistics, siege setup, material inspection, and sound.

Weapon selection normally retains your current component choices and numerical tuning. Choosing a preset replaces the relevant loadout choices. A world reset preserves these session settings, so check the displayed values before assuming that a new scene uses factory equipment.

The initial handheld loadout is **Havoc Cannon, Precision barrel, High Explosive payload, Plasma trail, Standard stabilizer,** and **Impact detonator**, with factory numerical tuning.

### Ordnance

Select one of the ten chassis, or use one of seven quick presets: **Bunker Buster, Seismic Fault, Tesla Overload, Plasma Torrent, Cryo Shatter, Apex Railgun,** or **Titan Breacher**. Presets choose a weapon and component/tuning combination and restore impact detonation.

Start with a preset when you want a coherent experiment quickly. Afterward, use Ballistics to change one property at a time. The preset name describes its intended use, not a guarantee that any particular building or contract will yield to one shot.

### Ballistics

| Control | Choices / legal range | What it changes |
| --- | --- | --- |
| Barrel | Precision, Tri-Spread, Cluster, Piercing | Tri-Spread affects Havoc, Frostbite, and Seismic; Cluster adds impact bomblets to Havoc and Seismic. Piercing currently fires like Precision. |
| Payload | High Explosive, Shaped Charge, Cryo Freeze, Thermite | Projectile impact response. Shaped Charge supplies the extra penetration jet. |
| Trail | Wisps, Plasma, Sparks, Tracer | Projectile appearance; choose the style you can follow easily. |
| Stabilizer | Standard, Compensator, Gyroscopic, Overclock | Recoil, eligible projectile gravity, and primary velocity/impulse. |
| Detonator | Impact, Remote Sticky | Impact response or charges waiting for right-mouse detonation. |
| Blast radius | **0.5-3.0x** | Scales eligible primary attack dimensions. |
| Impulse | **0.5-3.5x** | Scales eligible primary force. |
| Velocity | **0.5-2.5x** | Scales eligible primary projectile speed. |
| Fire rate | **0.5-3.0x** | Changes attack cooldowns. |
| Bomblets | **3-8** | Configured cluster count where applicable. |
| Recoil damping | **0.5-2.0x** | Reduces or increases the player's response to primary recoil. |

The six numerical factory values are **1.0x**, except bomblets, which start at **5**. **Reset** in this panel restores those numbers; it does not restore your chosen barrel, payload, stabilizer, trail, or detonator.

**Compensator** reduces primary recoil by 60%. **Gyroscopic** reduces eligible projectile gravity to 20% of normal. **Overclock** adds 30% primary velocity and impulse with increased recoil. These choices matter differently to a projectile shell, a hitscan bore, and a continuous drill. For example, a gravity stabilizer has no falling shell to steer when you are using the Railgun's immediate primary bore.

For a controlled drop, begin with moderate radius and low impulse. For a distant projectile shot, increase velocity before assuming you need more blast. For continuous fire, watch both your cut and your spending as the cadence increases.

### Siege Engineering

Select **Rail Artillery, Orbital Kinetic Relay, Hydraulic Ground Ram,** or **Tesla Siege Pylon**. The payload list is **High Explosive, Bunker Buster, Thermite, Cryo,** and **Antimatter**.

| Setting | Legal range | Factory setting |
| --- | --- | --- |
| Caliber | 500-2,500 mm | 1,500 mm |
| Propellant power | 0.5-3.0x | 1.5x |
| Elevation | 10-88° | 65° |
| Fuse delay | 0.05-0.45 s | 0.18 s |
| Azimuth | 0-360° | 0° |
| Bearing control | Reticle auto / manual | Reticle auto |

Trajectory presets set elevation to **65°** for High Mortar Lob, **18°** for Direct Breach, and **88°** for Orbital Beam. Selecting the Orbital Beam trajectory does not change Rail Artillery into an Orbital Relay; chassis and trajectory are separate controls.

Shell, payload, fuse, and trajectory options chiefly affect Rail Artillery. The Orbital Relay uses caliber, the Ground Ram uses power, and the Tesla Pylon uses its fixed attack behavior.

**Deploy again to apply changes.** A deployed battery retains the settings captured when you placed it. After editing Siege Engineering, close the panel and press **B** to deploy or reposition the selected type with the new values. Merely editing the sliders does not retune a battery already in the field.

### Material Telemetry

Browse eleven material cards for properties, fracture notes, and tactical advice. This is an inspection reference; it does not alter the world's materials. Basalt and Air do not have cards here. Bedrock is a highly resistant but destructible foundation material.

### Audio Synthesizer

| Control | Range | Default |
| --- | --- | --- |
| Master gain | 0-2.0x | 1.0x |
| Siege / blast gain | 0.5-2.5x | 1.2x |
| Duplicate-sound guard | 20-200 ms | 75 ms |
| Stress volume | 0.1-2.0x | 1.0x |
| Music volume | 0-1.0 | 0.65 |

Seven audition pads let you hear the generated effects while the world is paused. The waveform is a preview, not a recording of the current mix. **Stress pitch 210 Hz** and **bass boost 1.30x** are fixed synthesis readouts rather than adjustable controls.

The duplicate guard suppresses rapid repetitions of the same sound. A longer guard can make rapid cuts or repeated explosions sound sparser. Lower master or blast gain if the mix is too forceful, then audition it before returning to a collapse. **M** toggles music without muting the effects.

## 7. C4 and coordinated cuts

**E** plants a charge on the aimed surface, using a targeting ray up to 100 m. **F** detonates all placed charges. The game can hold **24 active charges**; individual contracts can impose a smaller cumulative planting allowance.

Nearby charges form visible det-cord links, up to **25 m** apart. The cords are part of the attack: detonation cuts a **2.8 m diameter** cylinder along each link, then produces a **5.5 m radius** blast at each charge site, shifted into the attached surface.

That makes a line of charges useful for separating several supports at once. Place enough charges to bridge the intended span, then inspect the cords from a new angle. Additional intermediate charges can change which pairs link. Do not assume that every possible pair is connected simply because all charges are close.

### A deliberate placement sequence

1. Choose the section you want to release and locate its remaining supports.
2. Place charges on those supports, checking the armed-charge count after each placement.
3. Look at the visible cord path. Reposition yourself to check whether it crosses the intended cut.
4. Move out of the falling section's footprint and press **F**.
5. Wait for the result before placing another group. In a precision attempt, inspect collateral as the debris settles.

A charge attached to a standing voxel disappears if its anchor is destroyed before detonation. A charge placed on a loose chunk stays at its placement location; it does not follow that moving chunk. For a coordinated cut, stable static supports are the most predictable anchors.

Campaign allocation counts **every successfully planted charge during the attempt**. Detonating does not refund that count. Each placement adds **$2,500** ordnance; detonation adds no extra cost. Check both the C4 allocation and spending budget before laying a long chain.

## 8. The siege fleet

Siege gives you four different battery behaviors. One of each type can be deployed, for four total. The initial fleet contains Rail Artillery, an Orbital Relay, and a Ground Ram.

**Z** cycles the selected type. **B** deploys or repositions it at the aimed horizontal location, anchored at ground height. **G** fires the selected battery. **Y** recalls the entire fleet and clears its active siege shells and strikes.

There is no drag-to-move command. Use **B** again when you need a new position or need to apply edited settings. If you fire a selected type that is absent, it automatically deploys at the default left-flank site before firing. Deliberate placement is better when the battery's own position matters.

| Battery | Behavior | Cooldown / placement advice |
| --- | --- | --- |
| **Rail Artillery** | Launches a ballistic shell. Auto targeting follows the reticle's horizontal bearing; elevation and power still determine its arc. | **2.4 s.** Use Direct Breach for nearby walls or a mortar lob to clear obstacles. |
| **Orbital Kinetic Relay** | After a **0.75 s warmup**, bores vertically through the aimed horizontal position and excavates a deep crater. Caliber changes beam radius. | **4.5 s.** Aim controls the column's horizontal location, not the beam's stopping height. |
| **Hydraulic Ground Ram** | Sends an expanding ground pulse from the battery's own position. Repeat within **6 s** to build five resonance stages, reaching **4x** resonance. | **1.4 s.** Deploy beside the intended structure. Aiming somewhere else does not relocate the pulse origin. |
| **Tesla Siege Pylon** | Seeks up to ten conductive nodes near the aim target, within **42 m of the battery**, and can induce nearby volatile detonations. | **2.0 s.** Deploy near exposed reinforcement or capacitors. |

Rail Artillery payloads change the impact: **HE** makes a broad explosion; **Bunker Buster** penetrates before a delayed detonation; **Thermite** applies thermal damage; **Cryo** freezes and shatters; **Antimatter** pulls loose matter before blasting it outward.

Siege shots cost **$600** each in active contracts. A cheap shot can still create expensive spill in a precision zone. Use the Orbital Relay carefully near boundaries: its deep excavation may remove the ground you were relying on to catch rubble.

## 9. Districts and demolition contracts

District number and contract number are different lists. Use **[ / ]** for the nine-district tour, or **F2-F9** to jump to districts 2-9. **F1** always opens help. To return to district 1 from another scene, cycle with the bracket keys.

### The nine-district map

| District | Name | Direct key | Associated contract |
| --- | --- | --- | --- |
| 1 | Corporate Mega-Block | Bracket cycle / initial scene | 1 · Corporate Drop |
| 2 | Industrial Fusion District | **F2** | 2 · Bridge Severance |
| 3 | High-Tech Aeropolis | **F3** | 7 · Aeropolis Foundation Breach |
| 4 | Fortress Citadel | **F4** | 4 · Fortress Siege |
| 5 | Dense Neon Metropolis | **F5** | 6 · Domino Perfection |
| 6 | Neo-Tokyo Cyber-Spire | **F6** | 8 · Neo-Tokyo Precision Drop |
| 7 | Chemical Refinery & Fuel Depot | **F7** | 3 · Fuel Chain Reaction |
| 8 | Orbital Launch Gantry | **F8** | Sandbox only |
| 9 | Quantum Fusion Tokamak | **F9** | 5 · Tokamak Containment Breach |

Selecting a district while campaign is active loads its associated contract. The Orbital Launch Gantry has no contract and enters sandbox. **R** or **Backspace** regenerates the current district; it does not take you back to district 1.

### Start, retry, and advance

Press **C** to enter campaign. It loads and starts the selected contract immediately. **N** or **P** loads and starts the next or previous contract. These transitions create a fresh world and discard the previous attempt's placed equipment and replay history.

**Enter** starts a Ready contract, advances after a pass, or reloads a failed attempt for a retry. During an Active attempt it does nothing. **R** also regenerates the current scene and restarts its active contract. Pressing **C** to leave campaign returns to sandbox in the current scene; it does not restore that scene's earlier state.

### Targets and limits

All listed ceilings are inclusive: reaching the limit is allowed; exceeding it fails. The C4 column is the total number planted during one attempt, not merely the number armed at once.

| Contract | District | Destruction target | Deadline | Ordnance cap | Collateral cap | C4 cap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| **1 · Corporate Drop** | 1 | 70% | 120 s | $300,000 | $100,000 | 12 |
| **2 · Bridge Severance** | 2 | 75% | 90 s | $50,000 | $50,000 | 4 |
| **3 · Fuel Chain Reaction** | 7 | 85% | 20 s | $100,000 | $150,000 | 8 |
| **4 · Fortress Siege** | 4 | 80% | 180 s | $150,000 | $120,000 | 10 |
| **5 · Tokamak Containment Breach** | 9 | 82% | 150 s | $250,000 | $200,000 | 12 |
| **6 · Domino Perfection** | 5 | 78% | 60 s | $35,000 | $80,000 | 2 |
| **7 · Aeropolis Foundation Breach** | 3 | 65% | 240 s | $400,000 | $180,000 | 16 |
| **8 · Neo-Tokyo Precision Drop** | 6 | 75% | 300 s | $500,000 | $250,000 | 24 |

Demolition progress compares the original above-surface structure with what remains standing. Detached sections count immediately even if they are still falling. Structural foundations above ground count; underground geology at or below ground level does not. Excavating a huge underground crater cannot substitute for removing the target building.

### Precision: where the rubble finishes

Contracts **1** and **8** add a green three-dimensional drop box. Corporate's box has a **40 x 40 m footprint**, from x/z -20 to +20 m, and extends from y 0 to 40 m. Neo-Tokyo's box has a **70 x 70 m footprint**, from x/z -35 to +35 m, and extends from y 0 to 60 m. A voxel exactly on a boundary counts inside.

Precision contracts wait for **every nonempty debris chunk to settle**. That means a supported body has stopped moving and rotating; it includes chunks created by underground excavation. The other six contracts can pass while rubble is still falling once their demolition target and limits are satisfied.

**Accuracy** measures settled rubble inside the box. It can display 100% before any rubble has settled, so do not read that early value as a completed precision objective. Airborne material must still finish moving.

**Collateral** is $100 per settled voxel outside the box, with the highest observed spill penalty retained. Later movement or destruction does not refund it. Repeated frames do not add the same stationary pile again. The six contracts without a drop zone currently accrue no collateral penalty.

To control spill, divide tall structures through their height, keep outward force low, and observe where early sections land. A last-second broad blast can turn a legal attempt into a collateral failure even after you have reached the demolition percentage.

### Grades

After the pass conditions are met, the game evaluates grades in this order:

| Grade | Collateral used | Ordnance used | Additional condition |
| --- | ---: | ---: | --- |
| **S** | At most 15% of cap | At most 50% of cap | At least 90% settled-rubble accuracy |
| **A** | At most 40% of cap | At most 75% of cap | - |
| **B** | At most 70% of cap | At most 90% of cap | - |
| **C** | Any remaining legal result | Any remaining legal result | - |

Finishing faster or exceeding the destruction percentage does not independently improve a grade. The current attempt's result remains until you start, retry, or select another contract. Once an attempt passes or fails, its checks stop; prepare a fresh retry before comparing a new plan.

### Choose an opening plan

| Situation | Useful first experiment |
| --- | --- |
| Bridge or suspended span | Separate its load paths; inspect which connections still hold the span. |
| Armored fortress | Open the shell with penetration or concentrated cuts, then attack the exposed supports. |
| Fuel or fusion machinery | Locate volatile material and use its chain reaction deliberately. |
| Very short deadline | Use normal speed, a prepared loadout, and a small number of decisive attacks. |
| Tall precision tower | Make descending cuts with restrained force, then allow time for settling. |
| Closely packed skyline | Watch for falling sections striking neighbours; a useful chain can also spill outside a precision box. |

These are starting tactics, not guaranteed solutions. Learn the geometry in sandbox, then restart the contract so the attempt begins with a fresh scene and full allowances.

## 10. A tested approach to Corporate Drop

Corporate Drop can punish a simple sideways blast. The following legal loadout and descending-cut plan passed a reproducible seeded simulation from the stock spawn. It is a useful starting strategy to adapt during play, rather than a promise that every mouse movement, frame rate, or random sequence produces the same score.

### Prepare the loadout

Use **Event Horizon**, with **Precision barrel, High Explosive payload, Impact detonator**, and these Ballistics settings:

| Setting | Value |
| --- | ---: |
| Blast radius | **3.00x** |
| Fire rate | **3.00x** |
| Velocity | **2.50x** |
| Impulse | **0.50x** |
| Stabilizer | **Compensator** |
| Recoil damping | **2.00x** |

Leave the other options at stock values. Prepare in sandbox, then load a fresh Corporate Drop attempt. The workbench pause also lets you verify the settings safely after loading.

### Work down the front faces

Stay near the initial position facing into the towers. Begin high on the central headquarters and make four descending cuts, about **25 m apart** vertically. Continue in this order: **near-left tech tower, right financial tower, farther-left financial tower, far-right tech tower**.

Wait for the weapon to accept the next primary shot. The qualified plan spaces accepted shots by at least **0.35 seconds** and uses at most **20 shots**. Stop firing earlier if demolition reaches 70%, then let the remaining singularities and rubble finish. It uses no ALT, C4, siege, or movement.

For players who want to reproduce the target layout precisely, the reference points below are metres in the district. The stock player's feet begin at **(0.75, 0.50, 29.75)**, facing toward decreasing Z.

| Order / face | Horizontal point x, z | Aim heights y, highest first |
| --- | --- | --- |
| 1 · Central headquarters | 0.75, 1.25 | 95.25 · 70.25 · 45.25 · 20.25 |
| 2 · Near-left tech tower | -22.75, 13.25 | 80.25 · 55.25 · 30.25 · 5.25 |
| 3 · Right financial tower | 24.75, 8.25 | 90.25 · 65.25 · 40.25 · 15.25 |
| 4 · Farther-left financial tower | -22.75, -7.75 | 90.25 · 65.25 · 40.25 · 15.25 |
| 5 · Far-right tech tower | 24.75, -12.75 | 80.25 · 55.25 · 30.25 · 5.25 |

The live game does not provide a coordinate-entry aiming tool. Use the visible facade bands as your guide, then watch destruction, collateral, and airborne chunks. If the early rubble is already spreading widely, stop and reassess before sending more outward blasts through it.

### What was verified

At a fixed **60 Hz** and seed **0xB04A**, this plan passed with grade **C**: **90.433% destruction in 18.133 seconds**, **$10,000 ordnance**, **$74,200 collateral**, and **146 chunks all settled**. It stayed within Corporate's unchanged mission limits and left no pending projectiles or singularities.

That is a bounded simulation result. Live aiming, timing, and the procedural random sequence can differ. The useful lesson is the combination of broad descending cuts, low impulse, and time reserved for settling. Try it, observe the first tower's outcome, and adapt rather than treating twenty clicks as an automatic win.

## 11. Replay and the flight camera

### Cinematic replay

**F10** pauses live play and opens recent recorded history, initially paused at its newest moment. Press **Space** to play, hold **Left / Right Arrow** to scrub, and use **Up / Down Arrow** to change playback speed from **0.1x to 4x**. Drag with either mouse button to orbit; use the wheel to zoom between **5 m and 120 m**. Press **F10** to return to the same live session.

The buffer retains up to **60 seconds**, sampled at up to **20 snapshots per second**, with up to **128 debris chunks per snapshot**. Older history rolls out. Slow frames can produce fewer snapshots. A level change or world regeneration clears the buffer.

Replay shows the recorded player, eligible projectiles, and rotated debris bounds. The static world remains the current world's backdrop; replay does not reconstruct every earlier wall, carve, fluid, charge, or visual effect. Its cyan debris shapes are a simplified view of recorded chunk positions, rather than the complete original voxel surfaces.

Use it to inspect the order and direction of a collapse. It does not undo destruction, rewind the live contract, or load an earlier save. There is no replay export or disk-saving control in this release.

### Live projectile camera

**X** toggles a live view following a suitable projectile. It favors the newest live siege shell; otherwise it can follow a sufficiently fast or large handheld projectile. Hitscan cuts, C4, and a scene with no eligible projectile have nothing for it to chase, so the view returns to the player camera.

The flight camera does not pause the world or contract. You can still be moving while watching the shot. Turn it off before walking across a crater or lining up your next precise cut.

## 12. Installation, session state, and troubleshooting

### Install the macOS release

1. Obtain the release ZIP and its checksum from the same trusted release location. Choose the **Apple Silicon / ARM64** archive for an Apple Silicon Mac.
2. Expand the ZIP and open the supplied **Voxel Demolition.app**. You may move it to **Applications** first. Keep the included manual and build/signing notes if you need support later.
3. macOS may ask you to confirm the first launch of an app downloaded from the internet. Check that the name and developer match the release you intended to run.
4. Once the scene opens, click into the game if necessary, then use **Esc** to free the cursor whenever you need another application.

Check the archive's **BUILD_INFO.txt** and **SIGNING.txt** for its actual architecture, source version, and signing state. An app's declared minimum OS is not a promise that every older Mac configuration has been tested.

If macOS reports that the app is **damaged**, do not disable security globally or assume it is a normal first-launch prompt. Download the verified archive again and compare its checksum; a modified or corrupted bundle may fail validation. If the developer cannot be verified, consult the signing notes and the release's support guidance before using a specific exception. [Apple's app-opening guidance](https://support.apple.com/en-us/102445) explains the different alerts.

### What a session retains

| Action / state | Result |
| --- | --- |
| Open help, workbench, free cursor, or replay | Live simulation and contract timer pause. |
| Return from replay | Resume the current world; replay does not restore an older one. |
| Regenerate / change district / load contract | Fresh world; clears debris, fluids, C4, siege deployment, and replay. |
| Handheld or audio settings | Remain in the running session across world changes. |
| Current contract result | Remains until you start, retry, or select another contract; no grade history is retained. |
| Quit and relaunch | Starts a new session; no autosave or saved campaign progression. |

There is no player save/load menu, autosave, exported replay file, or in-game control rebinding. Use replay before resetting if you want to inspect the current attempt. Quitting does not preserve a partially demolished district.

### Solve the common surprises

| Symptom | Check / remedy |
| --- | --- |
| I cannot look or move | Close help/workbench, leave replay with F10, and capture the cursor with Esc. |
| Function keys change brightness or volume | Hold Fn/Globe, or change the platform's function-key setting. |
| Holding fire gives only one shot | Hold applies to Lance and Flamer. Click again for other primaries. |
| Right mouse triggers an unexpected explosion | Remote sticky rounds take priority over ordinary ALT. Detonate them before charging Apex. |
| ALT effects appear to reach a wall but do no damage | Check the fixed attack distance and your position. Use primary for a distant wall. |
| A new weapon still has strange behavior | Components and tuning persist when switching. Check Ballistics or choose a preset. |
| Reset did not restore factory tuning | Resetting the world preserves session settings; Ballistics Reset restores only numerical tuning. |
| Siege sliders seem ineffective | Redeploy the selected battery with B to capture the edited settings. |
| The Ground Ram attacks the wrong place | Its pulse begins at the battery. Reposition the battery beside the target. |
| C4 disappears before I detonate | Its static anchor may have been destroyed. Place on surviving supports. |
| The contract fails after I plant more C4 | A contract counts cumulative placements; detonation does not replenish its allowance. |
| 100% precision accuracy does not pass | Accuracy counts settled rubble. Check demolition, all moving chunks, time, and budgets. |
| Collateral does not fall after I destroy spilled rubble | The highest observed settled spill penalty is retained. Retry for a fresh attempt. |
| Slow motion causes a timeout | The campaign clock continues at normal speed while physics and cooldowns slow. |
| Replay omits pieces or old geometry | It has a rolling history, chunk cap, and simplified recorded shapes. It is not a full world restore. |
| There is no music | Press M; check music and master gain. Check the Mac output device and system volume. |
| Effects sound intermittent | A longer duplicate-sound guard suppresses repeated copies of one effect. Try a shorter guard. |
| A dense collapse becomes less responsive | Let the active scene settle, pause to inspect it, or reset for a fresh experiment. No fixed FPS is promised. |

For an issue report, include the game version, archive name, Mac model/OS, district or contract, loadout, and a short sequence that reproduces it. If possible, attach a screenshot of the HUD and explain whether the problem survives a fresh world reset. The repository's [README](../README.md) links the project and developer guidance.

## 13. Glossary and pocket reference

| Term | Meaning in this game |
| --- | --- |
| **Voxel** | One small material block making up the structure or terrain. |
| **Static structure** | Material still attached to the standing world. |
| **Chunk** | A detached collection of voxels moving as one body. |
| **Settled** | A supported chunk whose movement and rotation have stopped. |
| **Integrity** | The remaining static portion of the original structure. |
| **Demolition target** | Required removal of the original above-surface standing structure. |
| **Drop zone** | The green 3D volume used by a precision contract. |
| **Accuracy** | Settled rubble inside the drop zone as a fraction of settled rubble. |
| **Collateral** | Retained penalty for settled rubble outside a precision zone. |
| **Impulse** | Force that changes the movement of rubble or a struck body. |
| **Cooldown** | Time before another accepted attack. |
| **ALT** | Right-mouse secondary action; weapon-specific. |
| **Singularity** | Event Horizon's temporary inward pull and erosion before its collapse blast. |
| **Det-cord** | The cutting link between connected C4 charges. |
| **Resonance** | Ground Ram strength built by consecutive timely shots. |
| **Reticle auto targeting** | Siege bearing follows the horizontal direction to the aim point. |
| **Precision contract** | A demolition contract that also judges settled rubble in a drop box. |

### Keep these keys beside you

- **Move:** W/A/S/D · **Sprint:** Left Shift · **Jump:** Space · **Grapple/dash:** Q
- **Weapons:** 1-9, 0 · **Primary:** left mouse · **ALT:** right mouse
- **C4:** E plant / F detonate · **Siege:** Z select / B deploy / G fire / Y recall
- **Workbench:** Tab · **Help:** F1 · **Pause/cursor:** Esc
- **Scanner:** V · **Slow motion:** T + wheel · **Music:** M · **Flight camera:** X
- **Districts:** [ / ] or F2-F9 · **Regenerate:** R / Backspace
- **Campaign:** C · **Contract next/previous:** N / P · **Retry/advance:** Enter
- **Replay:** F10 · **Replay play/pause:** Space · **Scrub:** Left/Right Arrow
