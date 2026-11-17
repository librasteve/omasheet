---
title: Starship Flight 15 Vignette 
files: [flight15.omx]
---

# Starship Flight 15: a vignette 

Booster 22 and Ship 42 stand {{ StackHeight }} tall and weigh {{ LaunchMass }} at liftoff, with {{ Satellites }} Starlink V3
satellites aboard. Every figure below is worked out from the sheets in [`flight15.omx`](flight15.omx); the ones read from the published description of the flight are listed, with their links, at the end.

_Authored in [omasheet](https://omasheet.org)._

## 1. Launch

The 33 Raptor 3 engines of the booster give {{ LiftoffThrust }} at liftoff, a thrust-to-weight of {{ AscentBoosterTWR }}. The booster burns for {{ Launch[BurnTime; 0] }} and the ship, once it has separated, for {{ Launch[BurnTime; 1] }}.

{{ Launch |> select(Stage, Thrust, Exhaust, MassFlow, Propellant, StartMass, EndMass, DeltaV) }}

Together the two stages give {{ AscentTotalDeltaV }}. A circular orbit at {{ OrbitAltitude }} needs {{ OrbitSpeed }}, so {{ AscentLost }} goes to gravity and drag. At that speed the {{ AscentPayloadMass }} of satellites carry {{ AscentPayloadKinetic }} of kinetic energy, and lifting them to altitude takes another {{ AscentPayloadLift }}. One orbit takes {{ AscentOrbitPeriod }}, so 14 of them are {{ AscentFourteenOrbits }} hours.

## 2. Fuel

Starship burns methane with liquid oxygen. The reaction gives out {{ Propellants.SpecificEnergy[0] }} of propellant, against {{ Propellants.SpecificEnergy[1] }} for RP-1 and {{ Propellants.SpecificEnergy[2] }} for hydrogen.

{{ Propellants |> select(Fuel, Reaction, MixRatio, SpecificEnergy, CO2PerKg) }}

The booster uses {{ Burn[Propellant; 0] }} in {{ Burn[BurnTime; 0] }}, {{ Burn[MassFlow; 0] }} between 33 engines, or {{ Burn[PerEngine; 0] }} each. That releases {{ Burn[Energy; 0] }}, a chemical power of {{ Burn[Power; 0] }}, of which {{ Burn[JetPower; 0] }} ends up in the exhaust: an efficiency of {{ Burn[Efficiency; 0] }}. Burning it makes {{ Burn[CO2; 0] }} of carbon dioxide. The same mass of RP-1 would give {{ Burn[RP1Energy; 0] }} and of hydrogen {{ Burn[HydrogenEnergy; 0] }}.

## 3. Re-entry

The ship enters the atmosphere {{ PeakEntryToBurn }} before its landing burn, carrying {{ PeakEntryEnergy }} of kinetic energy, as much as {{ PeakTNT }} of TNT. The air in front of it reaches {{ PeakAirTemp }} ({{ PeakAirCelsius }}), but the tiles, which glow until they radiate what arrives, settle at {{ PeakShieldTemp }} ({{ PeakShieldCelsius }}) under a peak heat flux of {{ PeakHeatFlux }}. That is {{ PeakTileMargin }} under their rating, and the peak dynamic pressure is {{ PeakDynamic }}.

{{ Reentry |> select(Altitude, Speed, Dynamic, StagTemp, HeatFlux, WallTemp, HeatLoad) }}

Over the whole descent each square metre takes {{ PeakHeatLoad }}. Tiles do not ablate, but a shield that did would lose {{ PeakIfAblative }} of thickness.

## Where the figures come from

{{ Flight15 |> select(Item, Value, Source) }}

Anything not in that table is an assumption: the Isp and throttle of each stage, the masses of a satellite and of the empty
stages, and the speeds, dwell times and heat coefficient of the re-entry bands. They are marked where they are written in
`flight15.omx`.

_This illustration is inspired by SpaceX Flight15 planned for Friday 13-Nov-2026 to show some of the cool features of
Omasheet._
More info: https://omasheet.org
